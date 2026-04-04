use crate::data_ops::{normalize_allowed_base_urls, AllowedBaseUrlPattern};
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_runtime::deno_permissions::{
    Permissions, PermissionsOptions, RuntimePermissionDescriptorParser,
};

use crate::deno_stubs::VlConvertNodeSys;

use super::config::VlcConfig;

/// Check if a domain matches any pattern in the allowlist.
/// Patterns: `"esm.sh"` = exact, `"*.jsdelivr.net"` = subdomain wildcard, `"*"` = any.
pub fn domain_matches_patterns(domain: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| {
        if pattern == "*" {
            true
        } else if let Some(suffix) = pattern.strip_prefix("*.") {
            domain == suffix || domain.ends_with(&format!(".{suffix}"))
        } else {
            domain == pattern
        }
    })
}

/// Helper to build parsed allowed_base_urls from a config's string patterns.
pub(crate) fn parse_allowed_base_urls_from_config(
    config: &VlcConfig,
) -> Result<Option<Vec<AllowedBaseUrlPattern>>, AnyError> {
    normalize_allowed_base_urls(config.allowed_base_urls.clone())
}

pub(crate) fn build_permissions(_config: &VlcConfig) -> Result<Permissions, AnyError> {
    // All network and filesystem access is denied at the Deno level.
    // Data fetching goes through Rust ops (op_vega_data_fetch, op_vega_file_read)
    // which enforce allowed_base_urls policies in Rust.
    Permissions::from_options(
        &RuntimePermissionDescriptorParser::new(VlConvertNodeSys),
        &PermissionsOptions {
            prompt: false,
            ..Default::default()
        },
    )
    .map_err(|err| anyhow!("Failed to build Deno permissions: {err}"))
}

/// Return the platform-standard path for the vl-convert JSONC config file.
///
/// The path is `<config_dir>/vl-convert/vlc-config.jsonc` where `config_dir`
/// is the OS config directory (`~/.config` on Linux, `~/Library/Application Support`
/// on macOS, `%APPDATA%` on Windows). The file may not exist.
pub fn vlc_config_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("vl-convert")
        .join("vlc-config.jsonc")
}

/// Check if a string looks like a filesystem path rather than a URL.
/// Detects absolute Unix paths (/...) and Windows drive letter paths (C:\..., C:/...).
pub(crate) fn is_filesystem_path(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.first() == Some(&b'/') {
        return true;
    }
    // Windows drive letter: single ASCII letter followed by :\ or :/
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_ops::normalize_allowed_base_url;

    #[test]
    fn test_domain_matches_exact() {
        let patterns = vec!["esm.sh".to_string()];
        assert!(domain_matches_patterns("esm.sh", &patterns));
        assert!(!domain_matches_patterns("cdn.esm.sh", &patterns));
        assert!(!domain_matches_patterns("esm.sh.evil.com", &patterns));
    }

    #[test]
    fn test_domain_matches_wildcard_subdomain() {
        let patterns = vec!["*.jsdelivr.net".to_string()];
        assert!(domain_matches_patterns("cdn.jsdelivr.net", &patterns));
        assert!(domain_matches_patterns("foo.bar.jsdelivr.net", &patterns));
        // The bare domain itself should match (*.x matches x)
        assert!(domain_matches_patterns("jsdelivr.net", &patterns));
        // Must not match a suffix attack
        assert!(!domain_matches_patterns("jsdelivr.net.evil.com", &patterns));
        assert!(!domain_matches_patterns("notjsdelivr.net", &patterns));
    }

    #[test]
    fn test_domain_matches_star_all() {
        let patterns = vec!["*".to_string()];
        assert!(domain_matches_patterns("esm.sh", &patterns));
        assert!(domain_matches_patterns("anything.example.com", &patterns));
        assert!(domain_matches_patterns("", &patterns));
    }

    #[test]
    fn test_domain_no_match_empty_list() {
        let patterns: Vec<String> = vec![];
        assert!(!domain_matches_patterns("esm.sh", &patterns));
    }

    #[test]
    fn test_domain_no_match_wrong_domain() {
        let patterns = vec!["esm.sh".to_string(), "*.jsdelivr.net".to_string()];
        assert!(!domain_matches_patterns("evil.com", &patterns));
        assert!(!domain_matches_patterns("esm.sh.evil.com", &patterns));
    }

    #[test]
    fn test_domain_multiple_patterns() {
        let patterns = vec![
            "esm.sh".to_string(),
            "*.jsdelivr.net".to_string(),
            "unpkg.com".to_string(),
        ];
        assert!(domain_matches_patterns("esm.sh", &patterns));
        assert!(domain_matches_patterns("cdn.jsdelivr.net", &patterns));
        assert!(domain_matches_patterns("unpkg.com", &patterns));
        assert!(!domain_matches_patterns("evil.com", &patterns));
    }

    #[test]
    fn test_allowed_base_url_normalization_and_validation() {
        assert_eq!(
            normalize_allowed_base_url("https://example.com").unwrap(),
            AllowedBaseUrlPattern::Prefix("https://example.com/".to_string())
        );
        assert_eq!(
            normalize_allowed_base_url("https://example.com/data").unwrap(),
            AllowedBaseUrlPattern::Prefix("https://example.com/data/".to_string())
        );

        assert!(normalize_allowed_base_url("https://user@example.com/").is_err());
        assert!(normalize_allowed_base_url("https://example.com/?q=1").is_err());
        assert!(normalize_allowed_base_url("https://example.com/#fragment").is_err());
        assert!(normalize_allowed_base_urls(Some(vec![])).is_ok());
    }

    #[test]
    fn test_with_config_accepts_empty_allowed_base_urls() {
        use super::super::VlConverter;
        // Empty list means no external access at all (valid config)
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: Some(vec![]),
            ..Default::default()
        });
        assert!(converter.is_ok());
    }

    #[test]
    fn test_with_config_accepts_csp_patterns() {
        assert_eq!(
            normalize_allowed_base_url("*").unwrap(),
            AllowedBaseUrlPattern::Any
        );
        assert_eq!(
            normalize_allowed_base_url("https:").unwrap(),
            AllowedBaseUrlPattern::Scheme("https".to_string())
        );
        assert_eq!(
            normalize_allowed_base_url("http:").unwrap(),
            AllowedBaseUrlPattern::Scheme("http".to_string())
        );
    }
}

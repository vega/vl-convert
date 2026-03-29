use crate::converter::{portable_canonicalize, ACCESS_DENIED_MARKER};
use crate::image_loading::VL_CONVERT_USER_AGENT;
use deno_core::anyhow::{anyhow, bail};
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::OpState;
use deno_error::JsErrorBox;
use regex::Regex;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

lazy_static! {
    static ref ASYNC_CLIENT: reqwest::Client = reqwest::Client::builder()
        .user_agent(VL_CONVERT_USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to construct async reqwest client");
    static ref SCHEME_PATTERN_RE: Regex = Regex::new(r"^[a-zA-Z][a-zA-Z0-9+.\-]*:$").unwrap();
    static ref WILDCARD_HOST_RE: Regex =
        Regex::new(r"^([a-zA-Z][a-zA-Z0-9+.\-]*)://\*\.([^/?#]+)(/[^?#]*)?$").unwrap();
}

/// A parsed allowlist pattern for URL access control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AllowedBaseUrlPattern {
    /// Matches any URL or path.
    Any,
    /// Matches any URL with the given scheme (e.g. "https", "http", "s3").
    Scheme(String),
    /// Matches URLs whose string representation starts with this prefix.
    Prefix(String),
    /// Matches URLs with the given scheme whose host equals or is a subdomain
    /// of `host_suffix`, and whose path starts with `path_prefix`.
    WildcardHost {
        scheme: String,
        host_suffix: String,
        path_prefix: String,
    },
    /// Matches filesystem paths (bare or file:// URLs) under this directory.
    FilePathPrefix(PathBuf),
}

/// Policy controlling which URLs and filesystem paths data-fetching ops may access.
/// Stored in Deno OpState; cloned at the start of each async op to avoid holding
/// the RefCell borrow across await points.
#[derive(Debug, Clone)]
pub struct DataAccessPolicy {
    /// Parsed allowlist patterns. `None` means allow any HTTP/HTTPS but deny
    /// filesystem. `Some(vec![])` means deny everything.
    pub allowed_base_urls: Option<Vec<AllowedBaseUrlPattern>>,
}

fn access_denied_message(detail: impl AsRef<str>) -> String {
    format!("{ACCESS_DENIED_MARKER}: {}", detail.as_ref())
}

/// Canonicalize a path for policy checks, handling non-existent files by
/// canonicalizing the parent directory and appending the file name.
pub(crate) fn canonicalize_path_for_policy_check(path: &Path) -> Result<PathBuf, AnyError> {
    if path.exists() {
        return portable_canonicalize(path)
            .map_err(|e| anyhow!("Failed to resolve path {}: {e}", path.display()));
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let canonical_parent = portable_canonicalize(parent)
        .map_err(|e| anyhow!("Failed to resolve path {}: {e}", parent.display()))?;
    let Some(file_name) = path.file_name() else {
        bail!(
            "Failed to resolve local path {}: missing file name",
            path.display()
        );
    };
    Ok(canonical_parent.join(file_name))
}

fn normalize_url_prefix(mut normalized: String) -> String {
    if !normalized.ends_with('/') {
        normalized.push('/');
    }
    normalized
}

fn url_to_local_path(url: &str) -> Result<PathBuf, AnyError> {
    if url.starts_with("file://") {
        let parsed = Url::parse(url)?;
        parsed
            .to_file_path()
            .map_err(|_| anyhow!("Cannot convert file URL to path: {url}"))
    } else if url.starts_with('/') {
        Ok(PathBuf::from(url))
    } else {
        bail!("Expected local file path or file URL, got: {url}")
    }
}

/// Parse a single allowed_base_url string into an `AllowedBaseUrlPattern`.
pub(crate) fn normalize_allowed_base_url(
    allowed_base_url: &str,
) -> Result<AllowedBaseUrlPattern, AnyError> {
    if allowed_base_url == "*" {
        return Ok(AllowedBaseUrlPattern::Any);
    }

    if SCHEME_PATTERN_RE.is_match(allowed_base_url) {
        return Ok(AllowedBaseUrlPattern::Scheme(
            allowed_base_url[..allowed_base_url.len() - 1].to_ascii_lowercase(),
        ));
    }

    // Filesystem paths (absolute paths or file:// URLs)
    let is_absolute_path = allowed_base_url.starts_with('/')
        || (allowed_base_url.len() >= 3
            && allowed_base_url.as_bytes()[0].is_ascii_alphabetic()
            && allowed_base_url.as_bytes()[1] == b':'
            && (allowed_base_url.as_bytes()[2] == b'\\' || allowed_base_url.as_bytes()[2] == b'/'));

    if is_absolute_path || allowed_base_url.starts_with("file:///") {
        let path = if allowed_base_url.starts_with("file:///") {
            let parsed = Url::parse(allowed_base_url)?;
            parsed
                .to_file_path()
                .map_err(|_| anyhow!("Cannot convert file URL to path: {allowed_base_url}"))?
        } else {
            PathBuf::from(allowed_base_url)
        };
        let canonical = portable_canonicalize(&path).map_err(|err| {
            anyhow!(
                "Failed to resolve filesystem path in allowed_base_urls '{}': {}",
                allowed_base_url,
                err
            )
        })?;
        if !canonical.is_dir() {
            bail!(
                "Filesystem path in allowed_base_urls must be a directory: {}",
                canonical.display()
            );
        }
        return Ok(AllowedBaseUrlPattern::FilePathPrefix(canonical));
    }

    if let Some(captures) = WILDCARD_HOST_RE.captures(allowed_base_url) {
        let scheme = captures.get(1).unwrap().as_str().to_ascii_lowercase();
        let host_suffix = captures.get(2).unwrap().as_str().to_ascii_lowercase();
        if host_suffix.is_empty() || host_suffix.contains('@') || host_suffix.contains(':') {
            bail!("Invalid wildcard host pattern in allowed_base_urls: {allowed_base_url}");
        }
        let path_prefix = normalize_url_prefix(
            captures
                .get(3)
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| "/".to_string()),
        );
        return Ok(AllowedBaseUrlPattern::WildcardHost {
            scheme,
            host_suffix,
            path_prefix,
        });
    }

    let parsed_url = Url::parse(allowed_base_url)
        .map_err(|err| anyhow!("Invalid allowed_base_url '{}': {}", allowed_base_url, err))?;

    if !parsed_url.username().is_empty() || parsed_url.password().is_some() {
        bail!(
            "allowed_base_url cannot include userinfo credentials: {}",
            allowed_base_url
        );
    }

    if parsed_url.query().is_some() {
        bail!(
            "allowed_base_url cannot include a query component: {}",
            allowed_base_url
        );
    }

    if parsed_url.fragment().is_some() {
        bail!(
            "allowed_base_url cannot include a fragment component: {}",
            allowed_base_url
        );
    }

    Ok(AllowedBaseUrlPattern::Prefix(normalize_url_prefix(
        parsed_url.to_string(),
    )))
}

/// Parse a list of allowed_base_url strings into `AllowedBaseUrlPattern`s.
pub(crate) fn normalize_allowed_base_urls(
    allowed_base_urls: Option<Vec<String>>,
) -> Result<Option<Vec<AllowedBaseUrlPattern>>, AnyError> {
    allowed_base_urls
        .map(|urls| {
            urls.into_iter()
                .map(|url| normalize_allowed_base_url(&url))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
}

/// Check whether a URL is permitted by the parsed allowlist patterns.
///
/// When `patterns` is `None`, any `http:`, `https:`, or `file:` URL is allowed,
/// as are bare filesystem paths.
///
/// When `patterns` is `Some(list)`, the URL must match at least one pattern.
pub(crate) fn is_access_allowed(url: &str, patterns: &Option<Vec<AllowedBaseUrlPattern>>) -> bool {
    match patterns {
        None => true,
        Some(list) => {
            if list.is_empty() {
                return false;
            }
            let parsed_url = Url::parse(url).ok();

            list.iter().any(|pattern| match pattern {
                AllowedBaseUrlPattern::Any => true,
                AllowedBaseUrlPattern::Scheme(scheme) => parsed_url
                    .as_ref()
                    .map(|parsed| parsed.scheme().eq_ignore_ascii_case(scheme))
                    .unwrap_or(false),
                AllowedBaseUrlPattern::Prefix(prefix) => parsed_url
                    .as_ref()
                    .map(|parsed| parsed.as_str().starts_with(prefix.as_str()))
                    .unwrap_or(false),
                AllowedBaseUrlPattern::WildcardHost {
                    scheme,
                    host_suffix,
                    path_prefix,
                } => parsed_url
                    .as_ref()
                    .and_then(|parsed| {
                        parsed.host_str().map(|host| {
                            parsed.scheme().eq_ignore_ascii_case(scheme)
                                && (host.eq_ignore_ascii_case(host_suffix)
                                    || host
                                        .to_ascii_lowercase()
                                        .ends_with(&format!(".{host_suffix}")))
                                && parsed.path().starts_with(path_prefix.as_str())
                        })
                    })
                    .unwrap_or(false),
                AllowedBaseUrlPattern::FilePathPrefix(prefix) => url_to_local_path(url)
                    .and_then(|path| canonicalize_path_for_policy_check(&path))
                    .map(|path| path.starts_with(prefix))
                    .unwrap_or(false),
            })
        }
    }
}

/// Validate an HTTP(S) URL against the data access policy.
/// Returns the appropriate reqwest client (redirect-following or not) on success.
fn validate_http_url(url: &str, policy: &DataAccessPolicy) -> Result<(), JsErrorBox> {
    let parsed =
        Url::parse(url).map_err(|e| JsErrorBox::generic(format!("Invalid URL '{url}': {e}")))?;

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(JsErrorBox::generic(access_denied_message(format!(
            "Only http and https URLs are allowed, got {scheme}:// in '{url}'"
        ))));
    }

    if !is_access_allowed(url, &policy.allowed_base_urls) {
        return Err(JsErrorBox::generic(access_denied_message(format!(
            "External data url not allowed: {url}"
        ))));
    }

    Ok(())
}

/// Validate a filesystem path against the data access policy.
/// Handles both bare paths and `file://` URLs. Returns the canonicalized path.
fn validate_file_path(path: &str, policy: &DataAccessPolicy) -> Result<PathBuf, JsErrorBox> {
    // Convert file:// URL to a filesystem path
    let fs_path = if path.starts_with("file://") {
        let url = Url::parse(path)
            .map_err(|e| JsErrorBox::generic(format!("Invalid file URL '{path}': {e}")))?;
        url.to_file_path()
            .map_err(|_| JsErrorBox::generic(format!("Cannot convert file URL to path: {path}")))?
    } else {
        PathBuf::from(path)
    };

    // Canonicalize to resolve symlinks and ..
    let canonical = portable_canonicalize(&fs_path).map_err(|e| {
        JsErrorBox::generic(format!(
            "Failed to resolve filesystem path '{}': {e}",
            fs_path.display()
        ))
    })?;

    // Convert canonicalized path to a file:// URL for allowlist comparison.
    // The FilePathPrefix pattern handles canonicalization internally, but for
    // Prefix patterns we need a file:// URL string.
    let file_url = Url::from_file_path(&canonical).map_err(|_| {
        JsErrorBox::generic(format!(
            "Cannot convert path to file URL: {}",
            canonical.display()
        ))
    })?;

    if !is_access_allowed(file_url.as_str(), &policy.allowed_base_urls) {
        return Err(JsErrorBox::generic(access_denied_message(format!(
            "Filesystem access denied for path: {}",
            canonical.display()
        ))));
    }

    Ok(canonical)
}

#[op2]
#[string]
pub async fn op_vega_data_fetch(
    state: Rc<RefCell<OpState>>,
    #[string] url: String,
) -> Result<String, JsErrorBox> {
    let policy = {
        let state = state.borrow();
        state.borrow::<DataAccessPolicy>().clone()
    };

    validate_http_url(&url, &policy)?;

    let response = ASYNC_CLIENT
        .get(&url)
        .send()
        .await
        .map_err(|e| JsErrorBox::generic(format!("HTTP request failed for '{url}': {e}")))?;

    if !response.status().is_success() {
        return Err(JsErrorBox::generic(format!(
            "HTTP request failed for '{url}': status {}",
            response.status()
        )));
    }

    response
        .text()
        .await
        .map_err(|e| JsErrorBox::generic(format!("Failed to read response body for '{url}': {e}")))
}

#[op2]
#[buffer]
pub async fn op_vega_data_fetch_bytes(
    state: Rc<RefCell<OpState>>,
    #[string] url: String,
) -> Result<Vec<u8>, JsErrorBox> {
    let policy = {
        let state = state.borrow();
        state.borrow::<DataAccessPolicy>().clone()
    };

    validate_http_url(&url, &policy)?;

    let response = ASYNC_CLIENT
        .get(&url)
        .send()
        .await
        .map_err(|e| JsErrorBox::generic(format!("HTTP request failed for '{url}': {e}")))?;

    if !response.status().is_success() {
        return Err(JsErrorBox::generic(format!(
            "HTTP request failed for '{url}': status {}",
            response.status()
        )));
    }

    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| JsErrorBox::generic(format!("Failed to read response body for '{url}': {e}")))
}

#[op2]
#[string]
pub async fn op_vega_file_read(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<String, JsErrorBox> {
    let policy = {
        let state = state.borrow();
        state.borrow::<DataAccessPolicy>().clone()
    };

    let canonical = validate_file_path(&path, &policy)?;

    tokio::fs::read_to_string(&canonical).await.map_err(|e| {
        JsErrorBox::generic(format!(
            "Failed to read file '{}': {e}",
            canonical.display()
        ))
    })
}

#[op2]
#[buffer]
pub async fn op_vega_file_read_bytes(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<Vec<u8>, JsErrorBox> {
    let policy = {
        let state = state.borrow();
        state.borrow::<DataAccessPolicy>().clone()
    };

    let canonical = validate_file_path(&path, &policy)?;

    tokio::fs::read(&canonical).await.map_err(|e| {
        JsErrorBox::generic(format!(
            "Failed to read file '{}': {e}",
            canonical.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to parse string patterns into AllowedBaseUrlPattern for tests.
    fn parse_patterns(patterns: Vec<&str>) -> Option<Vec<AllowedBaseUrlPattern>> {
        Some(
            patterns
                .into_iter()
                .map(|p| normalize_allowed_base_url(p).unwrap())
                .collect(),
        )
    }

    #[test]
    fn test_is_access_allowed_none_allows_http() {
        assert!(is_access_allowed("https://example.com/data.json", &None));
        assert!(is_access_allowed("http://example.com/data.json", &None));
    }

    #[test]
    fn test_is_access_allowed_none_allows_file() {
        assert!(is_access_allowed("file:///tmp/data.json", &None));
        assert!(is_access_allowed("/tmp/data.json", &None));
    }

    #[test]
    fn test_is_access_allowed_empty_denies_all() {
        let patterns: Option<Vec<AllowedBaseUrlPattern>> = Some(vec![]);
        assert!(!is_access_allowed("https://example.com/", &patterns));
        assert!(!is_access_allowed("file:///tmp/data.json", &patterns));
    }

    #[test]
    fn test_is_access_allowed_star_allows_all() {
        let patterns = parse_patterns(vec!["*"]);
        assert!(is_access_allowed(
            "https://example.com/data.json",
            &patterns
        ));
        assert!(is_access_allowed("file:///tmp/data.json", &patterns));
    }

    #[test]
    fn test_is_access_allowed_scheme_patterns() {
        let patterns = parse_patterns(vec!["https:"]);
        assert!(is_access_allowed(
            "https://example.com/data.json",
            &patterns
        ));
        assert!(!is_access_allowed(
            "http://example.com/data.json",
            &patterns
        ));
        assert!(!is_access_allowed("file:///tmp/data.json", &patterns));

        let patterns = parse_patterns(vec!["http:"]);
        assert!(!is_access_allowed(
            "https://example.com/data.json",
            &patterns
        ));
        assert!(is_access_allowed("http://example.com/data.json", &patterns));
    }

    #[test]
    fn test_is_access_allowed_url_prefix() {
        let patterns = parse_patterns(vec!["https://example.com/"]);
        assert!(is_access_allowed(
            "https://example.com/data.json",
            &patterns
        ));
        assert!(is_access_allowed(
            "https://example.com/sub/data.json",
            &patterns
        ));
        assert!(!is_access_allowed("https://other.com/data.json", &patterns));
    }

    #[test]
    fn test_is_access_allowed_wildcard_subdomain() {
        let patterns = parse_patterns(vec!["https://*.example.com/"]);
        assert!(is_access_allowed(
            "https://cdn.example.com/data.json",
            &patterns
        ));
        assert!(is_access_allowed(
            "https://foo.bar.example.com/data.json",
            &patterns
        ));
        // Bare domain also matches
        assert!(is_access_allowed(
            "https://example.com/data.json",
            &patterns
        ));
        // Must not match suffix attacks
        assert!(!is_access_allowed(
            "https://notexample.com/data.json",
            &patterns
        ));
        assert!(!is_access_allowed(
            "https://evil-example.com/data.json",
            &patterns
        ));
        // Wrong scheme
        assert!(!is_access_allowed(
            "http://cdn.example.com/data.json",
            &patterns
        ));
    }

    #[test]
    fn test_normalize_allowed_base_url_star() {
        assert_eq!(
            normalize_allowed_base_url("*").unwrap(),
            AllowedBaseUrlPattern::Any
        );
    }

    #[test]
    fn test_normalize_allowed_base_url_scheme() {
        assert_eq!(
            normalize_allowed_base_url("https:").unwrap(),
            AllowedBaseUrlPattern::Scheme("https".to_string())
        );
        assert_eq!(
            normalize_allowed_base_url("s3:").unwrap(),
            AllowedBaseUrlPattern::Scheme("s3".to_string())
        );
    }

    #[test]
    fn test_normalize_allowed_base_url_prefix() {
        assert_eq!(
            normalize_allowed_base_url("https://example.com/data").unwrap(),
            AllowedBaseUrlPattern::Prefix("https://example.com/data/".to_string())
        );
    }

    #[test]
    fn test_normalize_allowed_base_url_wildcard_host() {
        assert_eq!(
            normalize_allowed_base_url("https://*.example.com/data").unwrap(),
            AllowedBaseUrlPattern::WildcardHost {
                scheme: "https".to_string(),
                host_suffix: "example.com".to_string(),
                path_prefix: "/data/".to_string(),
            }
        );
    }

    #[test]
    fn test_normalize_allowed_base_url_rejects_query() {
        assert!(normalize_allowed_base_url("https://example.com/data?q=1").is_err());
    }

    #[test]
    fn test_normalize_allowed_base_url_rejects_fragment() {
        assert!(normalize_allowed_base_url("https://example.com/#fragment").is_err());
    }

    #[test]
    fn test_normalize_allowed_base_url_rejects_userinfo() {
        assert!(normalize_allowed_base_url("https://user@example.com/").is_err());
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_normalize_allowed_base_url_filesystem() {
        let tempdir = tempfile::tempdir().unwrap();
        let normalized = normalize_allowed_base_url(tempdir.path().to_str().unwrap()).unwrap();
        assert_eq!(
            normalized,
            AllowedBaseUrlPattern::FilePathPrefix(std::fs::canonicalize(tempdir.path()).unwrap())
        );
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_is_access_allowed_filesystem_canonicalization() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        let file_path = nested.join("data.json");
        std::fs::write(&file_path, "{}").unwrap();

        let patterns = parse_patterns(vec![root.path().to_str().unwrap()]);
        assert!(is_access_allowed(
            &format!("file://{}", file_path.display()),
            &patterns
        ));
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_is_access_allowed_rejects_parent_traversal() {
        let root = tempfile::tempdir().unwrap();
        let allowed = root.path().join("allowed");
        std::fs::create_dir_all(&allowed).unwrap();
        let outside = root.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let file_path = allowed.join("../outside/data.json");

        let patterns = parse_patterns(vec![allowed.to_str().unwrap()]);
        assert!(!is_access_allowed(
            &format!("file://{}", file_path.display()),
            &patterns
        ));
    }
}

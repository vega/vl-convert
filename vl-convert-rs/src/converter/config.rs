use crate::data_ops::{normalize_allowed_base_url, AllowedBaseUrlPattern};
use deno_core::anyhow::{anyhow, bail};
use deno_core::error::AnyError;
use deno_core::url::Url;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::path::Path;

use super::fonts::GoogleFontRequest;
use super::permissions::{domain_matches_patterns, is_filesystem_path};
use super::types::{FormatLocale, TimeFormatLocale};
use super::worker_pool::MIN_V8_HEAP_SIZE_MB;

/// How to handle fonts referenced in a spec but not available on the system.
///
/// Only the **first** non-generic font in each CSS `font-family` string is
/// checked (e.g. for `"Roboto, Arial, sans-serif"` only `Roboto` is examined).
/// This matches Vega's rendering behavior, which tries the first font and falls
/// back to system generics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingFontsPolicy {
    /// Silently fall back to the default font (no validation).
    #[default]
    Fallback,
    /// Log a warning for each missing first-choice font but continue rendering.
    Warn,
    /// Return an error if any first-choice font is missing.
    Error,
}

/// A plugin after resolution: URL fetched, HTTP imports bundled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPlugin {
    /// Original URL if this was a URL plugin (used for bundle=false HTML export).
    /// None for file-backed or inline plugins.
    pub original_url: Option<String>,
    /// Fully resolved, self-contained ESM source (HTTP imports inlined).
    /// This is what init_vega() loads into V8 and what bundle=true HTML embeds.
    pub bundled_source: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BaseUrlSetting {
    /// Resolve relative paths against the vega-datasets CDN
    #[default]
    Default,
    /// Relative paths produce an error
    Disabled,
    /// Resolve relative paths against a custom base URL or filesystem path
    Custom(String),
}

impl<'de> serde::Deserialize<'de> for BaseUrlSetting {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        match s.as_str() {
            "default" => Ok(BaseUrlSetting::Default),
            "disabled" => Ok(BaseUrlSetting::Disabled),
            _ => Ok(BaseUrlSetting::Custom(s)),
        }
    }
}

impl BaseUrlSetting {
    /// Resolve to the actual base URL string, or None if disabled.
    /// Filesystem paths are converted to file:// URLs.
    pub fn resolved_url(&self) -> Result<Option<String>, AnyError> {
        match self {
            Self::Default => Ok(Some(
                "https://cdn.jsdelivr.net/npm/vega-datasets@v2.9.0/".to_string(),
            )),
            Self::Disabled => Ok(None),
            Self::Custom(url) => {
                if !is_filesystem_path(url) && Url::parse(url).is_ok() {
                    Ok(Some(url.clone()))
                } else {
                    let path = super::portable_canonicalize(Path::new(url))
                        .map_err(|err| anyhow!("Failed to resolve base_url path {url}: {err}"))?;
                    let file_url = Url::from_directory_path(&path).map_err(|_| {
                        anyhow!(
                            "Failed to construct file URL from base_url path: {}",
                            path.display()
                        )
                    })?;
                    Ok(Some(file_url.to_string()))
                }
            }
        }
    }

    /// Whether this base URL resolves to a local filesystem path.
    pub fn is_filesystem(&self) -> bool {
        match self {
            Self::Default => false,
            Self::Disabled => false,
            Self::Custom(url) => {
                if is_filesystem_path(url) {
                    return true;
                }
                Url::parse(url)
                    .map(|parsed| parsed.scheme() == "file")
                    .unwrap_or(true) // Not a valid URL: bare filesystem path
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct VlcConfig {
    /// Number of persistent worker V8 isolates. Must be at least 1.
    pub num_workers: NonZeroU64,
    /// Base URL for resolving relative data paths in Vega specs.
    pub base_url: BaseUrlSetting,
    /// Allowlist for data access (HTTP URLs, filesystem paths).
    /// Uses CSP-style patterns: "https:" (scheme), "https://example.com/" (prefix),
    /// "/data/" (filesystem). Default is `["http:", "https:"]` — any
    /// HTTP/HTTPS URL is allowed; no filesystem access. Pass `Vec::new()`
    /// to block all network data; `["*"]` to allow everything.
    pub allowed_base_urls: Vec<String>,
    /// Whether to auto-download missing fonts from Google Fonts.
    pub auto_google_fonts: bool,
    /// Whether to embed locally-available fonts as base64 `@font-face` blocks
    /// in HTML and SVG output. Defaults to false.
    /// Does not apply to PDF/PNG/JPEG, which always embed fonts via fontdb.
    pub embed_local_fonts: bool,
    /// Whether to subset embedded fonts to only the characters used.
    /// Defaults to true. Applies to HTML and SVG output.
    /// When false, full font files are embedded and CDN URLs omit the `&text=` parameter.
    pub subset_fonts: bool,
    /// How to handle missing first-choice fonts: silently fallback, warn, or error.
    pub missing_fonts: MissingFontsPolicy,
    /// Google Fonts to register for all conversions. Each request specifies a
    /// family and optionally specific variants. Fonts are downloaded and
    /// registered per-request via the overlay mechanism. Empty = no
    /// configured fonts (the natural "unset" state).
    pub google_fonts: Vec<GoogleFontRequest>,
    /// Maximum V8 heap size in megabytes per worker. `None` = no cap;
    /// `Some(n)` = explicit cap.
    pub max_v8_heap_size_mb: Option<NonZeroU64>,
    /// Maximum V8 execution time in seconds. `None` = no cap; `Some(n)` =
    /// explicit cap. When exceeded, V8 execution is terminated and an error is
    /// returned. Only applies to the V8/JavaScript portion of the conversion
    /// (Vega evaluation, plugin loading); Rust-side post-processing is not
    /// subject to this limit.
    pub max_v8_execution_time_secs: Option<NonZeroU64>,
    /// Whether to run V8 garbage collection after each conversion to release
    /// memory back to the OS. Defaults to false. Enabling this reduces peak
    /// memory between conversions at the cost of slower throughput.
    pub gc_after_conversion: bool,
    /// User-provided Vega plugin ESM modules. Each string is either:
    /// - An HTTP/HTTPS URL (fetched and bundled at startup)
    /// - A file path ending in `.js` or `.mjs` (read at config normalization)
    /// - Raw ESM source code (used directly)
    ///
    /// Plugins must be single-entry ESM modules with a default export function
    /// that accepts a vega object. Empty = no plugins configured.
    pub vega_plugins: Vec<String>,
    /// Domain allowlist for HTTP/HTTPS imports inside plugins.
    /// Empty = disabled, `["*"]` = any domain, `["esm.sh"]` = specific domains.
    /// Domains from URL plugins are auto-added during normalization.
    /// Independent of `allowed_base_urls` (which controls data-fetching in specs).
    pub plugin_import_domains: Vec<String>,
    /// Whether to allow per-request plugins via `VgOpts`/`VlOpts`.
    /// Defaults to false. When enabled, requests can include a `vega_plugin`
    /// field that runs on an ephemeral V8 isolate (50-100ms overhead).
    pub allow_per_request_plugins: bool,
    /// Maximum number of concurrent ephemeral workers for per-request plugins.
    /// `None` = no limit. `Some(n)` = cap concurrent ephemeral V8 isolates.
    pub max_ephemeral_workers: Option<NonZeroU64>,
    /// Whether to allow per-request `google_fonts` / `auto_google_fonts` overrides.
    /// Defaults to false. When false, requests containing these fields are rejected.
    pub allow_google_fonts: bool,
    /// Domain allowlist for HTTP imports inside per-request plugins.
    /// Separate from `plugin_import_domains` (which controls config-level
    /// plugins). Defaults to empty (no HTTP imports allowed in per-request plugins).
    /// Set to `["esm.sh"]` to allow specific CDNs, or `["*"]` for any domain.
    pub per_request_plugin_import_domains: Vec<String>,
    /// Default theme applied to all Vega-Lite conversions.
    /// Per-request `VlOpts.theme` overrides this if set.
    pub default_theme: Option<String>,
    /// Default d3-format locale applied to all conversions.
    /// Per-request `format_locale` on VgOpts/VlOpts overrides this if set.
    pub default_format_locale: Option<FormatLocale>,
    /// Default d3-time-format locale applied to all conversions.
    /// Per-request `time_format_locale` on VgOpts/VlOpts overrides this if set.
    pub default_time_format_locale: Option<TimeFormatLocale>,
    /// Custom named themes (Vega config objects) registered alongside built-in
    /// vega-themes. Custom themes take priority if names collide. Empty =
    /// no custom themes.
    pub themes: HashMap<String, serde_json::Value>,
    /// Capacity (MB) of the on-disk Google Fonts LRU cache. `None` → library
    /// default. Backed by the process-global `GOOGLE_FONTS_CLIENT` via
    /// `apply_hot_font_cache`. Hot-applyable: `VlConverter::with_config`
    /// calls through on construction.
    pub google_fonts_cache_size_mb: Option<NonZeroU64>,
}

/// Shared context passed to all workers.
#[derive(Debug, Clone)]
pub(crate) struct ConverterContext {
    pub config: VlcConfig,
    /// Parsed allowlist patterns derived from `config.allowed_base_urls`.
    /// Computed once at construction to avoid re-parsing on every request.
    /// Empty = block everything.
    pub parsed_allowed_base_urls: Vec<AllowedBaseUrlPattern>,
    /// Resolved plugins after fetching URLs and bundling HTTP imports.
    /// Empty if no plugins configured.
    pub resolved_plugins: Vec<ResolvedPlugin>,
}

/// Backward-compatible alias for [`VlcConfig`].
#[deprecated(since = "2.0.0", note = "use VlcConfig instead")]
pub type VlConverterConfig = VlcConfig;

impl Default for VlcConfig {
    fn default() -> Self {
        // Sane profile for library, CLI, and server callers.
        //
        // - `allowed_base_urls = ["http:", "https:"]` — any HTTP/HTTPS URL is
        //   allowed; no filesystem access. Pass `Vec::new()` to block all
        //   network data; `["*"]` to allow everything (including
        //   filesystem reads).
        // - `max_v8_heap_size_mb = None` — no per-worker heap cap. Server
        //   deployments should set an explicit cap; local/embedded callers
        //   typically don't need one.
        // - `max_ephemeral_workers = Some(NZ(2))` — bounds ephemeral-worker
        //   concurrency (harmless when per-request plugins are disabled).
        Self {
            num_workers: NonZeroU64::new(1).expect("1 is non-zero"),
            base_url: BaseUrlSetting::Default,
            allowed_base_urls: vec!["http:".to_string(), "https:".to_string()],
            auto_google_fonts: false,
            embed_local_fonts: false,
            subset_fonts: true,
            missing_fonts: MissingFontsPolicy::Fallback,
            google_fonts: Vec::new(),
            max_v8_heap_size_mb: None,
            max_v8_execution_time_secs: None,
            gc_after_conversion: false,
            vega_plugins: Vec::new(),
            plugin_import_domains: Vec::new(),
            allow_per_request_plugins: false,
            max_ephemeral_workers: NonZeroU64::new(2),
            allow_google_fonts: false,
            per_request_plugin_import_domains: Vec::new(),
            default_theme: None,
            default_format_locale: None,
            default_time_format_locale: None,
            themes: HashMap::new(),
            google_fonts_cache_size_mb: None,
        }
    }
}

impl VlcConfig {
    /// Load a `VlcConfig` from a file. Supports JSONC (JSON with support for
    /// comments and trailing commas). Relative paths in `vega_plugins` and
    /// filesystem `base_url` values are resolved against the file's directory.
    pub fn from_file(path: &std::path::Path) -> Result<Self, AnyError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read config file {}: {}", path.display(), e))?;
        let value = jsonc_parser::parse_to_serde_value(
            &text,
            &jsonc_parser::ParseOptions {
                allow_comments: true,
                allow_trailing_commas: true,
                allow_loose_object_property_names: false,
                allow_missing_commas: false,
                allow_single_quoted_strings: false,
                allow_hexadecimal_numbers: false,
                allow_unary_plus_numbers: false,
            },
        )
        .map_err(|e| anyhow!("Failed to parse config file {}: {}", path.display(), e))?;
        let Some(value) = value else {
            return Ok(VlcConfig::default());
        };
        let mut config: VlcConfig = serde_json::from_value(value).map_err(|e| {
            anyhow!(
                "Failed to deserialize config file {}: {}",
                path.display(),
                e
            )
        })?;

        // Resolve relative filesystem paths against the config file's directory.
        // URL strings (containing "://") and inline ESM source (containing
        // newlines or starting with "export"/"import") are left untouched.
        if let Some(config_dir) = path.parent() {
            for plugin in config.vega_plugins.iter_mut() {
                if plugin.contains("://")
                    || plugin.contains('\n')
                    || plugin.starts_with("export")
                    || plugin.starts_with("import")
                {
                    continue;
                }
                let p = std::path::Path::new(plugin.as_str());
                if p.is_relative() {
                    *plugin = config_dir.join(p).to_string_lossy().to_string();
                }
            }
            if let BaseUrlSetting::Custom(ref mut url) = config.base_url {
                if !url.contains("://") {
                    let p = std::path::Path::new(url.as_str());
                    if p.is_relative() {
                        *url = config_dir.join(p).to_string_lossy().to_string();
                    }
                }
            }
        }

        Ok(config)
    }
}

pub fn normalize_converter_config(mut config: VlcConfig) -> Result<VlcConfig, AnyError> {
    // `num_workers` is `NonZeroU64` (type-level guarantee); no runtime check needed.

    // Validate allowed_base_urls by parsing them (the parsed patterns are
    // stored on ConverterContext, not on the config itself)
    for url in &config.allowed_base_urls {
        normalize_allowed_base_url(url)?;
    }

    if let Some(max_mb) = config.max_v8_heap_size_mb {
        if max_mb.get() < MIN_V8_HEAP_SIZE_MB {
            bail!(
                "max_v8_heap_size_mb is {} MB, which is too small for V8 to \
                 initialize. Set to {} or higher, or omit to use no limit.",
                max_mb.get(),
                MIN_V8_HEAP_SIZE_MB,
            );
        }
    }

    // Classify and resolve vega plugins (sync: file reads and URL validation only)
    let mut plugin_entries = std::mem::take(&mut config.vega_plugins);
    for (i, entry) in plugin_entries.iter_mut().enumerate() {
        if entry.starts_with("http://") || entry.starts_with("https://") {
            // URL plugin: validate URL, auto-allow the domain
            let url = Url::parse(entry)
                .map_err(|e| anyhow!("Invalid Vega plugin {i} URL: {entry}: {e}"))?;
            if let Some(domain) = url.host_str() {
                if !domain_matches_patterns(domain, &config.plugin_import_domains) {
                    config.plugin_import_domains.push(domain.to_string());
                }
            }
            // Leave the URL string in place; fetched at startup in spawn_worker_pool()
        } else {
            let path = Path::new(entry.as_str());
            if entry.ends_with(".js") || entry.ends_with(".mjs") {
                // File plugin: read source, replace entry
                if !path.is_file() {
                    bail!(
                        "Vega plugin {i} path '{}' does not exist or is not a file",
                        path.display()
                    );
                }
                *entry = std::fs::read_to_string(path).map_err(|e| {
                    anyhow!("Failed to read Vega plugin {i} at {}: {e}", path.display())
                })?;
            }
        }
    }
    config.vega_plugins = plugin_entries;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_empty_config() {
        let config: VlcConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config, VlcConfig::default());
    }

    #[test]
    fn test_base_url_setting_default() {
        let config: VlcConfig = serde_json::from_str(r#"{"base_url": "default"}"#).unwrap();
        assert_eq!(config.base_url, BaseUrlSetting::Default);
    }

    #[test]
    fn test_base_url_setting_disabled() {
        let config: VlcConfig = serde_json::from_str(r#"{"base_url": "disabled"}"#).unwrap();
        assert_eq!(config.base_url, BaseUrlSetting::Disabled);
    }

    #[test]
    fn test_base_url_setting_custom() {
        let config: VlcConfig =
            serde_json::from_str(r#"{"base_url": "https://example.com/"}"#).unwrap();
        assert_eq!(
            config.base_url,
            BaseUrlSetting::Custom("https://example.com/".to_string())
        );
    }

    #[test]
    fn test_missing_fonts_policy() {
        for (json, expected) in [
            ("\"fallback\"", MissingFontsPolicy::Fallback),
            ("\"warn\"", MissingFontsPolicy::Warn),
            ("\"error\"", MissingFontsPolicy::Error),
        ] {
            let policy: MissingFontsPolicy = serde_json::from_str(json).unwrap();
            assert_eq!(policy, expected);
        }
    }

    #[test]
    fn test_format_locale_name() {
        let locale: FormatLocale = serde_json::from_str(r#""de-DE""#).unwrap();
        assert!(matches!(locale, FormatLocale::Name(ref s) if s == "de-DE"));
    }

    #[test]
    fn test_format_locale_object() {
        let locale: FormatLocale =
            serde_json::from_str(r#"{"decimal": ",", "thousands": "."}"#).unwrap();
        assert!(matches!(locale, FormatLocale::Object(_)));
    }

    #[test]
    fn test_google_font_request() {
        let req: GoogleFontRequest = serde_json::from_str(r#"{"family": "Roboto"}"#).unwrap();
        assert_eq!(req.family, "Roboto");
        assert!(req.variants.is_none());
    }

    #[test]
    fn test_full_config() {
        let json = r##"{
            "num_workers": 2,
            "auto_google_fonts": true,
            "missing_fonts": "warn",
            "max_v8_heap_size_mb": 512,
            "default_theme": "dark",
            "themes": {
                "custom": {"background": "#333"}
            }
        }"##;
        let config: VlcConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.num_workers.get(), 2);
        assert!(config.auto_google_fonts);
        assert_eq!(config.missing_fonts, MissingFontsPolicy::Warn);
        assert_eq!(config.max_v8_heap_size_mb, NonZeroU64::new(512));
        assert_eq!(config.default_theme, Some("dark".to_string()));
        assert!(!config.themes.is_empty());
    }

    #[test]
    fn test_jsonc_comments() {
        let jsonc = r#"{
            // This is a comment
            "auto_google_fonts": true,
            /* Block comment */
            "missing_fonts": "warn"
        }"#;
        let value: serde_json::Value = jsonc_parser::parse_to_serde_value(
            jsonc,
            &jsonc_parser::ParseOptions {
                allow_comments: true,
                allow_trailing_commas: true,
                allow_loose_object_property_names: false,
                allow_missing_commas: false,
                allow_single_quoted_strings: false,
                allow_hexadecimal_numbers: false,
                allow_unary_plus_numbers: false,
            },
        )
        .unwrap();
        let config: VlcConfig = serde_json::from_value(value).unwrap();
        assert!(config.auto_google_fonts);
        assert_eq!(config.missing_fonts, MissingFontsPolicy::Warn);
    }

    #[test]
    fn test_unknown_fields_ignored() {
        let json = r#"{"unknown_field": 42, "auto_google_fonts": true}"#;
        let config: VlcConfig = serde_json::from_str(json).unwrap();
        assert!(config.auto_google_fonts);
    }

    #[test]
    fn test_from_file_url_base_url_not_rebased() {
        let mut config_file = tempfile::NamedTempFile::with_suffix(".jsonc").unwrap();
        writeln!(
            config_file,
            r#"{{"base_url": "https://cdn.example.com/data/"}}"#
        )
        .unwrap();
        let config = VlcConfig::from_file(config_file.path()).unwrap();
        assert_eq!(
            config.base_url,
            BaseUrlSetting::Custom("https://cdn.example.com/data/".to_string())
        );
    }

    #[test]
    fn test_from_file_url_plugin_not_rebased() {
        let mut config_file = tempfile::NamedTempFile::with_suffix(".jsonc").unwrap();
        writeln!(
            config_file,
            r#"{{"vega_plugins": ["https://esm.sh/my-plugin@1.0"]}}"#
        )
        .unwrap();
        let config = VlcConfig::from_file(config_file.path()).unwrap();
        assert_eq!(
            config.vega_plugins,
            vec!["https://esm.sh/my-plugin@1.0".to_string()]
        );
    }

    #[test]
    fn test_from_file_inline_esm_plugin_not_rebased() {
        let inline = "export default function(vega) {}";
        let mut config_file = tempfile::NamedTempFile::with_suffix(".jsonc").unwrap();
        writeln!(config_file, r#"{{"vega_plugins": ["{inline}"]}}"#).unwrap();
        let config = VlcConfig::from_file(config_file.path()).unwrap();
        assert_eq!(config.vega_plugins, vec![inline.to_string()]);
    }

    #[test]
    fn test_from_file_relative_plugin_rebased() {
        let mut config_file = tempfile::NamedTempFile::with_suffix(".jsonc").unwrap();
        writeln!(config_file, r#"{{"vega_plugins": ["./my-plugin.js"]}}"#).unwrap();
        let config = VlcConfig::from_file(config_file.path()).unwrap();
        let expected = config_file
            .path()
            .parent()
            .unwrap()
            .join("./my-plugin.js")
            .to_string_lossy()
            .to_string();
        assert_eq!(config.vega_plugins, vec![expected]);
    }

    #[test]
    fn default_config_values() {
        let cfg = VlcConfig::default();
        assert_eq!(cfg.num_workers.get(), 1, "num_workers default is 1");
        assert_eq!(
            cfg.allowed_base_urls,
            vec!["http:".to_string(), "https:".to_string()],
            "allowed_base_urls default permits HTTP/HTTPS"
        );
        assert_eq!(
            cfg.max_v8_heap_size_mb, None,
            "max_v8_heap_size_mb default is None (no cap)"
        );
        assert_eq!(
            cfg.max_ephemeral_workers,
            NonZeroU64::new(2),
            "max_ephemeral_workers default is Some(NZ(2))"
        );
        assert_eq!(cfg.max_v8_execution_time_secs, None);
        assert!(cfg.google_fonts.is_empty());
        assert!(cfg.vega_plugins.is_empty());
        assert!(cfg.themes.is_empty());
        assert_eq!(cfg.google_fonts_cache_size_mb, None);
    }

    #[test]
    fn num_workers_zero_is_deserialize_error() {
        let err = serde_json::from_str::<VlcConfig>(r#"{"num_workers": 0}"#).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("zero") || msg.contains("non-zero"),
            "expected NonZero error, got: {msg}"
        );
    }

    #[test]
    fn max_v8_heap_size_mb_below_minimum_rejected() {
        // Some(NZ(1)) is below MIN_V8_HEAP_SIZE_MB
        let err = normalize_converter_config(VlcConfig {
            max_v8_heap_size_mb: NonZeroU64::new(1),
            ..Default::default()
        })
        .err()
        .unwrap();
        let msg = err.to_string();
        assert!(
            msg.contains("max_v8_heap_size_mb"),
            "expected max_v8_heap_size_mb error, got: {msg}"
        );
    }
}

use crate::data_ops::{
    normalize_allowed_base_url, normalize_allowed_base_urls, AllowedBaseUrlPattern,
};
use deno_core::anyhow::{anyhow, bail};
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_runtime::deno_permissions::{
    Permissions, PermissionsOptions, RuntimePermissionDescriptorParser,
};
use std::collections::HashMap;
use std::path::Path;

use crate::deno_stubs::VlConvertNodeSys;
use vl_convert_google_fonts::VariantRequest;

use super::types::{FormatLocale, TimeFormatLocale};
use super::worker_pool::MIN_V8_HEAP_SIZE_MB;

/// A JSON value that may already be serialized to a string.
/// When the caller already has a JSON string (e.g. from Python), this avoids
/// a redundant parse->Value->serialize round-trip.
#[derive(Debug, Clone)]
pub enum ValueOrString {
    /// Pre-serialized JSON string -- stored directly, no serialization needed
    JsonString(String),
    /// Parsed serde_json::Value -- will be serialized to JSON when needed
    Value(serde_json::Value),
}

impl From<serde_json::Value> for ValueOrString {
    fn from(v: serde_json::Value) -> Self {
        ValueOrString::Value(v)
    }
}

impl From<&serde_json::Value> for ValueOrString {
    fn from(v: &serde_json::Value) -> Self {
        ValueOrString::Value(v.clone())
    }
}

impl From<String> for ValueOrString {
    fn from(s: String) -> Self {
        ValueOrString::JsonString(s)
    }
}

impl ValueOrString {
    /// Convert to a serde_json::Value, parsing if necessary.
    pub fn to_value(self) -> Result<serde_json::Value, AnyError> {
        match self {
            ValueOrString::Value(v) => Ok(v),
            ValueOrString::JsonString(s) => Ok(serde_json::from_str(&s)?),
        }
    }
}

/// Apply background, width, and height overrides to a spec.
/// Works for both Vega and Vega-Lite specs.
pub(crate) fn apply_spec_overrides(
    spec: ValueOrString,
    background: &Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> Result<ValueOrString, AnyError> {
    if background.is_none() && width.is_none() && height.is_none() {
        return Ok(spec);
    }
    let mut val = spec.to_value()?;
    if let Some(obj) = val.as_object_mut() {
        if let Some(bg) = background {
            obj.insert(
                "background".to_string(),
                serde_json::Value::String(bg.clone()),
            );
        }
        if let Some(w) = width {
            obj.insert(
                "width".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(w as f64).unwrap_or(serde_json::Number::from(0)),
                ),
            );
        }
        if let Some(h) = height {
            obj.insert(
                "height".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(h as f64).unwrap_or(serde_json::Number::from(0)),
                ),
            );
        }
    }
    Ok(ValueOrString::Value(val))
}

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

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct VlcConfig {
    pub num_workers: usize,
    /// Base URL for resolving relative data paths in Vega specs.
    pub base_url: BaseUrlSetting,
    /// Allowlist for data access (HTTP URLs, filesystem paths).
    /// Uses CSP-style patterns: "https:" (scheme), "https://example.com/" (prefix),
    /// "/data/" (filesystem). None = any HTTP/HTTPS, no filesystem (default).
    /// Some(vec![]) = no access. Some(vec!["*"]) = everything.
    pub allowed_base_urls: Option<Vec<String>>,
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
    /// registered per-request via the overlay mechanism.
    pub google_fonts: Option<Vec<GoogleFontRequest>>,
    /// Maximum V8 heap size in megabytes per worker. Defaults to 0 (no limit).
    /// Set to a positive value to cap V8 heap usage per worker.
    pub max_v8_heap_size_mb: usize,
    /// Maximum V8 execution time in seconds. Defaults to 0 (no limit).
    /// When exceeded, V8 execution is terminated and an error is returned.
    /// Only applies to the V8/JavaScript portion of the conversion (Vega
    /// evaluation, plugin loading); Rust-side post-processing is not subject
    /// to this limit.
    pub max_v8_execution_time_secs: u64,
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
    /// that accepts a vega object.
    pub vega_plugins: Option<Vec<String>>,
    /// Domain allowlist for HTTP/HTTPS imports inside plugins.
    /// Empty = disabled, `["*"]` = any domain, `["esm.sh"]` = specific domains.
    /// Domains from URL plugins are auto-added during normalization.
    /// Independent of `allowed_base_urls` (which controls data-fetching in specs).
    pub plugin_import_domains: Vec<String>,
    /// Whether to allow per-request plugins via `VgOpts`/`VlOpts`.
    /// Defaults to false. When enabled, requests can include a `vega_plugin`
    /// field that runs on an ephemeral V8 isolate (50-100ms overhead).
    pub allow_per_request_plugins: bool,
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
    /// vega-themes. Custom themes take priority if names collide.
    pub themes: Option<HashMap<String, serde_json::Value>>,
}

/// Shared context passed to all workers.
#[derive(Debug, Clone)]
pub(crate) struct ConverterContext {
    pub config: VlcConfig,
    /// Parsed allowlist patterns derived from `config.allowed_base_urls`.
    /// Computed once at construction to avoid re-parsing on every request.
    pub parsed_allowed_base_urls: Option<Vec<AllowedBaseUrlPattern>>,
    /// Resolved plugins after fetching URLs and bundling HTTP imports.
    /// None if `vega_plugins` is None.
    pub resolved_plugins: Option<Vec<ResolvedPlugin>>,
}

/// Backward-compatible alias for [`VlcConfig`].
#[deprecated(since = "2.0.0", note = "use VlcConfig instead")]
pub type VlConverterConfig = VlcConfig;

impl Default for VlcConfig {
    fn default() -> Self {
        Self {
            num_workers: 1,
            base_url: BaseUrlSetting::Default,
            allowed_base_urls: None,
            auto_google_fonts: false,
            embed_local_fonts: false,
            subset_fonts: true,
            missing_fonts: MissingFontsPolicy::Fallback,
            google_fonts: None,
            max_v8_heap_size_mb: 0,
            max_v8_execution_time_secs: 0,
            gc_after_conversion: false,
            vega_plugins: None,
            plugin_import_domains: Vec::new(),
            allow_per_request_plugins: false,
            allow_google_fonts: false,
            per_request_plugin_import_domains: Vec::new(),
            default_theme: None,
            default_format_locale: None,
            default_time_format_locale: None,
            themes: None,
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
            if let Some(ref mut plugins) = config.vega_plugins {
                for plugin in plugins.iter_mut() {
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct GoogleFontRequest {
    pub family: String,
    pub variants: Option<Vec<VariantRequest>>,
}

pub(crate) fn normalize_converter_config(mut config: VlcConfig) -> Result<VlcConfig, AnyError> {
    if config.num_workers < 1 {
        bail!("num_workers must be >= 1");
    }

    // Validate allowed_base_urls by parsing them (the parsed patterns are
    // stored on ConverterContext, not on the config itself)
    if let Some(ref urls) = config.allowed_base_urls {
        for url in urls {
            normalize_allowed_base_url(url)?;
        }
    }

    if config.max_v8_heap_size_mb > 0 && config.max_v8_heap_size_mb < MIN_V8_HEAP_SIZE_MB {
        bail!(
            "max_v8_heap_size_mb is {} MB, which is too small for V8 to \
             initialize. Set to {} or higher, or use 0 for no limit.",
            config.max_v8_heap_size_mb,
            MIN_V8_HEAP_SIZE_MB,
        );
    }

    // Classify and resolve vega plugins (sync: file reads and URL validation only)
    if let Some(ref mut plugins) = config.vega_plugins {
        for (i, entry) in plugins.iter_mut().enumerate() {
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
    }

    Ok(config)
}

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

use std::path::PathBuf;

const DEFAULT_MAX_BLOB_CACHE_BYTES: u64 = 512 * 1024 * 1024;
const DEFAULT_MAX_PARALLEL_DOWNLOADS: usize = 8;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_RETRIES: usize = 3;

/// Environment variable to override the font cache root directory.
///
/// Set to a path to override the default cache location.
/// Set to `"none"` to disable persistent caching entirely (in-memory only).
const ENV_FONT_CACHE_DIR: &str = "VL_CONVERT_FONT_CACHE_DIR";

/// Environment variable to override the Google Fonts CSS2 API base URL.
///
/// Set to a URL to use a custom mirror or local test server.
const ENV_GOOGLE_FONTS_CSS2_URL: &str = "VL_CONVERT_GOOGLE_FONTS_CSS2_URL";

/// Runtime configuration for [`GoogleFontsClient`](crate::GoogleFontsClient).
///
/// When `cache_dir` is `None`, the client operates without persistent caching:
/// fonts are always fetched from the network and blobs are never written to disk.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Absolute path to the cache root. `None` disables persistent caching.
    pub cache_dir: Option<PathBuf>,
    /// Maximum size of the on-disk blob cache in bytes. `0` disables eviction.
    pub max_blob_cache_bytes: u64,
    /// Maximum number of concurrent font file downloads.
    pub max_parallel_downloads: usize,
    /// Per-request HTTP timeout in seconds.
    pub request_timeout_secs: u64,
    /// Number of retries for transient HTTP failures.
    pub max_retries: usize,
    /// HTTP `User-Agent` header value.
    pub user_agent: String,
    /// Base URL for the Google Fonts CSS2 API.
    pub css2_base_url: String,
}

impl ClientConfig {
    /// Returns the blob subdirectory, or `None` if caching is disabled.
    pub fn blob_dir(&self) -> Option<PathBuf> {
        self.cache_dir.as_ref().map(|d| d.join("blobs"))
    }
}

/// Returns the resolved Google Fonts cache directory, or `None` if caching is disabled.
///
/// Resolution order:
/// 1. `VL_CONVERT_FONT_CACHE_DIR` env var set to `"none"` → `None`
/// 2. `VL_CONVERT_FONT_CACHE_DIR` env var set to a path → `Some(path)`
/// 3. OS cache dir fallback → `Some(<cache_dir>/vl-convert/google-fonts)`
pub fn google_fonts_cache_dir() -> Option<PathBuf> {
    match std::env::var(ENV_FONT_CACHE_DIR) {
        Ok(val) if val.eq_ignore_ascii_case("none") => None,
        Ok(val) => Some(PathBuf::from(val)),
        Err(_) => dirs::cache_dir().map(|base| base.join("vl-convert").join("google-fonts")),
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            cache_dir: google_fonts_cache_dir(),
            max_blob_cache_bytes: DEFAULT_MAX_BLOB_CACHE_BYTES,
            max_parallel_downloads: DEFAULT_MAX_PARALLEL_DOWNLOADS,
            request_timeout_secs: DEFAULT_TIMEOUT_SECS,
            max_retries: DEFAULT_MAX_RETRIES,
            user_agent: "vl-convert".to_string(),
            css2_base_url: std::env::var(ENV_GOOGLE_FONTS_CSS2_URL)
                .unwrap_or_else(|_| "https://fonts.googleapis.com/css2".to_string()),
        }
    }
}

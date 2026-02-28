use std::path::PathBuf;

const DEFAULT_MAX_BLOB_CACHE_BYTES: u64 = 512 * 1024 * 1024;
const DEFAULT_MAX_PARALLEL_DOWNLOADS: usize = 8;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_RETRIES: usize = 3;

/// Runtime configuration for [`FontsourceClient`](crate::FontsourceClient).
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub metadata_cache_dir: PathBuf,
    pub blob_cache_dir: PathBuf,
    pub max_blob_cache_bytes: u64,
    pub max_parallel_downloads: usize,
    pub request_timeout_secs: u64,
    pub max_retries: usize,
    pub user_agent: String,
    pub metadata_base_url: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        let base = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
        let vl_convert = base.join("vl-convert");
        Self {
            metadata_cache_dir: vl_convert.join("fontsource-metadata"),
            blob_cache_dir: vl_convert.join("fontsource-blobs"),
            max_blob_cache_bytes: DEFAULT_MAX_BLOB_CACHE_BYTES,
            max_parallel_downloads: DEFAULT_MAX_PARALLEL_DOWNLOADS,
            request_timeout_secs: DEFAULT_TIMEOUT_SECS,
            max_retries: DEFAULT_MAX_RETRIES,
            user_agent: "vl-convert".to_string(),
            metadata_base_url: "https://api.fontsource.org/v1/fonts".to_string(),
        }
    }
}

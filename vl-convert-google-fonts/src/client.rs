use crate::cache;
use crate::config::ClientConfig;
use crate::error::GoogleFontsError;
use crate::resolve::{
    build_css2_url, build_css2_url_all_variants, resolve_from_css2, ResolvedTtfFile,
};
use crate::types::{family_to_id, LoadedFontBatch, VariantRequest};
use backon::{BlockingRetryable, ExponentialBuilder, Retryable};
use dashmap::DashMap;
use futures_util::stream::{self, StreamExt};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Result of downloading/loading font blobs from cache or network.
struct EnsureBlobsResult {
    font_data: Vec<Arc<Vec<u8>>>,
    blob_keys: HashSet<String>,
    downloaded_any: bool,
}

/// Per-blob-key mutex that serializes concurrent download/load of the same font file
/// to avoid repeated simultaneous downloads.
struct DownloadGate {
    mutex: tokio::sync::Mutex<()>,
    active_users: AtomicUsize,
}

impl DownloadGate {
    fn new() -> Self {
        Self {
            mutex: tokio::sync::Mutex::new(()),
            active_users: AtomicUsize::new(0),
        }
    }
}

/// RAII guard that calls `release_download_gate` on drop, ensuring cleanup
/// even on early return, error, or async cancellation.
struct DownloadGateGuard<'a> {
    client: &'a GoogleFontsClient,
    key: String,
    gate: Arc<DownloadGate>,
}

impl Drop for DownloadGateGuard<'_> {
    fn drop(&mut self) {
        self.client.release_download_gate(&self.key, &self.gate);
    }
}

/// Google Fonts font loader client.
pub struct GoogleFontsClient {
    config: ClientConfig,
    /// Built eagerly — unlike the blocking client, this is safe to construct
    /// inside an async context.
    async_client: reqwest::Client,
    /// Lazily initialized: creates an internal tokio runtime, so must not be
    /// built inside an async context.
    blocking_client: Mutex<Option<reqwest::blocking::Client>>,
    max_blob_cache_bytes: AtomicU64,
    download_gates: DashMap<String, Arc<DownloadGate>>,
}

impl GoogleFontsClient {
    /// Create a new client from the given configuration.
    pub fn new(mut config: ClientConfig) -> Result<Self, GoogleFontsError> {
        if config.max_parallel_downloads == 0 {
            config.max_parallel_downloads = 1;
        }

        if let Some(ref dir) = config.cache_dir {
            if !dir.is_absolute() {
                return Err(GoogleFontsError::RelativeCacheDir(dir.clone()));
            }
            std::fs::create_dir_all(dir.join("blobs"))?;
        }

        let async_client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .map_err(|e| GoogleFontsError::Http(e.to_string()))?;

        Ok(Self {
            max_blob_cache_bytes: AtomicU64::new(config.max_blob_cache_bytes),
            config,
            async_client,
            blocking_client: Mutex::new(None),
            download_gates: DashMap::new(),
        })
    }

    /// Update the maximum blob cache size (in bytes) at runtime.
    pub fn set_max_blob_cache_bytes(&self, bytes: u64) {
        self.max_blob_cache_bytes.store(bytes, Ordering::Relaxed);
    }

    /// Load a font family from Google Fonts (async).
    pub async fn load(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<LoadedFontBatch, GoogleFontsError> {
        let font_id = Self::validate_load_request(family, variants)?;

        // Build the CSS2 URL
        let css_url = match variants {
            Some(requested) => build_css2_url(&self.config.css2_base_url, family, requested),
            None => build_css2_url_all_variants(&self.config.css2_base_url, family),
        };

        // Fetch CSS2 response (always re-fetch, no caching)
        let css_bytes = self.get_bytes_with_retry_async(&css_url).await?;
        let css = String::from_utf8(css_bytes)
            .map_err(|_| GoogleFontsError::Internal("CSS2 response was not valid UTF-8".into()))?;

        // Check for font not found (200 with no @font-face blocks)
        if !css.contains("@font-face") {
            return Err(GoogleFontsError::FontNotFound(font_id.clone()));
        }

        let plan = resolve_from_css2(&font_id, &css, variants)?;
        let blobs = self.ensure_blobs_async(&plan.files).await?;

        if blobs.downloaded_any {
            self.evict_if_needed(&blobs.blob_keys)?;
        }

        let loaded_variants = plan
            .files
            .iter()
            .map(|f| VariantRequest {
                weight: f.weight,
                style: f.style,
            })
            .collect();

        Ok(LoadedFontBatch::new(
            font_id,
            loaded_variants,
            blobs.font_data.len(),
            blobs.font_data,
        ))
    }

    /// Load a font family from Google Fonts (blocking).
    pub fn load_blocking(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<LoadedFontBatch, GoogleFontsError> {
        let font_id = Self::validate_load_request(family, variants)?;

        // Build the CSS2 URL
        let css_url = match variants {
            Some(requested) => build_css2_url(&self.config.css2_base_url, family, requested),
            None => build_css2_url_all_variants(&self.config.css2_base_url, family),
        };

        // Fetch CSS2 response (always re-fetch, no caching)
        let css_bytes = self.get_bytes_with_retry_blocking(&css_url)?;
        let css = String::from_utf8(css_bytes)
            .map_err(|_| GoogleFontsError::Internal("CSS2 response was not valid UTF-8".into()))?;

        // Check for font not found (200 with no @font-face blocks)
        if !css.contains("@font-face") {
            return Err(GoogleFontsError::FontNotFound(font_id.clone()));
        }

        let plan = resolve_from_css2(&font_id, &css, variants)?;
        let blobs = self.ensure_blobs_blocking(&plan.files)?;

        if blobs.downloaded_any {
            self.evict_if_needed(&blobs.blob_keys)?;
        }

        let loaded_variants = plan
            .files
            .iter()
            .map(|f| VariantRequest {
                weight: f.weight,
                style: f.style,
            })
            .collect();

        Ok(LoadedFontBatch::new(
            font_id,
            loaded_variants,
            blobs.font_data.len(),
            blobs.font_data,
        ))
    }

    /// Check whether a font exists on Google Fonts without downloading blobs (async).
    ///
    /// Probes the CSS2 API with a single weight to check for `@font-face` presence.
    /// `family` should be the display name (e.g., "Kalam", not "kalam") since the
    /// CSS2 API is case-sensitive for font family names.
    pub async fn is_known_font(&self, family: &str) -> Result<bool, GoogleFontsError> {
        let probe_url = format!(
            "{}?family={}:wght@400&display=swap",
            self.config.css2_base_url.trim_end_matches('/'),
            urlencoding::encode(family)
        );
        match self.get_bytes_with_retry_async(&probe_url).await {
            Ok(bytes) => {
                let css = String::from_utf8_lossy(&bytes);
                Ok(css.contains("@font-face"))
            }
            Err(GoogleFontsError::HttpStatus { status: 400, .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Validate load arguments and return the normalized font ID.
    fn validate_load_request(
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<String, GoogleFontsError> {
        let font_id = family_to_id(family)
            .ok_or_else(|| GoogleFontsError::InvalidFontId(family.to_string()))?;
        if let Some(requested) = variants {
            if requested.is_empty() {
                return Err(GoogleFontsError::NoVariantsRequested);
            }
        }
        Ok(font_id)
    }

    /// Try to read a blob from the cache, touching its mtime on hit.
    fn try_read_cached_blob(
        url: &str,
        blob_dir: &Option<std::path::PathBuf>,
    ) -> Result<Option<Vec<u8>>, GoogleFontsError> {
        if let Some(ref dir) = blob_dir {
            if let Some(bytes) = cache::read_blob(url, dir)? {
                let _ = cache::touch_blob(url, dir);
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }

    /// Write a downloaded blob to the cache if a cache directory is configured.
    fn cache_blob(
        url: &str,
        blob_dir: &Option<std::path::PathBuf>,
        bytes: &[u8],
    ) -> Result<(), GoogleFontsError> {
        if let Some(ref dir) = blob_dir {
            cache::write_blob_if_absent(url, dir, bytes)?;
        }
        Ok(())
    }

    /// Read the current blob cache size limit.
    fn max_blob_cache_bytes(&self) -> u64 {
        self.max_blob_cache_bytes.load(Ordering::Relaxed)
    }

    /// Download or retrieve from cache all resolved TTF files in parallel (async).
    async fn ensure_blobs_async(
        &self,
        files: &[ResolvedTtfFile],
    ) -> Result<EnsureBlobsResult, GoogleFontsError> {
        let limit = self.config.max_parallel_downloads.max(1);

        let results = stream::iter(files.iter().cloned().enumerate().map(
            |(index, file)| async move {
                self.ensure_blob_async(&file)
                    .await
                    .map(|(bytes, key, was_downloaded)| (index, bytes, key, was_downloaded))
            },
        ))
        .buffer_unordered(limit)
        .collect::<Vec<_>>()
        .await;

        let mut result_vec = Vec::with_capacity(results.len());
        for result in results {
            result_vec.push(result?);
        }
        result_vec.sort_by_key(|(idx, _, _, _)| *idx);

        let mut font_data = Vec::with_capacity(files.len());
        let mut blob_keys = HashSet::new();
        let mut downloaded_any = false;

        for (_, bytes, key, was_downloaded) in result_vec {
            font_data.push(Arc::new(bytes));
            blob_keys.insert(key);
            downloaded_any |= was_downloaded;
        }

        Ok(EnsureBlobsResult {
            font_data,
            blob_keys,
            downloaded_any,
        })
    }

    /// Return a single blob from cache or download it, using a gate for dedup (async).
    async fn ensure_blob_async(
        &self,
        file: &ResolvedTtfFile,
    ) -> Result<(Vec<u8>, String, bool), GoogleFontsError> {
        let key = cache::blob_key(&file.url);
        let blob_dir = self.config.blob_dir();

        if let Some(bytes) = Self::try_read_cached_blob(&file.url, &blob_dir)? {
            return Ok((bytes, key, false));
        }

        // Without a cache dir, the gate can't deduplicate (waiters would just
        // re-download anyway), so skip it and download directly.
        if blob_dir.is_none() {
            let bytes = self.get_bytes_with_retry_async(&file.url).await?;
            return Ok((bytes, key, true));
        }

        let gate_guard = DownloadGateGuard {
            client: self,
            key: key.clone(),
            gate: self.acquire_download_gate(&key),
        };
        let _lock = gate_guard.gate.mutex.lock().await;

        if let Some(bytes) = Self::try_read_cached_blob(&file.url, &blob_dir)? {
            return Ok((bytes, key, false));
        }

        let bytes = self.get_bytes_with_retry_async(&file.url).await?;
        Self::cache_blob(&file.url, &blob_dir, &bytes)?;
        Ok((bytes, key, true))
    }

    /// Download or retrieve from cache all resolved TTF files in parallel (blocking).
    fn ensure_blobs_blocking(
        &self,
        files: &[ResolvedTtfFile],
    ) -> Result<EnsureBlobsResult, GoogleFontsError> {
        if files.is_empty() {
            return Ok(EnsureBlobsResult {
                font_data: Vec::new(),
                blob_keys: HashSet::new(),
                downloaded_any: false,
            });
        }

        let workers = self.config.max_parallel_downloads.max(1).min(files.len());
        let chunk_size = files.len().div_ceil(workers);

        let thread_results: Vec<Vec<_>> = std::thread::scope(|scope| {
            files
                .chunks(chunk_size)
                .map(|chunk| {
                    scope.spawn(|| {
                        chunk
                            .iter()
                            .map(|file| self.ensure_blob_blocking(file))
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|h| {
                    h.join().map_err(|_| {
                        GoogleFontsError::Internal("Font download thread panicked".to_string())
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })?;

        let mut font_data = Vec::with_capacity(files.len());
        let mut blob_keys = HashSet::new();
        let mut downloaded_any = false;

        for chunk in thread_results {
            for result in chunk {
                let (bytes, key, was_downloaded) = result?;
                font_data.push(Arc::new(bytes));
                blob_keys.insert(key);
                downloaded_any |= was_downloaded;
            }
        }

        Ok(EnsureBlobsResult {
            font_data,
            blob_keys,
            downloaded_any,
        })
    }

    /// Return a single blob from cache or download it, using a gate for dedup (blocking).
    fn ensure_blob_blocking(
        &self,
        file: &ResolvedTtfFile,
    ) -> Result<(Vec<u8>, String, bool), GoogleFontsError> {
        let key = cache::blob_key(&file.url);
        let blob_dir = self.config.blob_dir();

        if let Some(bytes) = Self::try_read_cached_blob(&file.url, &blob_dir)? {
            return Ok((bytes, key, false));
        }

        // Without a cache dir, the gate can't deduplicate (waiters would just
        // re-download anyway), so skip it and download directly.
        if blob_dir.is_none() {
            let bytes = self.get_bytes_with_retry_blocking(&file.url)?;
            return Ok((bytes, key, true));
        }

        let gate_guard = DownloadGateGuard {
            client: self,
            key: key.clone(),
            gate: self.acquire_download_gate(&key),
        };

        // blocking_lock() panics if called from a Tokio async execution context.
        // ensure_blob_blocking is only reached from std::thread::scope threads,
        // which do not inherit a Tokio runtime handle.
        debug_assert!(
            tokio::runtime::Handle::try_current().is_err(),
            "ensure_blob_blocking must not be called from a Tokio runtime context"
        );
        let _lock = gate_guard.gate.mutex.blocking_lock();

        if let Some(bytes) = Self::try_read_cached_blob(&file.url, &blob_dir)? {
            return Ok((bytes, key, false));
        }

        let bytes = self.get_bytes_with_retry_blocking(&file.url)?;
        Self::cache_blob(&file.url, &blob_dir, &bytes)?;
        Ok((bytes, key, true))
    }

    /// Acquire (or create) a per-key download gate for in-process dedup.
    fn acquire_download_gate(&self, key: &str) -> Arc<DownloadGate> {
        let entry = self
            .download_gates
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(DownloadGate::new()));

        let gate = entry.clone();
        gate.active_users.fetch_add(1, Ordering::AcqRel);
        gate
    }

    /// Release a download gate, removing it from the map when the last user drops.
    fn release_download_gate(&self, key: &str, gate: &Arc<DownloadGate>) {
        let prev = gate.active_users.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(
            prev > 0,
            "release_download_gate called without matching acquire"
        );

        if prev != 1 {
            return;
        }

        if let dashmap::mapref::entry::Entry::Occupied(entry) =
            self.download_gates.entry(key.to_string())
        {
            if Arc::ptr_eq(entry.get(), gate) && gate.active_users.load(Ordering::Acquire) == 0 {
                entry.remove();
            }
        }
    }

    /// GET a URL as bytes with exponential-backoff retry (async).
    async fn get_bytes_with_retry_async(&self, url: &str) -> Result<Vec<u8>, GoogleFontsError> {
        let backoff = ExponentialBuilder::default().with_max_times(self.config.max_retries);
        (|| self.get_bytes_once_async(url))
            .retry(backoff)
            .when(|e| e.is_retryable())
            .await
    }

    /// GET a URL as bytes with exponential-backoff retry (blocking).
    fn get_bytes_with_retry_blocking(&self, url: &str) -> Result<Vec<u8>, GoogleFontsError> {
        let backoff = ExponentialBuilder::default().with_max_times(self.config.max_retries);
        (|| self.get_bytes_once_blocking(url))
            .retry(backoff)
            .when(|e| e.is_retryable())
            .call()
    }

    /// Execute a single GET request and return the response body (async).
    async fn get_bytes_once_async(&self, url: &str) -> Result<Vec<u8>, GoogleFontsError> {
        let response = self
            .async_client
            .get(url)
            .send()
            .await
            .map_err(|e| GoogleFontsError::from_reqwest(url, e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(GoogleFontsError::HttpStatus {
                url: url.to_string(),
                status: status.as_u16(),
            });
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| GoogleFontsError::from_reqwest(url, e))
    }

    /// Execute a single GET request and return the response body (blocking).
    fn get_bytes_once_blocking(&self, url: &str) -> Result<Vec<u8>, GoogleFontsError> {
        let client = self.get_blocking_client_clone()?;
        let response = client
            .get(url)
            .send()
            .map_err(|e| GoogleFontsError::from_reqwest(url, e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(GoogleFontsError::HttpStatus {
                url: url.to_string(),
                status: status.as_u16(),
            });
        }

        response
            .bytes()
            .map(|b| b.to_vec())
            .map_err(|e| GoogleFontsError::from_reqwest(url, e))
    }

    /// Run LRU eviction on the blob cache if it exceeds the configured limit.
    fn evict_if_needed(&self, exempt_keys: &HashSet<String>) -> Result<(), GoogleFontsError> {
        let Some(blob_dir) = self.config.blob_dir() else {
            return Ok(());
        };
        let max_bytes = self.max_blob_cache_bytes();
        if max_bytes == 0 {
            return Ok(());
        }

        let size = cache::calculate_blob_cache_size_bytes(&blob_dir)?;
        if size <= max_bytes {
            return Ok(());
        }

        cache::evict_blob_lru_until_size(&blob_dir, max_bytes, exempt_keys)
    }

    /// Lazily initialize and clone the blocking HTTP client.
    fn get_blocking_client_clone(&self) -> Result<reqwest::blocking::Client, GoogleFontsError> {
        let mut guard = self
            .blocking_client
            .lock()
            .map_err(|_| GoogleFontsError::Internal("Blocking client lock poisoned".to_string()))?;

        if let Some(client) = guard.as_ref() {
            return Ok(client.clone());
        }

        let user_agent = self.config.user_agent.clone();
        let timeout_secs = self.config.request_timeout_secs;

        let built = std::thread::spawn(move || {
            reqwest::blocking::Client::builder()
                .user_agent(user_agent)
                .timeout(Duration::from_secs(timeout_secs))
                .build()
                .map_err(|e| GoogleFontsError::Http(e.to_string()))
        })
        .join()
        .map_err(|_| {
            GoogleFontsError::Internal("Failed to join blocking client init thread".to_string())
        })??;

        let client = built.clone();
        *guard = Some(built);
        Ok(client)
    }
}

impl Drop for GoogleFontsClient {
    fn drop(&mut self) {
        // Drop the blocking client on a dedicated thread: its internal tokio
        // runtime shutdown might deadlock if run on an async worker thread.
        if let Ok(mut guard) = self.blocking_client.lock() {
            if let Some(client) = guard.take() {
                let _ = std::thread::spawn(move || drop(client)).join();
            }
        }
    }
}

impl Default for GoogleFontsClient {
    fn default() -> Self {
        Self::new(ClientConfig::default()).expect("Failed to construct default GoogleFontsClient")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_test_client(temp_root: &std::path::Path) -> GoogleFontsClient {
        let config = ClientConfig {
            cache_dir: Some(temp_root.to_path_buf()),
            css2_base_url: "http://127.0.0.1:1/css2".to_string(),
            ..ClientConfig::default()
        };
        GoogleFontsClient::new(config).unwrap()
    }

    #[test]
    fn test_download_gate_pruned_when_last_user_released() {
        let temp = tempdir().unwrap();
        let client = make_test_client(temp.path());
        let key = "roboto--KFOmCnqEu92Fr1Mu4mxK.ttf";

        let gate = client.acquire_download_gate(key);
        assert!(client.download_gates.contains_key(key));

        client.release_download_gate(key, &gate);
        assert!(!client.download_gates.contains_key(key));
        assert_eq!(gate.active_users.load(Ordering::Acquire), 0);
    }

    #[test]
    fn test_download_gate_retained_while_other_users_exist() {
        let temp = tempdir().unwrap();
        let client = make_test_client(temp.path());
        let key = "roboto--KFOmCnqEu92Fr1Mu4mxK.ttf";

        let gate_a = client.acquire_download_gate(key);
        let gate_b = client.acquire_download_gate(key);
        assert!(Arc::ptr_eq(&gate_a, &gate_b));

        client.release_download_gate(key, &gate_a);
        assert!(client.download_gates.contains_key(key));

        client.release_download_gate(key, &gate_b);
        assert!(!client.download_gates.contains_key(key));
    }

    #[test]
    fn test_download_gate_guard_releases_on_drop() {
        let temp = tempdir().unwrap();
        let client = make_test_client(temp.path());
        let key = "roboto--KFOmCnqEu92Fr1Mu4mxK.ttf";

        {
            let _guard = DownloadGateGuard {
                client: &client,
                key: key.to_string(),
                gate: client.acquire_download_gate(key),
            };
            assert!(client.download_gates.contains_key(key));
            // guard dropped here
        }

        assert!(!client.download_gates.contains_key(key));
    }

    #[test]
    fn test_download_gate_not_pruned_when_map_points_to_different_gate() {
        let temp = tempdir().unwrap();
        let client = make_test_client(temp.path());
        let key = "roboto--KFOmCnqEu92Fr1Mu4mxK.ttf";

        let old_gate = client.acquire_download_gate(key);

        let replacement = Arc::new(DownloadGate::new());
        replacement.active_users.store(1, Ordering::Release);
        client
            .download_gates
            .insert(key.to_string(), replacement.clone());

        client.release_download_gate(key, &old_gate);
        let current = client.download_gates.get(key).unwrap();
        assert!(Arc::ptr_eq(current.value(), &replacement));
        drop(current);

        client.release_download_gate(key, &replacement);
        assert!(!client.download_gates.contains_key(key));
    }
}

use crate::cache;
use crate::config::ClientConfig;
use crate::error::FontsourceError;
use crate::resolve::{dedupe_variants, resolve_download_plan, ResolvedTtfFile};
use crate::types::{family_to_id, LoadedFontBatch, VariantRequest};
use backon::{BlockingRetryable, ExponentialBuilder, Retryable};
use dashmap::DashMap;
use futures_util::stream::{self, StreamExt};
use reqwest::StatusCode;
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Return type for `ensure_blobs_*`: (font data, blob keys used, whether any were downloaded).
type EnsureBlobsResult = Result<(Vec<Arc<Vec<u8>>>, HashSet<String>, bool), FontsourceError>;

struct PreparedLoad {
    font_id: String,
    font_type: Option<String>,
    loaded_variants: Vec<VariantRequest>,
    font_data: Vec<Arc<Vec<u8>>>,
    ttf_file_count: usize,
}

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

/// Client-first Fontsource loader API.
pub struct FontsourceClient {
    config: ClientConfig,
    async_client: reqwest::Client,
    blocking_client: Mutex<Option<reqwest::blocking::Client>>,
    max_blob_cache_bytes: AtomicU64,
    download_gates: DashMap<String, Arc<DownloadGate>>,
}

impl FontsourceClient {
    pub fn new(mut config: ClientConfig) -> Result<Self, FontsourceError> {
        if config.max_parallel_downloads == 0 {
            config.max_parallel_downloads = 1;
        }

        if let Some(ref mut dir) = config.cache_dir {
            if !dir.is_absolute() {
                *dir = std::env::current_dir()?.join(&*dir);
            }
            std::fs::create_dir_all(dir.join("metadata"))?;
            std::fs::create_dir_all(dir.join("blobs"))?;
        }

        let async_client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .map_err(|e| FontsourceError::Http(e.to_string()))?;

        Ok(Self {
            max_blob_cache_bytes: AtomicU64::new(config.max_blob_cache_bytes),
            config,
            async_client,
            blocking_client: Mutex::new(None),
            download_gates: DashMap::new(),
        })
    }

    pub fn set_max_blob_cache_bytes(&self, bytes: u64) {
        self.max_blob_cache_bytes.store(bytes, Ordering::Relaxed);
    }

    pub async fn load(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<LoadedFontBatch, FontsourceError> {
        let prepared = self.prepare_load_async(family, variants).await?;
        Ok(LoadedFontBatch::new(
            prepared.font_id,
            prepared.font_type,
            prepared.loaded_variants,
            prepared.ttf_file_count,
            prepared.font_data,
        ))
    }

    pub fn load_blocking(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<LoadedFontBatch, FontsourceError> {
        let prepared = self.prepare_load_blocking(family, variants)?;
        Ok(LoadedFontBatch::new(
            prepared.font_id,
            prepared.font_type,
            prepared.loaded_variants,
            prepared.ttf_file_count,
            prepared.font_data,
        ))
    }

    fn metadata_url(&self, font_id: &str) -> String {
        format!(
            "{}/{}",
            self.config.metadata_base_url.trim_end_matches('/'),
            font_id
        )
    }

    fn max_blob_cache_bytes(&self) -> u64 {
        self.max_blob_cache_bytes.load(Ordering::Relaxed)
    }

    // -----------------------------------------------------------------------
    // Async pipeline
    // -----------------------------------------------------------------------

    async fn prepare_load_async(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<PreparedLoad, FontsourceError> {
        let font_id = family_to_id(family)
            .ok_or_else(|| FontsourceError::InvalidFontId(family.to_string()))?;

        if let Some(requested) = variants {
            if requested.is_empty() {
                return Err(FontsourceError::NoVariantsRequested);
            }
        }

        // 1. Metadata: read from cache or fetch
        let metadata_dir = self.config.metadata_dir();
        let cached_meta = metadata_dir
            .as_deref()
            .and_then(|dir| cache::read_metadata(&font_id, dir));
        let metadata = match cached_meta {
            Some(m) => m,
            None => {
                let m = self.fetch_metadata_async(&font_id).await?;
                if let Some(dir) = metadata_dir.as_deref() {
                    cache::write_metadata_if_absent(&font_id, dir, &m)?;
                }
                m
            }
        };

        // 2. Resolve download plan
        let plan = resolve_download_plan(&font_id, &metadata, variants)?;

        // 3. For each resolved file: check blob cache or download
        let (font_data, exempt_keys, downloaded_any) =
            self.ensure_blobs_async(&plan.files).await?;

        // 4. Evict if any new blobs written
        if downloaded_any {
            self.evict_if_needed(&exempt_keys)?;
        }

        Ok(PreparedLoad {
            font_id,
            font_type: Some(metadata.font_type),
            loaded_variants: if let Some(v) = variants {
                dedupe_variants(v)
            } else {
                plan.loaded_variants
            },
            ttf_file_count: font_data.len(),
            font_data,
        })
    }

    async fn ensure_blobs_async(
        &self,
        files: &[ResolvedTtfFile],
    ) -> EnsureBlobsResult {
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
        let mut exempt_keys = HashSet::new();
        let mut downloaded_any = false;

        for (_, bytes, key, was_downloaded) in result_vec {
            font_data.push(Arc::new(bytes));
            exempt_keys.insert(key);
            downloaded_any |= was_downloaded;
        }

        Ok((font_data, exempt_keys, downloaded_any))
    }

    async fn ensure_blob_async(
        &self,
        file: &ResolvedTtfFile,
    ) -> Result<(Vec<u8>, String, bool), FontsourceError> {
        let key = cache::blob_key(&file.url);
        let blob_dir = self.config.blob_dir();

        // Fast path: blob already cached
        if let Some(ref dir) = blob_dir {
            if let Some(bytes) = cache::read_blob(&file.url, dir)? {
                let _ = cache::touch_blob(&file.url, dir);
                return Ok((bytes, key, false));
            }
        }

        // Gate by blob key for in-process dedup
        let gate = self.acquire_download_gate(&key);
        let result = async {
            let _guard = gate.mutex.lock().await;

            // Re-check after acquiring gate
            if let Some(ref dir) = blob_dir {
                if let Some(bytes) = cache::read_blob(&file.url, dir)? {
                    let _ = cache::touch_blob(&file.url, dir);
                    return Ok((bytes, key.clone(), false));
                }
            }

            let bytes = self.get_bytes_with_retry_async(&file.url).await?;
            if let Some(ref dir) = blob_dir {
                cache::write_blob_if_absent(&file.url, dir, &bytes)?;
            }
            Ok((bytes, key.clone(), true))
        }
        .await;
        self.release_download_gate(&key, &gate);
        result
    }

    // -----------------------------------------------------------------------
    // Blocking pipeline
    // -----------------------------------------------------------------------

    fn prepare_load_blocking(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<PreparedLoad, FontsourceError> {
        let font_id = family_to_id(family)
            .ok_or_else(|| FontsourceError::InvalidFontId(family.to_string()))?;

        if let Some(requested) = variants {
            if requested.is_empty() {
                return Err(FontsourceError::NoVariantsRequested);
            }
        }

        // 1. Metadata: read from cache or fetch
        let metadata_dir = self.config.metadata_dir();
        let cached_meta = metadata_dir
            .as_deref()
            .and_then(|dir| cache::read_metadata(&font_id, dir));
        let metadata = match cached_meta {
            Some(m) => m,
            None => {
                let m = self.fetch_metadata_blocking(&font_id)?;
                if let Some(dir) = metadata_dir.as_deref() {
                    cache::write_metadata_if_absent(&font_id, dir, &m)?;
                }
                m
            }
        };

        // 2. Resolve download plan
        let plan = resolve_download_plan(&font_id, &metadata, variants)?;

        // 3. For each resolved file: check blob cache or download
        let (font_data, exempt_keys, downloaded_any) = self.ensure_blobs_blocking(&plan.files)?;

        // 4. Evict if any new blobs written
        if downloaded_any {
            self.evict_if_needed(&exempt_keys)?;
        }

        Ok(PreparedLoad {
            font_id,
            font_type: Some(metadata.font_type),
            loaded_variants: if let Some(v) = variants {
                dedupe_variants(v)
            } else {
                plan.loaded_variants
            },
            ttf_file_count: font_data.len(),
            font_data,
        })
    }

    fn ensure_blobs_blocking(
        &self,
        files: &[ResolvedTtfFile],
    ) -> EnsureBlobsResult {
        if files.is_empty() {
            return Ok((Vec::new(), HashSet::new(), false));
        }

        let workers = self.config.max_parallel_downloads.max(1).min(files.len());
        let queue = Arc::new(Mutex::new(
            files
                .iter()
                .cloned()
                .enumerate()
                .collect::<VecDeque<(usize, ResolvedTtfFile)>>(),
        ));
        let downloaded = Arc::new(AtomicUsize::new(0));
        let first_error = Arc::new(Mutex::new(None::<FontsourceError>));

        // Collect results: (index, bytes, key, was_downloaded)
        type BlobResult = (usize, Vec<u8>, String, bool);
        let results: Arc<Mutex<Vec<BlobResult>>> = Arc::new(Mutex::new(Vec::new()));
        std::thread::scope(|scope| {
            for _ in 0..workers {
                let queue = Arc::clone(&queue);
                let downloaded = Arc::clone(&downloaded);
                let first_error = Arc::clone(&first_error);
                let results = Arc::clone(&results);

                scope.spawn(move || loop {
                    if first_error.lock().expect("poisoned").is_some() {
                        break;
                    }

                    let (idx, next) = {
                        let mut guard = queue.lock().expect("poisoned");
                        match guard.pop_front() {
                            Some((idx, file)) => (idx, file),
                            None => break,
                        }
                    };

                    match self.ensure_blob_blocking(&next) {
                        Ok((bytes, key, was_downloaded)) => {
                            if was_downloaded {
                                downloaded.fetch_add(1, Ordering::Relaxed);
                            }
                            results.lock().expect("poisoned").push((
                                idx,
                                bytes,
                                key,
                                was_downloaded,
                            ));
                        }
                        Err(err) => {
                            let mut guard = first_error.lock().expect("poisoned");
                            if guard.is_none() {
                                *guard = Some(err);
                            }
                            break;
                        }
                    }
                });
            }
        });

        if let Some(err) = first_error.lock().expect("poisoned").take() {
            return Err(err);
        }

        let downloaded_any = downloaded.load(Ordering::Relaxed) > 0;

        let mut result_vec = results
            .lock()
            .expect("poisoned")
            .drain(..)
            .collect::<Vec<_>>();
        result_vec.sort_by_key(|(idx, _, _, _)| *idx);

        let mut font_data = Vec::with_capacity(result_vec.len());
        let mut exempt_keys = HashSet::new();

        for (_, bytes, key, _) in result_vec {
            font_data.push(Arc::new(bytes));
            exempt_keys.insert(key);
        }

        Ok((font_data, exempt_keys, downloaded_any))
    }

    fn ensure_blob_blocking(
        &self,
        file: &ResolvedTtfFile,
    ) -> Result<(Vec<u8>, String, bool), FontsourceError> {
        let key = cache::blob_key(&file.url);
        let blob_dir = self.config.blob_dir();

        // Fast path: blob already cached
        if let Some(ref dir) = blob_dir {
            if let Some(bytes) = cache::read_blob(&file.url, dir)? {
                let _ = cache::touch_blob(&file.url, dir);
                return Ok((bytes, key, false));
            }
        }

        // Gate by blob key for in-process dedup
        let gate = self.acquire_download_gate(&key);
        let result = (|| {
            let _guard = gate.mutex.blocking_lock();

            // Re-check after acquiring gate
            if let Some(ref dir) = blob_dir {
                if let Some(bytes) = cache::read_blob(&file.url, dir)? {
                    let _ = cache::touch_blob(&file.url, dir);
                    return Ok((bytes, key.clone(), false));
                }
            }

            let bytes = self.get_bytes_with_retry_blocking(&file.url)?;
            if let Some(ref dir) = blob_dir {
                cache::write_blob_if_absent(&file.url, dir, &bytes)?;
            }
            Ok((bytes, key.clone(), true))
        })();
        self.release_download_gate(&key, &gate);
        result
    }

    // -----------------------------------------------------------------------
    // Download gate
    // -----------------------------------------------------------------------

    fn acquire_download_gate(&self, key: &str) -> Arc<DownloadGate> {
        let entry = self
            .download_gates
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(DownloadGate::new()));

        let gate = entry.clone();
        gate.active_users.fetch_add(1, Ordering::AcqRel);
        gate
    }

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

    // -----------------------------------------------------------------------
    // Metadata fetching
    // -----------------------------------------------------------------------

    async fn fetch_metadata_async(
        &self,
        font_id: &str,
    ) -> Result<crate::types::FontsourceFont, FontsourceError> {
        let url = self.metadata_url(font_id);
        let bytes = match self.get_bytes_with_retry_async(&url).await {
            Err(FontsourceError::HttpStatus { status, .. })
                if status == StatusCode::NOT_FOUND.as_u16() =>
            {
                return Err(FontsourceError::FontNotFound(font_id.to_string()));
            }
            Err(err) => return Err(err),
            Ok(bytes) => bytes,
        };

        let metadata: crate::types::FontsourceFont = serde_json::from_slice(&bytes)?;
        Ok(metadata)
    }

    fn fetch_metadata_blocking(
        &self,
        font_id: &str,
    ) -> Result<crate::types::FontsourceFont, FontsourceError> {
        let url = self.metadata_url(font_id);
        let bytes = match self.get_bytes_with_retry_blocking(&url) {
            Err(FontsourceError::HttpStatus { status, .. })
                if status == StatusCode::NOT_FOUND.as_u16() =>
            {
                return Err(FontsourceError::FontNotFound(font_id.to_string()));
            }
            Err(err) => return Err(err),
            Ok(bytes) => bytes,
        };

        let metadata: crate::types::FontsourceFont = serde_json::from_slice(&bytes)?;
        Ok(metadata)
    }

    // -----------------------------------------------------------------------
    // HTTP retry + single-request helpers (unchanged)
    // -----------------------------------------------------------------------

    async fn get_bytes_with_retry_async(
        &self,
        url: &str,
    ) -> Result<Vec<u8>, FontsourceError> {
        let backoff = ExponentialBuilder::default().with_max_times(self.config.max_retries);
        (|| self.get_bytes_once_async(url))
            .retry(backoff)
            .when(|e| e.is_retryable())
            .await
    }

    fn get_bytes_with_retry_blocking(&self, url: &str) -> Result<Vec<u8>, FontsourceError> {
        let backoff = ExponentialBuilder::default().with_max_times(self.config.max_retries);
        (|| self.get_bytes_once_blocking(url))
            .retry(backoff)
            .when(|e| e.is_retryable())
            .call()
    }

    async fn get_bytes_once_async(&self, url: &str) -> Result<Vec<u8>, FontsourceError> {
        let response = self
            .async_client
            .get(url)
            .send()
            .await
            .map_err(|e| FontsourceError::from_reqwest(url, e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(FontsourceError::HttpStatus {
                url: url.to_string(),
                status: status.as_u16(),
            });
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| FontsourceError::from_reqwest(url, e))
    }

    fn get_bytes_once_blocking(&self, url: &str) -> Result<Vec<u8>, FontsourceError> {
        let client = self.get_blocking_client_clone()?;
        let response = client
            .get(url)
            .send()
            .map_err(|e| FontsourceError::from_reqwest(url, e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(FontsourceError::HttpStatus {
                url: url.to_string(),
                status: status.as_u16(),
            });
        }

        response
            .bytes()
            .map(|b| b.to_vec())
            .map_err(|e| FontsourceError::from_reqwest(url, e))
    }

    // -----------------------------------------------------------------------
    // Eviction
    // -----------------------------------------------------------------------

    fn evict_if_needed(&self, exempt_keys: &HashSet<String>) -> Result<(), FontsourceError> {
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

    // -----------------------------------------------------------------------
    // Blocking client helper (unchanged)
    // -----------------------------------------------------------------------

    fn get_blocking_client_clone(
        &self,
    ) -> Result<reqwest::blocking::Client, FontsourceError> {
        let mut guard = self.blocking_client.lock().map_err(|_| {
            FontsourceError::Internal("Blocking client lock poisoned".to_string())
        })?;

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
                .map_err(|e| FontsourceError::Http(e.to_string()))
        })
        .join()
        .map_err(|_| {
            FontsourceError::Internal(
                "Failed to join blocking client init thread".to_string(),
            )
        })??;

        let client = built.clone();
        *guard = Some(built);
        Ok(client)
    }
}

impl Drop for FontsourceClient {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.blocking_client.lock() {
            if let Some(client) = guard.take() {
                let _ = std::thread::spawn(move || drop(client)).join();
            }
        }
    }
}

impl Default for FontsourceClient {
    fn default() -> Self {
        Self::new(ClientConfig::default()).expect("Failed to construct default FontsourceClient")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_test_client(temp_root: &std::path::Path) -> FontsourceClient {
        let config = ClientConfig {
            cache_dir: Some(temp_root.to_path_buf()),
            metadata_base_url: "http://127.0.0.1:1/v1/fonts".to_string(),
            ..ClientConfig::default()
        };
        FontsourceClient::new(config).unwrap()
    }

    #[test]
    fn test_download_gate_pruned_when_last_user_released() {
        let temp = tempdir().unwrap();
        let client = make_test_client(temp.path());
        let key = "roboto--latin-400-normal";

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
        let key = "roboto--latin-400-normal";

        let gate_a = client.acquire_download_gate(key);
        let gate_b = client.acquire_download_gate(key);
        assert!(Arc::ptr_eq(&gate_a, &gate_b));

        client.release_download_gate(key, &gate_a);
        assert!(client.download_gates.contains_key(key));

        client.release_download_gate(key, &gate_b);
        assert!(!client.download_gates.contains_key(key));
    }

    #[test]
    fn test_download_gate_not_pruned_when_map_points_to_different_gate() {
        let temp = tempdir().unwrap();
        let client = make_test_client(temp.path());
        let key = "roboto--latin-400-normal";

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

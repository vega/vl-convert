use crate::cache;
use crate::config::ClientConfig;
use crate::error::FontsourceFontdbError;
use crate::resolve::{dedupe_variants, resolve_download_plan, ResolvedTtfFile};
use crate::types::{family_to_id, LoadedFontBatch, VariantRequest};
use dashmap::DashMap;
use futures_util::stream::{self, StreamExt};
use reqwest::StatusCode;
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct PreparedLoad {
    font_id: String,
    font_type: Option<String>,
    loaded_variants: Vec<VariantRequest>,
    sources: Vec<fontdb::Source>,
    ttf_file_count: usize,
}

/// Client-first Fontsource loader API.
pub struct FontsourceClient {
    config: ClientConfig,
    async_client: reqwest::Client,
    blocking_client: Mutex<Option<reqwest::blocking::Client>>,
    max_blob_cache_bytes: AtomicU64,
    download_gates: DashMap<String, Arc<tokio::sync::Mutex<()>>>,
}

impl FontsourceClient {
    pub fn new(mut config: ClientConfig) -> Result<Self, FontsourceFontdbError> {
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
            .map_err(|e| FontsourceFontdbError::Http(e.to_string()))?;

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
    ) -> Result<LoadedFontBatch, FontsourceFontdbError> {
        let prepared = self.prepare_load_async(family, variants).await?;
        Ok(LoadedFontBatch::new(
            prepared.font_id,
            prepared.font_type,
            prepared.loaded_variants,
            prepared.ttf_file_count,
            prepared.sources,
        ))
    }

    pub fn load_blocking(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<LoadedFontBatch, FontsourceFontdbError> {
        let prepared = self.prepare_load_blocking(family, variants)?;
        Ok(LoadedFontBatch::new(
            prepared.font_id,
            prepared.font_type,
            prepared.loaded_variants,
            prepared.ttf_file_count,
            prepared.sources,
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
    ) -> Result<PreparedLoad, FontsourceFontdbError> {
        let font_id = family_to_id(family)
            .ok_or_else(|| FontsourceFontdbError::InvalidFontId(family.to_string()))?;

        if let Some(requested) = variants {
            if requested.is_empty() {
                return Err(FontsourceFontdbError::NoVariantsRequested);
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
        let (sources, exempt_keys, downloaded_any) =
            self.ensure_blobs_async(&font_id, &plan.files).await?;

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
            ttf_file_count: sources.len(),
            sources,
        })
    }

    async fn ensure_blobs_async(
        &self,
        font_id: &str,
        files: &[ResolvedTtfFile],
    ) -> Result<(Vec<fontdb::Source>, HashSet<String>, bool), FontsourceFontdbError> {
        let limit = self.config.max_parallel_downloads.max(1);
        let font_id_owned = font_id.to_string();

        let results = stream::iter(files.iter().cloned().map(|file| {
            let font_id = font_id_owned.clone();
            async move { self.ensure_blob_async(&font_id, &file).await }
        }))
        .buffer_unordered(limit)
        .collect::<Vec<_>>()
        .await;

        let mut sources = Vec::with_capacity(files.len());
        let mut exempt_keys = HashSet::new();
        let mut downloaded_any = false;

        for result in results {
            let (bytes, key, was_downloaded) = result?;
            let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(bytes);
            sources.push(fontdb::Source::Binary(data));
            exempt_keys.insert(key);
            downloaded_any |= was_downloaded;
        }

        Ok((sources, exempt_keys, downloaded_any))
    }

    async fn ensure_blob_async(
        &self,
        font_id: &str,
        file: &ResolvedTtfFile,
    ) -> Result<(Vec<u8>, String, bool), FontsourceFontdbError> {
        let key = cache::blob_key(font_id, &file.filename);
        let blob_dir = self.config.blob_dir();

        // Fast path: blob already cached
        if let Some(ref dir) = blob_dir {
            if let Some(bytes) = cache::read_blob(&key, dir)? {
                let _ = cache::touch_blob(&key, dir);
                return Ok((bytes, key, false));
            }
        }

        // Gate by blob key for in-process dedup
        let gate = self.download_gate_for(&key);
        let _guard = gate.lock().await;

        // Re-check after acquiring gate
        if let Some(ref dir) = blob_dir {
            if let Some(bytes) = cache::read_blob(&key, dir)? {
                let _ = cache::touch_blob(&key, dir);
                return Ok((bytes, key, false));
            }
        }

        let bytes = self.get_bytes_with_retry_async(&file.url).await?;
        if let Some(ref dir) = blob_dir {
            cache::write_blob_if_absent(&key, dir, &bytes)?;
        }
        Ok((bytes, key, true))
    }

    // -----------------------------------------------------------------------
    // Blocking pipeline
    // -----------------------------------------------------------------------

    fn prepare_load_blocking(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<PreparedLoad, FontsourceFontdbError> {
        let font_id = family_to_id(family)
            .ok_or_else(|| FontsourceFontdbError::InvalidFontId(family.to_string()))?;

        if let Some(requested) = variants {
            if requested.is_empty() {
                return Err(FontsourceFontdbError::NoVariantsRequested);
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
        let (sources, exempt_keys, downloaded_any) =
            self.ensure_blobs_blocking(&font_id, &plan.files)?;

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
            ttf_file_count: sources.len(),
            sources,
        })
    }

    fn ensure_blobs_blocking(
        &self,
        font_id: &str,
        files: &[ResolvedTtfFile],
    ) -> Result<(Vec<fontdb::Source>, HashSet<String>, bool), FontsourceFontdbError> {
        if files.is_empty() {
            return Ok((Vec::new(), HashSet::new(), false));
        }

        let workers = self.config.max_parallel_downloads.max(1).min(files.len());
        let queue = Arc::new(Mutex::new(VecDeque::from(files.to_vec())));
        let downloaded = Arc::new(AtomicUsize::new(0));
        let first_error = Arc::new(Mutex::new(None::<FontsourceFontdbError>));

        // Collect results: (index, bytes, key, was_downloaded)
        type BlobResult = (usize, Vec<u8>, String, bool);
        let results: Arc<Mutex<Vec<BlobResult>>> = Arc::new(Mutex::new(Vec::new()));
        let index_counter = Arc::new(AtomicUsize::new(0));

        let font_id_owned = font_id.to_string();

        std::thread::scope(|scope| {
            for _ in 0..workers {
                let queue = Arc::clone(&queue);
                let downloaded = Arc::clone(&downloaded);
                let first_error = Arc::clone(&first_error);
                let results = Arc::clone(&results);
                let index_counter = Arc::clone(&index_counter);
                let font_id = font_id_owned.clone();

                scope.spawn(move || loop {
                    if first_error.lock().expect("poisoned").is_some() {
                        break;
                    }

                    let (next, idx) = {
                        let mut guard = queue.lock().expect("poisoned");
                        match guard.pop_front() {
                            Some(file) => {
                                let idx = index_counter.fetch_add(1, Ordering::Relaxed);
                                (file, idx)
                            }
                            None => break,
                        }
                    };

                    match self.ensure_blob_blocking(&font_id, &next) {
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

        let mut sources = Vec::with_capacity(result_vec.len());
        let mut exempt_keys = HashSet::new();

        for (_, bytes, key, _) in result_vec {
            let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(bytes);
            sources.push(fontdb::Source::Binary(data));
            exempt_keys.insert(key);
        }

        Ok((sources, exempt_keys, downloaded_any))
    }

    fn ensure_blob_blocking(
        &self,
        font_id: &str,
        file: &ResolvedTtfFile,
    ) -> Result<(Vec<u8>, String, bool), FontsourceFontdbError> {
        let key = cache::blob_key(font_id, &file.filename);
        let blob_dir = self.config.blob_dir();

        // Fast path: blob already cached
        if let Some(ref dir) = blob_dir {
            if let Some(bytes) = cache::read_blob(&key, dir)? {
                let _ = cache::touch_blob(&key, dir);
                return Ok((bytes, key, false));
            }
        }

        // Gate by blob key for in-process dedup
        let gate = self.download_gate_for(&key);
        let _guard = gate.blocking_lock();

        // Re-check after acquiring gate
        if let Some(ref dir) = blob_dir {
            if let Some(bytes) = cache::read_blob(&key, dir)? {
                let _ = cache::touch_blob(&key, dir);
                return Ok((bytes, key, false));
            }
        }

        let bytes = self.get_bytes_with_retry_blocking(&file.url)?;
        if let Some(ref dir) = blob_dir {
            cache::write_blob_if_absent(&key, dir, &bytes)?;
        }
        Ok((bytes, key, true))
    }

    // -----------------------------------------------------------------------
    // Download gate
    // -----------------------------------------------------------------------

    fn download_gate_for(&self, key: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.download_gates
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    // -----------------------------------------------------------------------
    // Metadata fetching
    // -----------------------------------------------------------------------

    async fn fetch_metadata_async(
        &self,
        font_id: &str,
    ) -> Result<crate::types::FontsourceFont, FontsourceFontdbError> {
        let url = self.metadata_url(font_id);
        let bytes = match self.get_bytes_with_retry_async(&url).await {
            Err(FontsourceFontdbError::HttpStatus { status, .. })
                if status == StatusCode::NOT_FOUND.as_u16() =>
            {
                return Err(FontsourceFontdbError::FontNotFound(font_id.to_string()));
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
    ) -> Result<crate::types::FontsourceFont, FontsourceFontdbError> {
        let url = self.metadata_url(font_id);
        let bytes = match self.get_bytes_with_retry_blocking(&url) {
            Err(FontsourceFontdbError::HttpStatus { status, .. })
                if status == StatusCode::NOT_FOUND.as_u16() =>
            {
                return Err(FontsourceFontdbError::FontNotFound(font_id.to_string()));
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
    ) -> Result<Vec<u8>, FontsourceFontdbError> {
        let mut attempts = 0usize;
        loop {
            match self.get_bytes_once_async(url).await {
                Ok(bytes) => return Ok(bytes),
                Err(err) if err.is_retryable() && attempts < self.config.max_retries => {
                    attempts += 1;
                }
                Err(err) => return Err(err),
            }
        }
    }

    fn get_bytes_with_retry_blocking(&self, url: &str) -> Result<Vec<u8>, FontsourceFontdbError> {
        let mut attempts = 0usize;
        loop {
            match self.get_bytes_once_blocking(url) {
                Ok(bytes) => return Ok(bytes),
                Err(err) if err.is_retryable() && attempts < self.config.max_retries => {
                    attempts += 1;
                }
                Err(err) => return Err(err),
            }
        }
    }

    async fn get_bytes_once_async(&self, url: &str) -> Result<Vec<u8>, FontsourceFontdbError> {
        let response = self
            .async_client
            .get(url)
            .send()
            .await
            .map_err(|e| FontsourceFontdbError::from_reqwest(url, e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(FontsourceFontdbError::HttpStatus {
                url: url.to_string(),
                status: status.as_u16(),
            });
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| FontsourceFontdbError::from_reqwest(url, e))
    }

    fn get_bytes_once_blocking(&self, url: &str) -> Result<Vec<u8>, FontsourceFontdbError> {
        let client = self.get_blocking_client_clone()?;
        let response = client
            .get(url)
            .send()
            .map_err(|e| FontsourceFontdbError::from_reqwest(url, e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(FontsourceFontdbError::HttpStatus {
                url: url.to_string(),
                status: status.as_u16(),
            });
        }

        response
            .bytes()
            .map(|b| b.to_vec())
            .map_err(|e| FontsourceFontdbError::from_reqwest(url, e))
    }

    // -----------------------------------------------------------------------
    // Eviction
    // -----------------------------------------------------------------------

    fn evict_if_needed(&self, exempt_keys: &HashSet<String>) -> Result<(), FontsourceFontdbError> {
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
    ) -> Result<reqwest::blocking::Client, FontsourceFontdbError> {
        let mut guard = self.blocking_client.lock().map_err(|_| {
            FontsourceFontdbError::Internal("Blocking client lock poisoned".to_string())
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
                .map_err(|e| FontsourceFontdbError::Http(e.to_string()))
        })
        .join()
        .map_err(|_| {
            FontsourceFontdbError::Internal(
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

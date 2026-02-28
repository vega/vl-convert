use crate::cache;
use crate::config::ClientConfig;
use crate::error::FontsourceFontdbError;
use crate::resolve::{dedupe_variants, resolve_download_plan, ResolvedTtfFile};
use crate::types::{family_to_id, LoadedFontBatch, VariantRequest};
use dashmap::DashMap;
use futures_util::stream::{self, StreamExt};
use reqwest::StatusCode;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct PreparedLoad {
    font_id: String,
    font_type: Option<String>,
    loaded_variants: Vec<VariantRequest>,
    file_paths: Vec<PathBuf>,
    ttf_file_count: usize,
}

/// Client-first Fontsource loader API.
pub struct FontsourceClient {
    config: ClientConfig,
    async_client: reqwest::Client,
    blocking_client: Mutex<Option<reqwest::blocking::Client>>,
    max_cache_bytes: AtomicU64,
    download_gates: DashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>,
}

impl FontsourceClient {
    pub fn new(mut config: ClientConfig) -> Result<Self, FontsourceFontdbError> {
        if config.max_parallel_downloads == 0 {
            config.max_parallel_downloads = 1;
        }

        if !config.cache_dir.is_absolute() {
            config.cache_dir = std::env::current_dir()?.join(&config.cache_dir);
        }

        std::fs::create_dir_all(&config.cache_dir)?;

        let async_client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .map_err(|e| FontsourceFontdbError::Http(e.to_string()))?;

        Ok(Self {
            max_cache_bytes: AtomicU64::new(config.max_cache_bytes),
            config,
            async_client,
            blocking_client: Mutex::new(None),
            download_gates: DashMap::new(),
        })
    }

    pub fn default() -> Self {
        Self::new(ClientConfig::default()).expect("Failed to construct default FontsourceClient")
    }

    pub fn set_max_cache_bytes(&self, bytes: u64) {
        self.max_cache_bytes.store(bytes, Ordering::Relaxed);
    }

    pub async fn load(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<LoadedFontBatch, FontsourceFontdbError> {
        let prepared = self.prepare_load_async(family, variants).await?;
        self.build_loaded_batch_async(prepared).await
    }

    pub fn load_blocking(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<LoadedFontBatch, FontsourceFontdbError> {
        let prepared = self.prepare_load_blocking(family, variants)?;
        self.build_loaded_batch_blocking(prepared)
    }

    async fn build_loaded_batch_async(
        &self,
        prepared: PreparedLoad,
    ) -> Result<LoadedFontBatch, FontsourceFontdbError> {
        let sources = self.sources_from_paths_async(&prepared.file_paths).await?;

        Ok(LoadedFontBatch::new(
            prepared.font_id,
            prepared.font_type,
            prepared.loaded_variants,
            prepared.ttf_file_count,
            sources,
        ))
    }

    fn build_loaded_batch_blocking(
        &self,
        prepared: PreparedLoad,
    ) -> Result<LoadedFontBatch, FontsourceFontdbError> {
        let sources = self.sources_from_paths_blocking(&prepared.file_paths)?;

        Ok(LoadedFontBatch::new(
            prepared.font_id,
            prepared.font_type,
            prepared.loaded_variants,
            prepared.ttf_file_count,
            sources,
        ))
    }

    async fn sources_from_paths_async(
        &self,
        file_paths: &[PathBuf],
    ) -> Result<Vec<fontdb::Source>, FontsourceFontdbError> {
        let mut sources = Vec::with_capacity(file_paths.len());
        for path in file_paths {
            let bytes = tokio::fs::read(path).await?;
            let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(bytes);
            sources.push(fontdb::Source::Binary(data));
        }
        Ok(sources)
    }

    fn sources_from_paths_blocking(
        &self,
        file_paths: &[PathBuf],
    ) -> Result<Vec<fontdb::Source>, FontsourceFontdbError> {
        let mut sources = Vec::with_capacity(file_paths.len());
        for path in file_paths {
            let bytes = std::fs::read(path)?;
            let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(bytes);
            sources.push(fontdb::Source::Binary(data));
        }
        Ok(sources)
    }

    fn metadata_url(&self, font_id: &str) -> String {
        format!(
            "{}/{}",
            self.config.metadata_base_url.trim_end_matches('/'),
            font_id
        )
    }

    fn font_dir(&self, font_id: &str) -> PathBuf {
        self.config.cache_dir.join(font_id)
    }

    fn max_cache_bytes(&self) -> u64 {
        self.max_cache_bytes.load(Ordering::Relaxed)
    }

    async fn prepare_load_async(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<PreparedLoad, FontsourceFontdbError> {
        let font_id = family_to_id(family)
            .ok_or_else(|| FontsourceFontdbError::InvalidFontId(family.to_string()))?;
        let font_dir = self.font_dir(&font_id);
        tokio::fs::create_dir_all(&font_dir).await?;

        if let Some(requested) = variants {
            if requested.is_empty() {
                return Err(FontsourceFontdbError::NoVariantsRequested);
            }

            if cache::has_requested_variants(&font_dir, requested)? {
                let file_paths = cache::list_variant_ttf_paths(&font_dir, requested)?;
                cache::touch_dir_mtime(&font_dir);
                return Ok(PreparedLoad {
                    font_id,
                    font_type: cache::read_local_metadata(&font_dir).map(|m| m.font_type),
                    loaded_variants: dedupe_variants(requested),
                    ttf_file_count: file_paths.len(),
                    file_paths,
                });
            }
        }

        if variants.is_none() && cache::has_any_ttf_files(&font_dir) {
            if let Some(metadata) = cache::read_local_metadata(&font_dir) {
                if cache::has_all_downloadable_ttf_files(&font_dir, &metadata) {
                    let plan = resolve_download_plan(&font_id, &metadata, None)?;
                    let file_paths: Vec<PathBuf> = plan
                        .files
                        .iter()
                        .map(|f| font_dir.join(&f.filename))
                        .collect();
                    cache::touch_dir_mtime(&font_dir);
                    return Ok(PreparedLoad {
                        font_id,
                        font_type: Some(metadata.font_type),
                        loaded_variants: plan.loaded_variants,
                        ttf_file_count: file_paths.len(),
                        file_paths,
                    });
                }
            }
        }

        let metadata = match cache::read_local_metadata(&font_dir) {
            Some(metadata) => metadata,
            None => {
                let metadata = self.fetch_metadata_async(&font_id).await?;
                cache::write_local_metadata(&font_dir, &metadata)?;
                metadata
            }
        };

        let plan = resolve_download_plan(&font_id, &metadata, variants)?;
        let filenames: Vec<String> = plan.files.iter().map(|f| f.filename.clone()).collect();

        let downloaded_any = if cache::all_filenames_exist(&font_dir, &filenames) {
            false
        } else {
            let downloaded = self.ensure_files_async(&font_dir, &plan.files).await?;
            cache::write_local_metadata(&font_dir, &metadata)?;
            downloaded
        };

        cache::touch_dir_mtime(&font_dir);

        if downloaded_any {
            self.evict_if_needed(&font_id)?;
        }

        let file_paths: Vec<PathBuf> = plan
            .files
            .iter()
            .map(|f| font_dir.join(&f.filename))
            .collect();
        Ok(PreparedLoad {
            font_id,
            font_type: Some(metadata.font_type),
            loaded_variants: plan.loaded_variants,
            ttf_file_count: file_paths.len(),
            file_paths,
        })
    }

    fn prepare_load_blocking(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<PreparedLoad, FontsourceFontdbError> {
        let font_id = family_to_id(family)
            .ok_or_else(|| FontsourceFontdbError::InvalidFontId(family.to_string()))?;
        let font_dir = self.font_dir(&font_id);
        std::fs::create_dir_all(&font_dir)?;

        if let Some(requested) = variants {
            if requested.is_empty() {
                return Err(FontsourceFontdbError::NoVariantsRequested);
            }

            if cache::has_requested_variants(&font_dir, requested)? {
                let file_paths = cache::list_variant_ttf_paths(&font_dir, requested)?;
                cache::touch_dir_mtime(&font_dir);
                return Ok(PreparedLoad {
                    font_id,
                    font_type: cache::read_local_metadata(&font_dir).map(|m| m.font_type),
                    loaded_variants: dedupe_variants(requested),
                    ttf_file_count: file_paths.len(),
                    file_paths,
                });
            }
        }

        if variants.is_none() && cache::has_any_ttf_files(&font_dir) {
            if let Some(metadata) = cache::read_local_metadata(&font_dir) {
                if cache::has_all_downloadable_ttf_files(&font_dir, &metadata) {
                    let plan = resolve_download_plan(&font_id, &metadata, None)?;
                    let file_paths: Vec<PathBuf> = plan
                        .files
                        .iter()
                        .map(|f| font_dir.join(&f.filename))
                        .collect();
                    cache::touch_dir_mtime(&font_dir);
                    return Ok(PreparedLoad {
                        font_id,
                        font_type: Some(metadata.font_type),
                        loaded_variants: plan.loaded_variants,
                        ttf_file_count: file_paths.len(),
                        file_paths,
                    });
                }
            }
        }

        let metadata = match cache::read_local_metadata(&font_dir) {
            Some(metadata) => metadata,
            None => {
                let metadata = self.fetch_metadata_blocking(&font_id)?;
                cache::write_local_metadata(&font_dir, &metadata)?;
                metadata
            }
        };

        let plan = resolve_download_plan(&font_id, &metadata, variants)?;
        let filenames: Vec<String> = plan.files.iter().map(|f| f.filename.clone()).collect();

        let downloaded_any = if cache::all_filenames_exist(&font_dir, &filenames) {
            false
        } else {
            let downloaded = self.ensure_files_blocking(&font_dir, &plan.files)?;
            cache::write_local_metadata(&font_dir, &metadata)?;
            downloaded
        };

        cache::touch_dir_mtime(&font_dir);

        if downloaded_any {
            self.evict_if_needed(&font_id)?;
        }

        let file_paths: Vec<PathBuf> = plan
            .files
            .iter()
            .map(|f| font_dir.join(&f.filename))
            .collect();
        Ok(PreparedLoad {
            font_id,
            font_type: Some(metadata.font_type),
            loaded_variants: plan.loaded_variants,
            ttf_file_count: file_paths.len(),
            file_paths,
        })
    }

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

    async fn ensure_files_async(
        &self,
        font_dir: &Path,
        files: &[ResolvedTtfFile],
    ) -> Result<bool, FontsourceFontdbError> {
        let limit = self.config.max_parallel_downloads.max(1);

        let results = stream::iter(
            files
                .iter()
                .cloned()
                .map(|file| async move { self.ensure_file_async(font_dir, &file).await }),
        )
        .buffer_unordered(limit)
        .collect::<Vec<_>>()
        .await;

        let mut downloaded_any = false;
        for result in results {
            downloaded_any |= result?;
        }

        Ok(downloaded_any)
    }

    fn ensure_files_blocking(
        &self,
        font_dir: &Path,
        files: &[ResolvedTtfFile],
    ) -> Result<bool, FontsourceFontdbError> {
        if files.is_empty() {
            return Ok(false);
        }

        let workers = self.config.max_parallel_downloads.max(1).min(files.len());
        let queue = Arc::new(Mutex::new(VecDeque::from(files.to_vec())));
        let downloaded = Arc::new(AtomicUsize::new(0));
        let first_error = Arc::new(Mutex::new(None::<FontsourceFontdbError>));

        std::thread::scope(|scope| {
            for _ in 0..workers {
                let queue = Arc::clone(&queue);
                let downloaded = Arc::clone(&downloaded);
                let first_error = Arc::clone(&first_error);

                scope.spawn(move || loop {
                    if first_error.lock().expect("poisoned").is_some() {
                        break;
                    }

                    let next = {
                        let mut guard = queue.lock().expect("poisoned");
                        guard.pop_front()
                    };

                    let Some(file) = next else {
                        break;
                    };

                    match self.ensure_file_blocking(font_dir, &file) {
                        Ok(wrote) => {
                            if wrote {
                                downloaded.fetch_add(1, Ordering::Relaxed);
                            }
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

        Ok(downloaded.load(Ordering::Relaxed) > 0)
    }

    async fn ensure_file_async(
        &self,
        font_dir: &Path,
        file: &ResolvedTtfFile,
    ) -> Result<bool, FontsourceFontdbError> {
        let target = font_dir.join(&file.filename);
        if target.exists() {
            return Ok(false);
        }

        let gate = self.download_gate_for(&target);
        let _guard = gate.lock().await;

        if target.exists() {
            return Ok(false);
        }

        let bytes = self.get_bytes_with_retry_async(&file.url).await?;
        cache::atomic_write_bytes(&target, &bytes)?;
        Ok(true)
    }

    fn ensure_file_blocking(
        &self,
        font_dir: &Path,
        file: &ResolvedTtfFile,
    ) -> Result<bool, FontsourceFontdbError> {
        let target = font_dir.join(&file.filename);
        if target.exists() {
            return Ok(false);
        }

        let gate = self.download_gate_for(&target);
        let _guard = gate.blocking_lock();

        if target.exists() {
            return Ok(false);
        }

        let bytes = self.get_bytes_with_retry_blocking(&file.url)?;
        cache::atomic_write_bytes(&target, &bytes)?;
        Ok(true)
    }

    fn download_gate_for(&self, target: &Path) -> Arc<tokio::sync::Mutex<()>> {
        self.download_gates
            .entry(target.to_path_buf())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

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

    fn evict_if_needed(&self, exempt_font_id: &str) -> Result<(), FontsourceFontdbError> {
        let max_bytes = self.max_cache_bytes();
        if max_bytes == 0 {
            return Ok(());
        }

        let size = cache::calculate_cache_size_bytes(&self.config.cache_dir)?;
        if size <= max_bytes {
            return Ok(());
        }

        cache::evict_lru_until_size(&self.config.cache_dir, max_bytes, exempt_font_id)
    }

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

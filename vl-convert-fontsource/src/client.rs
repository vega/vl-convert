use crate::cache;
use crate::config::ClientConfig;
use crate::error::FontsourceError;
use crate::resolve::{dedupe_variants, resolve_download_plan, ResolvedTtfFile};
use crate::types::{family_to_id, FontStyle, FontsourceFont, LoadedFontBatch, VariantRequest};
use backon::{BlockingRetryable, ExponentialBuilder, Retryable};
use dashmap::DashMap;
use futures_util::stream::{self, StreamExt};
use reqwest::StatusCode;
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Per-variant mutex that serializes concurrent download+merge of the same
/// font variant to avoid duplicate work.
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
    /// Built eagerly — unlike the blocking client, this is safe to construct
    /// inside an async context.
    async_client: reqwest::Client,
    /// Lazily initialized: creates an internal tokio runtime, so must not be
    /// built inside an async context.
    blocking_client: Mutex<Option<reqwest::blocking::Client>>,
    max_cache_bytes: AtomicU64,
    download_gates: DashMap<String, Arc<DownloadGate>>,
}

impl FontsourceClient {
    /// Create a new client from the given configuration.
    pub fn new(mut config: ClientConfig) -> Result<Self, FontsourceError> {
        if config.max_parallel_downloads == 0 {
            config.max_parallel_downloads = 1;
        }

        if let Some(ref dir) = config.cache_dir {
            if !dir.is_absolute() {
                return Err(FontsourceError::RelativeCacheDir(dir.clone()));
            }
            cache::check_or_init_cache_format(dir)?;
        }

        let async_client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .map_err(|e| FontsourceError::Http(e.to_string()))?;

        Ok(Self {
            max_cache_bytes: AtomicU64::new(config.max_cache_bytes),
            config,
            async_client,
            blocking_client: Mutex::new(None),
            download_gates: DashMap::new(),
        })
    }

    /// Update the maximum font cache size (in bytes) at runtime.
    pub fn set_max_cache_bytes(&self, bytes: u64) {
        self.max_cache_bytes.store(bytes, Ordering::Relaxed);
    }

    /// Load a font family from Fontsource (async).
    pub async fn load(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<LoadedFontBatch, FontsourceError> {
        let font_id = Self::validate_load_request(family, variants)?;

        let metadata = match self.try_read_cached_metadata(&font_id) {
            Some(m) => m,
            None => {
                let m = self.fetch_metadata_async(&font_id).await?;
                self.cache_metadata(&font_id, &m)?;
                m
            }
        };

        let plan = resolve_download_plan(&font_id, &metadata, variants)?;
        let variant_groups = group_files_by_variant(&plan.files);

        let (font_data, exempt_keys, downloaded_any) = self
            .load_font_variants_async(&font_id, &metadata, &plan.files, &variant_groups)
            .await?;

        if downloaded_any {
            self.evict_if_needed(&exempt_keys)?;
        }

        Ok(LoadedFontBatch::new(
            font_id,
            Some(metadata.font_type),
            if let Some(v) = variants {
                dedupe_variants(v)
            } else {
                plan.loaded_variants
            },
            font_data.len(),
            font_data,
        ))
    }

    /// Load a font family from Fontsource (blocking).
    pub fn load_blocking(
        &self,
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<LoadedFontBatch, FontsourceError> {
        let font_id = Self::validate_load_request(family, variants)?;

        let metadata = match self.try_read_cached_metadata(&font_id) {
            Some(m) => m,
            None => {
                let m = self.fetch_metadata_blocking(&font_id)?;
                self.cache_metadata(&font_id, &m)?;
                m
            }
        };

        let plan = resolve_download_plan(&font_id, &metadata, variants)?;
        let variant_groups = group_files_by_variant(&plan.files);

        let (font_data, exempt_keys, downloaded_any) =
            self.load_font_variants_blocking(&font_id, &metadata, &plan.files, &variant_groups)?;

        if downloaded_any {
            self.evict_if_needed(&exempt_keys)?;
        }

        Ok(LoadedFontBatch::new(
            font_id,
            Some(metadata.font_type),
            if let Some(v) = variants {
                dedupe_variants(v)
            } else {
                plan.loaded_variants
            },
            font_data.len(),
            font_data,
        ))
    }

    /// For each (weight, style) variant, check the font cache or
    /// download+merge+cache. Returns (font data, exempt keys, downloaded_any).
    #[allow(clippy::type_complexity)]
    async fn load_font_variants_async(
        &self,
        font_id: &str,
        metadata: &FontsourceFont,
        files: &[ResolvedTtfFile],
        variant_groups: &BTreeMap<(u16, FontStyle), Vec<usize>>,
    ) -> Result<(Vec<Arc<Vec<u8>>>, HashSet<String>, bool), FontsourceError> {
        let fonts_dir = self.config.fonts_dir();
        let can_cache = metadata.last_modified.is_some();
        let mut results = Vec::with_capacity(variant_groups.len());
        let mut exempt_keys = HashSet::new();
        let mut downloaded_any = false;

        for &(weight, style) in variant_groups.keys() {
            let cache_key = metadata
                .last_modified
                .as_ref()
                .map(|lm| cache::font_cache_key(font_id, weight, style, lm));

            // Fast path: check font cache before acquiring gate.
            if let Some(ref key) = cache_key {
                if let Some(ref dir) = fonts_dir {
                    if let Some(bytes) = cache::read_cached_font(key, dir)? {
                        let _ = cache::touch_cached_font(key, dir);
                        exempt_keys.insert(key.clone());
                        results.push(Arc::new(bytes));
                        continue;
                    }
                }
            }

            // Slow path: acquire gate, double-check, download+merge.
            let gate_key = match &cache_key {
                Some(k) => k.clone(),
                None => format!("{font_id}--{weight}-{}", style.as_str()),
            };

            let gate = self.acquire_download_gate(&gate_key);
            let variant_result: Result<(Arc<Vec<u8>>, String, bool), FontsourceError> = async {
                let _guard = gate.mutex.lock().await;

                // Double-check: another caller may have filled the cache.
                if can_cache {
                    if let Some(ref key) = cache_key {
                        if let Some(ref dir) = fonts_dir {
                            if let Some(bytes) = cache::read_cached_font(key, dir)? {
                                let _ = cache::touch_cached_font(key, dir);
                                return Ok((Arc::new(bytes), key.clone(), false));
                            }
                        }
                    }
                }

                // Download all subsets for this variant.
                let subset_data = self
                    .download_variant_subsets_async(font_id, weight, style, files, variant_groups)
                    .await?;

                let subset_refs: Vec<&[u8]> = subset_data.iter().map(|d| d.as_slice()).collect();
                let merged = crate::merge::merge_subsets(font_id, weight, style, &subset_refs)?;

                // Cache the merged result if we have a stable key.
                let key_used = if let Some(ref key) = cache_key {
                    if let Some(ref dir) = fonts_dir {
                        cache::write_cached_font_if_absent(key, dir, &merged)?;
                    }
                    key.clone()
                } else {
                    gate_key.clone()
                };

                Ok((Arc::new(merged), key_used, true))
            }
            .await;
            self.release_download_gate(&gate_key, &gate);

            let (data, key_used, was_downloaded) = variant_result?;
            exempt_keys.insert(key_used);
            downloaded_any |= was_downloaded;
            results.push(data);
        }

        Ok((results, exempt_keys, downloaded_any))
    }

    /// Download all subset files for a single (weight, style) variant (async).
    async fn download_variant_subsets_async(
        &self,
        font_id: &str,
        weight: u16,
        style: FontStyle,
        files: &[ResolvedTtfFile],
        variant_groups: &BTreeMap<(u16, FontStyle), Vec<usize>>,
    ) -> Result<Vec<Vec<u8>>, FontsourceError> {
        let file_indices = variant_groups.get(&(weight, style)).ok_or_else(|| {
            FontsourceError::Internal(format!(
                "No files for variant {font_id} {weight}-{}",
                style.as_str()
            ))
        })?;

        let urls: Vec<String> = file_indices.iter().map(|&i| files[i].url.clone()).collect();

        let results: Vec<Result<Vec<u8>, FontsourceError>> = stream::iter(urls)
            .map(|url| async move { self.get_bytes_with_retry_async(&url).await })
            .buffer_unordered(self.config.max_parallel_downloads)
            .collect()
            .await;

        results.into_iter().collect()
    }

    /// Blocking counterpart of `load_font_variants_async`.
    #[allow(clippy::type_complexity)]
    fn load_font_variants_blocking(
        &self,
        font_id: &str,
        metadata: &FontsourceFont,
        files: &[ResolvedTtfFile],
        variant_groups: &BTreeMap<(u16, FontStyle), Vec<usize>>,
    ) -> Result<(Vec<Arc<Vec<u8>>>, HashSet<String>, bool), FontsourceError> {
        let fonts_dir = self.config.fonts_dir();
        let can_cache = metadata.last_modified.is_some();
        let mut results = Vec::with_capacity(variant_groups.len());
        let mut exempt_keys = HashSet::new();
        let mut downloaded_any = false;

        for &(weight, style) in variant_groups.keys() {
            let cache_key = metadata
                .last_modified
                .as_ref()
                .map(|lm| cache::font_cache_key(font_id, weight, style, lm));

            // Fast path: check font cache.
            if let Some(ref key) = cache_key {
                if let Some(ref dir) = fonts_dir {
                    if let Some(bytes) = cache::read_cached_font(key, dir)? {
                        let _ = cache::touch_cached_font(key, dir);
                        exempt_keys.insert(key.clone());
                        results.push(Arc::new(bytes));
                        continue;
                    }
                }
            }

            // Slow path: acquire gate, double-check, download+merge.
            let gate_key = match &cache_key {
                Some(k) => k.clone(),
                None => format!("{font_id}--{weight}-{}", style.as_str()),
            };

            let gate = self.acquire_download_gate(&gate_key);
            let variant_result: Result<(Arc<Vec<u8>>, String, bool), FontsourceError> = (|| {
                let _guard = gate.mutex.blocking_lock();

                if can_cache {
                    if let Some(ref key) = cache_key {
                        if let Some(ref dir) = fonts_dir {
                            if let Some(bytes) = cache::read_cached_font(key, dir)? {
                                let _ = cache::touch_cached_font(key, dir);
                                return Ok((Arc::new(bytes), key.clone(), false));
                            }
                        }
                    }
                }

                let subset_data = self.download_variant_subsets_blocking(
                    font_id,
                    weight,
                    style,
                    files,
                    variant_groups,
                )?;

                let subset_refs: Vec<&[u8]> = subset_data.iter().map(|d| d.as_slice()).collect();
                let merged = crate::merge::merge_subsets(font_id, weight, style, &subset_refs)?;

                let key_used = if let Some(ref key) = cache_key {
                    if let Some(ref dir) = fonts_dir {
                        cache::write_cached_font_if_absent(key, dir, &merged)?;
                    }
                    key.clone()
                } else {
                    gate_key.clone()
                };

                Ok((Arc::new(merged), key_used, true))
            })(
            );
            self.release_download_gate(&gate_key, &gate);

            let (data, key_used, was_downloaded) = variant_result?;
            exempt_keys.insert(key_used);
            downloaded_any |= was_downloaded;
            results.push(data);
        }

        Ok((results, exempt_keys, downloaded_any))
    }

    /// Download all subset files for a single variant (blocking).
    fn download_variant_subsets_blocking(
        &self,
        font_id: &str,
        weight: u16,
        style: FontStyle,
        files: &[ResolvedTtfFile],
        variant_groups: &BTreeMap<(u16, FontStyle), Vec<usize>>,
    ) -> Result<Vec<Vec<u8>>, FontsourceError> {
        let file_indices = variant_groups.get(&(weight, style)).ok_or_else(|| {
            FontsourceError::Internal(format!(
                "No files for variant {font_id} {weight}-{}",
                style.as_str()
            ))
        })?;

        file_indices
            .iter()
            .map(|&i| self.get_bytes_with_retry_blocking(&files[i].url))
            .collect()
    }

    /// Check whether a font exists on Fontsource without downloading font files (async).
    pub async fn is_known_font(&self, font_id: &str) -> Result<bool, FontsourceError> {
        if self.try_read_cached_metadata(font_id).is_some() {
            return Ok(true);
        }
        match self.fetch_metadata_async(font_id).await {
            Ok(metadata) => {
                self.cache_metadata(font_id, &metadata)?;
                Ok(true)
            }
            Err(FontsourceError::FontNotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Check whether a font exists on Fontsource without downloading font files (blocking).
    pub fn is_known_font_blocking(&self, font_id: &str) -> Result<bool, FontsourceError> {
        if self.try_read_cached_metadata(font_id).is_some() {
            return Ok(true);
        }
        match self.fetch_metadata_blocking(font_id) {
            Ok(metadata) => {
                self.cache_metadata(font_id, &metadata)?;
                Ok(true)
            }
            Err(FontsourceError::FontNotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn validate_load_request(
        family: &str,
        variants: Option<&[VariantRequest]>,
    ) -> Result<String, FontsourceError> {
        let font_id = family_to_id(family)
            .ok_or_else(|| FontsourceError::InvalidFontId(family.to_string()))?;
        if let Some(requested) = variants {
            if requested.is_empty() {
                return Err(FontsourceError::NoVariantsRequested);
            }
        }
        Ok(font_id)
    }

    fn try_read_cached_metadata(&self, font_id: &str) -> Option<FontsourceFont> {
        self.config
            .metadata_dir()
            .as_deref()
            .and_then(|dir| cache::read_metadata(font_id, dir))
    }

    fn cache_metadata(
        &self,
        font_id: &str,
        metadata: &FontsourceFont,
    ) -> Result<(), FontsourceError> {
        if let Some(dir) = self.config.metadata_dir() {
            cache::write_metadata_if_absent(font_id, &dir, metadata)?;
        }
        Ok(())
    }

    fn parse_metadata_response(
        font_id: &str,
        result: Result<Vec<u8>, FontsourceError>,
    ) -> Result<FontsourceFont, FontsourceError> {
        let bytes = match result {
            Err(FontsourceError::HttpStatus { status, .. })
                if status == StatusCode::NOT_FOUND.as_u16() =>
            {
                return Err(FontsourceError::FontNotFound(font_id.to_string()));
            }
            other => other?,
        };
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn metadata_url(&self, font_id: &str) -> String {
        format!(
            "{}/{}",
            self.config.metadata_base_url.trim_end_matches('/'),
            font_id
        )
    }

    fn max_cache_bytes(&self) -> u64 {
        self.max_cache_bytes.load(Ordering::Relaxed)
    }

    fn evict_if_needed(&self, exempt_keys: &HashSet<String>) -> Result<(), FontsourceError> {
        let Some(fonts_dir) = self.config.fonts_dir() else {
            return Ok(());
        };
        let max_bytes = self.max_cache_bytes();
        if max_bytes == 0 {
            return Ok(());
        }

        let size = cache::calculate_cache_size_bytes(&fonts_dir)?;
        if size <= max_bytes {
            return Ok(());
        }

        cache::evict_lru_until_size(&fonts_dir, max_bytes, exempt_keys)
    }

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

    async fn fetch_metadata_async(&self, font_id: &str) -> Result<FontsourceFont, FontsourceError> {
        let url = self.metadata_url(font_id);
        Self::parse_metadata_response(font_id, self.get_bytes_with_retry_async(&url).await)
    }

    fn fetch_metadata_blocking(&self, font_id: &str) -> Result<FontsourceFont, FontsourceError> {
        let url = self.metadata_url(font_id);
        Self::parse_metadata_response(font_id, self.get_bytes_with_retry_blocking(&url))
    }

    async fn get_bytes_with_retry_async(&self, url: &str) -> Result<Vec<u8>, FontsourceError> {
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

    fn get_blocking_client_clone(&self) -> Result<reqwest::blocking::Client, FontsourceError> {
        let mut guard = self
            .blocking_client
            .lock()
            .map_err(|_| FontsourceError::Internal("Blocking client lock poisoned".to_string()))?;

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
            FontsourceError::Internal("Failed to join blocking client init thread".to_string())
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

/// Group resolved files by (weight, style), storing indices into the original
/// `files` slice. The BTreeMap gives deterministic ordering (weight then style).
fn group_files_by_variant(files: &[ResolvedTtfFile]) -> BTreeMap<(u16, FontStyle), Vec<usize>> {
    let mut groups: BTreeMap<(u16, FontStyle), Vec<usize>> = BTreeMap::new();
    for (i, file) in files.iter().enumerate() {
        groups.entry((file.weight, file.style)).or_default().push(i);
    }
    groups
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
        let key = "roboto--400-normal--2026-02-19";

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
        let key = "roboto--400-normal--2026-02-19";

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
        let key = "roboto--400-normal--2026-02-19";

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

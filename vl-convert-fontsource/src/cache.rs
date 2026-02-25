use crate::error::FontsourceError;
use crate::types::{family_to_id, FetchOutcome, FontsourceFont, FontsourceMarker, MARKER_FILENAME};
use backon::{ExponentialBuilder, Retryable};
use dashmap::DashMap;
use filetime::FileTime;
use fs4::fs_std::FileExt;
use log::{debug, info, warn};
use reqwest::StatusCode;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const FONTSOURCE_API: &str = "https://api.fontsource.org/v1/fonts";

/// A concurrent, disk-backed cache for Fontsource font files.
///
/// Downloads TTF files from the Fontsource API and caches them on disk.
/// Thread-safe: all methods take `&self` and use file locks + per-font
/// mutexes for coordination.
pub struct FontsourceCache {
    cache_dir: PathBuf,
    client: reqwest::Client,
    max_cache_bytes: AtomicU64, // 0 = unbounded
    download_gates: DashMap<String, Arc<tokio::sync::Mutex<()>>>,
    known_fonts: DashMap<String, bool>,
}

impl FontsourceCache {
    /// Create a new `FontsourceCache`.
    ///
    /// # Arguments
    /// * `cache_dir` - Directory for cached fonts. Defaults to the platform
    ///   cache directory under `vl-convert/fonts`.
    /// * `max_cache_bytes` - Optional maximum cache size. `None` means unbounded.
    pub fn new(
        cache_dir: Option<PathBuf>,
        max_cache_bytes: Option<u64>,
    ) -> Result<Self, FontsourceError> {
        let cache_dir = match cache_dir {
            Some(dir) => dir,
            None => dirs::cache_dir()
                .map(|d| d.join("vl-convert").join("fonts"))
                .ok_or(FontsourceError::NoCacheDir)?,
        };

        let client = reqwest::Client::builder()
            .user_agent("vl-convert")
            .build()
            .map_err(FontsourceError::Http)?;

        Ok(Self {
            cache_dir,
            client,
            max_cache_bytes: AtomicU64::new(max_cache_bytes.unwrap_or(0)),
            download_gates: DashMap::new(),
            known_fonts: DashMap::new(),
        })
    }

    /// Set the maximum cache size in bytes. 0 means unbounded.
    pub fn set_max_cache_bytes(&self, max_bytes: u64) {
        self.max_cache_bytes.store(max_bytes, Ordering::Relaxed);
    }

    /// Get the maximum cache size in bytes. 0 means unbounded.
    pub fn max_cache_bytes(&self) -> u64 {
        self.max_cache_bytes.load(Ordering::Relaxed)
    }

    /// Return the on-disk directory for a given font ID.
    pub fn font_dir(&self, font_id: &str) -> PathBuf {
        self.cache_dir.join(font_id)
    }

    /// Fetch font metadata from the Fontsource API.
    ///
    /// Maps HTTP 404 to [`FontsourceError::FontNotFound`]. Retries transient
    /// errors with exponential backoff.
    pub async fn fetch_metadata(&self, font_id: &str) -> Result<FontsourceFont, FontsourceError> {
        let url = format!("{}/{}", FONTSOURCE_API, font_id);
        let client = self.client.clone();
        let font_id_owned = font_id.to_string();

        let response = (|| {
            let client = client.clone();
            let url = url.clone();
            async move {
                let resp = client.get(&url).send().await?.error_for_status()?;
                Ok::<_, reqwest::Error>(resp)
            }
        })
        .retry(ExponentialBuilder::default())
        .when(|e: &reqwest::Error| {
            // Don't retry 404s
            if let Some(status) = e.status() {
                if status == StatusCode::NOT_FOUND {
                    return false;
                }
                // Retry server errors and rate limiting
                return status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS;
            }
            // Retry connection/timeout errors
            true
        })
        .await;

        match response {
            Ok(resp) => {
                let bytes = resp.bytes().await?;
                let font: FontsourceFont = serde_json::from_slice(&bytes)?;
                Ok(font)
            }
            Err(e) => {
                if e.status() == Some(StatusCode::NOT_FOUND) {
                    Err(FontsourceError::FontNotFound(font_id_owned))
                } else {
                    Err(FontsourceError::Http(e))
                }
            }
        }
    }

    /// Download a TTF file from `url` to `path`.
    ///
    /// If `!force` and `path` already exists, returns immediately.
    /// Downloads to a temporary file first, then atomically renames.
    /// Retries transient errors with exponential backoff.
    async fn download_ttf(
        &self,
        url: &str,
        path: &Path,
        force: bool,
    ) -> Result<(), FontsourceError> {
        if !force && path.exists() {
            return Ok(());
        }

        let client = self.client.clone();
        let url_owned = url.to_string();
        let path_owned = path.to_path_buf();

        (|| {
            let client = client.clone();
            let url = url_owned.clone();
            let path = path_owned.clone();
            async move {
                let bytes = client
                    .get(&url)
                    .send()
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await?;

                // Write to a unique temp file in the same directory
                let parent = path.parent().unwrap_or(&path);
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("font");
                let temp_name = format!(
                    "{}.{}.{}.tmp",
                    file_name,
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos()
                );
                let temp_path = parent.join(&temp_name);

                tokio::fs::write(&temp_path, &bytes)
                    .await
                    .inspect_err(|_e| {
                        // Clean up temp file on write error
                        let _ = std::fs::remove_file(&temp_path);
                    })?;

                if let Err(e) = atomic_rename(&temp_path, &path) {
                    // Clean up temp file on rename error
                    let _ = std::fs::remove_file(&temp_path);
                    return Err(FontsourceError::Io(e));
                }

                Ok::<_, FontsourceError>(())
            }
        })
        .retry(ExponentialBuilder::default())
        .when(|e: &FontsourceError| {
            matches!(e, FontsourceError::Http(re) if {
                if let Some(status) = re.status() {
                    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
                } else {
                    // Retry connection/timeout errors
                    true
                }
            })
        })
        .await
    }

    /// Fetch a font by family name, downloading if not already cached.
    ///
    /// Returns a [`FetchOutcome`] indicating whether a download occurred
    /// and the path to the font directory.
    ///
    /// # Fast path
    /// If the `.fontsource.json` marker exists and at least one `.ttf` file
    /// is present, the font is considered cached. The marker's mtime is
    /// touched for LRU bookkeeping.
    ///
    /// # Slow path
    /// Fetches metadata from the Fontsource API, downloads all TTF files
    /// (all subsets, weights, and styles), then writes the marker.
    pub async fn fetch(&self, family: &str) -> Result<FetchOutcome, FontsourceError> {
        let font_id = family_to_id(family)
            .ok_or_else(|| FontsourceError::InvalidFontId(family.to_string()))?;
        let font_dir = self.font_dir(&font_id);

        // ---- Fast path: marker exists + TTFs present ----
        if self.check_cache_hit(&font_dir).await? {
            let font_type = self.read_marker(&font_dir).await.and_then(|m| m.font_type);
            return Ok(FetchOutcome {
                path: font_dir,
                font_id,
                downloaded: false,
                font_type,
            });
        }

        // ---- Slow path: acquire per-font gate ----
        let gate = self
            .download_gates
            .entry(font_id.clone())
            .or_default()
            .clone();
        let _guard = gate.lock().await;

        // Re-check after acquiring gate (another task may have completed the download)
        if self.check_cache_hit(&font_dir).await? {
            let font_type = self.read_marker(&font_dir).await.and_then(|m| m.font_type);
            return Ok(FetchOutcome {
                path: font_dir,
                font_id,
                downloaded: false,
                font_type,
            });
        }

        // Acquire shared file lock for the mutation sequence
        let font_type = self.do_download(&font_id, family, &font_dir, false).await?;

        Ok(FetchOutcome {
            path: font_dir,
            font_id,
            downloaded: true,
            font_type: Some(font_type),
        })
    }

    /// Re-fetch a font, forcing re-download even if cached.
    ///
    /// Deletes existing marker and TTF files, then re-downloads everything.
    /// File deletion and re-download both happen under an exclusive cache lock
    /// inside `do_download` (when `force` is true) to prevent races with
    /// concurrent registration, eviction, or clear operations.
    pub async fn refetch(&self, family: &str) -> Result<FetchOutcome, FontsourceError> {
        let font_id = family_to_id(family)
            .ok_or_else(|| FontsourceError::InvalidFontId(family.to_string()))?;
        let font_dir = self.font_dir(&font_id);

        // Acquire per-font gate
        let gate = self
            .download_gates
            .entry(font_id.clone())
            .or_default()
            .clone();
        let _guard = gate.lock().await;

        // Delete + re-download under exclusive cache lock (force=true triggers delete)
        let font_type = self.do_download(&font_id, family, &font_dir, true).await?;

        Ok(FetchOutcome {
            path: font_dir,
            font_id,
            downloaded: true,
            font_type: Some(font_type),
        })
    }

    /// Clear cached files for a specific font family.
    ///
    /// Acquires an exclusive file lock to prevent concurrent reads/writes.
    pub fn clear(&self, family: &str) -> Result<(), FontsourceError> {
        let font_id = family_to_id(family)
            .ok_or_else(|| FontsourceError::InvalidFontId(family.to_string()))?;
        let font_dir = self.font_dir(&font_id);

        self.with_exclusive_lock(|| {
            if font_dir.exists() {
                std::fs::remove_dir_all(&font_dir)?;
            }
            Ok(())
        })
    }

    /// Clear the entire font cache.
    ///
    /// Acquires an exclusive file lock to prevent concurrent reads/writes.
    pub fn clear_all(&self) -> Result<(), FontsourceError> {
        self.with_exclusive_lock(|| {
            if self.cache_dir.exists() {
                // Remove all subdirectories but preserve the lock file
                let entries = std::fs::read_dir(&self.cache_dir)?;
                for entry in entries {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        std::fs::remove_dir_all(&path)?;
                    } else if path.file_name().and_then(|n| n.to_str()) != Some(".cache-lock") {
                        std::fs::remove_file(&path)?;
                    }
                }
            }
            Ok(())
        })
    }

    /// Run a closure while holding a shared file lock on `.cache-lock`.
    pub fn with_cache_lock<F, R>(&self, f: F) -> Result<R, FontsourceError>
    where
        F: FnOnce() -> R,
    {
        std::fs::create_dir_all(&self.cache_dir)?;
        let lock_path = self.cache_dir.join(".cache-lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)?;
        lock_file.lock_shared()?;
        let result = f();
        // lock released when lock_file is dropped
        Ok(result)
    }

    /// Check whether a font ID is known to Fontsource.
    ///
    /// Results are cached in-memory. Transport errors are propagated without
    /// caching (so subsequent calls can retry).
    pub async fn is_known_font(&self, font_id: &str) -> Result<bool, FontsourceError> {
        // Check in-memory cache first
        if let Some(entry) = self.known_fonts.get(font_id) {
            return Ok(*entry);
        }

        let url = format!("{}/{}", FONTSOURCE_API, font_id);
        let response = self.client.get(&url).send().await?;

        match response.status() {
            StatusCode::OK => {
                self.known_fonts.insert(font_id.to_string(), true);
                Ok(true)
            }
            StatusCode::NOT_FOUND => {
                self.known_fonts.insert(font_id.to_string(), false);
                Ok(false)
            }
            _status => {
                // Transport/server error: propagate without caching
                Err(FontsourceError::Http(
                    response.error_for_status().unwrap_err(),
                ))
            }
        }
    }

    /// Calculate the total size of all cached files in bytes.
    pub fn calculate_cache_size_bytes(&self) -> Result<u64, FontsourceError> {
        let mut total: u64 = 0;
        if !self.cache_dir.exists() {
            return Ok(0);
        }

        let entries = std::fs::read_dir(&self.cache_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let sub_entries = std::fs::read_dir(&path)?;
                for sub_entry in sub_entries {
                    let sub_entry = sub_entry?;
                    if sub_entry.path().is_file() {
                        total += sub_entry.metadata()?.len();
                    }
                }
            }
        }

        Ok(total)
    }

    /// Evict least-recently-used fonts until the cache size is at or below
    /// `target_bytes`.
    ///
    /// Acquires an exclusive file lock. Fonts in `exempt` are never evicted
    /// (used to protect fonts just downloaded in the current batch).
    pub fn evict_lru_until_size(
        &self,
        target_bytes: u64,
        exempt: &HashSet<String>,
    ) -> Result<(), FontsourceError> {
        self.with_exclusive_lock(|| {
            if !self.cache_dir.exists() {
                return Ok(());
            }

            // Collect font directories with their sizes and mtime
            let mut font_entries: Vec<(String, PathBuf, u64, std::time::SystemTime)> = Vec::new();
            let mut total_size: u64 = 0;

            let dir_entries = std::fs::read_dir(&self.cache_dir)?;
            for entry in dir_entries {
                let entry = entry?;
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let font_id = match path.file_name().and_then(|n| n.to_str()) {
                    Some(name) => name.to_string(),
                    None => continue,
                };

                // Calculate directory size
                let mut dir_size: u64 = 0;
                let sub_entries = std::fs::read_dir(&path)?;
                for sub_entry in sub_entries {
                    let sub_entry = sub_entry?;
                    if sub_entry.path().is_file() {
                        dir_size += sub_entry.metadata()?.len();
                    }
                }

                // Get mtime of .fontsource.json marker (LRU key)
                let marker_path = path.join(MARKER_FILENAME);
                let mtime = if marker_path.exists() {
                    marker_path
                        .metadata()?
                        .modified()
                        .unwrap_or(std::time::UNIX_EPOCH)
                } else {
                    std::time::UNIX_EPOCH
                };

                total_size += dir_size;
                font_entries.push((font_id, path, dir_size, mtime));
            }

            if total_size <= target_bytes {
                return Ok(());
            }

            // Sort by mtime ascending (oldest first) for LRU eviction
            font_entries.sort_by(|a, b| a.3.cmp(&b.3));

            for (font_id, path, dir_size, _) in &font_entries {
                if total_size <= target_bytes {
                    break;
                }

                if exempt.contains(font_id) {
                    continue;
                }

                info!("Evicting cached font '{}' ({} bytes)", font_id, dir_size);
                if let Err(e) = std::fs::remove_dir_all(path) {
                    warn!("Failed to evict font '{}': {}", font_id, e);
                    continue;
                }

                total_size = total_size.saturating_sub(*dir_size);
            }

            if total_size > target_bytes {
                warn!(
                    "Cache size ({} bytes) still exceeds target ({} bytes) \
                     after evicting all non-exempt fonts",
                    total_size, target_bytes
                );
            }

            Ok(())
        })
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Check if the cache has a valid entry for the given font directory.
    ///
    /// Valid means: marker file exists AND at least one `.ttf` file is present.
    /// On cache hit, touches the marker mtime for LRU bookkeeping.
    async fn check_cache_hit(&self, font_dir: &Path) -> Result<bool, FontsourceError> {
        let marker_path = font_dir.join(MARKER_FILENAME);

        if !marker_path.exists() {
            return Ok(false);
        }

        // Acquire shared lock to verify TTF files
        let lock_path = self.cache_dir.join(".cache-lock");
        std::fs::create_dir_all(&self.cache_dir)?;

        use fs4::tokio::AsyncFileExt;
        let lock_file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .await?;
        lock_file.lock_shared()?;

        let has_ttf = self.has_ttf_files(font_dir).await;

        // lock released when lock_file is dropped
        drop(lock_file);

        if has_ttf {
            // Touch marker mtime for LRU bookkeeping
            if let Err(e) = filetime::set_file_mtime(&marker_path, FileTime::now()) {
                warn!(
                    "Failed to touch marker mtime for {}: {}",
                    marker_path.display(),
                    e
                );
            }
            Ok(true)
        } else {
            // Stale marker: directory exists but no TTFs
            debug!(
                "Stale cache marker at {} (no TTF files found)",
                marker_path.display()
            );
            Ok(false)
        }
    }

    /// Check if a directory contains at least one `.ttf` file.
    async fn has_ttf_files(&self, dir: &Path) -> bool {
        let mut entries = match tokio::fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(_) => return false,
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("ttf"))
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Read the marker file from a font directory, if it exists.
    async fn read_marker(&self, font_dir: &Path) -> Option<FontsourceMarker> {
        let marker_path = font_dir.join(MARKER_FILENAME);
        let data = tokio::fs::read_to_string(&marker_path).await.ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Perform the full download sequence for a font.
    ///
    /// When `force` is false (normal fetch), acquires a **shared** file lock
    /// so multiple concurrent downloads of different fonts can proceed.
    ///
    /// When `force` is true (refetch), acquires an **exclusive** file lock
    /// to prevent readers (e.g. registration via `with_cache_lock`) from
    /// seeing a partially-deleted font directory.
    ///
    /// Returns the `font_type` string (`"google"` or `"other"`) from the API.
    async fn do_download(
        &self,
        font_id: &str,
        family: &str,
        font_dir: &Path,
        force: bool,
    ) -> Result<String, FontsourceError> {
        let lock_path = self.cache_dir.join(".cache-lock");
        std::fs::create_dir_all(&self.cache_dir)?;

        use fs4::tokio::AsyncFileExt;
        let lock_file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .await?;

        if force {
            // Exclusive lock: block readers while we delete + re-download
            lock_file.lock_exclusive()?;
            self.delete_font_files(font_dir).await?;
        } else {
            // Shared lock: compatible with other downloads and registrations
            lock_file.lock_shared()?;
        }

        // Create font directory
        tokio::fs::create_dir_all(font_dir).await?;

        // Fetch metadata
        info!("Fetching metadata for font '{}'", font_id);
        let metadata = self.fetch_metadata(font_id).await?;

        // Download all TTF files (all subsets, all weights, all styles)
        for (weight_key, styles) in &metadata.variants {
            for (style_key, subsets) in styles {
                for (subset, urls) in subsets {
                    if let Some(ref ttf_url) = urls.url.ttf {
                        let filename = format!("{}-{}-{}.ttf", subset, weight_key, style_key);
                        let file_path = font_dir.join(&filename);

                        debug!("Downloading {}", filename);
                        self.download_ttf(ttf_url, &file_path, force).await?;
                    }
                }
            }
        }

        let font_type = metadata.font_type.clone();

        // Write marker via atomic rename
        let marker = FontsourceMarker {
            id: font_id.to_string(),
            family: family.to_string(),
            version: metadata.version.clone(),
            fetched_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            font_type: Some(font_type.clone()),
        };
        let marker_json = serde_json::to_string_pretty(&marker)?;
        let marker_path = font_dir.join(MARKER_FILENAME);
        let temp_marker = font_dir.join(format!(
            ".fontsource.json.{}.{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::write(&temp_marker, marker_json)?;
        atomic_rename(&temp_marker, &marker_path)?;

        info!(
            "Font '{}' ({}) cached at {}",
            family,
            font_id,
            font_dir.display()
        );

        // lock released when lock_file is dropped
        Ok(font_type)
    }

    /// Delete marker and TTF files from a font directory.
    async fn delete_font_files(&self, font_dir: &Path) -> Result<(), FontsourceError> {
        if !font_dir.exists() {
            return Ok(());
        }

        let marker_path = font_dir.join(MARKER_FILENAME);
        if marker_path.exists() {
            tokio::fs::remove_file(&marker_path).await?;
        }

        let mut entries = tokio::fs::read_dir(font_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("ttf"))
                .unwrap_or(false)
            {
                tokio::fs::remove_file(&path).await?;
            }
        }

        Ok(())
    }

    /// Run a closure while holding an exclusive file lock on `.cache-lock`.
    fn with_exclusive_lock<F, R>(&self, f: F) -> Result<R, FontsourceError>
    where
        F: FnOnce() -> Result<R, FontsourceError>,
    {
        std::fs::create_dir_all(&self.cache_dir)?;
        let lock_path = self.cache_dir.join(".cache-lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)?;
        lock_file.lock_exclusive()?;
        // lock_file is held until end of scope, ensuring f() runs under the lock
        f()
    }
}

/// Atomically rename `src` to `dst`.
///
/// If the rename fails with `AlreadyExists`, treats it as success
/// (a concurrent download wrote the same file) and deletes the temp file.
fn atomic_rename(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Concurrent writer already placed the file — clean up our temp
            let _ = std::fs::remove_file(src);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// Create a fake font directory with a marker and a TTF file of the given size.
    fn create_fake_font(cache_dir: &Path, font_id: &str, ttf_size: usize, age_secs: i64) {
        let font_dir = cache_dir.join(font_id);
        std::fs::create_dir_all(&font_dir).unwrap();

        // Write a fake TTF file
        let ttf_path = font_dir.join("latin-400-normal.ttf");
        let data = vec![0u8; ttf_size];
        std::fs::write(&ttf_path, &data).unwrap();

        // Write marker
        let marker = FontsourceMarker {
            id: font_id.to_string(),
            family: font_id.to_string(),
            version: "1.0.0".to_string(),
            fetched_at: 1000000,
            font_type: Some("google".to_string()),
        };
        let marker_path = font_dir.join(MARKER_FILENAME);
        std::fs::write(&marker_path, serde_json::to_string(&marker).unwrap()).unwrap();

        // Set marker mtime to control LRU ordering
        let base = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        let mtime = filetime::FileTime::from_unix_time(1_700_000_000 + age_secs, 0);
        filetime::set_file_mtime(&marker_path, mtime).unwrap();
        filetime::set_file_mtime(&ttf_path, base).unwrap();
    }

    #[test]
    fn test_calculate_cache_size_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = FontsourceCache::new(Some(tmp.path().to_path_buf()), None).unwrap();
        assert_eq!(cache.calculate_cache_size_bytes().unwrap(), 0);
    }

    #[test]
    fn test_calculate_cache_size() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();

        create_fake_font(cache_dir, "roboto", 1000, 0);
        create_fake_font(cache_dir, "open-sans", 2000, 10);

        let cache = FontsourceCache::new(Some(cache_dir.to_path_buf()), None).unwrap();
        let size = cache.calculate_cache_size_bytes().unwrap();

        // Each font has a TTF file + marker file. Marker is ~80-100 bytes JSON.
        // TTF sizes: 1000 + 2000 = 3000, plus two markers
        assert!(size >= 3000, "Expected at least 3000 bytes, got {}", size);
        assert!(
            size < 4000,
            "Expected less than 4000 bytes (markers are small), got {}",
            size
        );
    }

    #[test]
    fn test_evict_lru_oldest_first() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();

        // Create three fonts with different ages (age_secs controls mtime)
        // oldest (age_secs=0) → middle (age_secs=100) → newest (age_secs=200)
        create_fake_font(cache_dir, "font-old", 1000, 0);
        create_fake_font(cache_dir, "font-mid", 1000, 100);
        create_fake_font(cache_dir, "font-new", 1000, 200);

        let cache = FontsourceCache::new(Some(cache_dir.to_path_buf()), None).unwrap();

        // Set target that requires evicting at least one font
        // Total is ~3000 + markers, target of 2500 should evict the oldest
        let exempt = HashSet::new();
        cache.evict_lru_until_size(2500, &exempt).unwrap();

        // Oldest font should be evicted
        assert!(
            !cache_dir.join("font-old").exists(),
            "Oldest font should be evicted"
        );
        assert!(
            cache_dir.join("font-mid").exists(),
            "Middle font should remain"
        );
        assert!(
            cache_dir.join("font-new").exists(),
            "Newest font should remain"
        );
    }

    #[test]
    fn test_evict_respects_exempt_set() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();

        create_fake_font(cache_dir, "font-old", 1000, 0);
        create_fake_font(cache_dir, "font-mid", 1000, 100);
        create_fake_font(cache_dir, "font-new", 1000, 200);

        let cache = FontsourceCache::new(Some(cache_dir.to_path_buf()), None).unwrap();

        // Exempt the oldest font — eviction should skip it
        let mut exempt = HashSet::new();
        exempt.insert("font-old".to_string());

        // Target requires evicting one font
        cache.evict_lru_until_size(2500, &exempt).unwrap();

        // Oldest is exempt, so middle (next oldest) should be evicted
        assert!(
            cache_dir.join("font-old").exists(),
            "Exempt font should not be evicted"
        );
        assert!(
            !cache_dir.join("font-mid").exists(),
            "Next oldest non-exempt font should be evicted"
        );
        assert!(
            cache_dir.join("font-new").exists(),
            "Newest font should remain"
        );
    }

    #[test]
    fn test_evict_no_op_when_under_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();

        create_fake_font(cache_dir, "roboto", 1000, 0);

        let cache = FontsourceCache::new(Some(cache_dir.to_path_buf()), None).unwrap();

        // Target is larger than current size — nothing should be evicted
        cache
            .evict_lru_until_size(1_000_000, &HashSet::new())
            .unwrap();

        assert!(
            cache_dir.join("roboto").exists(),
            "Font should remain when under limit"
        );
    }

    #[test]
    fn test_evict_all_exempt_logs_warning() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();

        create_fake_font(cache_dir, "font-a", 2000, 0);
        create_fake_font(cache_dir, "font-b", 2000, 100);

        let cache = FontsourceCache::new(Some(cache_dir.to_path_buf()), None).unwrap();

        // Exempt both fonts, target is tiny — should warn but not crash
        let mut exempt = HashSet::new();
        exempt.insert("font-a".to_string());
        exempt.insert("font-b".to_string());

        // This should not error, just log a warning
        cache.evict_lru_until_size(100, &exempt).unwrap();

        // Both fonts should still exist
        assert!(cache_dir.join("font-a").exists());
        assert!(cache_dir.join("font-b").exists());
    }

    #[test]
    fn test_atomic_rename_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src.txt");
        let dst = tmp.path().join("dst.txt");
        std::fs::write(&src, "hello").unwrap();

        atomic_rename(&src, &dst).unwrap();

        assert!(!src.exists());
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "hello");
    }

    #[test]
    fn test_atomic_rename_concurrent() {
        let tmp = tempfile::tempdir().unwrap();
        let dst = tmp.path().join("target.ttf");
        let num_threads = 8;

        // Create all temp files first
        let mut temp_paths = Vec::new();
        for i in 0..num_threads {
            let src = tmp.path().join(format!("target.ttf.{}.tmp", i));
            std::fs::write(&src, format!("content-{}", i)).unwrap();
            temp_paths.push(src);
        }

        // Spawn threads that all try to rename to the same target
        let handles: Vec<_> = temp_paths
            .into_iter()
            .map(|src| {
                let dst = dst.clone();
                thread::spawn(move || atomic_rename(&src, &dst))
            })
            .collect();

        for handle in handles {
            // All should succeed (no errors)
            handle.join().unwrap().unwrap();
        }

        // Exactly one file at target
        assert!(dst.exists());
        // All temp files should be cleaned up
        for i in 0..num_threads {
            let src = tmp.path().join(format!("target.ttf.{}.tmp", i));
            assert!(!src.exists(), "Temp file {} should be cleaned up", i);
        }
    }

    #[test]
    fn test_evict_multiple_to_reach_target() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path();

        // Create 5 fonts, each 1000 bytes, with increasing mtime
        for i in 0..5 {
            create_fake_font(cache_dir, &format!("font-{}", i), 1000, i * 100);
        }

        let cache = FontsourceCache::new(Some(cache_dir.to_path_buf()), None).unwrap();

        // Target: keep ~2 fonts worth of data (2500 bytes including markers)
        cache.evict_lru_until_size(2500, &HashSet::new()).unwrap();

        // The 3 oldest should be evicted
        assert!(
            !cache_dir.join("font-0").exists(),
            "Oldest should be evicted"
        );
        assert!(
            !cache_dir.join("font-1").exists(),
            "Second oldest should be evicted"
        );
        assert!(
            !cache_dir.join("font-2").exists(),
            "Third oldest should be evicted"
        );
        // The 2 newest should remain
        assert!(cache_dir.join("font-3").exists(), "Fourth should remain");
        assert!(cache_dir.join("font-4").exists(), "Newest should remain");
    }
}

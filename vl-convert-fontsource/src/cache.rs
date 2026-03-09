use crate::error::FontsourceError;
use crate::types::{FontStyle, FontsourceFont};
use filetime::FileTime;
use fs4::fs_std::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Bump this when the on-disk cache layout changes.
/// A mismatch causes the entire cache to be wiped and recreated.
const CACHE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct CacheMeta {
    format_version: u32,
}

/// Validate the on-disk cache format version, wiping stale data if needed.
///
/// Must be called once during client construction. Runs under an exclusive
/// file lock so concurrent processes sharing the same cache dir are safe.
pub(crate) fn check_or_init_cache_format(cache_dir: &Path) -> Result<(), FontsourceError> {
    std::fs::create_dir_all(cache_dir)?;

    with_exclusive_cache_lock(cache_dir, || {
        let meta_path = cache_dir.join("cache-meta.json");

        let needs_wipe = match std::fs::read(&meta_path) {
            Ok(bytes) => match serde_json::from_slice::<CacheMeta>(&bytes) {
                Ok(meta) => meta.format_version != CACHE_FORMAT_VERSION,
                Err(_) => true,
            },
            Err(_) => true,
        };

        if needs_wipe {
            // Remove all known subdirectories (current and legacy).
            for subdir in &["metadata", "fonts", "merged", "blobs"] {
                let path = cache_dir.join(subdir);
                if path.exists() {
                    let _ = std::fs::remove_dir_all(&path);
                }
            }

            // Recreate required directories.
            std::fs::create_dir_all(cache_dir.join("metadata"))?;
            std::fs::create_dir_all(cache_dir.join("fonts"))?;

            let meta = CacheMeta {
                format_version: CACHE_FORMAT_VERSION,
            };
            let data = serde_json::to_vec_pretty(&meta)?;
            // Write directly (not atomic) — we hold the exclusive lock.
            std::fs::write(&meta_path, data)?;
        }

        Ok(())
    })
}

pub(crate) fn read_metadata(font_id: &str, metadata_cache_dir: &Path) -> Option<FontsourceFont> {
    let path = metadata_cache_dir.join(format!("{font_id}.json"));
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(_) => return None,
    };

    match serde_json::from_slice(&bytes) {
        Ok(metadata) => Some(metadata),
        Err(_) => {
            // Self-heal a corrupt metadata cache entry.
            remove_path_if_present(&path);
            None
        }
    }
}

pub(crate) fn write_metadata_if_absent(
    font_id: &str,
    metadata_cache_dir: &Path,
    metadata: &FontsourceFont,
) -> Result<(), FontsourceError> {
    let path = metadata_cache_dir.join(format!("{font_id}.json"));
    if path.exists() {
        return Ok(());
    }
    let data = serde_json::to_vec_pretty(metadata)?;
    atomic_write_bytes(&path, &data)
}

pub(crate) fn font_cache_key(
    font_id: &str,
    weight: u16,
    style: FontStyle,
    last_modified: &str,
) -> String {
    // Sanitize inputs to prevent path traversal from API metadata.
    let safe_id = font_id.replace(['/', '\\', '.'], "_");
    let safe_modified = last_modified.replace(['/', '\\', '.'], "_");
    format!(
        "{safe_id}--{weight}-{}--{safe_modified}.ttf",
        style.as_str()
    )
}

fn font_cache_path(key: &str, fonts_dir: &Path) -> PathBuf {
    fonts_dir.join(key)
}

pub(crate) fn read_cached_font(
    key: &str,
    fonts_dir: &Path,
) -> Result<Option<Vec<u8>>, FontsourceError> {
    let path = font_cache_path(key, fonts_dir);
    match std::fs::read(&path) {
        Ok(bytes) => {
            if has_ttf_magic_bytes(&bytes) {
                Ok(Some(bytes))
            } else {
                remove_path_if_present(&path);
                Ok(None)
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => {
            remove_path_if_present(&path);
            Ok(None)
        }
    }
}

pub(crate) fn write_cached_font_if_absent(
    key: &str,
    fonts_dir: &Path,
    bytes: &[u8],
) -> Result<(), FontsourceError> {
    let path = font_cache_path(key, fonts_dir);
    if let Ok(meta) = std::fs::symlink_metadata(&path) {
        if !meta.is_file() {
            remove_path_if_present(&path);
        }
    }
    atomic_write_bytes(&path, bytes)
}

pub(crate) fn touch_cached_font(key: &str, fonts_dir: &Path) -> Result<(), FontsourceError> {
    let path = font_cache_path(key, fonts_dir);
    match filetime::set_file_mtime(&path, FileTime::now()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn is_ttf_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("ttf"))
            .unwrap_or(false)
}

pub(crate) fn calculate_cache_size_bytes(
    fonts_dir: &Path,
) -> Result<u64, FontsourceError> {
    if !fonts_dir.exists() {
        return Ok(0);
    }

    let mut total = 0u64;
    for entry in std::fs::read_dir(fonts_dir)? {
        let entry = match entry {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        let path = entry.path();
        if is_ttf_file(&path) {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            total = total.saturating_add(meta.len());
        }
    }

    Ok(total)
}

pub(crate) fn evict_lru_until_size(
    fonts_dir: &Path,
    target_bytes: u64,
    exempt_keys: &HashSet<String>,
) -> Result<(), FontsourceError> {
    with_exclusive_cache_lock(fonts_dir, || {
        let mut entries: Vec<(String, PathBuf, u64, std::time::SystemTime)> = Vec::new();
        let mut total_size = 0u64;

        if !fonts_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(fonts_dir)? {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            if !is_ttf_file(&path) {
                continue;
            }

            let key = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let size = meta.len();
            let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            total_size = total_size.saturating_add(size);
            entries.push((key, path, size, mtime));
        }

        if total_size <= target_bytes {
            return Ok(());
        }

        // Sort oldest first for LRU eviction.
        entries.sort_by(|a, b| a.3.cmp(&b.3));

        for (key, path, size, _) in entries {
            if total_size <= target_bytes {
                break;
            }
            if exempt_keys.contains(&key) {
                continue;
            }

            if std::fs::remove_file(&path).is_ok() || !path.exists() {
                total_size = total_size.saturating_sub(size);
            }
        }

        Ok(())
    })
}

/// Check whether `bytes` starts with a recognised font magic number.
fn has_ttf_magic_bytes(bytes: &[u8]) -> bool {
    bytes.len() >= 4
        && (bytes[..4] == [0, 1, 0, 0]  // TrueType
            || bytes[..4] == *b"OTTO"    // OpenType/CFF
            || bytes[..4] == *b"ttcf") // TrueType Collection
}

pub(crate) fn atomic_write_bytes(dst: &Path, bytes: &[u8]) -> Result<(), FontsourceError> {
    let parent = dst.parent().ok_or_else(|| {
        FontsourceError::Internal(format!("No parent directory for {}", dst.display()))
    })?;

    std::fs::create_dir_all(parent)?;

    let file_name = dst.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let temp_name = format!(
        "{}.{}.{}.tmp",
        file_name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let temp_path = parent.join(temp_name);

    if let Err(e) = std::fs::write(&temp_path, bytes) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(e.into());
    }

    // Hard-link the completed temp file into place so an existing destination
    // cannot be overwritten by a racy rename.
    match std::fs::hard_link(&temp_path, dst) {
        Ok(()) => {
            let _ = std::fs::remove_file(&temp_path);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists || dst.exists() => {
            let _ = std::fs::remove_file(&temp_path);
            Ok(())
        }
        Err(_) => match std::fs::rename(&temp_path, dst) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists || dst.exists() => {
                let _ = std::fs::remove_file(&temp_path);
                Ok(())
            }
            Err(e) => {
                let _ = std::fs::remove_file(&temp_path);
                Err(e.into())
            }
        },
    }
}

fn remove_path_if_present(path: &Path) {
    if std::fs::remove_file(path).is_ok() {
        return;
    }

    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.is_dir() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

pub(crate) fn with_exclusive_cache_lock<F, R>(cache_dir: &Path, f: F) -> Result<R, FontsourceError>
where
    F: FnOnce() -> Result<R, FontsourceError>,
{
    std::fs::create_dir_all(cache_dir)?;
    let lock_path = cache_dir.join(".cache-lock");
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    lock_file.lock_exclusive()?;

    // Lock released when `lock_file` is dropped on return
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_write_bytes_existing_file_preserved() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("font.ttf");

        std::fs::write(&path, b"first").unwrap();
        atomic_write_bytes(&path, b"second").unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes, b"first");
    }

    #[test]
    fn test_font_cache_key_format() {
        let key = font_cache_key("roboto", 400, FontStyle::Normal, "2026-02-19");
        assert_eq!(key, "roboto--400-normal--2026-02-19.ttf");
    }

    #[test]
    fn test_font_cache_key_italic() {
        let key = font_cache_key("roboto", 700, FontStyle::Italic, "2026-02-19");
        assert_eq!(key, "roboto--700-italic--2026-02-19.ttf");
    }

    /// Fake TTF bytes with a valid TrueType magic header.
    fn fake_ttf(payload: &[u8]) -> Vec<u8> {
        let mut v = vec![0x00, 0x01, 0x00, 0x00];
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn test_read_write_merged_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "test--400-normal--2026-01-01.ttf";
        let data = fake_ttf(b"font data");

        assert!(read_cached_font(key, tmp.path()).unwrap().is_none());
        write_cached_font_if_absent(key, tmp.path(), &data).unwrap();

        let read_back = read_cached_font(key, tmp.path()).unwrap().unwrap();
        assert_eq!(read_back, data);
    }

    #[test]
    fn test_write_merged_no_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "test--400-normal--2026-01-01.ttf";
        let first = fake_ttf(b"first");
        let second = fake_ttf(b"second");

        write_cached_font_if_absent(key, tmp.path(), &first).unwrap();
        write_cached_font_if_absent(key, tmp.path(), &second).unwrap();

        let read_back = read_cached_font(key, tmp.path()).unwrap().unwrap();
        assert_eq!(read_back, first);
    }

    #[test]
    fn test_read_cached_font_bad_magic_bytes_treated_as_miss_and_cleaned() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "test--400-normal--2026-01-01.ttf";
        let path = tmp.path().join(key);

        std::fs::write(&path, b"not a font file").unwrap();
        assert!(read_cached_font(key, tmp.path()).unwrap().is_none());
        assert!(!path.exists());
    }

    #[test]
    fn test_read_cached_font_corrupt_directory_treated_as_miss_and_cleaned() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "test--400-normal--2026-01-01.ttf";
        let path = tmp.path().join(key);

        std::fs::create_dir_all(&path).unwrap();
        assert!(read_cached_font(key, tmp.path()).unwrap().is_none());
        assert!(!path.exists());
    }

    #[test]
    fn test_has_ttf_magic_bytes() {
        assert!(has_ttf_magic_bytes(&[0, 1, 0, 0]));
        assert!(has_ttf_magic_bytes(b"OTTO"));
        assert!(has_ttf_magic_bytes(b"ttcf"));
        assert!(has_ttf_magic_bytes(&[0, 1, 0, 0, 0xFF, 0xFF]));
        assert!(!has_ttf_magic_bytes(b""));
        assert!(!has_ttf_magic_bytes(b"PK\x03\x04"));
        assert!(!has_ttf_magic_bytes(b"\x00\x00\x00"));
    }

    #[test]
    fn test_read_metadata_corrupt_file_treated_as_miss_and_cleaned() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("roboto.json");
        std::fs::write(&path, b"{bad json").unwrap();

        assert!(read_metadata("roboto", tmp.path()).is_none());
        assert!(!path.exists());
    }

    #[test]
    fn test_check_or_init_cache_format_fresh() {
        let tmp = tempfile::tempdir().unwrap();
        check_or_init_cache_format(tmp.path()).unwrap();

        let meta_path = tmp.path().join("cache-meta.json");
        assert!(meta_path.exists());
        let meta: CacheMeta = serde_json::from_slice(&std::fs::read(&meta_path).unwrap()).unwrap();
        assert_eq!(meta.format_version, CACHE_FORMAT_VERSION);
        assert!(tmp.path().join("metadata").is_dir());
        assert!(tmp.path().join("fonts").is_dir());
    }

    #[test]
    fn test_check_or_init_cache_format_wipes_legacy_blobs() {
        let tmp = tempfile::tempdir().unwrap();
        let blobs_dir = tmp.path().join("blobs");
        std::fs::create_dir_all(&blobs_dir).unwrap();
        std::fs::write(blobs_dir.join("old-font.ttf"), b"old data").unwrap();

        check_or_init_cache_format(tmp.path()).unwrap();

        assert!(!blobs_dir.exists());
        assert!(tmp.path().join("fonts").is_dir());
    }

    #[test]
    fn test_check_or_init_cache_format_wipes_on_version_mismatch() {
        let tmp = tempfile::tempdir().unwrap();

        // Write a cache-meta.json with a different version.
        let meta_path = tmp.path().join("cache-meta.json");
        let old_meta = CacheMeta {
            format_version: CACHE_FORMAT_VERSION + 1,
        };
        std::fs::write(&meta_path, serde_json::to_vec(&old_meta).unwrap()).unwrap();

        // Create fonts/ with a file that should be wiped.
        let fonts_dir = tmp.path().join("fonts");
        std::fs::create_dir_all(&fonts_dir).unwrap();
        std::fs::write(fonts_dir.join("stale.ttf"), b"stale").unwrap();

        check_or_init_cache_format(tmp.path()).unwrap();

        // Stale file should be gone, fresh cache-meta.json written.
        assert!(!fonts_dir.join("stale.ttf").exists());
        let meta: CacheMeta = serde_json::from_slice(&std::fs::read(&meta_path).unwrap()).unwrap();
        assert_eq!(meta.format_version, CACHE_FORMAT_VERSION);
    }

    #[test]
    fn test_check_or_init_cache_format_noop_when_current() {
        let tmp = tempfile::tempdir().unwrap();

        // First init.
        check_or_init_cache_format(tmp.path()).unwrap();

        // Write a file into fonts/ that should survive a second check.
        let marker = tmp.path().join("fonts").join("keep-me.ttf");
        std::fs::write(&marker, fake_ttf(b"data")).unwrap();

        // Second check — should be a no-op.
        check_or_init_cache_format(tmp.path()).unwrap();
        assert!(marker.exists());
    }
}

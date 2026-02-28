use crate::error::FontsourceFontdbError;
use crate::types::FontsourceFont;
use filetime::FileTime;
use fs4::fs_std::FileExt;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Metadata cache
// ---------------------------------------------------------------------------

pub(crate) fn read_metadata(font_id: &str, metadata_cache_dir: &Path) -> Option<FontsourceFont> {
    let path = metadata_cache_dir.join(format!("{font_id}.json"));
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn write_metadata_if_absent(
    font_id: &str,
    metadata_cache_dir: &Path,
    metadata: &FontsourceFont,
) -> Result<(), FontsourceFontdbError> {
    let path = metadata_cache_dir.join(format!("{font_id}.json"));
    if path.exists() {
        return Ok(());
    }
    let data = serde_json::to_vec_pretty(metadata)?;
    atomic_write_bytes(&path, &data)
}

// ---------------------------------------------------------------------------
// Blob cache
// ---------------------------------------------------------------------------

/// Derive a human-readable blob key from font_id and the resolved filename.
///
/// The resolver produces filenames like `{subset}-{weight}-{style}.ttf`.
/// Combined with `font_id`, the key becomes `{font_id}--{subset}-{weight}-{style}.ttf`.
pub(crate) fn blob_key(font_id: &str, filename: &str) -> String {
    let stem = filename.strip_suffix(".ttf").unwrap_or(filename);
    format!("{font_id}--{stem}.ttf")
}

pub(crate) fn blob_path(key: &str, blob_cache_dir: &Path) -> PathBuf {
    blob_cache_dir.join(key)
}

pub(crate) fn read_blob(
    key: &str,
    blob_cache_dir: &Path,
) -> Result<Option<Vec<u8>>, FontsourceFontdbError> {
    let path = blob_path(key, blob_cache_dir);
    match std::fs::read(&path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => {
            // Treat any read failure as a miss — delete the corrupt blob.
            let _ = std::fs::remove_file(&path);
            Ok(None)
        }
    }
}

pub(crate) fn write_blob_if_absent(
    key: &str,
    blob_cache_dir: &Path,
    bytes: &[u8],
) -> Result<(), FontsourceFontdbError> {
    let path = blob_path(key, blob_cache_dir);
    atomic_write_bytes(&path, bytes)
}

pub(crate) fn touch_blob(key: &str, blob_cache_dir: &Path) -> Result<(), FontsourceFontdbError> {
    let path = blob_path(key, blob_cache_dir);
    let _ = filetime::set_file_mtime(&path, FileTime::now());
    Ok(())
}

pub(crate) fn calculate_blob_cache_size_bytes(
    blob_cache_dir: &Path,
) -> Result<u64, FontsourceFontdbError> {
    if !blob_cache_dir.exists() {
        return Ok(0);
    }

    let mut total = 0u64;
    for entry in std::fs::read_dir(blob_cache_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("ttf"))
                .unwrap_or(false)
        {
            total = total.saturating_add(entry.metadata()?.len());
        }
    }

    Ok(total)
}

pub(crate) fn evict_blob_lru_until_size(
    blob_cache_dir: &Path,
    target_bytes: u64,
    exempt_keys: &HashSet<String>,
) -> Result<(), FontsourceFontdbError> {
    with_exclusive_cache_lock(blob_cache_dir, || {
        let mut entries: Vec<(String, PathBuf, u64, std::time::SystemTime)> = Vec::new();
        let mut total_size = 0u64;

        if !blob_cache_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(blob_cache_dir)? {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            if !path.is_file() {
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

            // Tolerate ENOENT from concurrent deletion.
            if std::fs::remove_file(&path).is_ok() {
                total_size = total_size.saturating_sub(size);
            }
        }

        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Shared primitives (kept from original)
// ---------------------------------------------------------------------------

pub(crate) fn atomic_write_bytes(dst: &Path, bytes: &[u8]) -> Result<(), FontsourceFontdbError> {
    let parent = dst.parent().ok_or_else(|| {
        FontsourceFontdbError::Internal(format!("No parent directory for {}", dst.display()))
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

fn with_exclusive_cache_lock<F, R>(cache_dir: &Path, f: F) -> Result<R, FontsourceFontdbError>
where
    F: FnOnce() -> Result<R, FontsourceFontdbError>,
{
    std::fs::create_dir_all(cache_dir)?;
    let lock_path = cache_dir.join(".cache-lock");
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    lock_file.lock_exclusive()?;

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
    fn test_blob_key_deterministic() {
        let key1 = blob_key("roboto", "latin-400-normal.ttf");
        let key2 = blob_key("roboto", "latin-400-normal.ttf");
        assert_eq!(key1, key2);
        assert_eq!(key1, "roboto--latin-400-normal.ttf");
    }

    #[test]
    fn test_read_write_blob_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "roboto--latin-400-normal.ttf";
        let data = b"fake ttf data";

        assert!(read_blob(key, tmp.path()).unwrap().is_none());

        write_blob_if_absent(key, tmp.path(), data).unwrap();

        let read_back = read_blob(key, tmp.path()).unwrap().unwrap();
        assert_eq!(read_back, data);
    }

    #[test]
    fn test_write_blob_no_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        let key = "roboto--latin-400-normal.ttf";

        write_blob_if_absent(key, tmp.path(), b"first").unwrap();
        write_blob_if_absent(key, tmp.path(), b"second").unwrap();

        let read_back = read_blob(key, tmp.path()).unwrap().unwrap();
        assert_eq!(read_back, b"first");
    }
}

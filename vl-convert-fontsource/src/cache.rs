use crate::error::FontsourceError;
use crate::types::FontsourceFont;
use filetime::FileTime;
use fs4::fs_std::FileExt;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const BLOB_EXTENSION: &str = "blob";

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

/// Stable blob key derived from URL bytes as lowercase SHA-256 hex.
pub(crate) fn blob_key(url: &str) -> String {
    let digest = Sha256::digest(url.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn blob_path_from_key(key: &str, blob_cache_dir: &Path) -> PathBuf {
    blob_cache_dir.join(format!("{key}.{BLOB_EXTENSION}"))
}

pub(crate) fn read_blob(
    url: &str,
    blob_cache_dir: &Path,
) -> Result<Option<Vec<u8>>, FontsourceError> {
    let path = blob_path_from_key(&blob_key(url), blob_cache_dir);
    match std::fs::read(&path) {
        Ok(bytes) => {
            if has_ttf_magic_bytes(&bytes) {
                Ok(Some(bytes))
            } else {
                // Corrupt or truncated — delete and treat as a miss.
                remove_path_if_present(&path);
                Ok(None)
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => {
            // Treat any read failure as a miss — delete the corrupt blob.
            remove_path_if_present(&path);
            Ok(None)
        }
    }
}

/// Check whether `bytes` starts with a recognised font magic number.
fn has_ttf_magic_bytes(bytes: &[u8]) -> bool {
    bytes.len() >= 4
        && (bytes[..4] == [0, 1, 0, 0]  // TrueType
            || bytes[..4] == *b"OTTO"    // OpenType/CFF
            || bytes[..4] == *b"ttcf") // TrueType Collection
}

pub(crate) fn write_blob_if_absent(
    url: &str,
    blob_cache_dir: &Path,
    bytes: &[u8],
) -> Result<(), FontsourceError> {
    let path = blob_path_from_key(&blob_key(url), blob_cache_dir);
    // If a corrupt path exists as a non-file (e.g. directory), clear it and
    // treat this write as filling a miss.
    if let Ok(meta) = std::fs::symlink_metadata(&path) {
        if !meta.is_file() {
            remove_path_if_present(&path);
        }
    }
    atomic_write_bytes(&path, bytes)
}

pub(crate) fn touch_blob(url: &str, blob_cache_dir: &Path) -> Result<(), FontsourceError> {
    let path = blob_path_from_key(&blob_key(url), blob_cache_dir);
    match filetime::set_file_mtime(&path, FileTime::now()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn is_blob_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case(BLOB_EXTENSION))
            .unwrap_or(false)
}

pub(crate) fn calculate_blob_cache_size_bytes(
    blob_cache_dir: &Path,
) -> Result<u64, FontsourceError> {
    if !blob_cache_dir.exists() {
        return Ok(0);
    }

    let mut total = 0u64;
    for entry in std::fs::read_dir(blob_cache_dir)? {
        let entry = match entry {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        let path = entry.path();
        if is_blob_file(&path) {
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

pub(crate) fn evict_blob_lru_until_size(
    blob_cache_dir: &Path,
    target_bytes: u64,
    exempt_keys: &HashSet<String>,
) -> Result<(), FontsourceError> {
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
            if !is_blob_file(&path) {
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
            if std::fs::remove_file(&path).is_ok() || !path.exists() {
                total_size = total_size.saturating_sub(size);
            }
        }

        Ok(())
    })
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

fn with_exclusive_cache_lock<F, R>(cache_dir: &Path, f: F) -> Result<R, FontsourceError>
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
        let key1 = blob_key("https://cdn.example/fonts/latin-400-normal.ttf");
        let key2 = blob_key("https://cdn.example/fonts/latin-400-normal.ttf");
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 64);
        assert!(key1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// Fake TTF bytes with a valid TrueType magic header.
    fn fake_ttf(payload: &[u8]) -> Vec<u8> {
        let mut v = vec![0x00, 0x01, 0x00, 0x00];
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn test_read_write_blob_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let url = "https://cdn.example/fonts/latin-400-normal.ttf";
        let data = fake_ttf(b"font data");

        assert!(read_blob(url, tmp.path()).unwrap().is_none());

        write_blob_if_absent(url, tmp.path(), &data).unwrap();

        let read_back = read_blob(url, tmp.path()).unwrap().unwrap();
        assert_eq!(read_back, data);
    }

    #[test]
    fn test_write_blob_no_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        let url = "https://cdn.example/fonts/latin-400-normal.ttf";
        let first = fake_ttf(b"first");
        let second = fake_ttf(b"second");

        write_blob_if_absent(url, tmp.path(), &first).unwrap();
        write_blob_if_absent(url, tmp.path(), &second).unwrap();

        let read_back = read_blob(url, tmp.path()).unwrap().unwrap();
        assert_eq!(read_back, first);
    }

    #[test]
    fn test_read_blob_bad_magic_bytes_treated_as_miss_and_cleaned() {
        let tmp = tempfile::tempdir().unwrap();
        let url = "https://cdn.example/fonts/latin-400-normal.ttf";
        let key = blob_key(url);
        let path = tmp.path().join(format!("{key}.{BLOB_EXTENSION}"));

        std::fs::write(&path, b"not a font file").unwrap();
        assert!(read_blob(url, tmp.path()).unwrap().is_none());
        assert!(!path.exists());
    }

    #[test]
    fn test_read_blob_corrupt_directory_treated_as_miss_and_cleaned() {
        let tmp = tempfile::tempdir().unwrap();
        let url = "https://cdn.example/fonts/latin-400-normal.ttf";
        let key = blob_key(url);
        let path = tmp.path().join(format!("{key}.{BLOB_EXTENSION}"));

        std::fs::create_dir_all(&path).unwrap();
        assert!(read_blob(url, tmp.path()).unwrap().is_none());
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
}

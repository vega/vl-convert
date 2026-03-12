use crate::error::GoogleFontsError;
use filetime::FileTime;
use fs4::fs_std::FileExt;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Human-readable font key derived from a gstatic URL.
///
/// Given `https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1MmgVxFIzIFKw.ttf`,
/// returns `roboto--KFOmCnqEu92Fr1MmgVxFIzIFKw.ttf`.
///
/// Flat key format (no path separators) ensures compatibility with
/// non-recursive eviction/size walkers.
pub(crate) fn font_key(url: &str) -> String {
    // Try to extract from gstatic URL pattern: /s/{font-name}/{version}/{hash}.ttf
    if let Some(idx) = url.find("/s/") {
        let after_s = &url[idx + 3..];
        let parts: Vec<&str> = after_s.splitn(3, '/').collect();
        if parts.len() == 3 {
            let font_name = parts[0];
            // parts[2] is "{hash}.ttf"
            let hash_file = parts[2];
            return format!("{font_name}--{hash_file}");
        }
    }
    // Fallback: sanitize the full URL into a flat key
    url.replace("https://", "")
        .replace("http://", "")
        .replace('/', "--")
}

/// Join `font_key(url)` onto `font_cache_dir`, then verify the resolved path
/// is still inside the cache directory. Returns an error on path traversal.
fn font_path(url: &str, font_cache_dir: &Path) -> Result<PathBuf, GoogleFontsError> {
    let key = font_key(url);
    let joined = font_cache_dir.join(&key);
    // Normalize away any `.` / `..` components without requiring the path to
    // exist on disk (unlike `canonicalize`).
    let resolved = normalize_path(&joined);
    let cache_dir_resolved = normalize_path(font_cache_dir);
    if !resolved.starts_with(&cache_dir_resolved) {
        return Err(GoogleFontsError::Internal(format!(
            "Font key {key:?} escapes cache directory"
        )));
    }
    Ok(resolved)
}

/// Pure-logical path normalization: resolves `.` and `..` without touching
/// the filesystem, preserving the absolute/relative nature of the input.
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

pub(crate) fn read_font(
    url: &str,
    font_cache_dir: &Path,
) -> Result<Option<Vec<u8>>, GoogleFontsError> {
    let path = font_path(url, font_cache_dir)?;

    // Reject symlinks and other non-regular entries without following them.
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if !meta.is_file() => {
            remove_path_if_present(&path);
            return Ok(None);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            remove_path_if_present(&path);
            return Ok(None);
        }
        Ok(_) => {} // regular file — proceed
    }

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

fn has_ttf_magic_bytes(bytes: &[u8]) -> bool {
    bytes.len() >= 4
        && (bytes[..4] == [0, 1, 0, 0]  // TrueType
            || bytes[..4] == *b"OTTO"    // OpenType/CFF
            || bytes[..4] == *b"ttcf") // TrueType Collection
}

pub(crate) fn write_font_if_absent(
    url: &str,
    font_cache_dir: &Path,
    bytes: &[u8],
) -> Result<(), GoogleFontsError> {
    let path = font_path(url, font_cache_dir)?;
    if let Ok(meta) = std::fs::symlink_metadata(&path) {
        if meta.is_file() {
            // Already present as a regular file — nothing to do.
            return Ok(());
        }
        // Non-file entry (symlink, directory, etc.) — remove before writing.
        remove_path_if_present(&path);
    }
    atomic_write_bytes(&path, bytes)
}

pub(crate) fn touch_font(url: &str, font_cache_dir: &Path) -> Result<(), GoogleFontsError> {
    let path = font_path(url, font_cache_dir)?;

    // Only touch regular files; skip symlinks and other non-file entries.
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() => {}
        _ => return Ok(()),
    }

    match filetime::set_file_mtime(&path, FileTime::now()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn is_font_file(path: &Path) -> bool {
    // Use symlink_metadata so symlinks are not followed.
    std::fs::symlink_metadata(path)
        .map(|m| m.is_file())
        .unwrap_or(false)
        && path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("ttf"))
            .unwrap_or(false)
}

pub(crate) fn calculate_font_cache_size_bytes(
    font_cache_dir: &Path,
) -> Result<u64, GoogleFontsError> {
    if !font_cache_dir.exists() {
        return Ok(0);
    }

    let mut total = 0u64;
    for entry in std::fs::read_dir(font_cache_dir)? {
        let entry = match entry {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        let path = entry.path();
        if is_font_file(&path) {
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

pub(crate) fn evict_font_lru_until_size(
    font_cache_dir: &Path,
    target_bytes: u64,
    exempt_keys: &HashSet<String>,
) -> Result<(), GoogleFontsError> {
    with_exclusive_cache_lock(font_cache_dir, || {
        let mut entries: Vec<(String, PathBuf, u64, std::time::SystemTime)> = Vec::new();
        let mut total_size = 0u64;

        if !font_cache_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(font_cache_dir)? {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            if !is_font_file(&path) {
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

            // Tolerate ENOENT in case the file was deleted outside of vl-convert.
            if std::fs::remove_file(&path).is_ok() || !path.exists() {
                total_size = total_size.saturating_sub(size);
            }
        }

        Ok(())
    })
}

/// Read a cached CSS2 response for a font, returning `None` on miss.
pub(crate) fn read_css(font_id: &str, css_dir: &Path) -> Result<Option<String>, GoogleFontsError> {
    let path = css_dir.join(format!("{font_id}.css"));

    // Reject symlinks and non-regular files without following them.
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if !meta.is_file() => {
            remove_path_if_present(&path);
            return Ok(None);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            remove_path_if_present(&path);
            return Ok(None);
        }
        Ok(_) => {}
    }

    match std::fs::read_to_string(&path) {
        Ok(s) if s.contains("@font-face") => Ok(Some(s)),
        Ok(_) => {
            remove_path_if_present(&path);
            Ok(None)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Write a CSS2 response to the cache (no-op if already present).
pub(crate) fn write_css_if_absent(
    font_id: &str,
    css_dir: &Path,
    css: &str,
) -> Result<(), GoogleFontsError> {
    let path = css_dir.join(format!("{font_id}.css"));
    if path.exists() {
        return Ok(());
    }
    atomic_write_bytes(&path, css.as_bytes())
}

/// Remove a cached CSS2 response so the next fetch re-downloads from the network.
pub(crate) fn invalidate_css(font_id: &str, css_dir: &Path) {
    let path = css_dir.join(format!("{font_id}.css"));
    remove_path_if_present(&path);
}

pub(crate) fn atomic_write_bytes(dst: &Path, bytes: &[u8]) -> Result<(), GoogleFontsError> {
    let parent = dst.parent().unwrap_or(dst);
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

fn with_exclusive_cache_lock<F, R>(cache_dir: &Path, f: F) -> Result<R, GoogleFontsError>
where
    F: FnOnce() -> Result<R, GoogleFontsError>,
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
    fn test_font_key_gstatic_url() {
        let key = font_key("https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1MmgVxFIzIFKw.ttf");
        assert_eq!(key, "roboto--KFOmCnqEu92Fr1MmgVxFIzIFKw.ttf");
    }

    #[test]
    fn test_font_key_deterministic() {
        let url = "https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1MmgVxFIzIFKw.ttf";
        assert_eq!(font_key(url), font_key(url));
    }

    #[test]
    fn test_font_key_flat_no_path_separators() {
        let key = font_key("https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1MmgVxFIzIFKw.ttf");
        assert!(!key.contains('/'));
        assert!(!key.contains('\\'));
    }

    #[test]
    fn test_font_key_fallback_for_non_gstatic_url() {
        let key = font_key("https://example.com/fonts/test.ttf");
        assert!(!key.contains('/'));
        assert!(key.contains("example.com"));
    }

    #[test]
    fn test_font_path_rejects_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let result = font_path(
            "https://fonts.gstatic.com/s/roboto/v30/../../../../etc/passwd",
            tmp.path(),
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("escapes cache directory"), "{err}");
    }

    #[test]
    fn test_font_path_accepts_normal_url() {
        let tmp = tempfile::tempdir().unwrap();
        let result = font_path(
            "https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1MmgVxFIzIFKw.ttf",
            tmp.path(),
        );
        assert!(result.is_ok());
        assert!(result.unwrap().starts_with(tmp.path()));
    }

    /// Fake TTF bytes with a valid TrueType magic header.
    fn fake_ttf(payload: &[u8]) -> Vec<u8> {
        let mut v = vec![0x00, 0x01, 0x00, 0x00];
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn test_read_write_font_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let url = "https://fonts.gstatic.com/s/testfont/v1/abc123.ttf";
        let data = fake_ttf(b"font data");

        assert!(read_font(url, tmp.path()).unwrap().is_none());

        write_font_if_absent(url, tmp.path(), &data).unwrap();

        let read_back = read_font(url, tmp.path()).unwrap().unwrap();
        assert_eq!(read_back, data);
    }

    #[test]
    fn test_write_font_no_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        let url = "https://fonts.gstatic.com/s/testfont/v1/abc123.ttf";
        let first = fake_ttf(b"first");
        let second = fake_ttf(b"second");

        write_font_if_absent(url, tmp.path(), &first).unwrap();
        write_font_if_absent(url, tmp.path(), &second).unwrap();

        let read_back = read_font(url, tmp.path()).unwrap().unwrap();
        assert_eq!(read_back, first);
    }

    #[test]
    fn test_read_font_bad_magic_bytes_treated_as_miss_and_cleaned() {
        let tmp = tempfile::tempdir().unwrap();
        let url = "https://fonts.gstatic.com/s/testfont/v1/abc123.ttf";
        let path = tmp.path().join(font_key(url));

        std::fs::write(&path, b"not a font file").unwrap();
        assert!(read_font(url, tmp.path()).unwrap().is_none());
        assert!(!path.exists());
    }

    #[test]
    fn test_read_font_corrupt_directory_treated_as_miss_and_cleaned() {
        let tmp = tempfile::tempdir().unwrap();
        let url = "https://fonts.gstatic.com/s/testfont/v1/abc123.ttf";
        let path = tmp.path().join(font_key(url));

        std::fs::create_dir_all(&path).unwrap();
        assert!(read_font(url, tmp.path()).unwrap().is_none());
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_read_font_symlink_treated_as_miss_and_cleaned() {
        let tmp = tempfile::tempdir().unwrap();
        let url = "https://fonts.gstatic.com/s/testfont/v1/abc123.ttf";
        let font = tmp.path().join(font_key(url));

        // Create a symlink where the font file should be.
        let target = tmp.path().join("target.ttf");
        std::fs::write(&target, fake_ttf(b"payload")).unwrap();
        std::os::unix::fs::symlink(&target, &font).unwrap();
        assert!(font.exists()); // follows symlink

        // read_font must reject the symlink and remove it.
        let result = read_font(url, tmp.path()).unwrap();
        assert!(result.is_none());
        assert!(!font.symlink_metadata().is_ok()); // symlink removed
        assert!(target.exists()); // target untouched
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
}

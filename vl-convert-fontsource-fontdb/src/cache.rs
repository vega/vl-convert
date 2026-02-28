use crate::error::FontsourceFontdbError;
use crate::types::{FontStyle, FontsourceFont, VariantRequest};
use filetime::FileTime;
use fs4::fs_std::FileExt;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(crate) const METADATA_FILENAME: &str = "fontsource-metadata.json";

#[derive(Debug, Clone, Copy)]
struct ParsedTtfFile {
    weight: u16,
    style: FontStyle,
}

fn parse_cached_filename(filename: &str) -> Option<ParsedTtfFile> {
    let stem = filename.strip_suffix(".ttf")?;
    let parts: Vec<&str> = stem.rsplitn(3, '-').collect();
    if parts.len() < 3 {
        return None;
    }

    let style = match parts[0] {
        "normal" => FontStyle::Normal,
        "italic" => FontStyle::Italic,
        _ => return None,
    };

    let weight = parts[1].parse().ok()?;
    Some(ParsedTtfFile { weight, style })
}

pub(crate) fn metadata_path(font_dir: &Path) -> PathBuf {
    font_dir.join(METADATA_FILENAME)
}

pub(crate) fn read_local_metadata(font_dir: &Path) -> Option<FontsourceFont> {
    let metadata_path = metadata_path(font_dir);
    let bytes = std::fs::read(&metadata_path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn write_local_metadata(
    font_dir: &Path,
    metadata: &FontsourceFont,
) -> Result<(), FontsourceFontdbError> {
    let data = serde_json::to_vec_pretty(metadata)?;
    let path = metadata_path(font_dir);
    atomic_write_bytes(&path, &data)
}

pub(crate) fn has_any_ttf_files(font_dir: &Path) -> bool {
    let entries = match std::fs::read_dir(font_dir) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    for entry in entries.flatten() {
        if entry
            .path()
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("ttf"))
            .unwrap_or(false)
        {
            return true;
        }
    }

    false
}

pub(crate) fn has_requested_variants(
    font_dir: &Path,
    variants: &[VariantRequest],
) -> Result<bool, FontsourceFontdbError> {
    let entries = std::fs::read_dir(font_dir)?;
    let mut found: HashSet<(u16, FontStyle)> = HashSet::new();

    for entry in entries.flatten() {
        let filename = match entry.file_name().to_str() {
            Some(name) => name.to_string(),
            None => continue,
        };

        if let Some(parsed) = parse_cached_filename(&filename) {
            found.insert((parsed.weight, parsed.style));
            if variants
                .iter()
                .all(|v| found.contains(&(v.weight, v.style)))
            {
                return Ok(true);
            }
        }
    }

    Ok(variants
        .iter()
        .all(|v| found.contains(&(v.weight, v.style))))
}

pub(crate) fn list_variant_ttf_paths(
    font_dir: &Path,
    variants: &[VariantRequest],
) -> Result<Vec<PathBuf>, FontsourceFontdbError> {
    let targets: HashSet<(u16, FontStyle)> = variants.iter().map(|v| (v.weight, v.style)).collect();

    let mut paths = Vec::new();
    let entries = std::fs::read_dir(font_dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        let filename = match path.file_name().and_then(|f| f.to_str()) {
            Some(name) => name,
            None => continue,
        };

        if let Some(parsed) = parse_cached_filename(filename) {
            if targets.contains(&(parsed.weight, parsed.style)) {
                paths.push(path);
            }
        }
    }

    paths.sort();
    Ok(paths)
}

pub(crate) fn has_all_downloadable_ttf_files(font_dir: &Path, metadata: &FontsourceFont) -> bool {
    for (weight_key, styles) in &metadata.variants {
        for (style_key, subsets) in styles {
            for (subset, urls) in subsets {
                if urls.url.ttf.is_some() {
                    let filename = format!("{}-{}-{}.ttf", subset, weight_key, style_key);
                    if !font_dir.join(filename).exists() {
                        return false;
                    }
                }
            }
        }
    }
    true
}

pub(crate) fn all_filenames_exist(font_dir: &Path, filenames: &[String]) -> bool {
    filenames.iter().all(|name| font_dir.join(name).exists())
}

pub(crate) fn touch_dir_mtime(font_dir: &Path) {
    let _ = filetime::set_file_mtime(font_dir, FileTime::now());
}

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

pub(crate) fn calculate_cache_size_bytes(cache_dir: &Path) -> Result<u64, FontsourceFontdbError> {
    let mut total = 0u64;
    if !cache_dir.exists() {
        return Ok(0);
    }

    for entry in std::fs::read_dir(cache_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        for sub_entry in std::fs::read_dir(path)? {
            let sub_entry = sub_entry?;
            let sub_path = sub_entry.path();
            if sub_path.is_file() {
                total = total.saturating_add(sub_entry.metadata()?.len());
            }
        }
    }

    Ok(total)
}

pub(crate) fn evict_lru_until_size(
    cache_dir: &Path,
    target_bytes: u64,
    exempt_font_id: &str,
) -> Result<(), FontsourceFontdbError> {
    with_exclusive_cache_lock(cache_dir, || {
        let mut font_entries: Vec<(String, PathBuf, u64, std::time::SystemTime)> = Vec::new();
        let mut total_size = 0u64;

        if !cache_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let font_id = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            let mut dir_size = 0u64;
            for sub_entry in std::fs::read_dir(&path)? {
                let sub_entry = sub_entry?;
                if sub_entry.path().is_file() {
                    dir_size = dir_size.saturating_add(sub_entry.metadata()?.len());
                }
            }

            let mtime = path.metadata()?.modified().unwrap_or(std::time::UNIX_EPOCH);
            total_size = total_size.saturating_add(dir_size);
            font_entries.push((font_id, path, dir_size, mtime));
        }

        if total_size <= target_bytes {
            return Ok(());
        }

        font_entries.sort_by(|a, b| a.3.cmp(&b.3));

        for (font_id, path, size, _) in font_entries {
            if total_size <= target_bytes {
                break;
            }
            if font_id == exempt_font_id {
                continue;
            }

            if std::fs::remove_dir_all(&path).is_ok() {
                total_size = total_size.saturating_sub(size);
            }
        }

        Ok(())
    })
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
    fn test_has_requested_variants_ignores_tmp_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("latin-400-normal.ttf"), b"ok").unwrap();
        std::fs::write(tmp.path().join("latin-700-normal.ttf.12345.tmp"), b"tmp").unwrap();

        let requested = [
            VariantRequest {
                weight: 400,
                style: FontStyle::Normal,
            },
            VariantRequest {
                weight: 700,
                style: FontStyle::Normal,
            },
        ];

        let hit = has_requested_variants(tmp.path(), &requested).unwrap();
        assert!(!hit);
    }

    #[test]
    fn test_atomic_write_bytes_existing_file_preserved() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("font.ttf");

        std::fs::write(&path, b"first").unwrap();
        atomic_write_bytes(&path, b"second").unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes, b"first");
    }
}

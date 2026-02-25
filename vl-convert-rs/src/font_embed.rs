use base64::{engine::general_purpose::STANDARD, Engine};
use deno_core::anyhow::{self, anyhow};
use font_subset::FontReader;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::converter::MissingFontsPolicy;
use crate::extract::{parse_css_font_family, FontFamilyEntry, FontForHtml};
use crate::text::FONTSOURCE_CACHE;

// ---------------------------------------------------------------------------
// Types for JS → Rust text extraction results
// ---------------------------------------------------------------------------

/// A single entry from the JS `vegaToTextByFont()` function.
#[derive(Debug, Clone, Deserialize)]
pub struct TextByFontEntry {
    /// CSS font-family string (e.g. "Roboto, sans-serif")
    pub font: String,
    /// Normalized weight (e.g. "400", "700")
    pub weight: String,
    /// Normalized style ("normal" or "italic")
    pub style: String,
    /// Unique characters used at this font/weight/style
    pub chars: String,
}

/// A (family, weight, style) key for font embedding.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct FontKey {
    pub family: String,
    pub weight: String,
    pub style: String,
}

// ---------------------------------------------------------------------------
// Aggregation: group characters by (first_named_family, weight, style)
// ---------------------------------------------------------------------------

/// Aggregate structured JS entries by (first_named_family, weight, style).
///
/// Only includes entries where the FIRST font in the family list is a Named
/// font. If the first font is a generic family (sans-serif, etc.) or the
/// list is empty, the entry is skipped.
pub fn aggregate_chars_by_font_key(
    entries: &[TextByFontEntry],
) -> HashMap<FontKey, BTreeSet<char>> {
    let mut result: HashMap<FontKey, BTreeSet<char>> = HashMap::new();
    for entry in entries {
        let families = parse_css_font_family(&entry.font);
        match families.first() {
            Some(FontFamilyEntry::Named(name)) => {
                let font_key = FontKey {
                    family: name.clone(),
                    weight: entry.weight.clone(),
                    style: entry.style.clone(),
                };
                let chars = result.entry(font_key).or_default();
                for ch in entry.chars.chars() {
                    chars.insert(ch);
                }
            }
            _ => {
                // First font is generic or list is empty — skip
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// TTF file indexing from fontsource cache
// ---------------------------------------------------------------------------

/// Parse weight and style from a fontsource TTF filename.
///
/// Fontsource filenames follow the pattern: `{subset}-{weight}-{style}.ttf`
/// Examples: "latin-400-normal.ttf", "latin-ext-700-italic.ttf"
///
/// Returns `(weight, style)` if the filename matches the pattern.
fn parse_weight_style_from_filename(path: &Path) -> Option<(String, String)> {
    let stem = path.file_stem()?.to_str()?;
    // Split from the right: last part is style, second-to-last is weight
    let parts: Vec<&str> = stem.rsplitn(3, '-').collect();
    if parts.len() < 3 {
        return None;
    }
    let style = parts[0]; // e.g. "normal" or "italic"
    let weight = parts[1]; // e.g. "400", "700"

    // Validate that weight is numeric
    if weight.parse::<u32>().is_err() {
        return None;
    }
    // Validate style
    if style != "normal" && style != "italic" {
        return None;
    }

    Some((weight.to_string(), style.to_string()))
}

/// Extract the Unicode subset name from a fontsource TTF filename.
///
/// For `latin-ext-400-normal.ttf` returns `"latin-ext"`.
fn extract_subset_name(path: &Path) -> &str {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    // rsplitn(3, '-') → [style, weight, subset_prefix]
    let parts: Vec<&str> = stem.rsplitn(3, '-').collect();
    if parts.len() >= 3 {
        parts[2]
    } else {
        ""
    }
}

/// Priority rank for Unicode subset names. Lower = processed first.
fn subset_priority(path: &Path) -> u8 {
    match extract_subset_name(path) {
        "latin" => 0,
        "latin-ext" => 1,
        "vietnamese" => 2,
        "greek" => 3,
        "greek-ext" => 4,
        "cyrillic" => 5,
        "cyrillic-ext" => 6,
        "math" => 7,
        "symbols" => 8,
        _ => 100,
    }
}

/// Index all TTF files in a fontsource cache directory by (weight, style).
///
/// Multiple subset files (latin, latin-ext, cyrillic, etc.) for the same
/// weight/style are collected together.
fn index_ttf_files(cache_dir: &Path) -> Result<HashMap<(String, String), Vec<PathBuf>>, anyhow::Error> {
    let mut index: HashMap<(String, String), Vec<PathBuf>> = HashMap::new();

    let entries = match std::fs::read_dir(cache_dir) {
        Ok(entries) => entries,
        Err(e) => {
            return Err(anyhow!(
                "Failed to read fontsource cache directory {}: {}",
                cache_dir.display(),
                e
            ));
        }
    };

    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("ttf") {
            if let Some((weight, style)) = parse_weight_style_from_filename(&path) {
                index.entry((weight, style)).or_default().push(path);
            }
        }
    }
    Ok(index)
}

// ---------------------------------------------------------------------------
// Font subsetting + CSS generation
// ---------------------------------------------------------------------------

/// Generate `@font-face` CSS blocks with subsetted WOFF2 fonts.
///
/// For each (family, weight, style) in `chars_by_font_key` that matches a
/// font in `html_fonts`, locates the TTF files in the fontsource cache,
/// subsets them to only the required characters, encodes as WOFF2, and
/// produces base64-encoded `@font-face` CSS blocks.
pub fn generate_font_face_css(
    chars_by_font_key: &HashMap<FontKey, BTreeSet<char>>,
    html_fonts: &[FontForHtml],
    mode: &MissingFontsPolicy,
) -> Result<String, anyhow::Error> {
    let mut css_blocks = Vec::new();

    for font_info in html_fonts {
        let cache_dir = FONTSOURCE_CACHE.font_dir(&font_info.font_id);
        let ttf_index = match index_ttf_files(&cache_dir) {
            Ok(idx) => idx,
            Err(e) => {
                match mode {
                    MissingFontsPolicy::Error => return Err(e),
                    _ => {
                        log::warn!(
                            "font_embed: skipping font '{}': {}",
                            font_info.family,
                            e
                        );
                        continue;
                    }
                }
            }
        };

        for (font_key, chars) in chars_by_font_key {
            if font_key.family != font_info.family || chars.is_empty() {
                continue;
            }

            let ws_key = (font_key.weight.clone(), font_key.style.clone());
            let Some(ttf_paths) = ttf_index.get(&ws_key) else {
                match mode {
                    MissingFontsPolicy::Error => {
                        return Err(anyhow!(
                            "No TTF found for {} weight={} style={}",
                            font_key.family,
                            font_key.weight,
                            font_key.style
                        ));
                    }
                    _ => {
                        log::warn!(
                            "font_embed: no TTF for {} weight={} style={}, skipping",
                            font_key.family,
                            font_key.weight,
                            font_key.style
                        );
                        continue;
                    }
                }
            };

            // Process subset files in priority order (latin first).
            // Track which characters are still needed; once all are
            // covered, skip remaining files.
            let mut ordered_paths = ttf_paths.clone();
            ordered_paths.sort_by_key(|p| subset_priority(p));

            let mut remaining = chars.clone();
            for ttf_path in &ordered_paths {
                if remaining.is_empty() {
                    break;
                }
                match subset_and_encode(ttf_path, &remaining) {
                    Ok(Some(artifact)) => {
                        css_blocks.push(format!(
                            "@font-face {{\n  font-family: \"{}\";\n  font-weight: {};\n  font-style: {};\n  src: url(data:font/woff2;base64,{}) format(\"woff2\");\n}}",
                            font_info.family, font_key.weight, font_key.style, artifact.woff2_b64
                        ));
                        for ch in &artifact.covered_chars {
                            remaining.remove(ch);
                        }
                    }
                    Ok(None) => {
                        // TTF didn't cover any of the remaining characters
                    }
                    Err(e) => {
                        match mode {
                            MissingFontsPolicy::Error => {
                                return Err(anyhow!(
                                    "Failed to subset {}: {}",
                                    ttf_path.display(),
                                    e
                                ));
                            }
                            _ => {
                                log::warn!(
                                    "font_embed: failed to subset {}: {}, skipping",
                                    ttf_path.display(),
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(css_blocks.join("\n"))
}

struct SubsetArtifact {
    woff2_b64: String,
    covered_chars: BTreeSet<char>,
}

/// Subset a single TTF file to the given characters and return the
/// base64-encoded WOFF2 data plus the set of characters actually covered.
/// Returns `Ok(None)` if the font doesn't cover any of the requested characters.
fn subset_and_encode(
    ttf_path: &Path,
    chars: &BTreeSet<char>,
) -> Result<Option<SubsetArtifact>, anyhow::Error> {
    let ttf_data = std::fs::read(ttf_path)?;
    let reader = FontReader::new(&ttf_data)?;
    let font = reader.read()?;

    // Cheap CMAP pre-filter: only request chars this font actually has
    let candidate: BTreeSet<char> = chars
        .iter()
        .copied()
        .filter(|ch| font.contains_char(*ch))
        .collect();
    if candidate.is_empty() {
        return Ok(None);
    }

    let subset = font.subset(&candidate)?;
    let woff2_bytes = subset.to_woff2();

    if woff2_bytes.is_empty() {
        return Ok(None);
    }

    Ok(Some(SubsetArtifact {
        woff2_b64: STANDARD.encode(&woff2_bytes),
        covered_chars: candidate,
    }))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // parse_weight_style_from_filename tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_weight_style_latin() {
        let path = Path::new("/tmp/fontsource/roboto/latin-400-normal.ttf");
        assert_eq!(
            parse_weight_style_from_filename(path),
            Some(("400".to_string(), "normal".to_string()))
        );
    }

    #[test]
    fn test_parse_weight_style_latin_ext_italic() {
        let path = Path::new("/tmp/fontsource/roboto/latin-ext-700-italic.ttf");
        assert_eq!(
            parse_weight_style_from_filename(path),
            Some(("700".to_string(), "italic".to_string()))
        );
    }

    #[test]
    fn test_parse_weight_style_cyrillic() {
        let path = Path::new("/tmp/fontsource/roboto/cyrillic-ext-300-normal.ttf");
        assert_eq!(
            parse_weight_style_from_filename(path),
            Some(("300".to_string(), "normal".to_string()))
        );
    }

    #[test]
    fn test_parse_weight_style_invalid_no_style() {
        let path = Path::new("/tmp/fontsource/roboto/latin-400.ttf");
        // Only 2 parts after rsplitn(3, '-'), not enough
        assert_eq!(parse_weight_style_from_filename(path), None);
    }

    #[test]
    fn test_parse_weight_style_non_numeric_weight() {
        let path = Path::new("/tmp/fontsource/roboto/latin-bold-normal.ttf");
        assert_eq!(parse_weight_style_from_filename(path), None);
    }

    #[test]
    fn test_parse_weight_style_invalid_style() {
        let path = Path::new("/tmp/fontsource/roboto/latin-400-oblique.ttf");
        assert_eq!(parse_weight_style_from_filename(path), None);
    }

    // -----------------------------------------------------------------------
    // aggregate_chars_by_font_key tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_aggregate_named_font() {
        let entries = vec![TextByFontEntry {
            font: "Roboto, sans-serif".to_string(),
            weight: "400".to_string(),
            style: "normal".to_string(),
            chars: "Hello".to_string(),
        }];
        let result = aggregate_chars_by_font_key(&entries);
        assert_eq!(result.len(), 1);
        let key = FontKey {
            family: "Roboto".to_string(),
            weight: "400".to_string(),
            style: "normal".to_string(),
        };
        let chars: BTreeSet<char> = "Hello".chars().collect();
        assert_eq!(result[&key], chars);
    }

    #[test]
    fn test_aggregate_generic_first_font_skipped() {
        let entries = vec![TextByFontEntry {
            font: "sans-serif".to_string(),
            weight: "400".to_string(),
            style: "normal".to_string(),
            chars: "Hello".to_string(),
        }];
        let result = aggregate_chars_by_font_key(&entries);
        assert!(result.is_empty());
    }

    #[test]
    fn test_aggregate_merges_same_key() {
        let entries = vec![
            TextByFontEntry {
                font: "Roboto".to_string(),
                weight: "400".to_string(),
                style: "normal".to_string(),
                chars: "Helo".to_string(),
            },
            TextByFontEntry {
                font: "Roboto, sans-serif".to_string(),
                weight: "400".to_string(),
                style: "normal".to_string(),
                chars: "World".to_string(),
            },
        ];
        let result = aggregate_chars_by_font_key(&entries);
        assert_eq!(result.len(), 1);
        let key = FontKey {
            family: "Roboto".to_string(),
            weight: "400".to_string(),
            style: "normal".to_string(),
        };
        let chars: BTreeSet<char> = "HeloWorld".chars().collect();
        assert_eq!(result[&key], chars);
    }

    #[test]
    fn test_aggregate_different_weights_separate() {
        let entries = vec![
            TextByFontEntry {
                font: "Roboto".to_string(),
                weight: "400".to_string(),
                style: "normal".to_string(),
                chars: "abc".to_string(),
            },
            TextByFontEntry {
                font: "Roboto".to_string(),
                weight: "700".to_string(),
                style: "normal".to_string(),
                chars: "xyz".to_string(),
            },
        ];
        let result = aggregate_chars_by_font_key(&entries);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_aggregate_empty_entries() {
        let entries: Vec<TextByFontEntry> = vec![];
        let result = aggregate_chars_by_font_key(&entries);
        assert!(result.is_empty());
    }
}

use base64::{engine::general_purpose::STANDARD, Engine};
use deno_core::anyhow::{self, anyhow};
use font_subset::FontReader;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use crate::converter::MissingFontsPolicy;
use crate::extract::{parse_css_font_family, FontFamilyEntry, FontForHtml, FontKey, FontSource};
use vl_convert_google_fonts::LoadedFontBatch;

/// Format a single `@font-face` CSS block from a WOFF2-encoded artifact.
fn format_font_face_block(family: &str, weight: &str, style: &str, woff2_b64: &str) -> String {
    format!(
        "@font-face {{\n  font-family: \"{}\";\n  font-weight: {};\n  font-style: {};\n  src: url(data:font/woff2;base64,{}) format(\"woff2\");\n}}",
        family, weight, style, woff2_b64
    )
}

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
// Variant extraction
// ---------------------------------------------------------------------------

/// Compute the set of (weight, style) variants used per font family.
///
/// Used to build accurate Google Fonts CSS2 API URLs that request only the
/// weight/style tuples the chart actually renders.
pub fn variants_by_family(
    chars_by_key: &HashMap<FontKey, BTreeSet<char>>,
) -> HashMap<String, BTreeSet<(String, String)>> {
    let mut result: HashMap<String, BTreeSet<(String, String)>> = HashMap::new();
    for key in chars_by_key.keys() {
        result
            .entry(key.family.clone())
            .or_default()
            .insert((key.weight.clone(), key.style.clone()));
    }
    result
}

// ---------------------------------------------------------------------------
// Font subsetting + CSS generation
// ---------------------------------------------------------------------------

/// Generate `@font-face` CSS blocks with subsetted WOFF2 fonts.
///
/// For each (family, weight, style) in `chars_by_font_key` that matches a
/// font in `html_fonts`, locates the font data (from Google Fonts loaded
/// batches or fontdb), subsets to only the required characters, encodes as
/// WOFF2, and produces base64-encoded `@font-face` CSS blocks.
pub fn generate_font_face_css(
    chars_by_font_key: &HashMap<FontKey, BTreeSet<char>>,
    html_fonts: &[FontForHtml],
    mode: &MissingFontsPolicy,
    fontdb: &fontdb::Database,
    loaded_batches: &[LoadedFontBatch],
) -> Result<Vec<String>, anyhow::Error> {
    let mut css_blocks = Vec::new();

    for font_info in html_fonts {
        match &font_info.source {
            FontSource::GoogleFonts { font_id } => {
                generate_google_fonts_css(
                    font_info,
                    font_id,
                    chars_by_font_key,
                    mode,
                    loaded_batches,
                    &mut css_blocks,
                )?;
            }
            FontSource::Local => {
                generate_local_font_css(
                    font_info,
                    chars_by_font_key,
                    mode,
                    fontdb,
                    &mut css_blocks,
                )?;
            }
        }
    }

    Ok(css_blocks)
}

/// Generate CSS for a Google Fonts font using in-memory loaded font data.
fn generate_google_fonts_css(
    font_info: &FontForHtml,
    font_id: &str,
    chars_by_font_key: &HashMap<FontKey, BTreeSet<char>>,
    mode: &MissingFontsPolicy,
    loaded_batches: &[LoadedFontBatch],
    css_blocks: &mut Vec<String>,
) -> Result<(), anyhow::Error> {
    // Find the matching batch for this font_id
    let batch = loaded_batches.iter().find(|b| b.font_id == font_id);
    let Some(batch) = batch else {
        match mode {
            MissingFontsPolicy::Error => {
                return Err(anyhow!(
                    "No loaded font data for Google Font '{}' (id: {})",
                    font_info.family,
                    font_id
                ));
            }
            _ => {
                log::warn!(
                    "font_embed: no loaded data for '{}', skipping",
                    font_info.family
                );
                return Ok(());
            }
        }
    };

    // Build index of font data by (weight, style) from the loaded variants
    let variant_index: HashMap<(String, String), &Arc<Vec<u8>>> = batch
        .loaded_variants
        .iter()
        .zip(batch.font_data.iter())
        .map(|(variant, data)| {
            let weight = variant.weight.to_string();
            let style = variant.style.as_str().to_string();
            ((weight, style), data)
        })
        .collect();

    for (font_key, chars) in chars_by_font_key {
        if font_key.family != font_info.family || chars.is_empty() {
            continue;
        }

        let ws_key = (font_key.weight.clone(), font_key.style.clone());
        let ttf_data = if let Some(data) = variant_index.get(&ws_key) {
            data
        } else {
            // Exact variant not available — fall back to the closest weight
            // with matching style, or any available variant as last resort.
            // This handles fonts like Bangers that only have a single weight.
            let target_weight: i32 = font_key.weight.parse().unwrap_or(400);
            let fallback = variant_index
                .iter()
                .filter(|((_, s), _)| *s == font_key.style)
                .min_by_key(|((w, _), _)| (w.parse::<i32>().unwrap_or(400) - target_weight).abs())
                .or_else(|| {
                    variant_index.iter().min_by_key(|((w, _), _)| {
                        (w.parse::<i32>().unwrap_or(400) - target_weight).abs()
                    })
                });
            match fallback {
                Some((_, data)) => data,
                None => match mode {
                    MissingFontsPolicy::Error => {
                        return Err(anyhow!(
                            "No font data for {} weight={} style={}",
                            font_key.family,
                            font_key.weight,
                            font_key.style
                        ));
                    }
                    _ => {
                        log::warn!(
                            "font_embed: no data for {} weight={} style={}, skipping",
                            font_key.family,
                            font_key.weight,
                            font_key.style
                        );
                        continue;
                    }
                },
            }
        };

        match subset_and_encode_bytes(ttf_data, chars) {
            Ok(Some(artifact)) => {
                css_blocks.push(format_font_face_block(
                    &font_info.family,
                    &font_key.weight,
                    &font_key.style,
                    &artifact.woff2_b64,
                ));
            }
            Ok(None) => {
                // Font didn't cover any of the requested characters
            }
            Err(e) => match mode {
                MissingFontsPolicy::Error => {
                    return Err(anyhow!(
                        "Failed to subset '{}' weight={} style={}: {}",
                        font_info.family,
                        font_key.weight,
                        font_key.style,
                        e
                    ));
                }
                _ => {
                    log::warn!(
                        "font_embed: failed to subset '{}': {}, skipping",
                        font_info.family,
                        e
                    );
                }
            },
        }
    }

    Ok(())
}

/// Generate CSS for a locally-available font via fontdb lookup.
fn generate_local_font_css(
    font_info: &FontForHtml,
    chars_by_font_key: &HashMap<FontKey, BTreeSet<char>>,
    mode: &MissingFontsPolicy,
    fontdb: &fontdb::Database,
    css_blocks: &mut Vec<String>,
) -> Result<(), anyhow::Error> {
    for (font_key, chars) in chars_by_font_key {
        if font_key.family != font_info.family || chars.is_empty() {
            continue;
        }

        let target_weight = font_key.weight.parse::<u16>().unwrap_or(400);
        let target_style = match font_key.style.as_str() {
            "italic" => fontdb::Style::Italic,
            _ => fontdb::Style::Normal,
        };

        let query = fontdb::Query {
            families: &[fontdb::Family::Name(&font_info.family)],
            weight: fontdb::Weight(target_weight),
            style: target_style,
            ..Default::default()
        };

        if let Some(face_id) = fontdb.query(&query) {
            let result = fontdb.with_face_data(face_id, |data, _face_index| {
                subset_and_encode_bytes(data, chars)
            });
            match result {
                Some(Ok(Some(artifact))) => {
                    css_blocks.push(format_font_face_block(
                        &font_info.family,
                        &font_key.weight,
                        &font_key.style,
                        &artifact.woff2_b64,
                    ));
                }
                Some(Ok(None)) => {
                    // Font didn't cover any of the requested characters
                }
                Some(Err(e)) => {
                    let msg = format!(
                        "Cannot subset local font '{}' weight={} style={}: {}. \
                         CFF/OTF and TTC fonts are not supported for embedding.",
                        font_info.family, font_key.weight, font_key.style, e
                    );
                    match mode {
                        MissingFontsPolicy::Error => return Err(anyhow!(msg)),
                        MissingFontsPolicy::Warn => log::warn!("font_embed: {msg}"),
                        MissingFontsPolicy::Fallback => {}
                    }
                }
                None => {
                    log::warn!(
                        "font_embed: fontdb could not provide face data for '{}'",
                        font_info.family
                    );
                }
            }
        } else {
            log::warn!(
                "font_embed: no fontdb match for '{}' weight={} style={}",
                font_info.family,
                font_key.weight,
                font_key.style
            );
        }
    }

    Ok(())
}

struct SubsetArtifact {
    woff2_b64: String,
}

/// Subset in-memory TTF data to the given characters and return the
/// base64-encoded WOFF2 data. Returns `Ok(None)` if the font doesn't
/// cover any of the requested characters.
fn subset_and_encode_bytes(
    ttf_data: &[u8],
    chars: &BTreeSet<char>,
) -> Result<Option<SubsetArtifact>, anyhow::Error> {
    let reader = FontReader::new(ttf_data)?;
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
    }))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

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

    // -----------------------------------------------------------------------
    // subset_and_encode_bytes tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_subset_and_encode_bytes_invalid_data() {
        let bad_data = b"not a font";
        let chars: BTreeSet<char> = "Hello".chars().collect();
        assert!(subset_and_encode_bytes(bad_data, &chars).is_err());
    }

    #[test]
    fn test_subset_and_encode_bytes_empty_chars() {
        let chars: BTreeSet<char> = BTreeSet::new();
        let bad_data = b"not a font";
        // Either an error (bad font) or None (no chars) is acceptable
        let _result = subset_and_encode_bytes(bad_data, &chars);
    }

    #[test]
    fn test_subset_and_encode_bytes_caveat() {
        let font_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fonts/Caveat/static/Caveat-Regular.ttf");
        let ttf_data = std::fs::read(&font_path).expect("failed to read font file");
        let chars: BTreeSet<char> = "Hello World".chars().collect();
        let result = subset_and_encode_bytes(&ttf_data, &chars);
        match result {
            Ok(Some(artifact)) => {
                assert!(!artifact.woff2_b64.is_empty());
            }
            Ok(None) => panic!("Expected subset artifact but got None"),
            Err(e) => panic!("subset_and_encode_bytes failed: {e}"),
        }
    }

    // -----------------------------------------------------------------------
    // generate_local_font_css tests
    // -----------------------------------------------------------------------

    fn make_fontdb_with_liberation_sans() -> fontdb::Database {
        let mut db = fontdb::Database::new();
        let font_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fonts/liberation-sans");
        db.load_fonts_dir(font_dir);
        db
    }

    fn make_fontdb_with_caveat() -> fontdb::Database {
        let mut db = fontdb::Database::new();
        let font_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Caveat/static");
        db.load_fonts_dir(font_dir);
        db
    }

    #[test]
    fn test_generate_local_font_css_happy_path() {
        let db = make_fontdb_with_caveat();
        let font = FontForHtml {
            family: "Caveat".to_string(),
            source: FontSource::Local,
        };
        let mut chars_map = HashMap::new();
        chars_map.insert(
            FontKey {
                family: "Caveat".to_string(),
                weight: "400".to_string(),
                style: "normal".to_string(),
            },
            "Hello".chars().collect(),
        );
        let mut css_blocks = Vec::new();
        let result = generate_local_font_css(
            &font,
            &chars_map,
            &MissingFontsPolicy::Fallback,
            &db,
            &mut css_blocks,
        );
        assert!(result.is_ok());
        assert_eq!(css_blocks.len(), 1);
        let block = &css_blocks[0];
        assert!(block.contains("font-family: \"Caveat\""));
        assert!(block.contains("font-weight: 400"));
        assert!(block.contains("font-style: normal"));
        assert!(block.contains("src: url(data:font/woff2;base64,"));
    }

    #[test]
    fn test_generate_local_font_css_fallback_on_subset_error() {
        let db = make_fontdb_with_liberation_sans();
        let font = FontForHtml {
            family: "Liberation Sans".to_string(),
            source: FontSource::Local,
        };
        let mut chars_map = HashMap::new();
        chars_map.insert(
            FontKey {
                family: "Liberation Sans".to_string(),
                weight: "400".to_string(),
                style: "normal".to_string(),
            },
            "Hello".chars().collect(),
        );
        let mut css_blocks = Vec::new();
        let result = generate_local_font_css(
            &font,
            &chars_map,
            &MissingFontsPolicy::Fallback,
            &db,
            &mut css_blocks,
        );
        assert!(result.is_ok());
        // No CSS blocks since font-subset rejects the font
        assert!(css_blocks.is_empty());
    }

    #[test]
    fn test_generate_local_font_css_error_on_subset_error() {
        let db = make_fontdb_with_liberation_sans();
        let font = FontForHtml {
            family: "Liberation Sans".to_string(),
            source: FontSource::Local,
        };
        let mut chars_map = HashMap::new();
        chars_map.insert(
            FontKey {
                family: "Liberation Sans".to_string(),
                weight: "400".to_string(),
                style: "normal".to_string(),
            },
            "Hello".chars().collect(),
        );
        let mut css_blocks = Vec::new();
        let result = generate_local_font_css(
            &font,
            &chars_map,
            &MissingFontsPolicy::Error,
            &db,
            &mut css_blocks,
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Cannot subset local font"));
    }

    #[test]
    fn test_generate_local_font_css_no_match() {
        let db = fontdb::Database::new(); // empty db
        let font = FontForHtml {
            family: "Nonexistent Font".to_string(),
            source: FontSource::Local,
        };
        let mut chars_map = HashMap::new();
        chars_map.insert(
            FontKey {
                family: "Nonexistent Font".to_string(),
                weight: "400".to_string(),
                style: "normal".to_string(),
            },
            "Hello".chars().collect(),
        );
        let mut css_blocks = Vec::new();
        let result = generate_local_font_css(
            &font,
            &chars_map,
            &MissingFontsPolicy::Fallback,
            &db,
            &mut css_blocks,
        );
        assert!(result.is_ok());
        assert!(css_blocks.is_empty());
    }

    #[test]
    fn test_generate_font_face_css_local_happy_path() {
        let db = make_fontdb_with_caveat();
        let fonts = vec![FontForHtml {
            family: "Caveat".to_string(),
            source: FontSource::Local,
        }];
        let mut chars_map = HashMap::new();
        chars_map.insert(
            FontKey {
                family: "Caveat".to_string(),
                weight: "400".to_string(),
                style: "normal".to_string(),
            },
            "Test".chars().collect(),
        );
        let result = generate_font_face_css(
            &chars_map,
            &fonts,
            &MissingFontsPolicy::Fallback,
            &db,
            &[], // no Google batches
        );
        assert!(result.is_ok());
        let blocks = result.unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("font-family: \"Caveat\""));
        assert!(blocks[0].contains("data:font/woff2;base64,"));
    }

    #[test]
    fn test_generate_font_face_css_empty_fontdb() {
        let db = fontdb::Database::new();
        let fonts = vec![FontForHtml {
            family: "Missing".to_string(),
            source: FontSource::Local,
        }];
        let mut chars_map = HashMap::new();
        chars_map.insert(
            FontKey {
                family: "Missing".to_string(),
                weight: "400".to_string(),
                style: "normal".to_string(),
            },
            "abc".chars().collect(),
        );
        let result = generate_font_face_css(
            &chars_map,
            &fonts,
            &MissingFontsPolicy::Fallback,
            &db,
            &[], // no loaded batches
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}

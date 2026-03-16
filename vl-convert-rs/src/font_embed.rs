use base64::{engine::general_purpose::STANDARD, Engine};
use deno_core::anyhow::{self, anyhow};
use font_subset::FontReader;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use crate::converter::MissingFontsPolicy;
use crate::extract::{FontForHtml, FontKey, FontSource};
use vl_convert_google_fonts::{find_closest_variant, FontStyle, LoadedFontBatch, VariantRequest};

/// Format a single `@font-face` CSS block from a WOFF2-encoded artifact.
fn format_font_face_block(family: &str, weight: &str, style: &str, woff2_b64: &str) -> String {
    format!(
        "@font-face {{\n  font-family: \"{}\";\n  font-weight: {};\n  font-style: {};\n  src: url(data:font/woff2;base64,{}) format(\"woff2\");\n}}",
        family, weight, style, woff2_b64
    )
}

/// Extract all characters from a JSON string or array-of-strings field.
fn extract_chars_from_value(value: &serde_json::Value, chars: &mut BTreeSet<char>) {
    match value {
        serde_json::Value::String(s) => {
            for ch in s.chars() {
                chars.insert(ch);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let serde_json::Value::String(s) = item {
                    for ch in s.chars() {
                        chars.insert(ch);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Inject locale-aware characters into every font variant so that pan/zoom
/// interactions can render axis labels that weren't in the initial view.
///
/// Resolves the format and time-format locales to concrete JSON objects,
/// defaulting to en-US when not provided. Extracts all characters that D3's
/// formatters might produce: digits (or locale `numerals`), decimal/thousands
/// separators, currency symbols, minus/percent signs, month/day names, etc.
///
/// A small set of characters not covered by any locale (scientific notation
/// `e`/`E`, sign `+`, padding space, `NaN`) is always included.
pub fn inject_locale_chars(
    chars_by_key: &mut HashMap<FontKey, BTreeSet<char>>,
    format_locale: Option<&serde_json::Value>,
    time_format_locale: Option<&serde_json::Value>,
) {
    use crate::module_loader::{FORMATE_LOCALE_MAP, TIME_FORMATE_LOCALE_MAP};

    // Resolve to concrete locale JSON, defaulting to en-US.
    let default_fmt: serde_json::Value;
    let fmt_locale = match format_locale {
        Some(v) => v,
        None => {
            default_fmt = FORMATE_LOCALE_MAP
                .get("en-US")
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            &default_fmt
        }
    };
    let default_time: serde_json::Value;
    let time_locale = match time_format_locale {
        Some(v) => v,
        None => {
            default_time = TIME_FORMATE_LOCALE_MAP
                .get("en-US")
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            &default_time
        }
    };

    let mut locale_chars: BTreeSet<char> = BTreeSet::new();

    // Characters not covered by any locale but used by D3 formatting:
    // scientific notation and sign padding.
    for ch in ['+', ' ', 'e', 'E'] {
        locale_chars.insert(ch);
    }

    // Format locale: digits, numerals, separators, currency, signs.
    // Always include ASCII 0-9 because d3-time-format uses them regardless
    // of formatLocale.numerals. Additionally include locale numerals when
    // present (they replace ASCII digits in d3-format number output).
    for d in '0'..='9' {
        locale_chars.insert(d);
    }
    if let Some(numerals) = fmt_locale.get("numerals") {
        extract_chars_from_value(numerals, &mut locale_chars);
    }
    for field in ["decimal", "thousands", "currency"] {
        if let Some(val) = fmt_locale.get(field) {
            extract_chars_from_value(val, &mut locale_chars);
        }
    }
    // D3 defaults for optional format locale fields
    if let Some(val) = fmt_locale.get("minus") {
        extract_chars_from_value(val, &mut locale_chars);
    } else {
        locale_chars.insert('\u{2212}');
        locale_chars.insert('-');
    }
    if let Some(val) = fmt_locale.get("percent") {
        extract_chars_from_value(val, &mut locale_chars);
    } else {
        locale_chars.insert('%');
    }
    if let Some(val) = fmt_locale.get("nan") {
        extract_chars_from_value(val, &mut locale_chars);
    } else {
        for ch in "NaN".chars() {
            locale_chars.insert(ch);
        }
    }

    // Time format locale: format template literals (dateTime/date/time contain
    // punctuation like "/" and ":" that appear in rendered labels) plus
    // period/month/day name strings.
    for field in [
        "dateTime",
        "date",
        "time",
        "periods",
        "shortMonths",
        "shortDays",
        "months",
        "days",
    ] {
        if let Some(val) = time_locale.get(field) {
            extract_chars_from_value(val, &mut locale_chars);
        }
    }

    // Inject into every font variant
    for chars in chars_by_key.values_mut() {
        chars.extend(&locale_chars);
    }
}

/// Compute the set of (weight, style) variants used per font family.
///
/// Used by `vega_fonts`/`vegalite_fonts` to populate `FontInfo.variants` and
/// build CSS2 API URLs with the specific tuples the chart renders. (The HTML
/// CDN fast path bypasses this and requests the full weight range instead.)
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

/// Generate `@font-face` CSS blocks with subsetted WOFF2 fonts, indexed by
/// `(family, weight, style)`.
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
    subset_fonts: bool,
) -> Result<HashMap<FontKey, String>, anyhow::Error> {
    let mut css_blocks = HashMap::new();

    for font_info in html_fonts {
        match &font_info.source {
            FontSource::Google { font_id } => {
                generate_google_fonts_css(
                    font_info,
                    font_id,
                    chars_by_font_key,
                    mode,
                    loaded_batches,
                    &mut css_blocks,
                    subset_fonts,
                )?;
            }
            FontSource::Local => {
                generate_local_font_css(
                    font_info,
                    chars_by_font_key,
                    mode,
                    fontdb,
                    &mut css_blocks,
                    subset_fonts,
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
    css_blocks: &mut HashMap<FontKey, String>,
    subset_fonts: bool,
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
            MissingFontsPolicy::Warn => {
                log::warn!(
                    "font_embed: no loaded data for '{}', skipping",
                    font_info.family
                );
                return Ok(());
            }
            MissingFontsPolicy::Fallback => {
                return Ok(());
            }
        }
    };

    // Index font data by position, with available variants for fallback lookup
    let font_data_list: Vec<&Arc<Vec<u8>>> = batch.font_data.iter().collect();
    let available_variants: Vec<VariantRequest> = batch
        .loaded_variants
        .iter()
        .map(|v| VariantRequest {
            weight: v.weight,
            style: v.style,
        })
        .collect();

    for (font_key, chars) in chars_by_font_key {
        if font_key.family != font_info.family || chars.is_empty() {
            continue;
        }

        let requested = VariantRequest {
            weight: font_key.weight.parse().unwrap_or(400),
            style: font_key.style.parse().unwrap_or(FontStyle::Normal),
        };
        let ttf_data = if let Some(idx) = find_closest_variant(&requested, &available_variants) {
            font_data_list[idx]
        } else {
            match mode {
                MissingFontsPolicy::Error => {
                    return Err(anyhow!(
                        "No font data for {} weight={} style={}",
                        font_key.family,
                        font_key.weight,
                        font_key.style
                    ));
                }
                MissingFontsPolicy::Warn => {
                    log::warn!(
                        "font_embed: no data for {} weight={} style={}, skipping",
                        font_key.family,
                        font_key.weight,
                        font_key.style
                    );
                    continue;
                }
                MissingFontsPolicy::Fallback => {
                    continue;
                }
            }
        };

        let encode_result = if subset_fonts {
            subset_and_encode_bytes(ttf_data, chars)
        } else {
            encode_full_font_bytes(ttf_data)
        };
        match encode_result {
            Ok(Some(artifact)) => {
                css_blocks.insert(
                    font_key.clone(),
                    format_font_face_block(
                        &font_info.family,
                        &font_key.weight,
                        &font_key.style,
                        &artifact.woff2_b64,
                    ),
                );
            }
            Ok(None) => {
                // Font didn't cover any of the requested characters
            }
            Err(e) => match mode {
                MissingFontsPolicy::Error => {
                    return Err(anyhow!(
                        "Failed to encode '{}' weight={} style={}: {}",
                        font_info.family,
                        font_key.weight,
                        font_key.style,
                        e
                    ));
                }
                MissingFontsPolicy::Warn => {
                    log::warn!(
                        "font_embed: failed to encode '{}': {}, skipping",
                        font_info.family,
                        e
                    );
                }
                MissingFontsPolicy::Fallback => {}
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
    css_blocks: &mut HashMap<FontKey, String>,
    subset_fonts: bool,
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
                if subset_fonts {
                    subset_and_encode_bytes(data, chars)
                } else {
                    encode_full_font_bytes(data)
                }
            });
            match result {
                Some(Ok(Some(artifact))) => {
                    css_blocks.insert(
                        font_key.clone(),
                        format_font_face_block(
                            &font_info.family,
                            &font_key.weight,
                            &font_key.style,
                            &artifact.woff2_b64,
                        ),
                    );
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
                    if matches!(mode, MissingFontsPolicy::Warn) {
                        log::warn!(
                            "font_embed: fontdb could not provide face data for '{}'",
                            font_info.family
                        );
                    }
                }
            }
        } else if matches!(mode, MissingFontsPolicy::Warn) {
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

/// Encode full TTF data as base64 without subsetting.
fn encode_full_font_bytes(ttf_data: &[u8]) -> Result<Option<SubsetArtifact>, anyhow::Error> {
    Ok(Some(SubsetArtifact {
        woff2_b64: STANDARD.encode(ttf_data),
    }))
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

    #[test]
    fn test_subset_and_encode_bytes_invalid_data() {
        let bad_data = b"not a font";
        let chars: BTreeSet<char> = "Hello".chars().collect();
        assert!(subset_and_encode_bytes(bad_data, &chars).is_err());
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
        let mut css_blocks = HashMap::new();
        let result = generate_local_font_css(
            &font,
            &chars_map,
            &MissingFontsPolicy::Fallback,
            &db,
            &mut css_blocks,
            true,
        );
        assert!(result.is_ok());
        assert_eq!(css_blocks.len(), 1);
        let key = FontKey {
            family: "Caveat".to_string(),
            weight: "400".to_string(),
            style: "normal".to_string(),
        };
        let block = &css_blocks[&key];
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
        let mut css_blocks = HashMap::new();
        let result = generate_local_font_css(
            &font,
            &chars_map,
            &MissingFontsPolicy::Fallback,
            &db,
            &mut css_blocks,
            true,
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
        let mut css_blocks = HashMap::new();
        let result = generate_local_font_css(
            &font,
            &chars_map,
            &MissingFontsPolicy::Error,
            &db,
            &mut css_blocks,
            true,
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Cannot subset local font"));
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
            true,
        );
        assert!(result.is_ok());
        let blocks = result.unwrap();
        assert_eq!(blocks.len(), 1);
        let key = FontKey {
            family: "Caveat".to_string(),
            weight: "400".to_string(),
            style: "normal".to_string(),
        };
        assert!(blocks[&key].contains("font-family: \"Caveat\""));
        assert!(blocks[&key].contains("data:font/woff2;base64,"));
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
            true,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    fn make_font_key() -> FontKey {
        FontKey {
            family: "TestFont".to_string(),
            weight: "400".to_string(),
            style: "normal".to_string(),
        }
    }

    fn make_chars_map(text: &str) -> HashMap<FontKey, BTreeSet<char>> {
        let mut map = HashMap::new();
        map.insert(make_font_key(), text.chars().collect());
        map
    }

    #[test]
    fn test_inject_locale_chars_defaults_only() {
        let mut map = make_chars_map("Hello");
        inject_locale_chars(&mut map, None, None);
        let chars = &map[&make_font_key()];
        // Original chars preserved
        assert!(chars.contains(&'H'));
        assert!(chars.contains(&'o'));
        // Digits
        for d in '0'..='9' {
            assert!(chars.contains(&d), "missing digit: {d}");
        }
        // Separators and formatting
        assert!(chars.contains(&'.'));
        assert!(chars.contains(&','));
        assert!(chars.contains(&'-'));
        assert!(chars.contains(&'+'));
        assert!(chars.contains(&'%'));
        assert!(chars.contains(&' '));
        assert!(chars.contains(&'e'));
        assert!(chars.contains(&'E'));
        assert!(chars.contains(&'\u{2212}')); // minus sign
                                              // NaN
        assert!(chars.contains(&'N'));
        assert!(chars.contains(&'a'));
    }

    #[test]
    fn test_inject_locale_chars_arabic_format() {
        let mut map = make_chars_map("X");
        let locale: serde_json::Value = serde_json::json!({
            "decimal": "\u{066b}",
            "thousands": "\u{066c}",
            "grouping": [3],
            "currency": ["", ""],
            "numerals": ["\u{0660}", "\u{0661}", "\u{0662}", "\u{0663}", "\u{0664}",
                         "\u{0665}", "\u{0666}", "\u{0667}", "\u{0668}", "\u{0669}"]
        });
        inject_locale_chars(&mut map, Some(&locale), None);
        let chars = &map[&make_font_key()];
        // Arabic-Indic decimal and thousands separators
        assert!(chars.contains(&'\u{066b}'));
        assert!(chars.contains(&'\u{066c}'));
        // Arabic-Indic numerals
        assert!(chars.contains(&'\u{0660}'));
        assert!(chars.contains(&'\u{0669}'));
        // ASCII digits still included (needed for d3-time-format)
        assert!(chars.contains(&'0'));
    }

    #[test]
    fn test_inject_locale_chars_european_format() {
        let mut map = make_chars_map("X");
        let locale: serde_json::Value = serde_json::json!({
            "decimal": ",",
            "thousands": ".",
            "grouping": [3],
            "currency": ["", "\u{00a0}€"]
        });
        inject_locale_chars(&mut map, Some(&locale), None);
        let chars = &map[&make_font_key()];
        // Euro sign and non-breaking space from currency suffix
        assert!(chars.contains(&'€'));
        assert!(chars.contains(&'\u{00a0}'));
    }

    #[test]
    fn test_inject_locale_chars_french_percent() {
        let mut map = make_chars_map("X");
        let locale: serde_json::Value = serde_json::json!({
            "decimal": ",",
            "thousands": "\u{00a0}",
            "grouping": [3],
            "currency": ["", "\u{00a0}€"],
            "percent": "\u{202f}%"
        });
        inject_locale_chars(&mut map, Some(&locale), None);
        let chars = &map[&make_font_key()];
        // Narrow no-break space from percent field
        assert!(chars.contains(&'\u{202f}'));
    }

    #[test]
    fn test_inject_locale_chars_time_format() {
        let mut map = make_chars_map("X");
        let locale: serde_json::Value = serde_json::json!({
            "dateTime": "%A, der %e. %B %Y, %X",
            "date": "%d.%m.%Y",
            "time": "%H:%M:%S",
            "periods": ["AM", "PM"],
            "days": ["Sonntag", "Montag"],
            "shortDays": ["So", "Mo"],
            "months": ["März"],
            "shortMonths": ["Mrz"]
        });
        inject_locale_chars(&mut map, None, Some(&locale));
        let chars = &map[&make_font_key()];
        // German month with umlaut
        assert!(chars.contains(&'ä'));
        // Day/month name chars
        assert!(chars.contains(&'S'));
        assert!(chars.contains(&'o'));
        // AM/PM
        assert!(chars.contains(&'A'));
        assert!(chars.contains(&'M'));
        assert!(chars.contains(&'P'));
        // Literals from date/time format strings
        assert!(chars.contains(&':')); // from time: "%H:%M:%S"
    }

    #[test]
    fn test_inject_locale_chars_arabic_numerals_with_time() {
        let mut map = make_chars_map("X");
        let fmt: serde_json::Value = serde_json::json!({
            "decimal": "\u{066b}",
            "thousands": "\u{066c}",
            "grouping": [3],
            "currency": ["", ""],
            "numerals": ["\u{0660}", "\u{0661}", "\u{0662}", "\u{0663}", "\u{0664}",
                         "\u{0665}", "\u{0666}", "\u{0667}", "\u{0668}", "\u{0669}"]
        });
        let time: serde_json::Value = serde_json::json!({
            "dateTime": "%x, %X",
            "date": "%-m/%-d/%Y",
            "time": "%-I:%M:%S %p",
            "periods": ["AM", "PM"],
            "shortMonths": ["Jan"],
            "shortDays": ["Sun"],
            "months": ["January"],
            "days": ["Sunday"]
        });
        inject_locale_chars(&mut map, Some(&fmt), Some(&time));
        let chars = &map[&make_font_key()];
        // Arabic-Indic numerals for number formatting
        assert!(chars.contains(&'\u{0660}'));
        assert!(chars.contains(&'\u{0669}'));
        // ASCII digits still needed for d3-time-format (%d, %H, %M, %S)
        assert!(chars.contains(&'0'));
        assert!(chars.contains(&'9'));
        // Time format literals
        assert!(chars.contains(&'/'));
        assert!(chars.contains(&':'));
    }
}

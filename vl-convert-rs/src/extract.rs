use serde_json::Value;
use std::collections::HashSet;

/// Metadata for a font that should be loaded via CDN in HTML output.
#[derive(Debug, Clone)]
pub struct FontForHtml {
    /// The font family name (e.g., "Roboto", "Playfair Display").
    pub family: String,
    /// The Fontsource font ID (e.g., "roboto", "playfair-display").
    pub font_id: String,
    /// Whether this is a Google font ("google") or other ("other").
    pub font_type: String,
}

// CSS generic family keywords per CSS Fonts Module Level 4:
// https://www.w3.org/TR/css-fonts-4/#generic-font-families
const GENERIC_FAMILIES: &[&str] = &[
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "ui-rounded",
    "emoji",
    "math",
    "fangsong",
];

/// A single entry from a parsed CSS `font-family` string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontFamilyEntry {
    /// A concrete font family name (e.g. "Roboto", "Playfair Display").
    Named(String),
    /// A CSS generic family keyword (e.g. "serif", "sans-serif").
    Generic(String),
}

/// Parse a CSS `font-family` string into a list of [`FontFamilyEntry`] values.
///
/// Uses `svgtypes::parse_font_families` for spec-compliant parsing (handles
/// quoting, escaping, and multi-word unquoted names). Returns an empty list
/// if the input is not valid CSS.
///
/// # Examples
///
/// ```
/// use vl_convert_rs::extract::{parse_css_font_family, FontFamilyEntry};
///
/// let entries = parse_css_font_family("Roboto, sans-serif");
/// assert_eq!(entries, vec![
///     FontFamilyEntry::Named("Roboto".into()),
///     FontFamilyEntry::Generic("sans-serif".into()),
/// ]);
/// ```
pub fn parse_css_font_family(s: &str) -> Vec<FontFamilyEntry> {
    let Ok(families) = svgtypes::parse_font_families(s) else {
        return Vec::new();
    };
    families
        .into_iter()
        .map(|f| match f {
            svgtypes::FontFamily::Serif => FontFamilyEntry::Generic("serif".into()),
            svgtypes::FontFamily::SansSerif => FontFamilyEntry::Generic("sans-serif".into()),
            svgtypes::FontFamily::Cursive => FontFamilyEntry::Generic("cursive".into()),
            svgtypes::FontFamily::Fantasy => FontFamilyEntry::Generic("fantasy".into()),
            svgtypes::FontFamily::Monospace => FontFamilyEntry::Generic("monospace".into()),
            svgtypes::FontFamily::Named(name) => {
                let lower = name.to_lowercase();
                if GENERIC_FAMILIES.iter().any(|g| *g == lower) {
                    FontFamilyEntry::Generic(name)
                } else {
                    FontFamilyEntry::Named(name)
                }
            }
        })
        .collect()
}

/// Extract all font-family CSS strings from a compiled Vega specification.
///
/// Returns a deduplicated set of raw CSS font-family strings found in static
/// positions throughout the spec (config, marks, axes, legends, title,
/// data transforms). Dynamic references (signal/field) are skipped.
pub fn extract_fonts_from_vega(spec: &Value) -> HashSet<String> {
    let mut fonts = HashSet::new();

    // Config
    if let Some(config) = spec.get("config") {
        extract_config_fonts(config, &mut fonts);
    }

    // Top-level marks (recursive)
    if let Some(marks) = spec.get("marks") {
        extract_marks_fonts(marks, &mut fonts);
    }

    // Top-level axes
    if let Some(axes) = spec.get("axes") {
        extract_axes_fonts(axes, &mut fonts);
    }

    // Top-level legends
    if let Some(legends) = spec.get("legends") {
        extract_legends_fonts(legends, &mut fonts);
    }

    // Top-level title
    if let Some(title) = spec.get("title") {
        extract_title_fonts(title, &mut fonts);
    }

    // Data transforms (e.g. wordcloud)
    if let Some(data) = spec.get("data").and_then(Value::as_array) {
        for dataset in data {
            if let Some(transforms) = dataset.get("transform").and_then(Value::as_array) {
                for transform in transforms {
                    extract_transform_fonts(transform, &mut fonts);
                }
            }
        }
    }

    fonts
}

/// Axis config key variants (matches Vega's AxisConfigKeys type).
const AXIS_CONFIG_KEYS: &[&str] = &[
    "axis",
    "axisX",
    "axisY",
    "axisTop",
    "axisBottom",
    "axisLeft",
    "axisRight",
    "axisBand",
];

/// Vega mark types whose config can carry a `font` property.
/// Only `text` renders text; other native marks (arc, area, etc.) ignore `font`.
const MARK_TYPE_KEYS: &[&str] = &["text"];

fn extract_config_fonts(config: &Value, fonts: &mut HashSet<String>) {
    // Title
    if let Some(title) = config.get("title") {
        collect_if_string(title, "font", fonts);
        collect_if_string(title, "subtitleFont", fonts);
    }

    // Axis variants
    for &key in AXIS_CONFIG_KEYS {
        if let Some(axis) = config.get(key) {
            collect_if_string(axis, "labelFont", fonts);
            collect_if_string(axis, "titleFont", fonts);
        }
    }

    // Legend
    if let Some(legend) = config.get("legend") {
        collect_if_string(legend, "labelFont", fonts);
        collect_if_string(legend, "titleFont", fonts);
    }

    // Mark type defaults
    for &key in MARK_TYPE_KEYS {
        if let Some(mark_cfg) = config.get(key) {
            collect_if_string(mark_cfg, "font", fonts);
        }
    }

    // Mark default: config.mark.font
    if let Some(mark) = config.get("mark") {
        collect_if_string(mark, "font", fonts);
    }

    // Named styles: config.style is an object { styleName: { font, ... } }
    if let Some(style) = config.get("style").and_then(Value::as_object) {
        for (_style_name, style_obj) in style {
            collect_if_string(style_obj, "font", fonts);
            collect_if_string(style_obj, "labelFont", fonts);
            collect_if_string(style_obj, "titleFont", fonts);
        }
    }
}

fn extract_marks_fonts(marks: &Value, fonts: &mut HashSet<String>) {
    let arr = match marks.as_array() {
        Some(a) => a,
        None => return,
    };

    for mark in arr {
        // Encode blocks: enter, update, hover, exit
        if let Some(encode) = mark.get("encode") {
            for &state in &[
                "enter", "update", "hover", "exit", "leave", "select", "release",
            ] {
                if let Some(font_val) = encode
                    .get(state)
                    .and_then(|s| s.get("font"))
                    .and_then(|f| f.get("value"))
                    .and_then(Value::as_str)
                {
                    fonts.insert(font_val.to_string());
                }
            }
        }

        // Group marks: recurse into nested marks, axes, legends
        if let Some(nested_marks) = mark.get("marks") {
            extract_marks_fonts(nested_marks, fonts);
        }
        if let Some(nested_axes) = mark.get("axes") {
            extract_axes_fonts(nested_axes, fonts);
        }
        if let Some(nested_legends) = mark.get("legends") {
            extract_legends_fonts(nested_legends, fonts);
        }
        // Also check for nested title within group marks
        if let Some(nested_title) = mark.get("title") {
            extract_title_fonts(nested_title, fonts);
        }
    }
}

fn extract_axes_fonts(axes: &Value, fonts: &mut HashSet<String>) {
    let arr = match axes.as_array() {
        Some(a) => a,
        None => return,
    };

    for axis in arr {
        // Direct properties
        collect_if_string(axis, "labelFont", fonts);
        collect_if_string(axis, "titleFont", fonts);

        // Encode paths: encode.{labels,title}.{state}.font.value
        if let Some(encode) = axis.get("encode") {
            for &part in &["labels", "title"] {
                if let Some(part_obj) = encode.get(part) {
                    for &state in &[
                        "enter", "update", "hover", "exit", "leave", "select", "release",
                    ] {
                        if let Some(font_val) = part_obj
                            .get(state)
                            .and_then(|s| s.get("font"))
                            .and_then(|f| f.get("value"))
                            .and_then(Value::as_str)
                        {
                            fonts.insert(font_val.to_string());
                        }
                    }
                }
            }
        }
    }
}

fn extract_legends_fonts(legends: &Value, fonts: &mut HashSet<String>) {
    let arr = match legends.as_array() {
        Some(a) => a,
        None => return,
    };

    for legend in arr {
        // Direct properties
        collect_if_string(legend, "labelFont", fonts);
        collect_if_string(legend, "titleFont", fonts);

        // Encode paths: encode.{labels,title}.{state}.font.value
        if let Some(encode) = legend.get("encode") {
            for &part in &["labels", "title"] {
                if let Some(part_obj) = encode.get(part) {
                    for &state in &[
                        "enter", "update", "hover", "exit", "leave", "select", "release",
                    ] {
                        if let Some(font_val) = part_obj
                            .get(state)
                            .and_then(|s| s.get("font"))
                            .and_then(|f| f.get("value"))
                            .and_then(Value::as_str)
                        {
                            fonts.insert(font_val.to_string());
                        }
                    }
                }
            }
        }
    }
}

fn extract_title_fonts(title: &Value, fonts: &mut HashSet<String>) {
    // Title can be a string (no font info) or an object.
    if title.is_string() {
        return;
    }
    collect_if_string(title, "font", fonts);
    collect_if_string(title, "subtitleFont", fonts);

    // Encode paths: encode.{title,subtitle}.{state}.font.value
    if let Some(encode) = title.get("encode") {
        for &part in &["title", "subtitle"] {
            if let Some(part_obj) = encode.get(part) {
                for &state in &[
                    "enter", "update", "hover", "exit", "leave", "select", "release",
                ] {
                    if let Some(font_val) = part_obj
                        .get(state)
                        .and_then(|s| s.get("font"))
                        .and_then(|f| f.get("value"))
                        .and_then(Value::as_str)
                    {
                        fonts.insert(font_val.to_string());
                    }
                }
            }
        }
    }
}

fn extract_transform_fonts(transform: &Value, fonts: &mut HashSet<String>) {
    // Wordcloud transforms: { "type": "wordcloud", "font": "..." }
    let is_wordcloud = transform
        .get("type")
        .and_then(Value::as_str)
        .map(|t| t == "wordcloud")
        .unwrap_or(false);

    if is_wordcloud {
        collect_if_string(transform, "font", fonts);
    }
}

/// If `obj[key]` is a JSON string, insert it into `fonts`.
fn collect_if_string(obj: &Value, key: &str, fonts: &mut HashSet<String>) {
    if let Some(val) = obj.get(key).and_then(Value::as_str) {
        if !val.is_empty() {
            fonts.insert(val.to_string());
        }
    }
}

/// Classification of the first font in a CSS `font-family` string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirstFontStatus {
    /// First entry is a CSS generic keyword (serif, sans-serif, etc.) —
    /// always satisfied by the system font configuration.
    Generic,
    /// First entry is already registered in fontdb.
    Available { name: String },
    /// First entry is downloadable from Fontsource.
    NeedsDownload { name: String },
    /// First entry is not on the system and not on Fontsource.
    Unavailable { name: String },
}

/// Classify each font-family string by examining only the **first** entry.
///
/// For each CSS font-family string, the first entry is checked:
///
/// 1. **Generic** keyword (serif, sans-serif, etc.) → [`FirstFontStatus::Generic`]
/// 2. **Named** family already in `available` → [`FirstFontStatus::Available`]
/// 3. **Named** family for which `downloadable(family)` returns `true` →
///    [`FirstFontStatus::NeedsDownload`]
/// 4. **Named** family that is neither available nor downloadable →
///    [`FirstFontStatus::Unavailable`]
///
/// Only the first entry matters — the rest of the fallback chain is ignored.
/// Results are deduplicated by CSS string.
pub fn resolve_first_fonts(
    font_strings: &[String],
    available: &HashSet<String>,
    downloadable: impl Fn(&str) -> bool,
) -> Vec<(String, FirstFontStatus)> {
    let mut results: Vec<(String, FirstFontStatus)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for font_string in font_strings {
        if !seen.insert(font_string.clone()) {
            continue;
        }

        let entries = parse_css_font_family(font_string);
        let status = match entries.first() {
            None => continue, // empty/whitespace-only string
            Some(FontFamilyEntry::Generic(_)) => FirstFontStatus::Generic,
            Some(FontFamilyEntry::Named(name)) => {
                if is_available(name, available) {
                    FirstFontStatus::Available { name: name.clone() }
                } else if downloadable(name) {
                    FirstFontStatus::NeedsDownload { name: name.clone() }
                } else {
                    FirstFontStatus::Unavailable { name: name.clone() }
                }
            }
        };

        results.push((font_string.clone(), status));
    }

    results
}

/// Case-insensitive membership check against the available font set.
///
/// The `available` set is expected to contain font names in their original
/// casing (as reported by fontdb). We check both the exact name and a
/// lowercased version.
pub fn is_available(name: &str, available: &HashSet<String>) -> bool {
    if available.contains(name) {
        return true;
    }
    let lower = name.to_lowercase();
    available.iter().any(|a| a.to_lowercase() == lower)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_single_named() {
        assert_eq!(
            parse_css_font_family("Roboto"),
            vec![FontFamilyEntry::Named("Roboto".into())]
        );
    }

    #[test]
    fn test_parse_named_and_generic() {
        assert_eq!(
            parse_css_font_family("Roboto, sans-serif"),
            vec![
                FontFamilyEntry::Named("Roboto".into()),
                FontFamilyEntry::Generic("sans-serif".into()),
            ]
        );
    }

    #[test]
    fn test_parse_single_quoted() {
        assert_eq!(
            parse_css_font_family("'Playfair Display', Georgia, serif"),
            vec![
                FontFamilyEntry::Named("Playfair Display".into()),
                FontFamilyEntry::Named("Georgia".into()),
                FontFamilyEntry::Generic("serif".into()),
            ]
        );
    }

    #[test]
    fn test_parse_double_quoted() {
        assert_eq!(
            parse_css_font_family("\"IBM Plex Sans\""),
            vec![FontFamilyEntry::Named("IBM Plex Sans".into())]
        );
    }

    #[test]
    fn test_parse_all_generics() {
        for &generic in GENERIC_FAMILIES {
            let entries = parse_css_font_family(generic);
            assert_eq!(
                entries,
                vec![FontFamilyEntry::Generic(generic.into())],
                "failed for generic: {}",
                generic
            );
        }
    }

    #[test]
    fn test_parse_empty_string() {
        assert!(parse_css_font_family("").is_empty());
    }

    #[test]
    fn test_parse_whitespace_only() {
        assert!(parse_css_font_family("   ").is_empty());
    }

    #[test]
    fn test_parse_only_commas() {
        assert!(parse_css_font_family(",,,").is_empty());
    }

    #[test]
    fn test_parse_quoted_font_with_comma() {
        let entries = parse_css_font_family("'Font, With Comma', serif");
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            FontFamilyEntry::Named("Font, With Comma".into())
        );
        assert_eq!(entries[1], FontFamilyEntry::Generic("serif".into()));
    }

    #[test]
    fn test_parse_whitespace_around_commas() {
        assert_eq!(
            parse_css_font_family("  Roboto  ,  Arial  ,  monospace  "),
            vec![
                FontFamilyEntry::Named("Roboto".into()),
                FontFamilyEntry::Named("Arial".into()),
                FontFamilyEntry::Generic("monospace".into()),
            ]
        );
    }

    #[test]
    fn test_parse_mixed_quotes() {
        assert_eq!(
            parse_css_font_family("'Times New Roman', \"Courier New\", monospace"),
            vec![
                FontFamilyEntry::Named("Times New Roman".into()),
                FontFamilyEntry::Named("Courier New".into()),
                FontFamilyEntry::Generic("monospace".into()),
            ]
        );
    }

    #[test]
    fn test_parse_unquoted_multi_word() {
        assert_eq!(
            parse_css_font_family("Segoe UI"),
            vec![FontFamilyEntry::Named("Segoe UI".into())]
        );
    }

    #[test]
    fn test_parse_power_bi_chain() {
        assert_eq!(
            parse_css_font_family("wf_standard-font, helvetica, arial, sans-serif"),
            vec![
                FontFamilyEntry::Named("wf_standard-font".into()),
                FontFamilyEntry::Named("helvetica".into()),
                FontFamilyEntry::Named("arial".into()),
                FontFamilyEntry::Generic("sans-serif".into()),
            ]
        );
    }

    #[test]
    fn test_extract_config_fonts() {
        let spec = json!({
            "config": {
                "title": {
                    "font": "Playfair Display, Georgia, serif",
                    "subtitleFont": "Source Sans Pro"
                },
                "axis": {
                    "labelFont": "Fira Code, monospace",
                    "titleFont": "Roboto"
                },
                "legend": {
                    "labelFont": "Noto Sans",
                    "titleFont": "Noto Serif"
                },
                "text": {
                    "font": "IBM Plex Mono"
                },
                "style": {
                    "guide-label": {
                        "font": "Lato"
                    },
                    "group-title": {
                        "font": "Oswald"
                    }
                }
            }
        });

        let fonts = extract_fonts_from_vega(&spec);

        assert!(fonts.contains("Playfair Display, Georgia, serif"));
        assert!(fonts.contains("Source Sans Pro"));
        assert!(fonts.contains("Fira Code, monospace"));
        assert!(fonts.contains("Roboto"));
        assert!(fonts.contains("Noto Sans"));
        assert!(fonts.contains("Noto Serif"));
        assert!(fonts.contains("IBM Plex Mono"));
        assert!(fonts.contains("Lato"));
        assert!(fonts.contains("Oswald"));
    }

    #[test]
    fn test_extract_mark_fonts() {
        let spec = json!({
            "marks": [
                {
                    "type": "text",
                    "encode": {
                        "enter": {
                            "font": { "value": "Merriweather" }
                        },
                        "update": {
                            "font": { "value": "Roboto Mono" }
                        }
                    }
                },
                {
                    "type": "text",
                    "encode": {
                        "update": {
                            "font": { "signal": "dynamicFont" }
                        }
                    }
                }
            ]
        });

        let fonts = extract_fonts_from_vega(&spec);

        assert!(fonts.contains("Merriweather"));
        assert!(fonts.contains("Roboto Mono"));
        // Signal-driven font should NOT be extracted.
        assert_eq!(fonts.len(), 2);
    }

    #[test]
    fn test_extract_nested_group_marks() {
        let spec = json!({
            "marks": [
                {
                    "type": "group",
                    "marks": [
                        {
                            "type": "text",
                            "encode": {
                                "update": {
                                    "font": { "value": "Cabin" }
                                }
                            }
                        }
                    ],
                    "axes": [
                        { "labelFont": "Inconsolata" }
                    ],
                    "legends": [
                        { "titleFont": "Open Sans" }
                    ]
                }
            ]
        });

        let fonts = extract_fonts_from_vega(&spec);

        assert!(fonts.contains("Cabin"));
        assert!(fonts.contains("Inconsolata"));
        assert!(fonts.contains("Open Sans"));
    }

    #[test]
    fn test_extract_axes_fonts() {
        let spec = json!({
            "axes": [
                {
                    "labelFont": "Fira Sans",
                    "titleFont": "Fira Sans Bold"
                },
                {
                    "encode": {
                        "labels": {
                            "update": {
                                "font": { "value": "Droid Sans" }
                            }
                        },
                        "title": {
                            "update": {
                                "font": { "value": "Droid Serif" }
                            }
                        }
                    }
                }
            ]
        });

        let fonts = extract_fonts_from_vega(&spec);

        assert!(fonts.contains("Fira Sans"));
        assert!(fonts.contains("Fira Sans Bold"));
        assert!(fonts.contains("Droid Sans"));
        assert!(fonts.contains("Droid Serif"));
    }

    #[test]
    fn test_extract_legends_fonts() {
        let spec = json!({
            "legends": [
                {
                    "labelFont": "PT Sans",
                    "titleFont": "PT Serif"
                }
            ]
        });

        let fonts = extract_fonts_from_vega(&spec);

        assert!(fonts.contains("PT Sans"));
        assert!(fonts.contains("PT Serif"));
    }

    #[test]
    fn test_extract_legends_encode_fonts() {
        let spec = json!({
            "legends": [
                {
                    "encode": {
                        "labels": {
                            "update": {
                                "font": { "value": "Droid Sans" }
                            }
                        },
                        "title": {
                            "update": {
                                "font": { "value": "Droid Serif" }
                            }
                        }
                    }
                }
            ]
        });

        let fonts = extract_fonts_from_vega(&spec);

        assert!(fonts.contains("Droid Sans"));
        assert!(fonts.contains("Droid Serif"));
        assert_eq!(fonts.len(), 2);
    }

    #[test]
    fn test_extract_title_fonts() {
        let spec = json!({
            "title": {
                "text": "My Chart",
                "font": "Montserrat, sans-serif",
                "subtitleFont": "Lora"
            }
        });

        let fonts = extract_fonts_from_vega(&spec);

        assert!(fonts.contains("Montserrat, sans-serif"));
        assert!(fonts.contains("Lora"));
    }

    #[test]
    fn test_extract_title_encode_fonts() {
        let spec = json!({
            "title": {
                "text": "My Chart",
                "encode": {
                    "title": {
                        "update": {
                            "font": { "value": "Montserrat" }
                        }
                    },
                    "subtitle": {
                        "update": {
                            "font": { "value": "Lora" }
                        }
                    }
                }
            }
        });

        let fonts = extract_fonts_from_vega(&spec);

        assert!(fonts.contains("Montserrat"));
        assert!(fonts.contains("Lora"));
        assert_eq!(fonts.len(), 2);
    }

    #[test]
    fn test_extract_title_string_only() {
        // When title is just a string, there's no font info.
        let spec = json!({
            "title": "My Chart"
        });

        let fonts = extract_fonts_from_vega(&spec);
        assert!(fonts.is_empty());
    }

    #[test]
    fn test_extract_wordcloud_transform() {
        let spec = json!({
            "data": [
                {
                    "name": "table",
                    "transform": [
                        {
                            "type": "wordcloud",
                            "font": "Pacifico"
                        },
                        {
                            "type": "formula",
                            "as": "weight"
                        }
                    ]
                }
            ]
        });

        let fonts = extract_fonts_from_vega(&spec);

        assert!(fonts.contains("Pacifico"));
        assert_eq!(fonts.len(), 1);
    }

    #[test]
    fn test_extract_wordcloud_signal_font_skipped() {
        let spec = json!({
            "data": [
                {
                    "name": "table",
                    "transform": [
                        {
                            "type": "wordcloud",
                            "font": { "signal": "fontChoice" }
                        }
                    ]
                }
            ]
        });

        let fonts = extract_fonts_from_vega(&spec);
        // Signal-based font in wordcloud → not a string, skipped.
        assert!(fonts.is_empty());
    }

    #[test]
    fn test_extract_axis_orientation_overrides() {
        let spec = json!({
            "config": {
                "axisX": { "labelFont": "Barlow" },
                "axisY": { "titleFont": "Barlow Condensed" },
                "axisTop": { "labelFont": "Rubik" },
                "axisBottom": { "titleFont": "Ubuntu" },
                "axisLeft": { "labelFont": "Quicksand" },
                "axisRight": { "titleFont": "Karla" },
                "axisBand": { "labelFont": "Manrope" }
            }
        });

        let fonts = extract_fonts_from_vega(&spec);

        assert!(fonts.contains("Barlow"));
        assert!(fonts.contains("Barlow Condensed"));
        assert!(fonts.contains("Rubik"));
        assert!(fonts.contains("Ubuntu"));
        assert!(fonts.contains("Quicksand"));
        assert!(fonts.contains("Karla"));
        assert!(fonts.contains("Manrope"));
    }

    #[test]
    fn test_extract_text_mark_config() {
        let spec = json!({
            "config": {
                "text": { "font": "IBM Plex Mono" }
            }
        });

        let fonts = extract_fonts_from_vega(&spec);
        assert!(fonts.contains("IBM Plex Mono"));
        assert_eq!(fonts.len(), 1);
    }

    #[test]
    fn test_extract_config_style_label_title_fonts() {
        let spec = json!({
            "config": {
                "style": {
                    "guide-label": {
                        "labelFont": "Asap",
                        "titleFont": "Assistant"
                    }
                }
            }
        });

        let fonts = extract_fonts_from_vega(&spec);

        assert!(fonts.contains("Asap"));
        assert!(fonts.contains("Assistant"));
    }

    #[test]
    fn test_extract_deduplicates() {
        let spec = json!({
            "config": {
                "axis": {
                    "labelFont": "Roboto",
                    "titleFont": "Roboto"
                }
            },
            "axes": [
                { "labelFont": "Roboto" }
            ]
        });

        let fonts = extract_fonts_from_vega(&spec);

        assert!(fonts.contains("Roboto"));
        assert_eq!(fonts.len(), 1);
    }

    #[test]
    fn test_extract_comprehensive_fixture() {
        let spec = json!({
            "config": {
                "title": { "font": "Playfair Display, serif" },
                "axis": { "labelFont": "Fira Code, monospace" },
                "text": { "font": "IBM Plex Sans" },
                "style": {
                    "guide-label": { "font": "Lato" }
                }
            },
            "marks": [
                {
                    "type": "text",
                    "encode": {
                        "update": {
                            "font": { "value": "Merriweather" }
                        }
                    }
                },
                {
                    "type": "group",
                    "marks": [
                        {
                            "type": "text",
                            "encode": {
                                "enter": {
                                    "font": { "value": "Cabin" }
                                }
                            }
                        }
                    ],
                    "axes": [
                        { "labelFont": "Inconsolata" }
                    ]
                }
            ],
            "axes": [
                { "titleFont": "Source Sans Pro" }
            ],
            "legends": [
                { "labelFont": "Noto Sans" }
            ],
            "title": {
                "text": "Chart Title",
                "font": "Montserrat",
                "subtitleFont": "Lora"
            },
            "data": [
                {
                    "name": "words",
                    "transform": [
                        { "type": "wordcloud", "font": "Pacifico" }
                    ]
                }
            ]
        });

        let fonts = extract_fonts_from_vega(&spec);

        let expected: HashSet<String> = [
            "Playfair Display, serif",
            "Fira Code, monospace",
            "IBM Plex Sans",
            "Lato",
            "Merriweather",
            "Cabin",
            "Inconsolata",
            "Source Sans Pro",
            "Noto Sans",
            "Montserrat",
            "Lora",
            "Pacifico",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        assert_eq!(fonts, expected);
    }

    #[test]
    fn test_resolve_first_font_generic() {
        // "serif" → first entry is generic → Generic
        let font_strings = vec!["serif".to_string()];
        let available: HashSet<String> = HashSet::new();
        let downloadable = |_: &str| true;

        let result = resolve_first_fonts(&font_strings, &available, downloadable);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1, FirstFontStatus::Generic);
    }

    #[test]
    fn test_resolve_first_font_available() {
        // "Arial, sans-serif" → first entry is Arial, which is available
        let font_strings = vec!["Arial, sans-serif".to_string()];
        let available: HashSet<String> = ["Arial".to_string()].into();
        let downloadable = |_: &str| false;

        let result = resolve_first_fonts(&font_strings, &available, downloadable);

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].1,
            FirstFontStatus::Available {
                name: "Arial".into()
            }
        );
    }

    #[test]
    fn test_resolve_first_font_downloadable() {
        // "Roboto, sans-serif" → first entry is Roboto, downloadable
        let font_strings = vec!["Roboto, sans-serif".to_string()];
        let available: HashSet<String> = HashSet::new();
        let downloadable = |name: &str| name == "Roboto";

        let result = resolve_first_fonts(&font_strings, &available, downloadable);

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].1,
            FirstFontStatus::NeedsDownload {
                name: "Roboto".into()
            }
        );
    }

    #[test]
    fn test_resolve_first_font_unavailable() {
        // "Benton Gothic, Roboto, sans-serif"
        // First entry is Benton Gothic: not available, not downloadable → Unavailable
        // Roboto (second in chain) is NOT considered.
        let font_strings = vec!["Benton Gothic, Roboto, sans-serif".to_string()];
        let available: HashSet<String> = HashSet::new();
        let downloadable = |name: &str| name == "Roboto";

        let result = resolve_first_fonts(&font_strings, &available, downloadable);

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].1,
            FirstFontStatus::Unavailable {
                name: "Benton Gothic".into()
            }
        );
    }

    #[test]
    fn test_resolve_first_font_case_insensitive_available() {
        // fontdb might report "arial" but the spec has "Arial"
        let font_strings = vec!["Arial, sans-serif".to_string()];
        let available: HashSet<String> = ["arial".to_string()].into();
        let downloadable = |_: &str| true;

        let result = resolve_first_fonts(&font_strings, &available, downloadable);

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].1,
            FirstFontStatus::Available {
                name: "Arial".into()
            }
        );
    }

    #[test]
    fn test_resolve_deduplicates() {
        // Same CSS string appears twice — only one result entry
        let font_strings = vec![
            "Roboto, sans-serif".to_string(),
            "Roboto, sans-serif".to_string(),
        ];
        let available: HashSet<String> = HashSet::new();
        let downloadable = |name: &str| name == "Roboto";

        let result = resolve_first_fonts(&font_strings, &available, downloadable);

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].1,
            FirstFontStatus::NeedsDownload {
                name: "Roboto".into()
            }
        );
    }

    #[test]
    fn test_resolve_multiple_different_fonts() {
        let font_strings = vec![
            "Inter".to_string(),
            "Playfair Display, Georgia, serif".to_string(),
            "Fira Code, Courier New, monospace".to_string(),
        ];
        let available: HashSet<String> = HashSet::new();
        let downloadable = |name: &str| matches!(name, "Inter" | "Playfair Display" | "Fira Code");

        let result = resolve_first_fonts(&font_strings, &available, downloadable);

        assert_eq!(result.len(), 3);
        assert_eq!(
            result[0].1,
            FirstFontStatus::NeedsDownload {
                name: "Inter".into()
            }
        );
        assert_eq!(
            result[1].1,
            FirstFontStatus::NeedsDownload {
                name: "Playfair Display".into()
            }
        );
        assert_eq!(
            result[2].1,
            FirstFontStatus::NeedsDownload {
                name: "Fira Code".into()
            }
        );
    }

    #[test]
    fn test_resolve_empty_input() {
        let font_strings: Vec<String> = vec![];
        let available: HashSet<String> = HashSet::new();
        let downloadable = |_: &str| true;

        let result = resolve_first_fonts(&font_strings, &available, downloadable);

        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_generic_case_insensitive() {
        // "Sans-Serif" (title-case) should be classified as Generic
        let entries = parse_css_font_family("Sans-Serif");
        assert_eq!(entries, vec![FontFamilyEntry::Generic("Sans-Serif".into())]);
    }

    #[test]
    fn test_parse_generic_uppercase() {
        let entries = parse_css_font_family("MONOSPACE");
        assert_eq!(entries, vec![FontFamilyEntry::Generic("MONOSPACE".into())]);
    }

    #[test]
    fn test_extract_config_mark_font() {
        let spec = json!({
            "config": {
                "mark": { "font": "Mark Default Font" }
            }
        });
        let fonts = extract_fonts_from_vega(&spec);
        assert!(fonts.contains("Mark Default Font"));
    }

    #[test]
    fn test_extract_axis_encode_enter_state() {
        let spec = json!({
            "axes": [{
                "encode": {
                    "labels": {
                        "enter": {
                            "font": { "value": "Enter Font" }
                        }
                    }
                }
            }]
        });
        let fonts = extract_fonts_from_vega(&spec);
        assert!(fonts.contains("Enter Font"));
    }

    #[test]
    fn test_extract_legend_encode_hover_state() {
        let spec = json!({
            "legends": [{
                "encode": {
                    "title": {
                        "hover": {
                            "font": { "value": "Hover Font" }
                        }
                    }
                }
            }]
        });
        let fonts = extract_fonts_from_vega(&spec);
        assert!(fonts.contains("Hover Font"));
    }

    #[test]
    fn test_extract_title_encode_enter_state() {
        let spec = json!({
            "title": {
                "text": "Chart",
                "encode": {
                    "subtitle": {
                        "enter": {
                            "font": { "value": "Subtitle Enter Font" }
                        }
                    }
                }
            }
        });
        let fonts = extract_fonts_from_vega(&spec);
        assert!(fonts.contains("Subtitle Enter Font"));
    }

    #[test]
    fn test_resolve_wordcloud_font() {
        let spec = json!({
            "data": [{
                "name": "words",
                "transform": [{ "type": "wordcloud", "font": "Pacifico" }]
            }]
        });

        let fonts = extract_fonts_from_vega(&spec);
        let font_strings: Vec<String> = fonts.into_iter().collect();
        let available: HashSet<String> = HashSet::new();
        let downloadable = |name: &str| name == "Pacifico";

        let result = resolve_first_fonts(&font_strings, &available, downloadable);

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].1,
            FirstFontStatus::NeedsDownload {
                name: "Pacifico".into()
            }
        );
    }
}

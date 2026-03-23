use deno_core::error::AnyError;
use roxmltree;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;

/// Where a font's data originates.
///
/// Serializes as a tagged enum: `{"type": "google", "font_id": "roboto"}`
/// or `{"type": "local"}`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum FontSource {
    /// Font was downloaded from Google Fonts.
    Google {
        /// Google Fonts font ID (e.g., "roboto", "playfair-display").
        font_id: String,
    },
    /// Font is already available in fontdb (system font, --font-dir, vendored).
    Local,
}

/// Metadata for a font that should be embedded or linked in HTML output.
#[derive(Debug, Clone)]
pub struct FontForHtml {
    /// The font family name (e.g., "Roboto", "Playfair Display").
    pub family: String,
    /// Where the font data comes from.
    pub source: FontSource,
}

/// A (family, weight, style) key for font embedding.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct FontKey {
    pub family: String,
    pub weight: String,
    pub style: String,
}

/// A weight/style variant of a font, with optional embedded @font-face CSS.
#[derive(Debug, Clone, Serialize)]
pub struct FontVariant {
    /// CSS font-weight (e.g. "400", "700").
    pub weight: String,
    /// CSS font-style ("normal" or "italic").
    pub style: String,
    /// `@font-face` CSS block with embedded base64 WOFF2 data, or `None` if
    /// font-face generation was not requested or subsetting failed.
    pub font_face: Option<String>,
}

/// Structured font metadata returned by `vega_fonts` / `vegalite_fonts`.
#[derive(Debug, Clone, Serialize)]
pub struct FontInfo {
    /// Font family name (e.g. "Roboto").
    pub name: String,
    /// Where the font originates.
    pub source: FontSource,
    /// Weight/style variants used by the chart.
    pub variants: Vec<FontVariant>,
    /// Google Fonts CSS2 API stylesheet URL, or `None` for local fonts.
    pub url: Option<String>,
    /// HTML `<link rel="stylesheet">` tag, or `None` for local fonts.
    pub link_tag: Option<String>,
    /// CSS `@import url(...)` rule, or `None` for local fonts.
    pub import_rule: Option<String>,
}

/// Walk a rendered Vega scenegraph and extract unique characters per
/// (font-family, weight, style).
///
/// The scenegraph must be the `"scenegraph"` value returned by
/// `vegaToScenegraph` — the root mark node with `marktype`, `items`, etc.
pub fn extract_text_by_font(scenegraph: &Value) -> HashMap<FontKey, BTreeSet<char>> {
    let mut result: HashMap<FontKey, BTreeSet<char>> = HashMap::new();
    walk_scenegraph_mark(scenegraph, &mut result);
    result
}

fn normalize_weight(v: Option<&Value>) -> String {
    match v {
        None => "400".to_string(),
        Some(Value::String(s)) => match s.as_str() {
            "normal" => "400".to_string(),
            "bold" => "700".to_string(),
            "bolder" => "700".to_string(),
            "lighter" => "100".to_string(),
            other => other
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite() && *n > 0.0)
                .map(|n| format!("{}", n as i32))
                .unwrap_or_else(|| "400".to_string()),
        },
        Some(Value::Number(n)) => n
            .as_f64()
            .filter(|f| f.is_finite() && *f > 0.0)
            .map(|f| format!("{}", f as i32))
            .unwrap_or_else(|| "400".to_string()),
        _ => "400".to_string(),
    }
}

fn normalize_style(v: Option<&Value>) -> String {
    match v {
        None => "normal".to_string(),
        Some(Value::String(s)) => {
            let lower = s.trim().to_lowercase();
            if lower == "italic" || lower == "oblique" {
                "italic".to_string()
            } else {
                "normal".to_string()
            }
        }
        _ => "normal".to_string(),
    }
}

/// Convert a scenegraph text item's `text` field to a `String`.
///
/// Mirrors the JS `String(text)` coercion: scalars are stringified directly,
/// arrays are joined with commas (Vega uses arrays for multiline text).
fn text_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Array(arr) => {
            let joined: String = arr
                .iter()
                .map(|elem| match elem {
                    Value::String(s) => s.as_str().to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>()
                .join(",");
            if joined.is_empty() {
                None
            } else {
                Some(joined)
            }
        }
        _ => None,
    }
}

fn walk_scenegraph_mark(node: &Value, result: &mut HashMap<FontKey, BTreeSet<char>>) {
    let marktype = node.get("marktype").and_then(|v| v.as_str());

    match marktype {
        Some("text") => {
            if let Some(items) = node.get("items").and_then(|v| v.as_array()) {
                for item in items {
                    let text = match item.get("text").and_then(text_to_string) {
                        Some(s) => s,
                        None => continue,
                    };

                    let font_str = item
                        .get("font")
                        .and_then(|v| v.as_str())
                        .unwrap_or("sans-serif")
                        .trim();

                    let families = parse_css_font_family(font_str);
                    let family = match families.first() {
                        Some(FontFamilyEntry::Named(name)) => name.clone(),
                        _ => continue,
                    };

                    let weight = normalize_weight(item.get("fontWeight"));
                    let style = normalize_style(item.get("fontStyle"));

                    let key = FontKey {
                        family,
                        weight,
                        style,
                    };
                    let chars = result.entry(key).or_default();
                    for ch in text.chars() {
                        chars.insert(ch);
                    }
                }
            }
        }
        Some("group") => {
            if let Some(items) = node.get("items").and_then(|v| v.as_array()) {
                for group_item in items {
                    if let Some(child_marks) = group_item.get("items").and_then(|v| v.as_array()) {
                        for child_mark in child_marks {
                            walk_scenegraph_mark(child_mark, result);
                        }
                    }
                }
            }
        }
        _ => {}
    }
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

/// Extract all font-family CSS strings from an SVG document.
///
/// Parses the SVG as XML and collects `font-family` values from:
/// - Direct `font-family` attributes on any element
/// - `font-family:` declarations inside inline `style` attributes
/// - `font-family:` declarations inside `<style>` element text
///
/// Returns an empty set on parse error (usvg will provide a better error later).
pub fn extract_fonts_from_svg(svg: &str) -> HashSet<String> {
    let xml_opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };

    let doc = match roxmltree::Document::parse_with_options(svg, xml_opt) {
        Ok(doc) => doc,
        Err(_) => return HashSet::new(),
    };

    let mut fonts = HashSet::new();

    for node in doc.descendants() {
        if node.is_element() {
            // Direct font-family attribute
            if let Some(ff) = node.attribute("font-family") {
                fonts.insert(ff.to_string());
            }

            // Inline style attribute: tokenize declarations with simplecss
            if let Some(style) = node.attribute("style") {
                collect_font_family_from_declarations(
                    simplecss::DeclarationTokenizer::from(style),
                    &mut fonts,
                );
            }
        }

        // <style> element text content: parse as a full stylesheet
        if node.is_element() && node.tag_name().name() == "style" {
            if let Some(text_node) = node.first_child().filter(|c| c.is_text()) {
                if let Some(text) = text_node.text() {
                    let sheet = simplecss::StyleSheet::parse(text);
                    for rule in &sheet.rules {
                        collect_font_family_from_declarations(
                            rule.declarations.iter().copied(),
                            &mut fonts,
                        );
                    }
                }
            }
        }
    }

    fonts
}

/// Collect `font-family` values from an iterator of CSS declarations.
fn collect_font_family_from_declarations<'a>(
    declarations: impl Iterator<Item = simplecss::Declaration<'a>>,
    fonts: &mut HashSet<String>,
) {
    for decl in declarations {
        if decl.name == "font-family" {
            let value = decl.value.trim();
            if !value.is_empty() {
                fonts.insert(value.to_string());
            }
        }
    }
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
    /// First entry is downloadable from Google Fonts.
    NeedsDownload { name: String },
    /// First entry is not on the system and not on Google Fonts.
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

/// A non-data-URI `<image>` href found in the SVG, with the byte range of
/// the attribute value so it can be replaced in-place.
#[derive(Debug, Clone)]
pub struct ImageRef {
    /// The href string (URL or file path).
    pub href: String,
    /// Byte range of the href attribute *value* in the original SVG string.
    pub value_range: Range<usize>,
}

/// Result of analyzing a rendered SVG for font embedding and image inlining.
#[derive(Debug)]
pub struct SvgAnalysis {
    /// Characters used per (family, weight, style) — extracted from `<text>` direct attributes.
    pub chars_by_key: HashMap<FontKey, BTreeSet<char>>,
    /// Unique font families found (for classification).
    pub families: HashSet<String>,
    /// Byte offset where `<defs><style>` should be inserted (before first child element).
    pub insert_pos: usize,
    /// Non-data-URI `<image>` hrefs with their attribute value byte ranges.
    pub image_refs: Vec<ImageRef>,
}

/// Normalize a font-weight string value to a numeric string.
fn normalize_weight_str(s: &str) -> String {
    match s.trim() {
        "normal" | "" => "400".to_string(),
        "bold" => "700".to_string(),
        "bolder" => "700".to_string(),
        "lighter" => "100".to_string(),
        other => other
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && *n > 0.0)
            .map(|n| format!("{}", n as i32))
            .unwrap_or_else(|| "400".to_string()),
    }
}

/// Normalize a font-style string value.
fn normalize_style_str(s: &str) -> String {
    let lower = s.trim().to_lowercase();
    if lower == "italic" || lower == "oblique" {
        "italic".to_string()
    } else {
        "normal".to_string()
    }
}

/// Parse an SVG once and extract all information needed for font embedding
/// and image inlining.
///
/// Extracts:
/// - Characters per (family, weight, style) from `<text>` element direct attributes
///   (Vega always emits font-family/weight/style as explicit attributes)
/// - Insertion point byte offset (before first child element of root `<svg>`)
/// - Non-data-URI `<image>` hrefs with byte ranges for replacement
pub fn analyze_svg(svg: &str) -> Result<SvgAnalysis, AnyError> {
    let xml_opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };

    let doc = roxmltree::Document::parse_with_options(svg, xml_opt)
        .map_err(|e| deno_core::anyhow::anyhow!("Failed to parse SVG: {e}"))?;

    let root = doc.root_element();

    // Insertion point: before the first child element of <svg>
    let insert_pos = root
        .children()
        .find(|n| n.is_element())
        .map(|n| n.range().start)
        .unwrap_or_else(|| {
            // No child elements — insert before closing </svg>
            let r = root.range();
            // Find the start of </svg> by searching backwards
            svg[..r.end]
                .rfind("</svg>")
                .unwrap_or(r.end.saturating_sub("</svg>".len()))
        });

    let mut chars_by_key: HashMap<FontKey, BTreeSet<char>> = HashMap::new();
    let mut families: HashSet<String> = HashSet::new();
    let mut image_refs: Vec<ImageRef> = Vec::new();

    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }

        let tag = node.tag_name().name();

        if tag == "text" {
            // Extract font info from direct attributes
            let font_family_str = node.attribute("font-family").unwrap_or("sans-serif");
            let parsed = parse_css_font_family(font_family_str);
            let family = match parsed.first() {
                Some(FontFamilyEntry::Named(name)) => name.clone(),
                _ => continue,
            };

            let weight = normalize_weight_str(node.attribute("font-weight").unwrap_or(""));
            let style = normalize_style_str(node.attribute("font-style").unwrap_or(""));

            families.insert(family.clone());

            // Collect text content from this element and all text descendants
            let text_content = collect_text_content(&node);
            if !text_content.is_empty() {
                let key = FontKey {
                    family,
                    weight,
                    style,
                };
                let chars = chars_by_key.entry(key).or_default();
                for ch in text_content.chars() {
                    chars.insert(ch);
                }
            }
        } else if tag == "image" {
            // Extract image href for potential inlining
            // Try href first, then xlink:href
            let attr = node
                .attribute_node("href")
                .or_else(|| node.attribute_node(("http://www.w3.org/1999/xlink", "href")));
            let attr = match attr {
                Some(a) => a,
                None => continue,
            };

            let href = attr.value();
            if href.starts_with("data:") {
                continue; // already inlined
            }

            // Compute the byte range of the attribute value from the full attribute range.
            // The attribute range covers: name='value' or name="value"
            // We search for the first quote after '=' within the attribute range.
            let attr_range = attr.range();
            let attr_str = &svg[attr_range.clone()];
            let eq_pos = match attr_str.find('=') {
                Some(p) => p,
                None => continue,
            };
            let after_eq = &attr_str[eq_pos + 1..];
            let quote_char = after_eq.chars().find(|c| *c == '"' || *c == '\'');
            let quote_char = match quote_char {
                Some(q) => q,
                None => continue,
            };
            let value_start_in_attr = eq_pos + 1 + after_eq.find(quote_char).unwrap() + 1;
            let value_end_in_attr = attr_str.len() - 1; // before closing quote
            let value_range =
                (attr_range.start + value_start_in_attr)..(attr_range.start + value_end_in_attr);

            image_refs.push(ImageRef {
                href: href.to_string(),
                value_range,
            });
        }
    }

    Ok(SvgAnalysis {
        chars_by_key,
        families,
        insert_pos,
        image_refs,
    })
}

/// Collect all text content from a node and its text-node descendants.
fn collect_text_content(node: &roxmltree::Node) -> String {
    let mut text = String::new();
    for child in node.descendants() {
        if child.is_text() {
            if let Some(t) = child.text() {
                text.push_str(t);
            }
        }
    }
    text
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

    #[test]
    fn test_svg_font_family_attribute() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <text font-family="Roboto, sans-serif">Hello</text>
        </svg>"#;
        let fonts = extract_fonts_from_svg(svg);
        assert!(fonts.contains("Roboto, sans-serif"));
        assert_eq!(fonts.len(), 1);
    }

    #[test]
    fn test_svg_inline_style() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <text style="font-family: Playfair Display; font-size: 14px;">Hello</text>
        </svg>"#;
        let fonts = extract_fonts_from_svg(svg);
        assert!(fonts.contains("Playfair Display"));
        assert_eq!(fonts.len(), 1);
    }

    #[test]
    fn test_svg_style_block() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <style>
                .title { font-family: Montserrat, sans-serif; font-size: 16px; }
                .label { font-family: Fira Code; }
            </style>
            <text class="title">Title</text>
        </svg>"#;
        let fonts = extract_fonts_from_svg(svg);
        assert!(fonts.contains("Montserrat, sans-serif"));
        assert!(fonts.contains("Fira Code"));
        assert_eq!(fonts.len(), 2);
    }

    #[test]
    fn test_svg_deduplication() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <text font-family="Roboto">One</text>
            <text font-family="Roboto">Two</text>
        </svg>"#;
        let fonts = extract_fonts_from_svg(svg);
        assert!(fonts.contains("Roboto"));
        assert_eq!(fonts.len(), 1);
    }

    #[test]
    fn test_svg_empty() {
        let fonts = extract_fonts_from_svg("");
        assert!(fonts.is_empty());
    }

    #[test]
    fn test_svg_invalid() {
        let fonts = extract_fonts_from_svg("<not valid xml");
        assert!(fonts.is_empty());
    }

    #[test]
    fn test_svg_no_fonts() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <rect width="100" height="100" fill="red"/>
        </svg>"#;
        let fonts = extract_fonts_from_svg(svg);
        assert!(fonts.is_empty());
    }

    #[test]
    fn test_svg_mixed_sources() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <style>.cls { font-family: Lato; }</style>
            <text font-family="Roboto">Attr</text>
            <text style="font-family: Open Sans;">Inline</text>
        </svg>"#;
        let fonts = extract_fonts_from_svg(svg);
        assert!(fonts.contains("Lato"));
        assert!(fonts.contains("Roboto"));
        assert!(fonts.contains("Open Sans"));
        assert_eq!(fonts.len(), 3);
    }

    // -----------------------------------------------------------------------
    // analyze_svg tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_analyze_svg_basic_text() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="Roboto" font-weight="bold">Hello</text></svg>"#;
        let result = analyze_svg(svg).unwrap();
        assert!(result.families.contains("Roboto"));
        let key = FontKey {
            family: "Roboto".into(),
            weight: "700".into(),
            style: "normal".into(),
        };
        let chars = result.chars_by_key.get(&key).unwrap();
        assert!(chars.contains(&'H'));
        assert!(chars.contains(&'o'));
    }

    #[test]
    fn test_analyze_svg_font_weight_normalization() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="Arial" font-weight="bold">X</text></svg>"#;
        let result = analyze_svg(svg).unwrap();
        let key = FontKey {
            family: "Arial".into(),
            weight: "700".into(),
            style: "normal".into(),
        };
        assert!(result.chars_by_key.contains_key(&key));
    }

    #[test]
    fn test_analyze_svg_default_weight_style() {
        let svg =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="Arial">X</text></svg>"#;
        let result = analyze_svg(svg).unwrap();
        let key = FontKey {
            family: "Arial".into(),
            weight: "400".into(),
            style: "normal".into(),
        };
        assert!(result.chars_by_key.contains_key(&key));
    }

    #[test]
    fn test_analyze_svg_italic_style() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="Arial" font-style="italic">X</text></svg>"#;
        let result = analyze_svg(svg).unwrap();
        let key = FontKey {
            family: "Arial".into(),
            weight: "400".into(),
            style: "italic".into(),
        };
        assert!(result.chars_by_key.contains_key(&key));
    }

    #[test]
    fn test_analyze_svg_insert_position() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="100"/></svg>"#;
        let result = analyze_svg(svg).unwrap();
        // Insert position should be at the start of <rect>
        assert_eq!(&svg[result.insert_pos..result.insert_pos + 5], "<rect");
    }

    #[test]
    fn test_analyze_svg_image_http() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="https://example.com/img.png"/></svg>"#;
        let result = analyze_svg(svg).unwrap();
        assert_eq!(result.image_refs.len(), 1);
        assert_eq!(result.image_refs[0].href, "https://example.com/img.png");
        // Verify the value range points to the correct bytes
        assert_eq!(
            &svg[result.image_refs[0].value_range.clone()],
            "https://example.com/img.png"
        );
    }

    #[test]
    fn test_analyze_svg_image_data_uri_skipped() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/png;base64,ABC"/></svg>"#;
        let result = analyze_svg(svg).unwrap();
        assert!(result.image_refs.is_empty());
    }

    #[test]
    fn test_analyze_svg_image_local_path() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="local.png"/></svg>"#;
        let result = analyze_svg(svg).unwrap();
        assert_eq!(result.image_refs.len(), 1);
        assert_eq!(result.image_refs[0].href, "local.png");
    }

    #[test]
    fn test_analyze_svg_generic_font_skipped() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="sans-serif">X</text></svg>"#;
        let result = analyze_svg(svg).unwrap();
        assert!(result.chars_by_key.is_empty());
        assert!(result.families.is_empty());
    }

    #[test]
    fn test_analyze_svg_no_text_content() {
        let svg =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="Arial"></text></svg>"#;
        let result = analyze_svg(svg).unwrap();
        assert!(result.chars_by_key.is_empty());
    }
}

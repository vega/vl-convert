use serde_json::Value;
use std::collections::HashSet;

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        // Signal-based font in wordcloud -> not a string, skipped.
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
        use super::super::types::{resolve_first_fonts, FirstFontStatus};

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

use std::collections::HashSet;
use vl_convert_rs::extract::{
    extract_fonts_from_vega, parse_css_font_family, resolve_first_fonts, FirstFontStatus,
    FontFamilyEntry,
};

#[test]
fn test_css_parser_edge_cases() {
    // Empty string
    assert!(parse_css_font_family("").is_empty());

    // Only whitespace
    assert!(parse_css_font_family("   ").is_empty());

    // Only commas
    assert!(parse_css_font_family(",,,").is_empty());

    // Quoted font with commas inside — parser splits on commas naively,
    // which is acceptable since real font families never contain commas.
    // This test documents the behavior rather than asserting ideal parsing.
    let entries = parse_css_font_family("'Font, With Comma', serif");
    assert_eq!(entries.len(), 3); // 'Font | With Comma' | serif

    // Double-quoted
    let entries = parse_css_font_family(r#""Playfair Display", sans-serif"#);
    assert_eq!(entries.len(), 2);
    assert!(matches!(&entries[0], FontFamilyEntry::Named(n) if n == "Playfair Display"));
    assert!(matches!(&entries[1], FontFamilyEntry::Generic(g) if g == "sans-serif"));

    // wf_standard-font (real-world non-standard name)
    let entries = parse_css_font_family("wf_standard-font, sans-serif");
    assert_eq!(entries.len(), 2);
    assert!(matches!(&entries[0], FontFamilyEntry::Named(n) if n == "wf_standard-font"));
}

#[test]
fn test_extraction_from_vega_fixture() {
    // A comprehensive Vega spec with fonts in multiple locations
    let spec: serde_json::Value = serde_json::json!({
        "config": {
            "title": {"font": "Playfair Display"},
            "axis": {"labelFont": "Open Sans", "titleFont": "Lato"},
            "legend": {"labelFont": "Merriweather"},
            "text": {"font": "Roboto"}
        },
        "marks": [
            {
                "type": "text",
                "encode": {
                    "enter": {
                        "font": {"value": "Source Sans Pro"}
                    }
                }
            },
            {
                "type": "group",
                "marks": [
                    {
                        "type": "text",
                        "encode": {
                            "update": {
                                "font": {"value": "Montserrat"}
                            }
                        }
                    }
                ]
            }
        ],
        "axes": [
            {"orient": "bottom", "labelFont": "Inter"}
        ],
        "legends": [
            {"titleFont": "Oswald"}
        ],
        "title": {"text": "My Chart", "font": "Raleway", "subtitleFont": "PT Sans"}
    });

    let fonts = extract_fonts_from_vega(&spec);

    assert!(fonts.contains("Roboto"), "Text mark config font missing");
    assert!(fonts.contains("Playfair Display"), "Title font missing");
    assert!(fonts.contains("Open Sans"), "Axis labelFont missing");
    assert!(fonts.contains("Lato"), "Axis titleFont missing");
    assert!(fonts.contains("Merriweather"), "Legend labelFont missing");
    assert!(fonts.contains("Source Sans Pro"), "Mark enter font missing");
    assert!(
        fonts.contains("Montserrat"),
        "Nested group mark font missing"
    );
    assert!(fonts.contains("Inter"), "Axes labelFont missing");
    assert!(fonts.contains("Oswald"), "Legends titleFont missing");
    assert!(fonts.contains("Raleway"), "Title font missing");
    assert!(fonts.contains("PT Sans"), "Subtitle font missing");
}

#[test]
fn test_extraction_with_css_fallback_chains() {
    // Fonts specified as CSS fallback chains
    let spec: serde_json::Value = serde_json::json!({
        "config": {
            "text": { "font": "Benton Gothic, Roboto, sans-serif" }
        }
    });

    let fonts = extract_fonts_from_vega(&spec);

    // Should contain the raw font string with the full chain
    assert!(fonts.contains("Benton Gothic, Roboto, sans-serif"));
}

#[test]
fn test_resolve_first_font_unavailable() {
    // First font is not available or downloadable → Unavailable
    let font_strings = vec!["Benton Gothic, Roboto, sans-serif".to_string()];
    let available: HashSet<String> = HashSet::new();
    let downloadable = |family: &str| -> bool { family == "Roboto" };

    let result = resolve_first_fonts(&font_strings, &available, downloadable);

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        (
            "Benton Gothic, Roboto, sans-serif".to_string(),
            FirstFontStatus::Unavailable {
                name: "Benton Gothic".to_string()
            }
        )
    );
}

#[test]
fn test_resolve_first_font_available() {
    // First font is locally available → Available
    let font_strings = vec!["Arial, Roboto, sans-serif".to_string()];
    let available: HashSet<String> = HashSet::from(["Arial".to_string()]);
    let downloadable = |_family: &str| -> bool { true };

    let result = resolve_first_fonts(&font_strings, &available, downloadable);

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        (
            "Arial, Roboto, sans-serif".to_string(),
            FirstFontStatus::Available {
                name: "Arial".to_string()
            }
        )
    );
}

#[test]
fn test_resolve_first_font_needs_download() {
    // First font is downloadable → NeedsDownload
    let font_strings = vec!["Roboto, sans-serif".to_string()];
    let available: HashSet<String> = HashSet::new();
    let downloadable = |family: &str| -> bool { family == "Roboto" };

    let result = resolve_first_fonts(&font_strings, &available, downloadable);

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        (
            "Roboto, sans-serif".to_string(),
            FirstFontStatus::NeedsDownload {
                name: "Roboto".to_string()
            }
        )
    );
}

#[test]
fn test_resolve_first_font_generic() {
    // First font is a generic keyword → Generic
    let font_strings = vec!["sans-serif".to_string()];
    let available: HashSet<String> = HashSet::new();
    let downloadable = |_family: &str| -> bool { false };

    let result = resolve_first_fonts(&font_strings, &available, downloadable);

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        ("sans-serif".to_string(), FirstFontStatus::Generic)
    );
}

#[test]
fn test_resolve_deduplicates_font_strings() {
    // Duplicate font strings should be deduplicated
    let font_strings = vec![
        "Roboto, sans-serif".to_string(),
        "Roboto, sans-serif".to_string(),
        "Open Sans, serif".to_string(),
    ];
    let available: HashSet<String> = HashSet::new();
    let downloadable = |family: &str| -> bool { family == "Roboto" || family == "Open Sans" };

    let result = resolve_first_fonts(&font_strings, &available, downloadable);

    assert_eq!(result.len(), 2);
    assert_eq!(
        result[0],
        (
            "Roboto, sans-serif".to_string(),
            FirstFontStatus::NeedsDownload {
                name: "Roboto".to_string()
            }
        )
    );
    assert_eq!(
        result[1],
        (
            "Open Sans, serif".to_string(),
            FirstFontStatus::NeedsDownload {
                name: "Open Sans".to_string()
            }
        )
    );
}

#[test]
fn test_resolve_nothing_downloadable() {
    // First font is not available or downloadable → Unavailable
    let font_strings = vec!["Benton Gothic, Proprietary Font, sans-serif".to_string()];
    let available: HashSet<String> = HashSet::new();
    let downloadable = |_family: &str| -> bool { false };

    let result = resolve_first_fonts(&font_strings, &available, downloadable);

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        (
            "Benton Gothic, Proprietary Font, sans-serif".to_string(),
            FirstFontStatus::Unavailable {
                name: "Benton Gothic".to_string()
            }
        )
    );
}

#[test]
fn test_extraction_wordcloud_transform() {
    let spec: serde_json::Value = serde_json::json!({
        "data": [
            {
                "name": "table",
                "transform": [
                    {
                        "type": "wordcloud",
                        "font": "Lobster"
                    }
                ]
            }
        ]
    });

    let fonts = extract_fonts_from_vega(&spec);
    assert!(
        fonts.contains("Lobster"),
        "Wordcloud font should be extracted"
    );
}

use serde::Serialize;
use std::collections::HashSet;

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

/// A font classified by source (Google or Local) for embedding or linking in SVG/HTML output.
#[derive(Debug, Clone)]
pub struct ClassifiedFont {
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

// CSS generic family keywords per CSS Fonts Module Level 4:
// https://www.w3.org/TR/css-fonts-4/#generic-font-families
pub(crate) const GENERIC_FAMILIES: &[&str] = &[
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

/// Classification of the first font in a CSS `font-family` string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirstFontStatus {
    /// First entry is a CSS generic keyword (serif, sans-serif, etc.) --
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
/// 1. **Generic** keyword (serif, sans-serif, etc.) -> [`FirstFontStatus::Generic`]
/// 2. **Named** family already in `available` -> [`FirstFontStatus::Available`]
/// 3. **Named** family for which `downloadable(family)` returns `true` ->
///    [`FirstFontStatus::NeedsDownload`]
/// 4. **Named** family that is neither available nor downloadable ->
///    [`FirstFontStatus::Unavailable`]
///
/// Only the first entry matters -- the rest of the fallback chain is ignored.
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
    fn test_resolve_first_font_generic() {
        // "serif" -> first entry is generic -> Generic
        let font_strings = vec!["serif".to_string()];
        let available: HashSet<String> = HashSet::new();
        let downloadable = |_: &str| true;

        let result = resolve_first_fonts(&font_strings, &available, downloadable);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1, FirstFontStatus::Generic);
    }

    #[test]
    fn test_resolve_first_font_available() {
        // "Arial, sans-serif" -> first entry is Arial, which is available
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
        // "Roboto, sans-serif" -> first entry is Roboto, downloadable
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
        // First entry is Benton Gothic: not available, not downloadable -> Unavailable
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
        // Same CSS string appears twice -- only one result entry
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
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Marker file written to each font directory after successful download.
pub const MARKER_FILENAME: &str = ".fontsource.json";

/// CSS font style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontStyle {
    Normal,
    Italic,
}

impl FontStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            FontStyle::Normal => "normal",
            FontStyle::Italic => "italic",
        }
    }
}

/// A request for a specific weight + style combination.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VariantRequest {
    pub weight: u16,
    pub style: FontStyle,
}

/// Default variants to download: 400/700 normal, 400/700 italic.
pub fn default_variants() -> Vec<VariantRequest> {
    vec![
        VariantRequest {
            weight: 400,
            style: FontStyle::Normal,
        },
        VariantRequest {
            weight: 700,
            style: FontStyle::Normal,
        },
        VariantRequest {
            weight: 400,
            style: FontStyle::Italic,
        },
        VariantRequest {
            weight: 700,
            style: FontStyle::Italic,
        },
    ]
}

/// A cached TTF file with parsed metadata from its filename.
#[derive(Debug, Clone)]
pub struct CachedFontFile {
    pub subset: String,
    pub weight: u16,
    pub style: FontStyle,
}

/// Convert a font family name to a Fontsource font ID.
///
/// Rules:
/// 1. Trim whitespace
/// 2. Lowercase
/// 3. Replace spaces with hyphens
///
/// Returns `None` if the resulting ID doesn't match `^[a-z0-9][a-z0-9_-]*$`.
pub fn family_to_id(family: &str) -> Option<String> {
    let id = family.trim().to_lowercase().replace(' ', "-");
    if is_valid_font_id(&id) {
        Some(id)
    } else {
        None
    }
}

/// Check if a string is a valid Fontsource font ID.
pub fn is_valid_font_id(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    let bytes = id.as_bytes();
    // First char must be lowercase alphanumeric
    if !(bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit()) {
        return false;
    }
    // Remaining chars must be lowercase alphanumeric, hyphen, or underscore
    bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Parse a cached TTF filename into its components.
///
/// Filenames follow the pattern: `{subset}-{weight}-{style}.ttf`
/// Since subsets can contain hyphens (e.g. `latin-ext`), we split from the right.
pub fn parse_cached_filename(filename: &str) -> Option<CachedFontFile> {
    let stem = filename.strip_suffix(".ttf")?;
    let parts: Vec<&str> = stem.rsplitn(3, '-').collect();
    if parts.len() < 3 {
        return None;
    }
    // rsplitn gives [style, weight, subset] (reversed)
    let style_str = parts[0];
    let weight_str = parts[1];
    let subset = parts[2];

    let style = match style_str {
        "normal" => FontStyle::Normal,
        "italic" => FontStyle::Italic,
        _ => return None,
    };

    let weight: u16 = weight_str.parse().ok()?;

    Some(CachedFontFile {
        subset: subset.to_string(),
        weight,
        style,
    })
}

/// Top-level response from `GET /v1/fonts/{id}`
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontsourceFont {
    pub id: String,
    pub family: String,
    pub subsets: Vec<String>,
    pub weights: Vec<u16>,
    pub styles: Vec<String>,
    pub version: String,
    /// `"google"` or `"other"` — from the Fontsource API `type` field.
    #[serde(rename = "type")]
    pub font_type: String,
    /// weight (string) -> style -> subset -> urls
    pub variants: HashMap<String, HashMap<String, HashMap<String, FontsourceUrls>>>,
}

#[derive(Debug, Deserialize)]
pub struct FontsourceUrls {
    pub url: FontsourceFileUrls,
}

#[derive(Debug, Deserialize)]
pub struct FontsourceFileUrls {
    pub ttf: Option<String>,
    pub woff2: Option<String>,
    pub woff: Option<String>,
}

/// Marker data written to `.fontsource.json` in each font directory.
#[derive(Debug, Serialize, Deserialize)]
pub struct FontsourceMarker {
    pub id: String,
    pub family: String,
    pub version: String,
    pub fetched_at: u64, // Unix timestamp
    /// `"google"` or `"other"`. `None` for markers written before this field existed.
    #[serde(default)]
    pub font_type: Option<String>,
}

/// Outcome of a fetch or refetch operation.
#[derive(Debug)]
pub struct FetchOutcome {
    /// Path to the font directory.
    pub path: std::path::PathBuf,
    /// Normalized font ID.
    pub font_id: String,
    /// `true` if a fresh download occurred, `false` if cache hit.
    pub downloaded: bool,
    /// `"google"` or `"other"`. `None` for old cached markers without this field.
    pub font_type: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_family_to_id() {
        assert_eq!(family_to_id("Roboto"), Some("roboto".to_string()));
        assert_eq!(
            family_to_id("Playfair Display"),
            Some("playfair-display".to_string())
        );
        assert_eq!(
            family_to_id("IBM Plex Sans"),
            Some("ibm-plex-sans".to_string())
        );
        assert_eq!(
            family_to_id("Noto Sans JP"),
            Some("noto-sans-jp".to_string())
        );
        assert_eq!(family_to_id("  Roboto  "), Some("roboto".to_string()));
        // Invalid: starts with hyphen
        assert_eq!(family_to_id("-invalid"), None);
        // Invalid: empty
        assert_eq!(family_to_id(""), None);
        assert_eq!(family_to_id("   "), None);
    }

    #[test]
    fn test_is_valid_font_id() {
        assert!(is_valid_font_id("roboto"));
        assert!(is_valid_font_id("playfair-display"));
        assert!(is_valid_font_id("ibm-plex-sans"));
        assert!(is_valid_font_id("wf_standard-font"));
        assert!(is_valid_font_id("123abc"));
        assert!(!is_valid_font_id(""));
        assert!(!is_valid_font_id("-starts-with-hyphen"));
        assert!(!is_valid_font_id("_starts-with-underscore"));
        assert!(!is_valid_font_id("has spaces"));
        assert!(!is_valid_font_id("HAS-CAPS"));
    }

    #[test]
    fn test_parse_cached_filename() {
        let f = parse_cached_filename("latin-400-normal.ttf").unwrap();
        assert_eq!(f.subset, "latin");
        assert_eq!(f.weight, 400);
        assert_eq!(f.style, FontStyle::Normal);

        let f = parse_cached_filename("latin-ext-700-italic.ttf").unwrap();
        assert_eq!(f.subset, "latin-ext");
        assert_eq!(f.weight, 700);
        assert_eq!(f.style, FontStyle::Italic);

        let f = parse_cached_filename("cyrillic-ext-400-normal.ttf").unwrap();
        assert_eq!(f.subset, "cyrillic-ext");
        assert_eq!(f.weight, 400);
        assert_eq!(f.style, FontStyle::Normal);

        // Invalid cases
        assert!(parse_cached_filename("not-a-ttf.woff2").is_none());
        assert!(parse_cached_filename("400-normal.ttf").is_none());
        assert!(parse_cached_filename("latin-400-bold.ttf").is_none());
    }

    #[test]
    fn test_fontsource_font_deserializes_type_field() {
        let json = r#"{
            "id": "roboto",
            "family": "Roboto",
            "subsets": ["latin"],
            "weights": [400, 700],
            "styles": ["normal", "italic"],
            "version": "v30",
            "type": "google",
            "variants": {}
        }"#;
        let font: FontsourceFont = serde_json::from_str(json).unwrap();
        assert_eq!(font.font_type, "google");
    }

    #[test]
    fn test_fontsource_font_deserializes_other_type() {
        let json = r#"{
            "id": "custom-font",
            "family": "Custom Font",
            "subsets": ["latin"],
            "weights": [400],
            "styles": ["normal"],
            "version": "v1",
            "type": "other",
            "variants": {}
        }"#;
        let font: FontsourceFont = serde_json::from_str(json).unwrap();
        assert_eq!(font.font_type, "other");
    }

    #[test]
    fn test_fontsource_marker_backward_compat() {
        // Old markers don't have font_type — should deserialize with None
        let json = r#"{
            "id": "roboto",
            "family": "Roboto",
            "version": "v30",
            "fetched_at": 1700000000
        }"#;
        let marker: FontsourceMarker = serde_json::from_str(json).unwrap();
        assert_eq!(marker.font_type, None);
    }

    #[test]
    fn test_fontsource_marker_with_font_type() {
        let json = r#"{
            "id": "roboto",
            "family": "Roboto",
            "version": "v30",
            "fetched_at": 1700000000,
            "font_type": "google"
        }"#;
        let marker: FontsourceMarker = serde_json::from_str(json).unwrap();
        assert_eq!(marker.font_type, Some("google".to_string()));
    }

    #[test]
    fn test_default_variants() {
        let variants = default_variants();
        assert_eq!(variants.len(), 4);
        assert!(variants.contains(&VariantRequest {
            weight: 400,
            style: FontStyle::Normal
        }));
        assert!(variants.contains(&VariantRequest {
            weight: 700,
            style: FontStyle::Normal
        }));
        assert!(variants.contains(&VariantRequest {
            weight: 400,
            style: FontStyle::Italic
        }));
        assert!(variants.contains(&VariantRequest {
            weight: 700,
            style: FontStyle::Italic
        }));
    }
}

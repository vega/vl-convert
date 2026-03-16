use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

/// CSS font style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl FromStr for FontStyle {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "normal" => Ok(FontStyle::Normal),
            "italic" => Ok(FontStyle::Italic),
            _ => Err(()),
        }
    }
}

/// A request for a specific weight + style combination.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VariantRequest {
    pub weight: u16,
    pub style: FontStyle,
}

impl fmt::Display for VariantRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.weight, self.style.as_str())
    }
}

/// Find the closest available variant to a requested (weight, style).
///
/// Tries: exact match → closest weight with matching style → closest weight
/// any style. Returns the index into `available`, or `None` if `available`
/// is empty.
pub fn find_closest_variant(
    requested: &VariantRequest,
    available: &[VariantRequest],
) -> Option<usize> {
    // Exact match
    if let Some(i) = available
        .iter()
        .position(|v| v.weight == requested.weight && v.style == requested.style)
    {
        return Some(i);
    }
    // Closest weight with matching style
    let same_style = available
        .iter()
        .enumerate()
        .filter(|(_, v)| v.style == requested.style)
        .min_by_key(|(_, v)| (v.weight as i32 - requested.weight as i32).abs());
    if let Some((i, _)) = same_style {
        return Some(i);
    }
    // Closest weight any style
    available
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| (v.weight as i32 - requested.weight as i32).abs())
        .map(|(i, _)| i)
}

/// Convert a font family name to a normalized font ID.
///
/// Lowercases and replaces spaces with hyphens. The result must match
/// `^[a-z0-9][a-z0-9_-]*$` or `None` is returned.
pub fn family_to_id(family: &str) -> Option<String> {
    let id = family.trim().to_lowercase().replace(' ', "-");
    if is_valid_font_id(&id) {
        Some(id)
    } else {
        None
    }
}

/// Check if a string is a valid font ID.
pub fn is_valid_font_id(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }

    let bytes = id.as_bytes();
    if !(bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit()) {
        return false;
    }

    bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// A font resolved from a CSS2 API response: one TTF URL per weight/style.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedFont {
    pub weight: u16,
    pub style: FontStyle,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct LoadedFontBatch {
    pub font_id: String,
    pub loaded_variants: Vec<VariantRequest>,
    pub ttf_file_count: usize,
    pub font_data: Vec<Arc<Vec<u8>>>,
}

impl LoadedFontBatch {
    pub(crate) fn new(
        font_id: String,
        loaded_variants: Vec<VariantRequest>,
        ttf_file_count: usize,
        font_data: Vec<Arc<Vec<u8>>>,
    ) -> Self {
        Self {
            font_id,
            loaded_variants,
            ttf_file_count,
            font_data,
        }
    }
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
        assert_eq!(family_to_id("  Roboto  "), Some("roboto".to_string()));
        assert_eq!(family_to_id(""), None);
        assert_eq!(family_to_id("-invalid"), None);
    }
}

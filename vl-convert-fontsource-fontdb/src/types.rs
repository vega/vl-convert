use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use tinyvec::TinyVec;

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

/// Convert a font family name to a Fontsource font ID.
///
/// Mirrors Fontsource's [`normalizeKebabCase`][ref] with an extra validation
/// step: the result must match `^[a-z0-9][a-z0-9_-]*$` or `None` is returned.
///
/// [ref]: https://github.com/fontsource/fontsource/blob/9b536498e689d90a8dd77b4638bb60ff7b6756c0/packages/core/src/utils.ts#L4
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
    if !(bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit()) {
        return false;
    }

    bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// weight (string) -> style -> subset -> urls
pub type VariantMap = HashMap<String, HashMap<String, HashMap<String, FontsourceUrls>>>;

/// Top-level response from `GET /v1/fonts/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontsourceFont {
    pub id: String,
    pub family: String,
    pub subsets: Vec<String>,
    pub weights: Vec<u16>,
    pub styles: Vec<String>,
    pub version: String,
    #[serde(rename = "type")]
    pub font_type: String,
    pub variants: VariantMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontsourceUrls {
    pub url: FontsourceFileUrls,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontsourceFileUrls {
    pub ttf: Option<String>,
    pub woff2: Option<String>,
    pub woff: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LoadedFontBatch {
    pub font_id: String,
    pub font_type: Option<String>,
    pub loaded_variants: Vec<VariantRequest>,
    pub ttf_file_count: usize,
    sources: Vec<fontdb::Source>,
}

impl LoadedFontBatch {
    pub(crate) fn new(
        font_id: String,
        font_type: Option<String>,
        loaded_variants: Vec<VariantRequest>,
        ttf_file_count: usize,
        sources: Vec<fontdb::Source>,
    ) -> Self {
        Self {
            font_id,
            font_type,
            loaded_variants,
            ttf_file_count,
            sources,
        }
    }

    pub fn sources(&self) -> &[fontdb::Source] {
        &self.sources
    }

    pub fn into_sources(self) -> Vec<fontdb::Source> {
        self.sources
    }
}

#[derive(Debug, Clone)]
pub struct RegisteredFontBatch {
    per_source_ids: Vec<TinyVec<[fontdb::ID; 8]>>,
    all_ids: Vec<fontdb::ID>,
}

impl RegisteredFontBatch {
    pub(crate) fn new(
        per_source_ids: Vec<TinyVec<[fontdb::ID; 8]>>,
        all_ids: Vec<fontdb::ID>,
    ) -> Self {
        Self {
            per_source_ids,
            all_ids,
        }
    }

    pub fn per_source_ids(&self) -> &[TinyVec<[fontdb::ID; 8]>] {
        &self.per_source_ids
    }

    pub fn face_ids(&self) -> &[fontdb::ID] {
        &self.all_ids
    }

    pub(crate) fn into_face_ids(self) -> Vec<fontdb::ID> {
        self.all_ids
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

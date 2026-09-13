use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

/// A font family, weight, and style resolved during Google Fonts processing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UsedGoogleFontVariant {
    /// Requested/display family name.
    pub family: String,
    /// CSS font-weight value.
    pub weight: u16,
    /// CSS font-style value.
    pub style: FontStyle,
}

/// Counters for Google Fonts resolution and network activity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoogleFontStats {
    /// CSS2 stylesheet requests that missed the local cache.
    pub css_cache_misses: u64,
    /// Font file requests that missed the local cache.
    pub font_file_cache_misses: u64,
    /// Bytes downloaded from Google Fonts.
    pub downloaded_bytes: u64,
    /// Number of variants resolved across Google Fonts operations.
    pub resolved_variants: u64,
}

impl GoogleFontStats {
    /// Total stylesheet and font-file cache misses, using saturating addition.
    pub fn cache_misses(&self) -> u64 {
        self.css_cache_misses
            .saturating_add(self.font_file_cache_misses)
    }

    /// Accumulate counters with saturating addition.
    pub fn add_assign(&mut self, other: impl Borrow<Self>) {
        let other = other.borrow();
        self.css_cache_misses = self.css_cache_misses.saturating_add(other.css_cache_misses);
        self.font_file_cache_misses = self
            .font_file_cache_misses
            .saturating_add(other.font_file_cache_misses);
        self.downloaded_bytes = self.downloaded_bytes.saturating_add(other.downloaded_bytes);
        self.resolved_variants = self
            .resolved_variants
            .saturating_add(other.resolved_variants);
    }
}

/// Google Fonts counters and the distinct variants resolved for an operation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoogleFontUsage {
    /// Numeric counters for Google Fonts work.
    pub stats: GoogleFontStats,
    /// Deduplicated variants resolved by Google Fonts operations.
    #[serde(default)]
    pub used_variants: Vec<UsedGoogleFontVariant>,
}

impl GoogleFontUsage {
    /// Total stylesheet and font-file cache misses, using saturating addition.
    pub fn cache_misses(&self) -> u64 {
        self.stats.cache_misses()
    }

    /// Accumulate counters with saturating addition and, when present, merge distinct variants.
    pub fn add_assign(&mut self, other: impl Borrow<Self>) {
        let other = other.borrow();
        self.stats.add_assign(other.stats);
        for variant in &other.used_variants {
            if !self.used_variants.contains(variant) {
                self.used_variants.push(variant.clone());
            }
        }
    }

    /// Add counters without changing the resolved-variant list.
    pub fn add_stats(&mut self, stats: impl Borrow<GoogleFontStats>) {
        self.stats.add_assign(*stats.borrow());
    }

    /// Replace the resolved variants with those supplied for one font family.
    /// Also set `stats.resolved_variants` to the length of the new list.
    pub fn set_used_variants(
        &mut self,
        family: &str,
        variants: impl IntoIterator<Item = VariantRequest>,
    ) {
        self.used_variants = variants
            .into_iter()
            .map(|variant| UsedGoogleFontVariant {
                family: family.to_string(),
                weight: variant.weight,
                style: variant.style,
            })
            .collect();
        self.stats.resolved_variants = u64::try_from(self.used_variants.len()).unwrap_or(u64::MAX);
    }

    /// Whether all counters are zero and no resolved variants are recorded.
    pub fn is_empty(&self) -> bool {
        self.stats == GoogleFontStats::default() && self.used_variants.is_empty()
    }
}

impl From<GoogleFontStats> for GoogleFontUsage {
    fn from(stats: GoogleFontStats) -> Self {
        Self {
            stats,
            ..Default::default()
        }
    }
}

/// A font family to download, optionally restricted to selected variants.
#[derive(Debug, Clone, Copy)]
pub struct FontLoadRequest<'a> {
    /// Google Fonts display name, such as `"Roboto"` (case-sensitive).
    pub family: &'a str,
    /// Requested weights and styles. `None` loads all available variants.
    /// An empty slice is an error. Unavailable variants use the closest match.
    /// Availability is checked at weights 100 through 900 in steps of 100,
    /// for both normal and italic styles.
    pub variants: Option<&'a [VariantRequest]>,
}

impl<'a> FontLoadRequest<'a> {
    /// Request all available variants of a font family.
    pub fn new(family: &'a str) -> Self {
        Self {
            family,
            variants: None,
        }
    }
}

/// Downloaded font data and the usage recorded while loading it.
#[derive(Debug)]
pub struct FontLoadResult {
    /// Font files and the variants they provide.
    pub batch: LoadedFontBatch,
    /// Resolution and download activity, including cache misses.
    pub usage: GoogleFontUsage,
}

/// Whether Google Fonts recognizes a family, without downloading font files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontProbeResult {
    /// Whether the family has available variants.
    pub known: bool,
    /// Activity recorded while checking the family.
    pub usage: GoogleFontUsage,
}

/// Available variants selected for a request, without downloading font files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantResolutionResult {
    /// Selected weights and styles after fallback and deduplication.
    pub variants: Vec<VariantRequest>,
    /// Activity recorded while resolving the variants.
    pub usage: GoogleFontUsage,
}

/// CSS font style encoded as lowercase `"normal"` / `"italic"` across serde,
/// `Display`, `FromStr`, Python, and server-admin JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontStyle {
    /// Upright text, serialized as `"normal"`.
    Normal,
    /// Italic text, serialized as `"italic"`.
    Italic,
}

impl FontStyle {
    /// Return the CSS keyword `"normal"` or `"italic"`.
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
    /// CSS font weight, such as `400` for regular or `700` for bold.
    pub weight: u16,
    /// Upright or italic style.
    pub style: FontStyle,
}

impl fmt::Display for VariantRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.weight, self.style.as_str())
    }
}

/// Find the closest available variant to a requested (weight, style).
///
/// Prefers an exact match, then the closest weight with matching style, then
/// the closest weight regardless of style. Returns the index into `available`,
/// or `None` if `available` is empty.
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
fn is_valid_font_id(id: &str) -> bool {
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

/// Font files loaded for one family, ready for embedding or registration.
#[derive(Debug, Clone)]
pub struct LoadedFontBatch {
    /// Normalized family identifier, such as `"playfair-display"`.
    pub font_id: String,
    /// Weights and styles provided by the loaded files.
    pub loaded_variants: Vec<VariantRequest>,
    /// Number of font files in `font_data`.
    pub ttf_file_count: usize,
    /// Raw TTF or OTF data, shared so callers can reuse it without copying.
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

    #[test]
    fn test_google_font_usage_add_assign_dedupes_used_variants() {
        let mut usage = GoogleFontUsage::default();
        usage.set_used_variants(
            "Roboto",
            [VariantRequest {
                weight: 400,
                style: FontStyle::Normal,
            }],
        );

        let mut other = GoogleFontUsage {
            stats: GoogleFontStats {
                css_cache_misses: 1,
                resolved_variants: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        other.set_used_variants(
            "Roboto",
            [
                VariantRequest {
                    weight: 400,
                    style: FontStyle::Normal,
                },
                VariantRequest {
                    weight: 700,
                    style: FontStyle::Normal,
                },
            ],
        );

        usage.add_assign(other);

        assert_eq!(usage.stats.css_cache_misses, 1);
        assert_eq!(usage.stats.resolved_variants, 3);
        assert_eq!(
            usage.used_variants,
            vec![
                UsedGoogleFontVariant {
                    family: "Roboto".to_string(),
                    weight: 400,
                    style: FontStyle::Normal,
                },
                UsedGoogleFontVariant {
                    family: "Roboto".to_string(),
                    weight: 700,
                    style: FontStyle::Normal,
                },
            ]
        );
    }
}

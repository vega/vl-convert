//! CSS font string parsing for Canvas 2D.
//!
//! Parses CSS font strings like "12px Arial" or "bold italic 14pt 'Times New Roman'"
//! into components usable with cosmic-text.

use crate::error::{Canvas2dError, Canvas2dResult};
use cosmic_text::{Style, Weight};

/// Parsed font specification from a CSS font string.
#[derive(Debug, Clone)]
pub struct ParsedFont {
    /// Font style (normal, italic, oblique).
    pub style: Style,
    /// Font weight (100-900 or keywords like bold).
    pub weight: Weight,
    /// Font size in pixels.
    pub size_px: f32,
    /// Font families in order of preference.
    pub families: Vec<String>,
}

impl Default for ParsedFont {
    fn default() -> Self {
        Self {
            style: Style::Normal,
            weight: Weight::NORMAL,
            size_px: 10.0,
            families: vec!["sans-serif".to_string()],
        }
    }
}

/// Parse a CSS font string into components.
///
/// Supports format: `[style] [variant] [weight] size[/line-height] family[, family]*`
///
/// Examples:
/// - "12px Arial"
/// - "bold 14px sans-serif"
/// - "italic bold 12pt 'Times New Roman', serif"
/// - "700 16px/20px Helvetica"
pub fn parse_font(font_str: &str) -> Canvas2dResult<ParsedFont> {
    let font_str = font_str.trim();
    if font_str.is_empty() {
        return Ok(ParsedFont::default());
    }

    let mut result = ParsedFont::default();
    let mut remaining = font_str;

    // Parse optional style, variant, and weight (in any order)
    loop {
        let trimmed = remaining.trim_start();
        if trimmed.is_empty() {
            break;
        }

        // Try to parse style
        if let Some(rest) = trimmed.strip_prefix("italic") {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                result.style = Style::Italic;
                remaining = rest;
                continue;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("oblique") {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                result.style = Style::Oblique;
                remaining = rest;
                continue;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("normal") {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                // normal can be style, variant, or weight - just consume it
                remaining = rest;
                continue;
            }
        }

        // Try to parse variant (small-caps - we just consume it)
        if let Some(rest) = trimmed.strip_prefix("small-caps") {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                remaining = rest;
                continue;
            }
        }

        // Try to parse weight
        if let Some(rest) = trimmed.strip_prefix("bold") {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                result.weight = Weight::BOLD;
                remaining = rest;
                continue;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("bolder") {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                result.weight = Weight::EXTRA_BOLD;
                remaining = rest;
                continue;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("lighter") {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                result.weight = Weight::LIGHT;
                remaining = rest;
                continue;
            }
        }

        // Try numeric weight (100-900)
        if let Some(weight_match) = parse_numeric_weight(trimmed) {
            result.weight = weight_match.0;
            remaining = weight_match.1;
            continue;
        }

        // No more style/weight to parse
        break;
    }

    // Parse required size
    remaining = remaining.trim_start();
    let (size, rest) = parse_font_size(remaining)?;
    result.size_px = size;
    remaining = rest;

    // Skip optional line-height
    remaining = remaining.trim_start();
    if let Some(rest) = remaining.strip_prefix('/') {
        remaining = skip_line_height(rest);
    }

    // Parse font families
    remaining = remaining.trim_start();
    if !remaining.is_empty() {
        result.families = parse_font_families(remaining);
    }

    Ok(result)
}

/// Try to parse a numeric weight (100-900) at the start of the string.
fn parse_numeric_weight(s: &str) -> Option<(Weight, &str)> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }

    let weight: u16 = digits.parse().ok()?;
    if !(100..=900).contains(&weight) || !weight.is_multiple_of(100) {
        return None;
    }

    let rest = &s[digits.len()..];
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }

    let weight = Weight(weight);
    Some((weight, rest))
}

/// Parse font size from string.
fn parse_font_size(s: &str) -> Canvas2dResult<(f32, &str)> {
    // Find the numeric part
    let num_end = s
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit() && *c != '.')
        .map(|(i, _)| i)
        .unwrap_or(s.len());

    if num_end == 0 {
        return Err(Canvas2dError::FontParseError(format!(
            "Expected font size, got: {}",
            s
        )));
    }

    let num_str = &s[..num_end];
    let rest = &s[num_end..];

    let size: f32 = num_str.parse().map_err(|_| {
        Canvas2dError::FontParseError(format!("Invalid font size number: {}", num_str))
    })?;

    // Parse unit
    let (multiplier, unit_len) = if rest.starts_with("px") {
        (1.0, 2)
    } else if rest.starts_with("pt") {
        (4.0 / 3.0, 2) // 1pt = 4/3 px
    } else if rest.starts_with("em") {
        (16.0, 2) // Assume 1em = 16px
    } else if rest.starts_with("rem") {
        (16.0, 3)
    } else if rest.starts_with('%') {
        (16.0 / 100.0, 1) // Percentage of default 16px
    } else {
        // Assume pixels if no unit
        (1.0, 0)
    };

    Ok((size * multiplier, &rest[unit_len..]))
}

/// Skip line-height specification after '/'.
fn skip_line_height(s: &str) -> &str {
    // Skip digits, dots, and units
    let end = s
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    &s[end..]
}

/// Parse font family list.
fn parse_font_families(s: &str) -> Vec<String> {
    let mut families = Vec::new();
    let mut remaining = s.trim();

    while !remaining.is_empty() {
        let (family, rest) = parse_single_family(remaining);
        if !family.is_empty() {
            families.push(family);
        }
        remaining = rest.trim_start();
        if let Some(rest) = remaining.strip_prefix(',') {
            remaining = rest.trim_start();
        } else {
            break;
        }
    }

    if families.is_empty() {
        families.push("sans-serif".to_string());
    }

    families
}

/// Parse a single font family name.
fn parse_single_family(s: &str) -> (String, &str) {
    let s = s.trim_start();

    // Check for quoted family name
    if s.starts_with('"') || s.starts_with('\'') {
        let quote = s.chars().next().unwrap();
        let end = s[1..].find(quote).map(|i| i + 1).unwrap_or(s.len() - 1);
        let family = s[1..end].to_string();
        let rest = if end + 1 < s.len() { &s[end + 1..] } else { "" };
        return (family, rest);
    }

    // Unquoted family name - ends at comma or end of string
    let end = s.find(',').unwrap_or(s.len());
    let family = s[..end].trim().to_string();
    let rest = &s[end..];
    (family, rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_font() {
        let font = parse_font("12px Arial").unwrap();
        assert_eq!(font.size_px, 12.0);
        assert_eq!(font.families, vec!["Arial"]);
        assert_eq!(font.weight, Weight::NORMAL);
        assert_eq!(font.style, Style::Normal);
    }

    #[test]
    fn test_bold_font() {
        let font = parse_font("bold 14px sans-serif").unwrap();
        assert_eq!(font.size_px, 14.0);
        assert_eq!(font.weight, Weight::BOLD);
    }

    #[test]
    fn test_italic_font() {
        let font = parse_font("italic 16pt 'Times New Roman'").unwrap();
        assert!((font.size_px - 16.0 * 4.0 / 3.0).abs() < 0.01);
        assert_eq!(font.style, Style::Italic);
        assert_eq!(font.families, vec!["Times New Roman"]);
    }

    #[test]
    fn test_numeric_weight() {
        let font = parse_font("600 12px Helvetica").unwrap();
        assert_eq!(font.weight, Weight(600));
    }

    #[test]
    fn test_multiple_families() {
        let font = parse_font("12px Arial, Helvetica, sans-serif").unwrap();
        assert_eq!(font.families, vec!["Arial", "Helvetica", "sans-serif"]);
    }

    #[test]
    fn test_line_height() {
        let font = parse_font("16px/20px Arial").unwrap();
        assert_eq!(font.size_px, 16.0);
        assert_eq!(font.families, vec!["Arial"]);
    }
}

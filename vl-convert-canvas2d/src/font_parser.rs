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

/// Parse a CSS font shorthand string into components.
///
/// Supports the full CSS font shorthand grammar:
/// ```text
/// font = [ [ <font-style> || <font-variant-css2> || <font-weight> || <font-stretch> ]?
///          <font-size> [ / <line-height> ]? <font-family># ]
///        | caption | icon | menu | message-box | small-caption | status-bar
/// ```
///
/// Examples:
/// - "12px Arial"
/// - "bold 14px sans-serif"
/// - "italic bold 12pt 'Times New Roman', serif"
/// - "700 16px/20px Helvetica"
/// - "oblique 14deg condensed small xx-large serif"
/// - "caption"
pub fn parse_font(font_str: &str) -> Canvas2dResult<ParsedFont> {
    let font_str = font_str.trim();
    if font_str.is_empty() {
        return Ok(ParsedFont::default());
    }

    // System font keywords are mutually exclusive with the rest of the grammar
    match font_str {
        "caption" | "icon" | "menu" | "message-box" | "small-caption" | "status-bar" => {
            return Ok(ParsedFont {
                style: Style::Normal,
                weight: Weight::NORMAL,
                size_px: 16.0,
                families: vec!["sans-serif".to_string()],
            });
        }
        _ => {}
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
                // Try to consume optional angle (e.g., "14deg", "-10deg")
                remaining = try_consume_oblique_angle(rest);
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

        // Try numeric weight (1-1000)
        if let Some(weight_match) = parse_numeric_weight(trimmed) {
            result.weight = weight_match.0;
            remaining = weight_match.1;
            continue;
        }

        // Try to parse font-stretch keyword (consume but discard)
        if let Some(rest) = try_consume_font_stretch(trimmed) {
            remaining = rest;
            continue;
        }

        // No more style/weight/stretch to parse
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

/// Try to parse a numeric weight (1-1000) at the start of the string.
fn parse_numeric_weight(s: &str) -> Option<(Weight, &str)> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }

    let weight: u16 = digits.parse().ok()?;
    if !(1..=1000).contains(&weight) {
        return None;
    }

    let rest = &s[digits.len()..];
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }

    let weight = Weight(weight);
    Some((weight, rest))
}

/// Try to consume an oblique angle after the "oblique" keyword.
/// Handles formats like "14deg", "-10deg", "0.5rad", "20grad", "0.25turn".
/// The angle is parsed but discarded (cosmic-text doesn't support oblique angles).
/// Returns the remaining string after consuming the angle, or the input unchanged.
fn try_consume_oblique_angle(s: &str) -> &str {
    let trimmed = s.trim_start();
    if trimmed.is_empty() {
        return s;
    }

    // Check if next token starts with a sign or digit
    let first = match trimmed.as_bytes().first() {
        Some(&b) if b == b'-' || b == b'+' || b.is_ascii_digit() || b == b'.' => b,
        _ => return s,
    };

    // Find end of numeric part
    let has_sign = first == b'-' || first == b'+';
    let num_start = if has_sign { 1 } else { 0 };

    let num_end = trimmed[num_start..]
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .map(|i| num_start + i)
        .unwrap_or(trimmed.len());

    // Must have at least one digit
    if num_end == num_start {
        return s;
    }

    let after_num = &trimmed[num_end..];

    // Check for angle unit
    let after_unit = if let Some(rest) = after_num.strip_prefix("grad") {
        rest
    } else if let Some(rest) = after_num.strip_prefix("turn") {
        rest
    } else if let Some(rest) = after_num.strip_prefix("deg") {
        rest
    } else if let Some(rest) = after_num.strip_prefix("rad") {
        rest
    } else {
        return s; // No angle unit — not an angle
    };

    // Must be followed by whitespace or end of string
    if after_unit.is_empty() || after_unit.starts_with(char::is_whitespace) {
        after_unit
    } else {
        s
    }
}

/// Try to consume a font-stretch keyword at the start of the string.
/// Font-stretch is parsed but discarded (cosmic-text doesn't support it).
fn try_consume_font_stretch(s: &str) -> Option<&str> {
    // Ordered longest-first to avoid prefix conflicts
    let stretch_keywords = [
        "ultra-condensed",
        "extra-condensed",
        "semi-condensed",
        "semi-expanded",
        "extra-expanded",
        "ultra-expanded",
        "condensed",
        "expanded",
    ];

    for keyword in &stretch_keywords {
        if let Some(rest) = s.strip_prefix(keyword) {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                return Some(rest);
            }
        }
    }

    None
}

/// Parse font size from string.
fn parse_font_size(s: &str) -> Canvas2dResult<(f32, &str)> {
    // Check for size keywords first (ordered longest-first to avoid prefix conflicts)
    let size_keywords: &[(&str, f32)] = &[
        ("xxx-large", 48.0),
        ("xx-small", 9.0),
        ("xx-large", 32.0),
        ("x-small", 10.0),
        ("x-large", 24.0),
        ("smaller", 13.0),
        ("larger", 19.0),
        ("medium", 16.0),
        ("small", 13.0),
        ("large", 18.0),
    ];

    for &(keyword, size_px) in size_keywords {
        if let Some(rest) = s.strip_prefix(keyword) {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) || rest.starts_with('/') {
                return Ok((size_px, rest));
            }
        }
    }

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

    // Parse unit — context-dependent units error, physical units convert exactly
    // Check unsupported context-dependent units first
    for unit in &["vmin", "vmax", "vw", "vh", "ch", "ex"] {
        if let Some(after) = rest.strip_prefix(unit) {
            if after.is_empty() || after.starts_with(char::is_whitespace) || after.starts_with('/')
            {
                return Err(Canvas2dError::FontParseError(format!(
                    "Unsupported font size unit: {}",
                    unit
                )));
            }
        }
    }

    let (multiplier, unit_len) = if rest.starts_with("px") {
        (1.0, 2)
    } else if rest.starts_with("pt") {
        (4.0 / 3.0, 2) // 1pt = 4/3 px
    } else if rest.starts_with("rem") {
        (16.0, 3) // Assume 1rem = 16px (check before "em")
    } else if rest.starts_with("em") {
        (16.0, 2) // Assume 1em = 16px
    } else if rest.starts_with("cm") {
        (96.0 / 2.54, 2) // 1cm = 96/2.54 px
    } else if rest.starts_with("mm") {
        (96.0 / 25.4, 2) // 1mm = 96/25.4 px
    } else if rest.starts_with("in") {
        (96.0, 2) // 1in = 96px
    } else if rest.starts_with("pc") {
        (16.0, 2) // 1pc = 12pt = 16px
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

    // --- System font keywords ---

    #[test]
    fn test_system_font_keywords() {
        for keyword in &[
            "caption",
            "icon",
            "menu",
            "message-box",
            "small-caption",
            "status-bar",
        ] {
            let font = parse_font(keyword).unwrap();
            assert_eq!(font.size_px, 16.0, "failed for: {}", keyword);
            assert_eq!(font.families, vec!["sans-serif"], "failed for: {}", keyword);
            assert_eq!(font.weight, Weight::NORMAL, "failed for: {}", keyword);
            assert_eq!(font.style, Style::Normal, "failed for: {}", keyword);
        }
    }

    // --- Numeric weights ---

    #[test]
    fn test_numeric_weight_non_multiple_of_100() {
        let font = parse_font("450 12px Arial").unwrap();
        assert_eq!(font.weight, Weight(450));
        assert_eq!(font.size_px, 12.0);
    }

    #[test]
    fn test_numeric_weight_boundaries() {
        let font = parse_font("1 12px Arial").unwrap();
        assert_eq!(font.weight, Weight(1));

        let font = parse_font("1000 12px Arial").unwrap();
        assert_eq!(font.weight, Weight(1000));
    }

    #[test]
    fn test_numeric_weight_out_of_range_becomes_size() {
        // 1001 is not a valid weight, so it's parsed as font-size (1001px)
        let font = parse_font("1001 Arial").unwrap();
        assert_eq!(font.size_px, 1001.0);
        assert_eq!(font.weight, Weight::NORMAL);
    }

    // --- Font-stretch keywords ---

    #[test]
    fn test_stretch_keyword_condensed() {
        let font = parse_font("condensed 12px Arial").unwrap();
        assert_eq!(font.size_px, 12.0);
        assert_eq!(font.families, vec!["Arial"]);
    }

    #[test]
    fn test_stretch_keyword_with_weight() {
        let font = parse_font("bold semi-expanded 14px serif").unwrap();
        assert_eq!(font.weight, Weight::BOLD);
        assert_eq!(font.size_px, 14.0);
        assert_eq!(font.families, vec!["serif"]);
    }

    #[test]
    fn test_stretch_all_keywords() {
        let keywords = [
            "ultra-condensed",
            "extra-condensed",
            "condensed",
            "semi-condensed",
            "semi-expanded",
            "expanded",
            "extra-expanded",
            "ultra-expanded",
        ];
        for keyword in &keywords {
            let input = format!("{} 12px Arial", keyword);
            let font = parse_font(&input).unwrap();
            assert_eq!(font.size_px, 12.0, "failed for stretch: {}", keyword);
        }
    }

    // --- Oblique with angle ---

    #[test]
    fn test_oblique_with_angle_deg() {
        let font = parse_font("oblique 14deg 12px Arial").unwrap();
        assert_eq!(font.style, Style::Oblique);
        assert_eq!(font.size_px, 12.0);
        assert_eq!(font.families, vec!["Arial"]);
    }

    #[test]
    fn test_oblique_with_negative_angle() {
        let font = parse_font("oblique -10deg 16px sans-serif").unwrap();
        assert_eq!(font.style, Style::Oblique);
        assert_eq!(font.size_px, 16.0);
    }

    #[test]
    fn test_oblique_with_rad_angle() {
        let font = parse_font("oblique 0.5rad 12px Arial").unwrap();
        assert_eq!(font.style, Style::Oblique);
        assert_eq!(font.size_px, 12.0);
    }

    #[test]
    fn test_oblique_without_angle() {
        // "oblique" followed by font-size (12px is not an angle)
        let font = parse_font("oblique 12px Arial").unwrap();
        assert_eq!(font.style, Style::Oblique);
        assert_eq!(font.size_px, 12.0);
    }

    // --- Size keywords ---

    #[test]
    fn test_size_keyword_medium() {
        let font = parse_font("medium Arial").unwrap();
        assert_eq!(font.size_px, 16.0);
        assert_eq!(font.families, vec!["Arial"]);
    }

    #[test]
    fn test_size_keyword_small() {
        let font = parse_font("bold small sans-serif").unwrap();
        assert_eq!(font.weight, Weight::BOLD);
        assert_eq!(font.size_px, 13.0);
    }

    #[test]
    fn test_size_keyword_xx_large() {
        let font = parse_font("xx-large serif").unwrap();
        assert_eq!(font.size_px, 32.0);
    }

    #[test]
    fn test_size_keyword_xxx_large() {
        let font = parse_font("xxx-large monospace").unwrap();
        assert_eq!(font.size_px, 48.0);
    }

    #[test]
    fn test_size_keyword_larger() {
        let font = parse_font("larger Arial").unwrap();
        assert_eq!(font.size_px, 19.0);
    }

    #[test]
    fn test_size_keyword_with_line_height() {
        let font = parse_font("medium/1.5 Arial").unwrap();
        assert_eq!(font.size_px, 16.0);
        assert_eq!(font.families, vec!["Arial"]);
    }

    // --- Physical size units ---

    #[test]
    fn test_size_unit_cm() {
        let font = parse_font("1cm Arial").unwrap();
        assert!((font.size_px - 96.0 / 2.54).abs() < 0.01);
    }

    #[test]
    fn test_size_unit_in() {
        let font = parse_font("0.5in Arial").unwrap();
        assert!((font.size_px - 48.0).abs() < 0.01);
    }

    #[test]
    fn test_size_unit_mm() {
        let font = parse_font("10mm Arial").unwrap();
        assert!((font.size_px - 96.0 / 2.54).abs() < 0.1);
    }

    #[test]
    fn test_size_unit_pc() {
        let font = parse_font("1pc Arial").unwrap();
        assert!((font.size_px - 16.0).abs() < 0.01);
    }

    // --- Context-dependent units error ---

    #[test]
    fn test_unsupported_unit_vw() {
        assert!(parse_font("2vw Arial").is_err());
    }

    #[test]
    fn test_unsupported_unit_vh() {
        assert!(parse_font("2vh Arial").is_err());
    }

    #[test]
    fn test_unsupported_unit_ch() {
        assert!(parse_font("2ch Arial").is_err());
    }

    #[test]
    fn test_unsupported_unit_ex() {
        assert!(parse_font("2ex Arial").is_err());
    }

    #[test]
    fn test_unsupported_unit_vmin() {
        assert!(parse_font("2vmin Arial").is_err());
    }

    #[test]
    fn test_unsupported_unit_vmax() {
        assert!(parse_font("2vmax Arial").is_err());
    }

    // --- Full combination ---

    #[test]
    fn test_full_css_font_shorthand() {
        let font = parse_font(
            "italic small-caps bold condensed 16px/1.2 \"Helvetica Neue\", Arial, sans-serif",
        )
        .unwrap();
        assert_eq!(font.style, Style::Italic);
        assert_eq!(font.weight, Weight::BOLD);
        assert_eq!(font.size_px, 16.0);
        assert_eq!(font.families, vec!["Helvetica Neue", "Arial", "sans-serif"]);
    }

    #[test]
    fn test_oblique_angle_with_stretch_and_weight() {
        let font = parse_font("oblique 14deg 450 semi-condensed 12px Arial").unwrap();
        assert_eq!(font.style, Style::Oblique);
        assert_eq!(font.weight, Weight(450));
        assert_eq!(font.size_px, 12.0);
    }
}

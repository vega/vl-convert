//! Text measurement and rendering using cosmic-text.

use crate::error::Canvas2dResult;
use crate::font_parser::ParsedFont;
use crate::style::{TextAlign, TextBaseline};
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight};

/// Text metrics returned by measureText().
#[derive(Debug, Clone, Default)]
pub struct TextMetrics {
    /// Width of the text in pixels.
    pub width: f32,
    /// Distance from baseline to top of the bounding box.
    pub actual_bounding_box_ascent: f32,
    /// Distance from baseline to bottom of the bounding box.
    pub actual_bounding_box_descent: f32,
    /// Font ascent.
    pub font_bounding_box_ascent: f32,
    /// Font descent.
    pub font_bounding_box_descent: f32,
    /// Distance from alignment point to left of the bounding box.
    pub actual_bounding_box_left: f32,
    /// Distance from alignment point to right of the bounding box.
    pub actual_bounding_box_right: f32,
}

/// Result of resolving a font family by post_script_name.
/// Includes both the actual family name and the font weight from the matched face.
struct PostScriptNameMatch {
    family_name: String,
    weight: Weight,
}

/// Try to resolve a font family name by checking post_script_names in the fontdb.
/// This handles cases like "Matter SemiBold" where the CSS font name doesn't match
/// the font family name but matches the PostScript name (e.g., "Matter-SemiBold").
///
/// Returns the actual font family name and weight if found via post_script_name lookup,
/// or None if no match is found.
fn resolve_family_by_postscript_name(
    font_system: &FontSystem,
    family: &str,
) -> Option<PostScriptNameMatch> {
    let normalized_name = family.replace('-', " ");

    for face in font_system.db().faces() {
        let post_script_normalized = face.post_script_name.replace('-', " ");
        if post_script_normalized.eq_ignore_ascii_case(&normalized_name) {
            // Found a match - return the actual family name and weight from this face
            if let Some((family_name, _)) = face.families.first() {
                return Some(PostScriptNameMatch {
                    family_name: family_name.clone(),
                    weight: Weight(face.weight.0),
                });
            }
        }
    }

    None
}

/// Result of family resolution with optional weight override.
struct FamilyResolution<'a> {
    family: Family<'a>,
    weight_override: Option<Weight>,
}

/// Get the Family enum for a font, with post_script_name fallback lookup.
/// When a font is found via post_script_name, also returns the font's actual weight
/// to override any weight parsed from the CSS font string (e.g., "bold 13px Matter SemiBold"
/// should use weight 600 from "Matter-SemiBold", not weight 700 from "bold").
fn get_family_with_fallback<'a>(
    font_system: &FontSystem,
    family: &'a str,
    resolved_name: &'a mut Option<String>,
) -> FamilyResolution<'a> {
    // First check for generic family names
    match family.to_lowercase().as_str() {
        "sans-serif" => {
            return FamilyResolution {
                family: Family::SansSerif,
                weight_override: None,
            }
        }
        "serif" => {
            return FamilyResolution {
                family: Family::Serif,
                weight_override: None,
            }
        }
        "monospace" => {
            return FamilyResolution {
                family: Family::Monospace,
                weight_override: None,
            }
        }
        "cursive" => {
            return FamilyResolution {
                family: Family::Cursive,
                weight_override: None,
            }
        }
        "fantasy" => {
            return FamilyResolution {
                family: Family::Fantasy,
                weight_override: None,
            }
        }
        _ => {}
    }

    // Try post_script_name fallback lookup
    if let Some(matched) = resolve_family_by_postscript_name(font_system, family) {
        *resolved_name = Some(matched.family_name);
        // Return reference to the resolved name stored in the Option
        // Also return the weight from the matched font face
        return FamilyResolution {
            family: Family::Name(resolved_name.as_ref().unwrap()),
            weight_override: Some(matched.weight),
        };
    }

    // Use the original family name
    FamilyResolution {
        family: Family::Name(family),
        weight_override: None,
    }
}

/// Measure text using cosmic-text.
pub fn measure_text(
    font_system: &mut FontSystem,
    text: &str,
    font: &ParsedFont,
) -> Canvas2dResult<TextMetrics> {
    let metrics = Metrics::new(font.size_px, font.size_px * 1.2);
    let mut buffer = Buffer::new(font_system, metrics);

    // Build attributes from parsed font
    // Convert generic family names to proper Family enum variants,
    // with fallback lookup by post_script_name for custom fonts like "Matter SemiBold"
    let mut resolved_name: Option<String> = None;
    let resolution = font
        .families
        .first()
        .map(|f| get_family_with_fallback(font_system, f, &mut resolved_name))
        .unwrap_or(FamilyResolution {
            family: Family::SansSerif,
            weight_override: None,
        });

    // Use weight from post_script_name match if available, otherwise use parsed CSS weight.
    // This handles cases like "bold 13px Matter SemiBold" where the CSS "bold" (700) should
    // be overridden by the actual font weight (600) from "Matter-SemiBold".
    let weight = resolution.weight_override.unwrap_or(font.weight);

    let attrs = Attrs::new()
        .family(resolution.family)
        .weight(weight)
        .style(font.style);

    buffer.set_text(font_system, text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);

    // Calculate width from layout runs
    let mut width: f32 = 0.0;
    for run in buffer.layout_runs() {
        width = width.max(run.line_w);
    }

    // Get font metrics for ascent/descent
    let font_ascent = font.size_px * 0.8; // Approximation
    let font_descent = font.size_px * 0.2;

    Ok(TextMetrics {
        width,
        actual_bounding_box_ascent: font_ascent,
        actual_bounding_box_descent: font_descent,
        font_bounding_box_ascent: font_ascent,
        font_bounding_box_descent: font_descent,
        actual_bounding_box_left: 0.0,
        actual_bounding_box_right: width,
    })
}

/// Calculate X offset for text alignment.
pub fn calculate_text_x_offset(width: f32, align: TextAlign) -> f32 {
    match align {
        TextAlign::Left | TextAlign::Start => 0.0,
        TextAlign::Right | TextAlign::End => -width,
        TextAlign::Center => -width / 2.0,
    }
}

/// Calculate Y offset for text baseline.
pub fn calculate_text_y_offset(font_size: f32, baseline: TextBaseline) -> f32 {
    let ascent = font_size * 0.8;
    let descent = font_size * 0.2;

    match baseline {
        TextBaseline::Top => ascent,
        TextBaseline::Hanging => ascent * 0.8,
        TextBaseline::Middle => ascent / 2.0 - descent / 2.0,
        TextBaseline::Alphabetic => 0.0,
        TextBaseline::Ideographic => -descent * 0.5,
        TextBaseline::Bottom => -descent,
    }
}

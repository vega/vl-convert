//! Text measurement and rendering using cosmic-text.

use crate::error::Canvas2dResult;
use crate::font_parser::ParsedFont;
use crate::style::{TextAlign, TextBaseline};
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};

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

/// Measure text using cosmic-text.
pub fn measure_text(
    font_system: &mut FontSystem,
    text: &str,
    font: &ParsedFont,
) -> Canvas2dResult<TextMetrics> {
    let metrics = Metrics::new(font.size_px, font.size_px * 1.2);
    let mut buffer = Buffer::new(font_system, metrics);

    // Build attributes from parsed font
    let family = font
        .families
        .first()
        .map(|f| Family::Name(f))
        .unwrap_or(Family::SansSerif);

    let attrs = Attrs::new()
        .family(family)
        .weight(font.weight)
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

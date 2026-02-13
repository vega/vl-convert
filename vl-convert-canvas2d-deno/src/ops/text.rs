//! Text rendering operations: font, alignment, baseline, measure, fill/stroke text.

use crate::{CanvasResource, SharedFontConfig};
use deno_core::op2;
use deno_core::{OpState, ResourceId};
use deno_error::JsErrorBox;
use vl_convert_canvas2d::{FontStretch, TextAlign, TextBaseline};

/// If the shared font configuration has been updated since this canvas was
/// created (or last refreshed), update the canvas context's font database so
/// that newly-registered fonts are available for text measurement / rendering.
fn refresh_canvas_fonts_if_needed(state: &OpState, resource: &CanvasResource) {
    if let Some(shared_config) = state.try_borrow::<SharedFontConfig>() {
        if shared_config.version != resource.font_config_version.get() {
            resource
                .ctx
                .borrow_mut()
                .update_font_database(&shared_config.resolved);
            resource.font_config_version.set(shared_config.version);
        }
    }
}

/// Set the font from a CSS font string.
#[op2(fast)]
pub fn op_canvas_set_font(
    state: &mut OpState,
    rid: u32,
    #[string] font: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    refresh_canvas_fonts_if_needed(state, &resource);

    resource
        .ctx
        .borrow_mut()
        .set_font(&font)
        .map_err(|e| JsErrorBox::generic(format!("Invalid font: {}", e)))?;
    Ok(())
}

/// Set the text alignment.
#[op2(fast)]
pub fn op_canvas_set_text_align(
    state: &mut OpState,
    rid: u32,
    #[string] align: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let text_align = match align.as_str() {
        "left" => TextAlign::Left,
        "right" => TextAlign::Right,
        "center" => TextAlign::Center,
        "start" => TextAlign::Start,
        "end" => TextAlign::End,
        _ => {
            return Err(JsErrorBox::generic(format!(
                "Invalid text align: {}",
                align
            )))
        }
    };

    resource.ctx.borrow_mut().set_text_align(text_align);
    Ok(())
}

/// Set the text baseline.
#[op2(fast)]
pub fn op_canvas_set_text_baseline(
    state: &mut OpState,
    rid: u32,
    #[string] baseline: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let text_baseline = match baseline.as_str() {
        "top" => TextBaseline::Top,
        "hanging" => TextBaseline::Hanging,
        "middle" => TextBaseline::Middle,
        "alphabetic" => TextBaseline::Alphabetic,
        "ideographic" => TextBaseline::Ideographic,
        "bottom" => TextBaseline::Bottom,
        _ => {
            return Err(JsErrorBox::generic(format!(
                "Invalid text baseline: {}",
                baseline
            )))
        }
    };

    resource.ctx.borrow_mut().set_text_baseline(text_baseline);
    Ok(())
}

/// Set the font stretch.
#[op2(fast)]
pub fn op_canvas_set_font_stretch(
    state: &mut OpState,
    rid: u32,
    #[string] stretch: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let font_stretch = match FontStretch::from_css_keyword(&stretch) {
        Some(s) => s,
        None => {
            // Ignore invalid values per spec
            return Ok(());
        }
    };

    resource.ctx.borrow_mut().set_font_stretch(font_stretch);
    Ok(())
}

/// Get the font stretch.
#[op2]
#[string]
pub fn op_canvas_get_font_stretch(state: &mut OpState, rid: u32) -> Result<String, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let result = resource
        .ctx
        .borrow()
        .get_font_stretch()
        .as_css_keyword()
        .to_string();
    Ok(result)
}

/// Set the letter spacing for text rendering.
#[op2(fast)]
pub fn op_canvas_set_letter_spacing(
    state: &mut OpState,
    rid: u32,
    spacing: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().set_letter_spacing(spacing as f32);
    Ok(())
}

/// Get the current letter spacing.
#[op2(fast)]
pub fn op_canvas_get_letter_spacing(state: &mut OpState, rid: u32) -> Result<f64, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let spacing = resource.ctx.borrow().get_letter_spacing();
    Ok(spacing as f64)
}

/// Measure text and return the width.
#[op2(fast)]
pub fn op_canvas_measure_text(
    state: &mut OpState,
    rid: u32,
    #[string] text: String,
) -> Result<f64, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    refresh_canvas_fonts_if_needed(state, &resource);

    let metrics = resource
        .ctx
        .borrow_mut()
        .measure_text(&text)
        .map_err(|e| JsErrorBox::generic(format!("Failed to measure text: {}", e)))?;

    Ok(metrics.width as f64)
}

/// Fill text at the specified position.
#[op2(fast)]
pub fn op_canvas_fill_text(
    state: &mut OpState,
    rid: u32,
    #[string] text: String,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    refresh_canvas_fonts_if_needed(state, &resource);

    resource
        .ctx
        .borrow_mut()
        .fill_text(&text, x as f32, y as f32);
    Ok(())
}

/// Stroke text at the specified position.
#[op2(fast)]
pub fn op_canvas_stroke_text(
    state: &mut OpState,
    rid: u32,
    #[string] text: String,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    refresh_canvas_fonts_if_needed(state, &resource);

    resource
        .ctx
        .borrow_mut()
        .stroke_text(&text, x as f32, y as f32);
    Ok(())
}

/// Fill text at the specified position with max width constraint.
#[op2(fast)]
pub fn op_canvas_fill_text_max_width(
    state: &mut OpState,
    rid: u32,
    #[string] text: String,
    x: f64,
    y: f64,
    max_width: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    refresh_canvas_fonts_if_needed(state, &resource);

    resource
        .ctx
        .borrow_mut()
        .fill_text_max_width(&text, x as f32, y as f32, max_width as f32);
    Ok(())
}

/// Stroke text at the specified position with max width constraint.
#[op2(fast)]
pub fn op_canvas_stroke_text_max_width(
    state: &mut OpState,
    rid: u32,
    #[string] text: String,
    x: f64,
    y: f64,
    max_width: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    refresh_canvas_fonts_if_needed(state, &resource);

    resource
        .ctx
        .borrow_mut()
        .stroke_text_max_width(&text, x as f32, y as f32, max_width as f32);
    Ok(())
}

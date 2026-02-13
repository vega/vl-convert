//! Canvas state property setters and getters.
//!
//! Ops for fill/stroke style (colors), line properties (width, cap, join, miter,
//! dash), global alpha, composite operations, and image smoothing.

use crate::CanvasResource;
use deno_core::op2;
use deno_core::{OpState, ResourceId};
use deno_error::JsErrorBox;
use vl_convert_canvas2d::{LineCap, LineJoin};

// --- Style setters ---

/// Set the fill style from a CSS color string.
#[op2(fast)]
pub fn op_canvas_set_fill_style(
    state: &mut OpState,
    rid: u32,
    #[string] style: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .set_fill_style(&style)
        .map_err(|e| JsErrorBox::generic(format!("Invalid fill style: {}", e)))?;
    Ok(())
}

/// Set the stroke style from a CSS color string.
#[op2(fast)]
pub fn op_canvas_set_stroke_style(
    state: &mut OpState,
    rid: u32,
    #[string] style: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .set_stroke_style(&style)
        .map_err(|e| JsErrorBox::generic(format!("Invalid stroke style: {}", e)))?;
    Ok(())
}

// --- Line properties ---

/// Set the line width.
#[op2(fast)]
pub fn op_canvas_set_line_width(
    state: &mut OpState,
    rid: u32,
    width: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().set_line_width(width as f32);
    Ok(())
}

/// Set the line cap style.
#[op2(fast)]
pub fn op_canvas_set_line_cap(
    state: &mut OpState,
    rid: u32,
    #[string] cap: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let line_cap = match cap.as_str() {
        "butt" => LineCap::Butt,
        "round" => LineCap::Round,
        "square" => LineCap::Square,
        _ => return Err(JsErrorBox::generic(format!("Invalid line cap: {}", cap))),
    };

    resource.ctx.borrow_mut().set_line_cap(line_cap);
    Ok(())
}

/// Set the line join style.
#[op2(fast)]
pub fn op_canvas_set_line_join(
    state: &mut OpState,
    rid: u32,
    #[string] join: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let line_join = match join.as_str() {
        "miter" => LineJoin::Miter,
        "round" => LineJoin::Round,
        "bevel" => LineJoin::Bevel,
        _ => return Err(JsErrorBox::generic(format!("Invalid line join: {}", join))),
    };

    resource.ctx.borrow_mut().set_line_join(line_join);
    Ok(())
}

/// Set the miter limit.
#[op2(fast)]
pub fn op_canvas_set_miter_limit(
    state: &mut OpState,
    rid: u32,
    limit: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().set_miter_limit(limit as f32);
    Ok(())
}

/// Set the global alpha (opacity).
#[op2(fast)]
pub fn op_canvas_set_global_alpha(
    state: &mut OpState,
    rid: u32,
    alpha: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().set_global_alpha(alpha as f32);
    Ok(())
}

/// Set the global composite operation. Returns true if accepted.
#[op2(fast)]
pub fn op_canvas_set_global_composite_operation(
    state: &mut OpState,
    rid: u32,
    #[string] op: String,
) -> Result<bool, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let accepted = resource
        .ctx
        .borrow_mut()
        .set_global_composite_operation(&op);
    Ok(accepted)
}

// --- Line dash ---

/// Set the line dash pattern.
#[op2]
pub fn op_canvas_set_line_dash(
    state: &mut OpState,
    rid: u32,
    #[serde] segments: Vec<f64>,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let segments: Vec<f32> = segments.into_iter().map(|s| s as f32).collect();
    resource.ctx.borrow_mut().set_line_dash(segments);
    Ok(())
}

/// Set the line dash offset.
#[op2(fast)]
pub fn op_canvas_set_line_dash_offset(
    state: &mut OpState,
    rid: u32,
    offset: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .set_line_dash_offset(offset as f32);
    Ok(())
}

// --- Image smoothing ---

/// Set image smoothing enabled.
#[op2(fast)]
pub fn op_canvas_set_image_smoothing_enabled(
    state: &mut OpState,
    rid: u32,
    enabled: bool,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .set_image_smoothing_enabled(enabled);
    Ok(())
}

/// Get image smoothing enabled.
#[op2(fast)]
pub fn op_canvas_get_image_smoothing_enabled(
    state: &mut OpState,
    rid: u32,
) -> Result<bool, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let enabled = resource.ctx.borrow().get_image_smoothing_enabled();
    Ok(enabled)
}

/// Set image smoothing quality.
#[op2(fast)]
pub fn op_canvas_set_image_smoothing_quality(
    state: &mut OpState,
    rid: u32,
    #[string] quality: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let quality = match quality.as_str() {
        "low" => vl_convert_canvas2d::ImageSmoothingQuality::Low,
        "medium" => vl_convert_canvas2d::ImageSmoothingQuality::Medium,
        "high" => vl_convert_canvas2d::ImageSmoothingQuality::High,
        _ => {
            return Err(JsErrorBox::generic(format!(
                "Invalid image smoothing quality: {}",
                quality
            )))
        }
    };

    resource
        .ctx
        .borrow_mut()
        .set_image_smoothing_quality(quality);
    Ok(())
}

/// Get image smoothing quality.
#[op2]
#[string]
pub fn op_canvas_get_image_smoothing_quality(
    state: &mut OpState,
    rid: u32,
) -> Result<String, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let quality = resource.ctx.borrow().get_image_smoothing_quality();
    let quality_str = match quality {
        vl_convert_canvas2d::ImageSmoothingQuality::Low => "low",
        vl_convert_canvas2d::ImageSmoothingQuality::Medium => "medium",
        vl_convert_canvas2d::ImageSmoothingQuality::High => "high",
    };
    Ok(quality_str.to_string())
}

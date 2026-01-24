//! Deno ops for Canvas 2D API.
//!
//! These ops wrap the vl-convert-canvas2d crate and expose the Canvas 2D API
//! to JavaScript code running in Deno.

use crate::canvas2d::CanvasResource;
use deno_core::op2;
use deno_core::{OpState, ResourceId};
use deno_error::JsErrorBox;
use vl_convert_canvas2d::{Canvas2dContext, LineCap, LineJoin, TextAlign, TextBaseline};

// --- Canvas creation and lifecycle ---

/// Create a new canvas with the given dimensions.
#[op2(fast)]
pub fn op_canvas_create(state: &mut OpState, width: u32, height: u32) -> Result<u32, JsErrorBox> {
    let ctx = Canvas2dContext::new(width, height)
        .map_err(|e| JsErrorBox::generic(format!("Failed to create canvas: {}", e)))?;

    let resource = CanvasResource::new(ctx);
    let rid = state.resource_table.add(resource);
    Ok(rid)
}

/// Destroy a canvas and free its resources.
#[op2(fast)]
pub fn op_canvas_destroy(state: &mut OpState, rid: u32) -> Result<(), JsErrorBox> {
    let _resource = state
        .resource_table
        .take::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Failed to close canvas: {}", e)))?;
    // Resource is dropped here, freeing the canvas
    Ok(())
}

// --- State management ---

/// Save the current drawing state.
#[op2(fast)]
pub fn op_canvas_save(state: &mut OpState, rid: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().save();
    Ok(())
}

/// Restore the previously saved drawing state.
#[op2(fast)]
pub fn op_canvas_restore(state: &mut OpState, rid: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().restore();
    Ok(())
}

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

/// Set the global composite operation.
#[op2(fast)]
pub fn op_canvas_set_global_composite_operation(
    state: &mut OpState,
    rid: u32,
    #[string] op: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .set_global_composite_operation(&op);
    Ok(())
}

// --- Font and text ---

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

    resource
        .ctx
        .borrow_mut()
        .stroke_text(&text, x as f32, y as f32);
    Ok(())
}

// --- Path operations ---

/// Begin a new path.
#[op2(fast)]
pub fn op_canvas_begin_path(state: &mut OpState, rid: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().begin_path();
    Ok(())
}

/// Move to a point without drawing.
#[op2(fast)]
pub fn op_canvas_move_to(state: &mut OpState, rid: u32, x: f64, y: f64) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().move_to(x as f32, y as f32);
    Ok(())
}

/// Draw a line to a point.
#[op2(fast)]
pub fn op_canvas_line_to(state: &mut OpState, rid: u32, x: f64, y: f64) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().line_to(x as f32, y as f32);
    Ok(())
}

/// Close the current subpath.
#[op2(fast)]
pub fn op_canvas_close_path(state: &mut OpState, rid: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().close_path();
    Ok(())
}

/// Add a cubic bezier curve.
#[op2(fast)]
pub fn op_canvas_bezier_curve_to(
    state: &mut OpState,
    rid: u32,
    cp1x: f64,
    cp1y: f64,
    cp2x: f64,
    cp2y: f64,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().bezier_curve_to(
        cp1x as f32,
        cp1y as f32,
        cp2x as f32,
        cp2y as f32,
        x as f32,
        y as f32,
    );
    Ok(())
}

/// Add a quadratic bezier curve.
#[op2(fast)]
pub fn op_canvas_quadratic_curve_to(
    state: &mut OpState,
    rid: u32,
    cpx: f64,
    cpy: f64,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .quadratic_curve_to(cpx as f32, cpy as f32, x as f32, y as f32);
    Ok(())
}

/// Add a rectangle to the path.
#[op2(fast)]
pub fn op_canvas_rect(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .rect(x as f32, y as f32, width as f32, height as f32);
    Ok(())
}

/// Add an arc to the path.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_canvas_arc(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    anticlockwise: bool,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().arc(
        x as f32,
        y as f32,
        radius as f32,
        start_angle as f32,
        end_angle as f32,
        anticlockwise,
    );
    Ok(())
}

/// Add an arcTo segment to the path.
#[op2(fast)]
pub fn op_canvas_arc_to(
    state: &mut OpState,
    rid: u32,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    radius: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .arc_to(x1 as f32, y1 as f32, x2 as f32, y2 as f32, radius as f32);
    Ok(())
}

/// Add an ellipse to the path.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_canvas_ellipse(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    radius_x: f64,
    radius_y: f64,
    rotation: f64,
    start_angle: f64,
    end_angle: f64,
    anticlockwise: bool,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().ellipse(
        x as f32,
        y as f32,
        radius_x as f32,
        radius_y as f32,
        rotation as f32,
        start_angle as f32,
        end_angle as f32,
        anticlockwise,
    );
    Ok(())
}

// --- Drawing operations ---

/// Fill the current path.
#[op2(fast)]
pub fn op_canvas_fill(state: &mut OpState, rid: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().fill();
    Ok(())
}

/// Stroke the current path.
#[op2(fast)]
pub fn op_canvas_stroke(state: &mut OpState, rid: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().stroke();
    Ok(())
}

/// Fill a rectangle.
#[op2(fast)]
pub fn op_canvas_fill_rect(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .fill_rect(x as f32, y as f32, width as f32, height as f32);
    Ok(())
}

/// Stroke a rectangle.
#[op2(fast)]
pub fn op_canvas_stroke_rect(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .stroke_rect(x as f32, y as f32, width as f32, height as f32);
    Ok(())
}

/// Clear a rectangle.
#[op2(fast)]
pub fn op_canvas_clear_rect(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .clear_rect(x as f32, y as f32, width as f32, height as f32);
    Ok(())
}

/// Clip to the current path.
#[op2(fast)]
pub fn op_canvas_clip(state: &mut OpState, rid: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().clip();
    Ok(())
}

// --- Transform operations ---

/// Translate the canvas.
#[op2(fast)]
pub fn op_canvas_translate(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().translate(x as f32, y as f32);
    Ok(())
}

/// Rotate the canvas.
#[op2(fast)]
pub fn op_canvas_rotate(state: &mut OpState, rid: u32, angle: f64) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().rotate(angle as f32);
    Ok(())
}

/// Scale the canvas.
#[op2(fast)]
pub fn op_canvas_scale(state: &mut OpState, rid: u32, x: f64, y: f64) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().scale(x as f32, y as f32);
    Ok(())
}

/// Apply a transform matrix.
#[op2(fast)]
pub fn op_canvas_transform(
    state: &mut OpState,
    rid: u32,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .transform(a as f32, b as f32, c as f32, d as f32, e as f32, f as f32);
    Ok(())
}

/// Set the transform matrix.
#[op2(fast)]
pub fn op_canvas_set_transform(
    state: &mut OpState,
    rid: u32,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .set_transform(a as f32, b as f32, c as f32, d as f32, e as f32, f as f32);
    Ok(())
}

/// Reset the transform to identity.
#[op2(fast)]
pub fn op_canvas_reset_transform(state: &mut OpState, rid: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().reset_transform();
    Ok(())
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

// --- Output ---

/// Get image data for a region of the canvas.
#[op2]
#[serde]
pub fn op_canvas_get_image_data(
    state: &mut OpState,
    rid: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let data = resource.ctx.borrow().get_image_data(x, y, width, height);
    Ok(data)
}

/// Export the canvas as PNG data.
#[op2]
#[serde]
pub fn op_canvas_to_png(state: &mut OpState, rid: u32) -> Result<Vec<u8>, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let png_data = resource
        .ctx
        .borrow()
        .to_png()
        .map_err(|e| JsErrorBox::generic(format!("Failed to export PNG: {}", e)))?;
    Ok(png_data)
}

/// Get the canvas width.
#[op2(fast)]
pub fn op_canvas_width(state: &mut OpState, rid: u32) -> Result<u32, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let width = resource.ctx.borrow().width();
    Ok(width)
}

/// Get the canvas height.
#[op2(fast)]
pub fn op_canvas_height(state: &mut OpState, rid: u32) -> Result<u32, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let height = resource.ctx.borrow().height();
    Ok(height)
}

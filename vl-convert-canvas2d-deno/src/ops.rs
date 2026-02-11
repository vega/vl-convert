//! Deno ops for Canvas 2D API.
//!
//! These ops wrap the vl-convert-canvas2d crate and expose the Canvas 2D API
//! to JavaScript code running in Deno.

use crate::{CanvasResource, SharedFontConfig};
use deno_core::op2;
use deno_core::{OpState, ResourceId};
use deno_error::JsErrorBox;
use serde::Serialize;
use vl_convert_canvas2d::{
    ArcParams, ArcToParams, Canvas2dContext, CanvasColor, CanvasImageDataRef, CornerRadius,
    CubicBezierParams, DOMMatrix, DirtyRect, EllipseParams, FontStretch, ImageCropParams, LineCap,
    LineJoin, QuadraticBezierParams, RadialGradientParams, RectParams, RoundRectParams, TextAlign,
    TextBaseline,
};

// --- Canvas creation and lifecycle ---

/// Create a new canvas with the given dimensions.
/// If a SharedFontConfig is available in OpState, it will be used for the canvas.
#[op2(fast)]
pub fn op_canvas_create(state: &mut OpState, width: u32, height: u32) -> Result<u32, JsErrorBox> {
    let ctx = if let Some(shared_config) = state.try_borrow::<SharedFontConfig>() {
        let config = shared_config.0.as_ref().clone();
        Canvas2dContext::with_config(width, height, config)
            .map_err(|e| JsErrorBox::generic(format!("Failed to create canvas: {}", e)))?
    } else {
        Canvas2dContext::new(width, height)
            .map_err(|e| JsErrorBox::generic(format!("Failed to create canvas: {}", e)))?
    };

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

    resource
        .ctx
        .borrow_mut()
        .bezier_curve_to(&CubicBezierParams {
            cp1x: cp1x as f32,
            cp1y: cp1y as f32,
            cp2x: cp2x as f32,
            cp2y: cp2y as f32,
            x: x as f32,
            y: y as f32,
        });
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
        .quadratic_curve_to(&QuadraticBezierParams {
            cpx: cpx as f32,
            cpy: cpy as f32,
            x: x as f32,
            y: y as f32,
        });
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

    resource.ctx.borrow_mut().rect(&RectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
    });
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

    resource.ctx.borrow_mut().arc(&ArcParams {
        x: x as f32,
        y: y as f32,
        radius: radius as f32,
        start_angle: start_angle as f32,
        end_angle: end_angle as f32,
        anticlockwise,
    });
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

    resource.ctx.borrow_mut().arc_to(&ArcToParams {
        x1: x1 as f32,
        y1: y1 as f32,
        x2: x2 as f32,
        y2: y2 as f32,
        radius: radius as f32,
    });
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

    resource.ctx.borrow_mut().ellipse(&EllipseParams {
        x: x as f32,
        y: y as f32,
        radius_x: radius_x as f32,
        radius_y: radius_y as f32,
        rotation: rotation as f32,
        start_angle: start_angle as f32,
        end_angle: end_angle as f32,
        anticlockwise,
    });
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

    resource.ctx.borrow_mut().fill_rect(&RectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
    });
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

    resource.ctx.borrow_mut().stroke_rect(&RectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
    });
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

    resource.ctx.borrow_mut().clear_rect(&RectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
    });
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

    resource.ctx.borrow_mut().transform(DOMMatrix::new(
        a as f32, b as f32, c as f32, d as f32, e as f32, f as f32,
    ));
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

    resource.ctx.borrow_mut().set_transform(DOMMatrix::new(
        a as f32, b as f32, c as f32, d as f32, e as f32, f as f32,
    ));
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
#[buffer]
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
///
/// # Arguments
/// * `rid` - Canvas resource ID
/// * `ppi` - Optional pixels per inch for PNG metadata. Defaults to 72 if not specified.
#[op2]
#[buffer]
pub fn op_canvas_to_png(
    state: &mut OpState,
    rid: u32,
    ppi: Option<f32>,
) -> Result<Vec<u8>, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let png_data = resource
        .ctx
        .borrow()
        .to_png(ppi)
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

// --- Gradients ---

/// Create a linear gradient.
#[op2(fast)]
pub fn op_canvas_create_linear_gradient(
    state: &mut OpState,
    rid: u32,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> Result<u32, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let gradient = resource
        .ctx
        .borrow()
        .create_linear_gradient(x0 as f32, y0 as f32, x1 as f32, y1 as f32);
    let gradient_id = resource.add_gradient(gradient);
    Ok(gradient_id)
}

/// Create a radial gradient.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_canvas_create_radial_gradient(
    state: &mut OpState,
    rid: u32,
    x0: f64,
    y0: f64,
    r0: f64,
    x1: f64,
    y1: f64,
    r1: f64,
) -> Result<u32, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let gradient = resource
        .ctx
        .borrow()
        .create_radial_gradient(&RadialGradientParams {
            x0: x0 as f32,
            y0: y0 as f32,
            r0: r0 as f32,
            x1: x1 as f32,
            y1: y1 as f32,
            r1: r1 as f32,
        });
    let gradient_id = resource.add_gradient(gradient);
    Ok(gradient_id)
}

/// Add a color stop to a gradient.
#[op2(fast)]
pub fn op_canvas_gradient_add_color_stop(
    state: &mut OpState,
    rid: u32,
    gradient_id: u32,
    offset: f64,
    #[string] color: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    // Parse the color string
    let parsed_color: csscolorparser::Color = color
        .parse()
        .map_err(|e| JsErrorBox::generic(format!("Invalid color: {}", e)))?;
    let [r, g, b, a] = parsed_color.to_array();
    let color = CanvasColor::from_rgba_f32(r, g, b, a);

    let mut gradient = resource
        .get_gradient_mut(gradient_id)
        .ok_or_else(|| JsErrorBox::generic(format!("Invalid gradient id: {}", gradient_id)))?;
    gradient.add_color_stop(offset, color);
    Ok(())
}

/// Set the fill style to a gradient.
#[op2(fast)]
pub fn op_canvas_set_fill_style_gradient(
    state: &mut OpState,
    rid: u32,
    gradient_id: u32,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let gradient = resource
        .take_gradient(gradient_id)
        .ok_or_else(|| JsErrorBox::generic(format!("Invalid gradient id: {}", gradient_id)))?;
    resource.ctx.borrow_mut().set_fill_style_gradient(gradient);
    Ok(())
}

/// Set the stroke style to a gradient.
#[op2(fast)]
pub fn op_canvas_set_stroke_style_gradient(
    state: &mut OpState,
    rid: u32,
    gradient_id: u32,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let gradient = resource
        .take_gradient(gradient_id)
        .ok_or_else(|| JsErrorBox::generic(format!("Invalid gradient id: {}", gradient_id)))?;
    resource
        .ctx
        .borrow_mut()
        .set_stroke_style_gradient(gradient);
    Ok(())
}

// --- Text with maxWidth ---

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

    resource
        .ctx
        .borrow_mut()
        .stroke_text_max_width(&text, x as f32, y as f32, max_width as f32);
    Ok(())
}

// --- drawImage ---

/// Draw an image (from RGBA pixel data) at the specified position.
#[op2(fast)]
pub fn op_canvas_draw_image(
    state: &mut OpState,
    rid: u32,
    #[buffer] data: &[u8],
    img_width: u32,
    img_height: u32,
    dx: f64,
    dy: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let image = CanvasImageDataRef {
        data,
        width: img_width,
        height: img_height,
    };

    resource
        .ctx
        .borrow_mut()
        .draw_image_data(&image, dx as f32, dy as f32);
    Ok(())
}

/// Draw an image scaled to fit destination dimensions.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_canvas_draw_image_scaled(
    state: &mut OpState,
    rid: u32,
    #[buffer] data: &[u8],
    img_width: u32,
    img_height: u32,
    dx: f64,
    dy: f64,
    dw: f64,
    dh: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let image = CanvasImageDataRef {
        data,
        width: img_width,
        height: img_height,
    };

    resource
        .ctx
        .borrow_mut()
        .draw_image_data_scaled(&image, dx as f32, dy as f32, dw as f32, dh as f32);
    Ok(())
}

/// Draw a cropped region of an image to a destination region.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_canvas_draw_image_cropped(
    state: &mut OpState,
    rid: u32,
    #[buffer] data: &[u8],
    img_width: u32,
    img_height: u32,
    sx: f64,
    sy: f64,
    sw: f64,
    sh: f64,
    dx: f64,
    dy: f64,
    dw: f64,
    dh: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let image = CanvasImageDataRef {
        data,
        width: img_width,
        height: img_height,
    };

    resource.ctx.borrow_mut().draw_image_data_cropped(
        &image,
        &ImageCropParams {
            sx: sx as f32,
            sy: sy as f32,
            sw: sw as f32,
            sh: sh as f32,
            dx: dx as f32,
            dy: dy as f32,
            dw: dw as f32,
            dh: dh as f32,
        },
    );
    Ok(())
}

/// Draw from one canvas to another.
#[op2(fast)]
pub fn op_canvas_draw_canvas(
    state: &mut OpState,
    rid: u32,
    source_rid: u32,
    dx: f64,
    dy: f64,
) -> Result<(), JsErrorBox> {
    let source_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(source_rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid source canvas resource: {}", e)))?;

    let dest_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let source_ctx = source_resource.ctx.borrow();
    dest_resource
        .ctx
        .borrow_mut()
        .draw_canvas(&source_ctx, dx as f32, dy as f32);
    Ok(())
}

/// Draw from one canvas to another, scaled.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_canvas_draw_canvas_scaled(
    state: &mut OpState,
    rid: u32,
    source_rid: u32,
    dx: f64,
    dy: f64,
    dw: f64,
    dh: f64,
) -> Result<(), JsErrorBox> {
    let source_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(source_rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid source canvas resource: {}", e)))?;

    let dest_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let source_ctx = source_resource.ctx.borrow();
    dest_resource.ctx.borrow_mut().draw_canvas_scaled(
        &source_ctx,
        dx as f32,
        dy as f32,
        dw as f32,
        dh as f32,
    );
    Ok(())
}

/// Draw a cropped region from one canvas to another.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_canvas_draw_canvas_cropped(
    state: &mut OpState,
    rid: u32,
    source_rid: u32,
    sx: f64,
    sy: f64,
    sw: f64,
    sh: f64,
    dx: f64,
    dy: f64,
    dw: f64,
    dh: f64,
) -> Result<(), JsErrorBox> {
    let source_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(source_rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid source canvas resource: {}", e)))?;

    let dest_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let source_ctx = source_resource.ctx.borrow();
    dest_resource.ctx.borrow_mut().draw_canvas_cropped(
        &source_ctx,
        &ImageCropParams {
            sx: sx as f32,
            sy: sy as f32,
            sw: sw as f32,
            sh: sh as f32,
            dx: dx as f32,
            dy: dy as f32,
            dw: dw as f32,
            dh: dh as f32,
        },
    );
    Ok(())
}

// --- Phase 2: Patterns ---

/// Create a pattern from image data.
#[op2(fast)]
pub fn op_canvas_create_pattern(
    state: &mut OpState,
    rid: u32,
    #[buffer] data: &[u8],
    width: u32,
    height: u32,
    #[string] repetition: String,
) -> Result<u32, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let pattern = resource
        .ctx
        .borrow()
        .create_pattern(data, width, height, &repetition)
        .map_err(|e| JsErrorBox::generic(format!("Failed to create pattern: {}", e)))?;

    let pattern_id = resource.add_pattern(pattern);
    Ok(pattern_id)
}

/// Create a pattern from another canvas.
#[op2(fast)]
pub fn op_canvas_create_pattern_from_canvas(
    state: &mut OpState,
    rid: u32,
    source_rid: u32,
    #[string] repetition: String,
) -> Result<u32, JsErrorBox> {
    let source_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(source_rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid source canvas resource: {}", e)))?;

    let dest_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let source_ctx = source_resource.ctx.borrow();
    let pattern = dest_resource
        .ctx
        .borrow()
        .create_pattern_from_canvas(&source_ctx, &repetition)
        .map_err(|e| JsErrorBox::generic(format!("Failed to create pattern: {}", e)))?;

    let pattern_id = dest_resource.add_pattern(pattern);
    Ok(pattern_id)
}

/// Set the fill style to a pattern.
#[op2(fast)]
pub fn op_canvas_set_fill_style_pattern(
    state: &mut OpState,
    rid: u32,
    pattern_id: u32,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let pattern = resource
        .get_pattern(pattern_id)
        .ok_or_else(|| JsErrorBox::generic(format!("Invalid pattern id: {}", pattern_id)))?;

    resource.ctx.borrow_mut().set_fill_style_pattern(pattern);
    Ok(())
}

/// Set the stroke style to a pattern.
#[op2(fast)]
pub fn op_canvas_set_stroke_style_pattern(
    state: &mut OpState,
    rid: u32,
    pattern_id: u32,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let pattern = resource
        .get_pattern(pattern_id)
        .ok_or_else(|| JsErrorBox::generic(format!("Invalid pattern id: {}", pattern_id)))?;

    resource.ctx.borrow_mut().set_stroke_style_pattern(pattern);
    Ok(())
}

// --- Phase 2: putImageData ---

/// Put image data onto the canvas.
#[op2(fast)]
pub fn op_canvas_put_image_data(
    state: &mut OpState,
    rid: u32,
    #[buffer] data: &[u8],
    width: u32,
    height: u32,
    dx: i32,
    dy: i32,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .put_image_data(data, width, height, dx, dy);
    Ok(())
}

/// Put image data onto the canvas with dirty rect.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_canvas_put_image_data_dirty(
    state: &mut OpState,
    rid: u32,
    #[buffer] data: &[u8],
    width: u32,
    height: u32,
    dx: i32,
    dy: i32,
    dirty_x: i32,
    dirty_y: i32,
    dirty_width: i32,
    dirty_height: i32,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().put_image_data_dirty(
        data,
        width,
        height,
        dx,
        dy,
        &DirtyRect {
            x: dirty_x,
            y: dirty_y,
            width: dirty_width,
            height: dirty_height,
        },
    );
    Ok(())
}

// --- Phase 2: imageSmoothingEnabled/Quality ---

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

// --- Phase 2: fillRule support ---

/// Fill the current path with a fill rule.
#[op2(fast)]
pub fn op_canvas_fill_with_rule(
    state: &mut OpState,
    rid: u32,
    #[string] fill_rule: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let rule = match fill_rule.as_str() {
        "nonzero" => vl_convert_canvas2d::CanvasFillRule::NonZero,
        "evenodd" => vl_convert_canvas2d::CanvasFillRule::EvenOdd,
        _ => {
            return Err(JsErrorBox::generic(format!(
                "Invalid fill rule: {}",
                fill_rule
            )))
        }
    };

    resource.ctx.borrow_mut().fill_with_rule(rule);
    Ok(())
}

/// Clip to the current path with a fill rule.
#[op2(fast)]
pub fn op_canvas_clip_with_rule(
    state: &mut OpState,
    rid: u32,
    #[string] fill_rule: String,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let rule = match fill_rule.as_str() {
        "nonzero" => vl_convert_canvas2d::CanvasFillRule::NonZero,
        "evenodd" => vl_convert_canvas2d::CanvasFillRule::EvenOdd,
        _ => {
            return Err(JsErrorBox::generic(format!(
                "Invalid fill rule: {}",
                fill_rule
            )))
        }
    };

    resource.ctx.borrow_mut().clip_with_rule(rule);
    Ok(())
}

// --- Phase 2: Path2D ---

use crate::Path2DResource;

/// Create an empty Path2D.
#[op2(fast)]
pub fn op_path2d_create(state: &mut OpState) -> u32 {
    let path = vl_convert_canvas2d::Path2D::new();
    let resource = Path2DResource::new(path);
    state.resource_table.add(resource)
}

/// Create a Path2D from SVG path data string.
#[op2(fast)]
pub fn op_path2d_create_from_svg(
    state: &mut OpState,
    #[string] svg_path: String,
) -> Result<u32, JsErrorBox> {
    let path = vl_convert_canvas2d::Path2D::from_svg_path_data(&svg_path)
        .map_err(|e| JsErrorBox::generic(format!("Invalid SVG path data: {}", e)))?;
    let resource = Path2DResource::new(path);
    Ok(state.resource_table.add(resource))
}

/// Create a Path2D by copying another Path2D.
#[op2(fast)]
pub fn op_path2d_create_from_path(
    state: &mut OpState,
    source_path_id: u32,
) -> Result<u32, JsErrorBox> {
    let source = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(source_path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let new_path = vl_convert_canvas2d::Path2D::from_path(&source.path.borrow());
    let resource = Path2DResource::new(new_path);
    Ok(state.resource_table.add(resource))
}

/// Destroy a Path2D.
#[op2(fast)]
pub fn op_path2d_destroy(state: &mut OpState, path_id: u32) -> Result<(), JsErrorBox> {
    state
        .resource_table
        .take::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Failed to destroy Path2D: {}", e)))?;
    Ok(())
}

/// Move to a point in Path2D.
#[op2(fast)]
pub fn op_path2d_move_to(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().move_to(x as f32, y as f32);
    Ok(())
}

/// Line to a point in Path2D.
#[op2(fast)]
pub fn op_path2d_line_to(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().line_to(x as f32, y as f32);
    Ok(())
}

/// Close the current subpath in Path2D.
#[op2(fast)]
pub fn op_path2d_close_path(state: &mut OpState, path_id: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().close_path();
    Ok(())
}

/// Add a cubic bezier curve to Path2D.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_path2d_bezier_curve_to(
    state: &mut OpState,
    path_id: u32,
    cp1x: f64,
    cp1y: f64,
    cp2x: f64,
    cp2y: f64,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource
        .path
        .borrow_mut()
        .bezier_curve_to(&CubicBezierParams {
            cp1x: cp1x as f32,
            cp1y: cp1y as f32,
            cp2x: cp2x as f32,
            cp2y: cp2y as f32,
            x: x as f32,
            y: y as f32,
        });
    Ok(())
}

/// Add a quadratic bezier curve to Path2D.
#[op2(fast)]
pub fn op_path2d_quadratic_curve_to(
    state: &mut OpState,
    path_id: u32,
    cpx: f64,
    cpy: f64,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource
        .path
        .borrow_mut()
        .quadratic_curve_to(&QuadraticBezierParams {
            cpx: cpx as f32,
            cpy: cpy as f32,
            x: x as f32,
            y: y as f32,
        });
    Ok(())
}

/// Add a rectangle to Path2D.
#[op2(fast)]
pub fn op_path2d_rect(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().rect(&RectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
    });
    Ok(())
}

/// Add an arc to Path2D.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_path2d_arc(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    anticlockwise: bool,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().arc(&ArcParams {
        x: x as f32,
        y: y as f32,
        radius: radius as f32,
        start_angle: start_angle as f32,
        end_angle: end_angle as f32,
        anticlockwise,
    });
    Ok(())
}

/// Add an arcTo segment to Path2D.
#[op2(fast)]
pub fn op_path2d_arc_to(
    state: &mut OpState,
    path_id: u32,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    radius: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().arc_to(&ArcToParams {
        x1: x1 as f32,
        y1: y1 as f32,
        x2: x2 as f32,
        y2: y2 as f32,
        radius: radius as f32,
    });
    Ok(())
}

/// Add an ellipse to Path2D.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_path2d_ellipse(
    state: &mut OpState,
    path_id: u32,
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
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().ellipse(&EllipseParams {
        x: x as f32,
        y: y as f32,
        radius_x: radius_x as f32,
        radius_y: radius_y as f32,
        rotation: rotation as f32,
        start_angle: start_angle as f32,
        end_angle: end_angle as f32,
        anticlockwise,
    });
    Ok(())
}

/// Fill a Path2D on the canvas.
#[op2(fast)]
pub fn op_canvas_fill_path2d(
    state: &mut OpState,
    rid: u32,
    path_id: u32,
) -> Result<(), JsErrorBox> {
    let path_resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let canvas_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    canvas_resource
        .ctx
        .borrow_mut()
        .fill_path2d(&mut path_resource.path.borrow_mut());
    Ok(())
}

/// Fill a Path2D on the canvas with a fill rule.
#[op2(fast)]
pub fn op_canvas_fill_path2d_with_rule(
    state: &mut OpState,
    rid: u32,
    path_id: u32,
    #[string] fill_rule: String,
) -> Result<(), JsErrorBox> {
    let path_resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let canvas_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let rule = match fill_rule.as_str() {
        "nonzero" => vl_convert_canvas2d::CanvasFillRule::NonZero,
        "evenodd" => vl_convert_canvas2d::CanvasFillRule::EvenOdd,
        _ => {
            return Err(JsErrorBox::generic(format!(
                "Invalid fill rule: {}",
                fill_rule
            )))
        }
    };

    canvas_resource
        .ctx
        .borrow_mut()
        .fill_path2d_with_rule(&mut path_resource.path.borrow_mut(), rule);
    Ok(())
}

/// Stroke a Path2D on the canvas.
#[op2(fast)]
pub fn op_canvas_stroke_path2d(
    state: &mut OpState,
    rid: u32,
    path_id: u32,
) -> Result<(), JsErrorBox> {
    let path_resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let canvas_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    canvas_resource
        .ctx
        .borrow_mut()
        .stroke_path2d(&mut path_resource.path.borrow_mut());
    Ok(())
}

/// Clip to a Path2D on the canvas.
#[op2(fast)]
pub fn op_canvas_clip_path2d(
    state: &mut OpState,
    rid: u32,
    path_id: u32,
) -> Result<(), JsErrorBox> {
    let path_resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let canvas_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    canvas_resource
        .ctx
        .borrow_mut()
        .clip_path2d(&mut path_resource.path.borrow_mut());
    Ok(())
}

/// Clip to a Path2D on the canvas with a fill rule.
#[op2(fast)]
pub fn op_canvas_clip_path2d_with_rule(
    state: &mut OpState,
    rid: u32,
    path_id: u32,
    #[string] fill_rule: String,
) -> Result<(), JsErrorBox> {
    let path_resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let canvas_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let rule = match fill_rule.as_str() {
        "nonzero" => vl_convert_canvas2d::CanvasFillRule::NonZero,
        "evenodd" => vl_convert_canvas2d::CanvasFillRule::EvenOdd,
        _ => {
            return Err(JsErrorBox::generic(format!(
                "Invalid fill rule: {}",
                fill_rule
            )))
        }
    };

    canvas_resource
        .ctx
        .borrow_mut()
        .clip_path2d_with_rule(&mut path_resource.path.borrow_mut(), rule);
    Ok(())
}

// --- Phase 3: Nice-to-Have Features ---

/// Reset the canvas context to its default state.
#[op2(fast)]
pub fn op_canvas_reset(state: &mut OpState, rid: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().reset();
    Ok(())
}

/// Add a rounded rectangle to the path with a single radius for all corners.
#[op2(fast)]
pub fn op_canvas_round_rect(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let r = CornerRadius {
        x: radius as f32,
        y: radius as f32,
    };
    resource.ctx.borrow_mut().round_rect(&RoundRectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
        radii: [r, r, r, r],
    });
    Ok(())
}

/// Add a rounded rectangle to the path with individual corner radii.
/// Each radius is an [x, y] pair for independent horizontal/vertical radii.
#[op2]
pub fn op_canvas_round_rect_radii(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    #[serde] radii: Vec<[f64; 2]>,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    // Convert radii array - Canvas spec allows 1, 2, 3, or 4 values
    let to_cr = |pair: &[f64; 2]| CornerRadius {
        x: pair[0] as f32,
        y: pair[1] as f32,
    };
    let radii_array = match radii.len() {
        1 => {
            let r = to_cr(&radii[0]);
            [r, r, r, r]
        }
        2 => {
            let a = to_cr(&radii[0]);
            let b = to_cr(&radii[1]);
            [a, b, a, b]
        }
        3 => {
            let a = to_cr(&radii[0]);
            let b = to_cr(&radii[1]);
            let c = to_cr(&radii[2]);
            [a, b, c, b]
        }
        4 => [
            to_cr(&radii[0]),
            to_cr(&radii[1]),
            to_cr(&radii[2]),
            to_cr(&radii[3]),
        ],
        _ => return Err(JsErrorBox::generic("Invalid radii array length")),
    };

    resource.ctx.borrow_mut().round_rect(&RoundRectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
        radii: radii_array,
    });
    Ok(())
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

/// Get the current transformation matrix as [a, b, c, d, e, f].
#[op2]
#[serde]
pub fn op_canvas_get_transform(state: &mut OpState, rid: u32) -> Result<Vec<f64>, JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let matrix = resource.ctx.borrow().get_transform();
    Ok(vec![
        matrix.a as f64,
        matrix.b as f64,
        matrix.c as f64,
        matrix.d as f64,
        matrix.e as f64,
        matrix.f as f64,
    ])
}

/// Add a rounded rectangle to Path2D with a single radius.
#[op2(fast)]
pub fn op_path2d_round_rect(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let r = CornerRadius {
        x: radius as f32,
        y: radius as f32,
    };
    resource.path.borrow_mut().round_rect(&RoundRectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
        radii: [r, r, r, r],
    });
    Ok(())
}

/// Add a rounded rectangle to Path2D with individual corner radii.
/// Each radius is an [x, y] pair for independent horizontal/vertical radii.
#[op2]
pub fn op_path2d_round_rect_radii(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    #[serde] radii: Vec<[f64; 2]>,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    // Convert radii array - Canvas spec allows 1, 2, 3, or 4 values
    let to_cr = |pair: &[f64; 2]| CornerRadius {
        x: pair[0] as f32,
        y: pair[1] as f32,
    };
    let radii_array = match radii.len() {
        1 => {
            let r = to_cr(&radii[0]);
            [r, r, r, r]
        }
        2 => {
            let a = to_cr(&radii[0]);
            let b = to_cr(&radii[1]);
            [a, b, a, b]
        }
        3 => {
            let a = to_cr(&radii[0]);
            let b = to_cr(&radii[1]);
            let c = to_cr(&radii[2]);
            [a, b, c, b]
        }
        4 => [
            to_cr(&radii[0]),
            to_cr(&radii[1]),
            to_cr(&radii[2]),
            to_cr(&radii[3]),
        ],
        _ => return Err(JsErrorBox::generic("Invalid radii array length")),
    };

    resource.path.borrow_mut().round_rect(&RoundRectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
        radii: radii_array,
    });
    Ok(())
}

/// Add another path's segments to a Path2D, optionally with a transform.
#[op2]
pub fn op_path2d_add_path(
    state: &mut OpState,
    path_id: u32,
    other_path_id: u32,
    #[serde] transform: Option<[f64; 6]>,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let other_resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(other_path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid source Path2D resource: {}", e)))?;

    let dom_matrix = transform.map(|t| {
        DOMMatrix::new(
            t[0] as f32,
            t[1] as f32,
            t[2] as f32,
            t[3] as f32,
            t[4] as f32,
            t[5] as f32,
        )
    });

    let mut other_path = other_resource.path.borrow_mut();
    resource
        .path
        .borrow_mut()
        .add_path(&mut other_path, dom_matrix);
    Ok(())
}

// --- Image decoding ---

/// Result of decoding an image - contains RGBA data and dimensions.
#[derive(Serialize)]
pub struct DecodedImage {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

/// Information about an image (for SVG detection and native size).
#[derive(Serialize)]
pub struct ImageInfo {
    pub is_svg: bool,
    pub width: u32,
    pub height: u32,
}

/// Get image info - checks if SVG and returns native dimensions without full decode.
#[op2]
#[serde]
pub fn op_canvas_get_image_info(#[buffer] bytes: &[u8]) -> Result<ImageInfo, JsErrorBox> {
    #[cfg(feature = "svg")]
    if is_svg(bytes) {
        let (width, height) = get_svg_native_size(bytes)?;
        return Ok(ImageInfo {
            is_svg: true,
            width,
            height,
        });
    }

    // For raster images, decode to get dimensions
    let img = image::load_from_memory(bytes)
        .map_err(|e| JsErrorBox::generic(format!("Failed to decode image: {}", e)))?;

    Ok(ImageInfo {
        is_svg: false,
        width: img.width(),
        height: img.height(),
    })
}

/// Decode raster image bytes (PNG, JPEG, GIF, WebP) into RGBA pixel data.
/// For SVG images, use op_canvas_decode_svg_at_size instead.
#[op2]
#[serde]
pub fn op_canvas_decode_image(#[buffer] bytes: &[u8]) -> Result<DecodedImage, JsErrorBox> {
    // Use the image crate to decode the image
    let img = image::load_from_memory(bytes)
        .map_err(|e| JsErrorBox::generic(format!("Failed to decode image: {}", e)))?;

    let width = img.width();
    let height = img.height();

    // Convert to RGBA8
    let rgba = img.to_rgba8();
    let data = rgba.into_raw();

    Ok(DecodedImage {
        data,
        width,
        height,
    })
}

/// Decode SVG at a specific target size with 2x supersampling for quality.
#[cfg(feature = "svg")]
#[op2]
#[serde]
pub fn op_canvas_decode_svg_at_size(
    #[buffer] bytes: &[u8],
    target_width: u32,
    target_height: u32,
) -> Result<DecodedImage, JsErrorBox> {
    use resvg::tiny_skia::Pixmap;
    use usvg::{Options, Tree};

    let svg_str = std::str::from_utf8(bytes)
        .map_err(|e| JsErrorBox::generic(format!("Invalid UTF-8 in SVG: {}", e)))?;

    let opt = Options::default();
    let tree = Tree::from_str(svg_str, &opt)
        .map_err(|e| JsErrorBox::generic(format!("Failed to parse SVG: {}", e)))?;

    // Render at 2x the target size for quality, then downsample
    let render_width = target_width * 2;
    let render_height = target_height * 2;

    let mut pixmap = Pixmap::new(render_width, render_height)
        .ok_or_else(|| JsErrorBox::generic("Failed to create pixmap for SVG"))?;

    // Calculate scale to fit SVG into render size
    let svg_size = tree.size();
    let scale_x = render_width as f32 / svg_size.width();
    let scale_y = render_height as f32 / svg_size.height();
    let transform = usvg::Transform::from_scale(scale_x, scale_y);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Convert from premultiplied RGBA to straight RGBA
    let data = unpremultiply_alpha(pixmap.take());

    Ok(DecodedImage {
        data,
        width: render_width,
        height: render_height,
    })
}

/// Fallback when SVG support is not compiled in.
#[cfg(not(feature = "svg"))]
#[op2]
#[serde]
pub fn op_canvas_decode_svg_at_size(
    #[buffer] _bytes: &[u8],
    _target_width: u32,
    _target_height: u32,
) -> Result<DecodedImage, JsErrorBox> {
    Err(JsErrorBox::generic(
        "SVG decoding is not available: the 'svg' feature is not enabled",
    ))
}

#[cfg(feature = "svg")]
fn is_svg(bytes: &[u8]) -> bool {
    let s = std::str::from_utf8(bytes).unwrap_or("");
    let trimmed = s.trim_start();
    trimmed.starts_with("<?xml")
        || trimmed.starts_with("<svg")
        || trimmed.starts_with("<!DOCTYPE svg")
}

#[cfg(feature = "svg")]
fn get_svg_native_size(bytes: &[u8]) -> Result<(u32, u32), JsErrorBox> {
    use usvg::{Options, Tree};

    let svg_str = std::str::from_utf8(bytes)
        .map_err(|e| JsErrorBox::generic(format!("Invalid UTF-8 in SVG: {}", e)))?;

    let opt = Options::default();
    let tree = Tree::from_str(svg_str, &opt)
        .map_err(|e| JsErrorBox::generic(format!("Failed to parse SVG: {}", e)))?;

    let size = tree.size();
    let width = size.width().round() as u32;
    let height = size.height().round() as u32;

    Ok((width, height))
}

#[cfg(feature = "svg")]
fn unpremultiply_alpha(mut data: Vec<u8>) -> Vec<u8> {
    // Convert from premultiplied RGBA to straight RGBA
    for chunk in data.chunks_exact_mut(4) {
        let a = chunk[3] as f32;
        if a > 0.0 && a < 255.0 {
            let alpha_factor = 255.0 / a;
            chunk[0] = (chunk[0] as f32 * alpha_factor).min(255.0) as u8;
            chunk[1] = (chunk[1] as f32 * alpha_factor).min(255.0) as u8;
            chunk[2] = (chunk[2] as f32 * alpha_factor).min(255.0) as u8;
        }
    }
    data
}

//! Drawing operations: fill, stroke, clip, fill rules, gradients, and patterns.

use crate::CanvasResource;
use deno_core::op2;
use deno_core::{OpState, ResourceId};
use deno_error::JsErrorBox;
use vl_convert_canvas2d::{CanvasColor, RadialGradientParams, RectParams};

// --- Fill / stroke / clip ---

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

// --- Fill rules ---

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

// --- Patterns ---

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

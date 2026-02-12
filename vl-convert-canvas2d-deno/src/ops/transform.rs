//! Transform operations: translate, rotate, scale, and matrix manipulation.

use crate::CanvasResource;
use deno_core::op2;
use deno_core::{OpState, ResourceId};
use deno_error::JsErrorBox;
use vl_convert_canvas2d::DOMMatrix;

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

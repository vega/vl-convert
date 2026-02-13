//! Deno ops for Canvas 2D API.
//!
//! These ops wrap the vl-convert-canvas2d crate and expose the Canvas 2D API
//! to JavaScript code running in Deno.

mod drawing;
mod image_ops;
mod path_ops;
mod state;
mod text;
mod transform;

pub use drawing::*;
pub use image_ops::*;
pub use path_ops::*;
pub use state::*;
pub use text::*;
pub use transform::*;

use crate::{CanvasResource, SharedFontConfig};
use deno_core::op2;
use deno_core::{OpState, ResourceId};
use deno_error::JsErrorBox;
use vl_convert_canvas2d::Canvas2dContext;

// --- Canvas creation and lifecycle ---

/// Create a new canvas with the given dimensions.
/// If a SharedFontConfig is available in OpState, it will be used for the canvas.
#[op2(fast)]
pub fn op_canvas_create(state: &mut OpState, width: u32, height: u32) -> Result<u32, JsErrorBox> {
    let ctx = if let Some(shared_config) = state.try_borrow::<SharedFontConfig>() {
        Canvas2dContext::with_resolved(width, height, &shared_config.0)
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

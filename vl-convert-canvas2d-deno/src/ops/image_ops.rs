//! Image drawing, pixel data, PNG export, and image decoding operations.

use crate::CanvasResource;
use deno_core::op2;
use deno_core::{OpState, ResourceId};
use deno_error::JsErrorBox;
use serde::Serialize;
use vl_convert_canvas2d::{CanvasImageDataRef, DirtyRect, ImageCropParams};

// --- drawImage (from RGBA pixel data) ---

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

// --- drawCanvas (canvas to canvas) ---

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

// --- putImageData ---

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

// --- getImageData / PNG export / canvas dimensions ---

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

// --- Helpers ---

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

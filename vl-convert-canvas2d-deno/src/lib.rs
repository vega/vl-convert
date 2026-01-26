// Allow uninlined format args for cleaner bail!/anyhow! macros
#![allow(clippy::uninlined_format_args)]

//! Deno extension for Canvas 2D API.
//!
//! This crate provides a Deno extension that wraps the vl-convert-canvas2d crate,
//! enabling JavaScript code to use Canvas 2D API via Rust ops.
//!
//! Key insight: Ops are never *called* during snapshot creation - they're just
//! registered. So we can use the real implementations in both build.rs and runtime.

mod ops;
mod resource;

use std::sync::Arc;

pub use ops::*;
pub use resource::{CanvasResource, Path2DResource};

/// Shared font database for canvas contexts.
/// This allows vl-convert-rs to pass its configured fontdb to canvas contexts.
#[derive(Clone)]
pub struct SharedFontDb(pub Arc<fontdb::Database>);

impl SharedFontDb {
    pub fn new(db: fontdb::Database) -> Self {
        Self(Arc::new(db))
    }

    pub fn from_arc(db: Arc<fontdb::Database>) -> Self {
        Self(db)
    }
}

// Define the extension with ops and ESM files
deno_core::extension!(
    vl_convert_canvas2d,
    ops = [
        op_canvas_create,
        op_canvas_destroy,
        op_canvas_save,
        op_canvas_restore,
        op_canvas_set_fill_style,
        op_canvas_set_stroke_style,
        op_canvas_set_line_width,
        op_canvas_set_line_cap,
        op_canvas_set_line_join,
        op_canvas_set_miter_limit,
        op_canvas_set_global_alpha,
        op_canvas_set_global_composite_operation,
        op_canvas_set_font,
        op_canvas_set_text_align,
        op_canvas_set_text_baseline,
        op_canvas_measure_text,
        op_canvas_fill_text,
        op_canvas_stroke_text,
        op_canvas_begin_path,
        op_canvas_move_to,
        op_canvas_line_to,
        op_canvas_close_path,
        op_canvas_bezier_curve_to,
        op_canvas_quadratic_curve_to,
        op_canvas_rect,
        op_canvas_arc,
        op_canvas_arc_to,
        op_canvas_ellipse,
        op_canvas_fill,
        op_canvas_stroke,
        op_canvas_fill_rect,
        op_canvas_stroke_rect,
        op_canvas_clear_rect,
        op_canvas_clip,
        op_canvas_translate,
        op_canvas_rotate,
        op_canvas_scale,
        op_canvas_transform,
        op_canvas_set_transform,
        op_canvas_reset_transform,
        op_canvas_set_line_dash,
        op_canvas_set_line_dash_offset,
        op_canvas_get_image_data,
        op_canvas_to_png,
        op_canvas_width,
        op_canvas_height,
        // Phase 1: Gradients
        op_canvas_create_linear_gradient,
        op_canvas_create_radial_gradient,
        op_canvas_gradient_add_color_stop,
        op_canvas_set_fill_style_gradient,
        op_canvas_set_stroke_style_gradient,
        // Phase 1: Text with maxWidth
        op_canvas_fill_text_max_width,
        op_canvas_stroke_text_max_width,
        // Phase 1: drawImage
        op_canvas_draw_image,
        op_canvas_draw_image_scaled,
        op_canvas_draw_image_cropped,
        op_canvas_draw_canvas,
        op_canvas_draw_canvas_scaled,
        op_canvas_draw_canvas_cropped,
        // Phase 2: Patterns
        op_canvas_create_pattern,
        op_canvas_create_pattern_from_canvas,
        op_canvas_set_fill_style_pattern,
        op_canvas_set_stroke_style_pattern,
        // Phase 2: putImageData
        op_canvas_put_image_data,
        op_canvas_put_image_data_dirty,
        // Phase 2: imageSmoothingEnabled/Quality
        op_canvas_set_image_smoothing_enabled,
        op_canvas_get_image_smoothing_enabled,
        op_canvas_set_image_smoothing_quality,
        op_canvas_get_image_smoothing_quality,
        // Phase 2: fillRule support
        op_canvas_fill_with_rule,
        op_canvas_clip_with_rule,
        // Phase 2: Path2D
        op_path2d_create,
        op_path2d_create_from_svg,
        op_path2d_create_from_path,
        op_path2d_destroy,
        op_path2d_move_to,
        op_path2d_line_to,
        op_path2d_close_path,
        op_path2d_bezier_curve_to,
        op_path2d_quadratic_curve_to,
        op_path2d_rect,
        op_path2d_arc,
        op_path2d_arc_to,
        op_path2d_ellipse,
        op_canvas_fill_path2d,
        op_canvas_fill_path2d_with_rule,
        op_canvas_stroke_path2d,
        op_canvas_clip_path2d,
        op_canvas_clip_path2d_with_rule,
        // Phase 3: Nice-to-Have
        op_canvas_reset,
        op_canvas_round_rect,
        op_canvas_round_rect_radii,
        op_canvas_set_letter_spacing,
        op_canvas_get_letter_spacing,
        op_canvas_get_transform,
        op_path2d_round_rect,
        op_path2d_round_rect_radii,
        // Image decoding
        op_canvas_decode_image,
        op_canvas_get_image_info,
        #[cfg(feature = "svg")]
        op_canvas_decode_svg_at_size,
        // Logging
        op_canvas_log,
    ],
    esm_entry_point = "ext:vl_convert_canvas2d/canvas_polyfill.js",
    esm = [
        dir "src/js",
        "canvas_polyfill.js",
    ],
);

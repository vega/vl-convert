//! Build script for vl-convert-rs
//!
//! Generates a V8 snapshot at build time that embeds the deno_runtime extensions
//! PLUS our vl_convert_runtime extension. This is required for container compatibility
//! (manylinux, slim images) and improves startup performance.
//!
//! Uses `deno_runtime::snapshot::create_runtime_snapshot()` which:
//! 1. Includes all deno_runtime extensions in the correct order
//! 2. Allows adding custom extensions at the end
//! 3. Produces a snapshot compatible with MainWorker
//!
//! Key insight from studying the Deno blog posts and source code:
//! - MainWorker expects extensions in a specific order
//! - The snapshot must include ALL of these extensions in the same order
//! - deno_runtime provides `create_runtime_snapshot()` for this purpose

use deno_core::extension;
use deno_core::op2;
use deno_error::JsErrorBox;
use deno_runtime::ops::bootstrap::SnapshotOptions;
use deno_runtime::snapshot::create_runtime_snapshot;
use std::path::PathBuf;

// Stub ops for snapshot creation - these match the signatures of the real ops
// but will never be called during snapshot creation. At runtime, the real ops
// are registered and used instead.

#[op2]
#[string]
fn op_get_json_arg(_arg_id: i32) -> Result<String, JsErrorBox> {
    Err(JsErrorBox::generic(
        "op_get_json_arg stub called during snapshot creation",
    ))
}

#[op2(fast)]
fn op_text_width(#[string] _text_info_str: String) -> Result<f64, JsErrorBox> {
    Err(JsErrorBox::generic(
        "op_text_width stub called during snapshot creation",
    ))
}

// Canvas 2D stub ops - these match the signatures in src/canvas2d/ops.rs
// They will never be called during snapshot creation

#[op2(fast)]
fn op_canvas_create(_width: u32, _height: u32) -> Result<u32, JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_destroy(_rid: u32) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_save(_rid: u32) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_restore(_rid: u32) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_fill_style(_rid: u32, #[string] _style: String) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_stroke_style(_rid: u32, #[string] _style: String) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_line_width(_rid: u32, _width: f64) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_line_cap(_rid: u32, #[string] _cap: String) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_line_join(_rid: u32, #[string] _join: String) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_miter_limit(_rid: u32, _limit: f64) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_global_alpha(_rid: u32, _alpha: f64) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_global_composite_operation(
    _rid: u32,
    #[string] _op: String,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_font(_rid: u32, #[string] _font: String) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_text_align(_rid: u32, #[string] _align: String) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_text_baseline(_rid: u32, #[string] _baseline: String) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_measure_text(_rid: u32, #[string] _text: String) -> Result<f64, JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_fill_text(
    _rid: u32,
    #[string] _text: String,
    _x: f64,
    _y: f64,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_stroke_text(
    _rid: u32,
    #[string] _text: String,
    _x: f64,
    _y: f64,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_begin_path(_rid: u32) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_move_to(_rid: u32, _x: f64, _y: f64) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_line_to(_rid: u32, _x: f64, _y: f64) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_close_path(_rid: u32) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_bezier_curve_to(
    _rid: u32,
    _cp1x: f64,
    _cp1y: f64,
    _cp2x: f64,
    _cp2y: f64,
    _x: f64,
    _y: f64,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_quadratic_curve_to(
    _rid: u32,
    _cpx: f64,
    _cpy: f64,
    _x: f64,
    _y: f64,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_rect(
    _rid: u32,
    _x: f64,
    _y: f64,
    _width: f64,
    _height: f64,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_arc(
    _rid: u32,
    _x: f64,
    _y: f64,
    _radius: f64,
    _start_angle: f64,
    _end_angle: f64,
    _anticlockwise: bool,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_arc_to(
    _rid: u32,
    _x1: f64,
    _y1: f64,
    _x2: f64,
    _y2: f64,
    _radius: f64,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_ellipse(
    _rid: u32,
    _x: f64,
    _y: f64,
    _radius_x: f64,
    _radius_y: f64,
    _rotation: f64,
    _start_angle: f64,
    _end_angle: f64,
    _anticlockwise: bool,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_fill(_rid: u32) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_stroke(_rid: u32) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_fill_rect(
    _rid: u32,
    _x: f64,
    _y: f64,
    _width: f64,
    _height: f64,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_stroke_rect(
    _rid: u32,
    _x: f64,
    _y: f64,
    _width: f64,
    _height: f64,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_clear_rect(
    _rid: u32,
    _x: f64,
    _y: f64,
    _width: f64,
    _height: f64,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_clip(_rid: u32) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_translate(_rid: u32, _x: f64, _y: f64) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_rotate(_rid: u32, _angle: f64) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_scale(_rid: u32, _x: f64, _y: f64) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_transform(
    _rid: u32,
    _a: f64,
    _b: f64,
    _c: f64,
    _d: f64,
    _e: f64,
    _f: f64,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_transform(
    _rid: u32,
    _a: f64,
    _b: f64,
    _c: f64,
    _d: f64,
    _e: f64,
    _f: f64,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_reset_transform(_rid: u32) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2]
fn op_canvas_set_line_dash(_rid: u32, #[serde] _segments: Vec<f64>) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_set_line_dash_offset(_rid: u32, _offset: f64) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2]
#[serde]
fn op_canvas_get_image_data(
    _rid: u32,
    _x: i32,
    _y: i32,
    _width: u32,
    _height: u32,
) -> Result<Vec<u8>, JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2]
#[serde]
fn op_canvas_to_png(_rid: u32) -> Result<Vec<u8>, JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_width(_rid: u32) -> Result<u32, JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

#[op2(fast)]
fn op_canvas_height(_rid: u32) -> Result<u32, JsErrorBox> {
    Err(JsErrorBox::generic("stub"))
}

// Define the extension with lazy_init for snapshot creation
// This must match the extension defined in converter.rs
extension!(
    vl_convert_runtime,
    ops = [
        op_get_json_arg,
        op_text_width,
        // Canvas 2D ops
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
    ],
    esm_entry_point = "ext:vl_convert_runtime/bootstrap.js",
    esm = [
        dir "src/js",
        "canvas_polyfill.js",
        "bootstrap.js",
    ],
);

fn main() {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let snapshot_path = out_dir.join("VL_CONVERT_SNAPSHOT.bin");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/js/bootstrap.js");
    println!("cargo:rerun-if-changed=src/js/canvas_polyfill.js");
    println!("cargo:warning=Creating V8 snapshot at {snapshot_path:?}");

    // Use deno_runtime's create_runtime_snapshot which includes all
    // the built-in extensions in the correct order, plus our custom extension
    create_runtime_snapshot(
        snapshot_path,
        SnapshotOptions::default(),
        vec![vl_convert_runtime::lazy_init()], // Our extension added at the end
    );

    println!("cargo:warning=V8 snapshot created successfully");
}

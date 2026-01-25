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
//! Key insight: Ops are never *called* during snapshot creation - they're just
//! registered. So we can use the real implementations from vl-convert-canvas2d-deno
//! in both build.rs and runtime.

use deno_core::extension;
use deno_core::op2;
use deno_error::JsErrorBox;
use deno_runtime::ops::bootstrap::SnapshotOptions;
use deno_runtime::snapshot::create_runtime_snapshot;
use std::path::PathBuf;

// Stub ops for vl-convert-specific ops (not canvas-related)
// These match the signatures of the real ops but will never be called during snapshot creation.

#[op2]
#[string]
fn op_get_json_arg(_arg_id: i32) -> Result<String, JsErrorBox> {
    Err(JsErrorBox::generic(
        "op_get_json_arg stub called during snapshot creation",
    ))
}

// Define the extension with lazy_init for snapshot creation
// This must match the extension defined in converter.rs
extension!(
    vl_convert_runtime,
    ops = [
        op_get_json_arg,
    ],
    esm_entry_point = "ext:vl_convert_runtime/bootstrap.js",
    esm = [
        dir "src/js",
        "bootstrap.js",
    ],
);

fn main() {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let snapshot_path = out_dir.join("VL_CONVERT_SNAPSHOT.bin");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/js/bootstrap.js");
    println!("cargo:warning=Creating V8 snapshot at {snapshot_path:?}");

    // Use deno_runtime's create_runtime_snapshot which includes all
    // the built-in extensions in the correct order, plus our custom extensions
    create_runtime_snapshot(
        snapshot_path,
        SnapshotOptions::default(),
        vec![
            // Canvas 2D extension from vl-convert-canvas2d-deno crate
            vl_convert_canvas2d_deno::vl_convert_canvas2d::lazy_init(),
            // Our runtime extension (text width, JSON args)
            vl_convert_runtime::lazy_init(),
        ],
    );

    println!("cargo:warning=V8 snapshot created successfully");
}

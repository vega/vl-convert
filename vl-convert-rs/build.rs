//! Build script for vl-convert-rs
//!
//! When the `snapshot` feature is enabled, this generates a V8 snapshot at build time
//! that embeds the deno_runtime extensions PLUS our vl_convert_runtime extension.
//!
//! This uses `deno_runtime::snapshot::create_runtime_snapshot()` which:
//! 1. Includes all deno_runtime extensions in the correct order
//! 2. Allows adding custom extensions at the end
//! 3. Produces a snapshot compatible with MainWorker
//!
//! Key insight from studying the Deno blog posts and source code:
//! - MainWorker expects extensions in a specific order
//! - The snapshot must include ALL of these extensions in the same order
//! - deno_runtime provides `create_runtime_snapshot()` for this purpose

fn main() {
    #[cfg(feature = "snapshot")]
    {
        snapshot::create_vl_convert_snapshot();
    }
}

#[cfg(feature = "snapshot")]
mod snapshot {
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
        // This is a stub - never called during snapshot creation
        Err(JsErrorBox::generic(
            "op_get_json_arg stub called during snapshot creation",
        ))
    }

    #[op2(fast)]
    fn op_text_width(#[string] _text_info_str: String) -> Result<f64, JsErrorBox> {
        // This is a stub - never called during snapshot creation
        Err(JsErrorBox::generic(
            "op_text_width stub called during snapshot creation",
        ))
    }

    // Define the extension with lazy_init for snapshot creation
    // This must match the extension defined in converter.rs
    extension!(
        vl_convert_runtime,
        ops = [op_get_json_arg, op_text_width],
        esm_entry_point = "ext:vl_convert_runtime/bootstrap.js",
        esm = ["ext:vl_convert_runtime/bootstrap.js" = {
            source = r#"
                import { op_text_width, op_get_json_arg } from "ext:core/ops";

                // Expose our custom ops on globalThis for vega-scenegraph text measurement
                globalThis.op_text_width = op_text_width;
                globalThis.op_get_json_arg = op_get_json_arg;
            "#
        }],
    );

    pub fn create_vl_convert_snapshot() {
        let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let snapshot_path = out_dir.join("VL_CONVERT_SNAPSHOT.bin");

        println!("cargo:rerun-if-changed=build.rs");
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
}

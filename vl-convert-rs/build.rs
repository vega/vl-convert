//! Build script for vl-convert-rs
//!
//! Generates a V8 snapshot at build time that embeds the deno_runtime extensions
//! PLUS our vl_convert_runtime extension. This is required for container compatibility
//! (manylinux, slim images) and improves startup performance.
use deno_core::extension;
use deno_core::op2;
use deno_error::JsErrorBox;
use deno_runtime::ops::bootstrap::SnapshotOptions;
use deno_runtime::snapshot::create_runtime_snapshot;
use deno_runtime::snapshot::LazyExtensionFileKind;
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;
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

#[op2(fast)]
fn op_set_msgpack_result(_result_id: i32, #[buffer] _data: &[u8]) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic(
        "op_set_msgpack_result stub called during snapshot creation",
    ))
}

#[op2]
#[string]
async fn op_vega_data_fetch(#[string] _url: String) -> Result<String, JsErrorBox> {
    Err(JsErrorBox::generic("op_vega_data_fetch stub"))
}

#[op2]
#[buffer]
async fn op_vega_data_fetch_bytes(#[string] _url: String) -> Result<Vec<u8>, JsErrorBox> {
    Err(JsErrorBox::generic("op_vega_data_fetch_bytes stub"))
}

#[op2]
#[string]
async fn op_vega_file_read(#[string] _path: String) -> Result<String, JsErrorBox> {
    Err(JsErrorBox::generic("op_vega_file_read stub"))
}

#[op2]
#[buffer]
async fn op_vega_file_read_bytes(#[string] _path: String) -> Result<Vec<u8>, JsErrorBox> {
    Err(JsErrorBox::generic("op_vega_file_read_bytes stub"))
}

extension!(
    vl_convert_runtime,
    ops = [
        op_get_json_arg,
        op_set_msgpack_result,
        op_vega_data_fetch,
        op_vega_data_fetch_bytes,
        op_vega_file_read,
        op_vega_file_read_bytes,
    ],
    esm_entry_point = "ext:vl_convert_runtime/bootstrap.js",
    esm = [
        dir "src/js",
        "bootstrap.js",
    ],
);

fn transpile_residual_source(
    out_dir: &Path,
    specifier: &str,
    src_path: &Path,
    minify: bool,
) -> PathBuf {
    use deno_runtime::deno_core::ModuleCodeString;
    use deno_runtime::deno_core::ModuleName;
    use deno_runtime::transpile::maybe_transpile_and_minify_source;
    use deno_runtime::transpile::maybe_transpile_source;

    let source = std::fs::read_to_string(src_path).unwrap_or_else(|e| {
        panic!(
            "failed to read residual lazy source {}: {e}",
            src_path.display()
        )
    });
    let name = ModuleName::from(specifier.to_string());
    let source = ModuleCodeString::from(source);
    let (transpiled, _source_map) = if minify {
        maybe_transpile_and_minify_source(name, source)
    } else {
        maybe_transpile_source(name, source)
    }
    .unwrap_or_else(|e| panic!("failed to transpile residual lazy source {specifier}: {e}"));

    let sanitized: String = specifier
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let out_path = out_dir.join(format!("{sanitized}.js"));
    std::fs::write(&out_path, transpiled.as_bytes()).unwrap();
    out_path
}

fn wrap_residual_js_source(path: &Path) {
    let source = std::fs::read_to_string(path).unwrap();
    let wrapped = deno_runtime::deno_core::wrap_lazy_ext_script(&source);
    std::fs::write(path, wrapped.as_bytes()).unwrap();
}

fn assert_residual_entry_ascii(path: &Path, specifier: &str) {
    assert!(
        specifier.is_ascii(),
        "generated residual source specifier {specifier} contains non-ASCII bytes"
    );
    let source = std::fs::read(path).unwrap();
    assert!(
        source.is_ascii(),
        "generated residual source for {specifier} at {} contains non-ASCII bytes",
        path.display()
    );
}

fn write_residual_table(
    file: &mut std::fs::File,
    out_dir: &Path,
    name: &str,
    entries: &[(&str, PathBuf)],
) {
    writeln!(file, "pub(crate) static {name}: &[(&str, &str)] = &[").unwrap();
    let mut entries = entries.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(specifier, _)| *specifier);
    for (specifier, transpiled_path) in entries {
        let relative_path = transpiled_path.strip_prefix(out_dir).unwrap();
        writeln!(
            file,
            "  ({specifier:?}, include_str!(concat!(env!(\"OUT_DIR\"), {:?}))),",
            format!("/{}", relative_path.display())
        )
        .unwrap();
    }
    writeln!(file, "];").unwrap();
}

fn main() {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let snapshot_path = out_dir.join("VL_CONVERT_SNAPSHOT.bin");
    let residual_path = out_dir.join("VL_CONVERT_RESIDUAL_SOURCES.rs");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/js/bootstrap.js");
    println!("cargo:rerun-if-changed=src/data_ops.rs");
    println!("cargo:rerun-if-env-changed=DENO_SNAPSHOT_IMPORT_GRAPH");
    println!("cargo:rerun-if-env-changed=DENO_SNAPSHOT_MINIFY_SOURCES");
    // Use deno_runtime's create_runtime_snapshot which includes all
    // the built-in extensions in the correct order, plus our custom extensions
    let output = create_runtime_snapshot(
        snapshot_path,
        SnapshotOptions::default(),
        vec![
            // Canvas 2D extension from vl-convert-canvas2d-deno crate
            vl_convert_canvas2d_deno::vl_convert_canvas2d::lazy_init(),
            // Our runtime extension (text width, JSON args)
            vl_convert_runtime::lazy_init(),
        ],
    );

    // Deno's snapshot only contains lazy extension modules that were reached
    // while the snapshot was created. Embed the rest so they can be loaded on
    // demand by the runtime.
    let consumed: HashSet<&str> = output
        .consumed_lazy_specifiers
        .iter()
        .map(String::as_str)
        .collect();
    let residual_sources_dir = out_dir.join("residual_sources");
    std::fs::create_dir_all(&residual_sources_dir).unwrap();
    let minify_sources = std::env::var_os("DENO_SNAPSHOT_MINIFY_SOURCES").is_some();
    let mut residual_js = Vec::new();
    let mut residual_esm = Vec::new();

    for file in &output.lazy_extension_files {
        if consumed.contains(file.specifier.as_str()) {
            continue;
        }

        println!("cargo:rerun-if-changed={}", file.path.display());
        let transpiled_path = transpile_residual_source(
            &residual_sources_dir,
            &file.specifier,
            &file.path,
            minify_sources,
        );
        match file.kind {
            LazyExtensionFileKind::Js => {
                wrap_residual_js_source(&transpiled_path);
                assert_residual_entry_ascii(&transpiled_path, &file.specifier);
                residual_js.push((file.specifier.as_str(), transpiled_path));
            }
            LazyExtensionFileKind::Esm => {
                assert_residual_entry_ascii(&transpiled_path, &file.specifier);
                residual_esm.push((file.specifier.as_str(), transpiled_path));
            }
        }
    }

    let mut residual_file = std::fs::File::create(residual_path).unwrap();
    writeln!(
        residual_file,
        "// @generated by vl-convert-rs/build.rs; do not edit."
    )
    .unwrap();
    write_residual_table(
        &mut residual_file,
        &out_dir,
        "VL_CONVERT_RESIDUAL_LAZY_JS",
        &residual_js,
    );
    write_residual_table(
        &mut residual_file,
        &out_dir,
        "VL_CONVERT_RESIDUAL_LAZY_ESM",
        &residual_esm,
    );
}

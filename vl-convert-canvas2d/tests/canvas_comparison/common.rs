//! Shared test infrastructure for canvas comparison tests.
//!
//! Each test renders the same drawing operations in both Rust and JavaScript (node-canvas),
//! then compares the resulting images using pixelmatch algorithm.

use pixelmatch::pixelmatch;
use std::fs::File;
use std::io::BufReader;
use std::process::Command;
use std::sync::OnceLock;
use tempfile::TempDir;

// Re-export commonly used types so test modules can `use super::common::*`
pub use std::f32::consts::PI;
pub use vl_convert_canvas2d::{
    ArcParams, ArcToParams, Canvas2dContext, CanvasColor, CanvasFillRule, CornerRadius,
    CubicBezierParams, DOMMatrix, DirtyRect, EllipseParams, Path2D, QuadraticBezierParams,
    RadialGradientParams, RectParams, RoundRectParams, TextAlign, TextBaseline,
};

/// Get path to node_modules for canvas tests.
fn get_node_modules_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("node_baseline")
        .join("node_modules")
}

/// Check if node-canvas is available.
pub fn node_canvas_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let node_modules = get_node_modules_path();

        let output = Command::new("node")
            .arg("-e")
            .arg("require('canvas')")
            .env("NODE_PATH", &node_modules)
            .output();
        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    })
}

/// Skip test if node-canvas is not available.
macro_rules! skip_if_no_node_canvas {
    () => {
        if !crate::common::node_canvas_available() {
            eprintln!("Skipping test: node-canvas not available. Install with: npm install canvas");
            return;
        }
    };
}
pub(crate) use skip_if_no_node_canvas;

/// Threshold for pixel comparison (0-255). Higher values allow more difference.
/// Text rendering typically needs higher tolerance due to font differences.
pub const DEFAULT_THRESHOLD: u8 = 20;
pub const TEXT_THRESHOLD: u8 = 30;

/// Maximum percentage of pixels that can differ before test fails.
pub const MAX_DIFF_PERCENT: f64 = 1.0;
pub const TEXT_MAX_DIFF_PERCENT: f64 = 2.0;

/// Test case definition for canvas comparison.
pub struct CanvasTestCase {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    /// JavaScript code to run with node-canvas. The canvas context is available as `ctx`.
    pub js_code: &'static str,
    /// Rust code to execute on the context. Takes &mut Canvas2dContext.
    pub rust_fn: fn(&mut Canvas2dContext),
    /// Threshold for pixel matching (0-255).
    pub threshold: u8,
    /// Maximum percentage of pixels that can differ.
    pub max_diff_percent: f64,
}

/// Generate node-canvas JavaScript wrapper code.
fn generate_node_script(js_code: &str, width: u32, height: u32, output_path: &str) -> String {
    format!(
        r#"
const {{ createCanvas }} = require('canvas');
const fs = require('fs');

const canvas = createCanvas({width}, {height});
const ctx = canvas.getContext('2d');

// Run the test code
{js_code}

// Save to PNG
const buffer = canvas.toBuffer('image/png');
fs.writeFileSync('{output_path}', buffer);
"#,
        width = width,
        height = height,
        js_code = js_code,
        output_path = output_path.replace('\\', "\\\\").replace('\'', "\\'")
    )
}

/// Run a canvas comparison test.
pub fn run_comparison_test(test: &CanvasTestCase) -> Result<(), String> {
    // Create temp directory for outputs
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let rust_png_path = temp_dir.path().join("rust_output.png");
    let node_png_path = temp_dir.path().join("node_output.png");
    let diff_png_path = temp_dir.path().join("diff.png");
    let script_path = temp_dir.path().join("test_script.js");

    // 1. Generate Rust canvas output
    let mut ctx = Canvas2dContext::new(test.width, test.height)
        .map_err(|e| format!("Failed to create canvas: {}", e))?;
    (test.rust_fn)(&mut ctx);
    let rust_png = ctx
        .to_png(None)
        .map_err(|e| format!("Failed to export PNG: {}", e))?;
    std::fs::write(&rust_png_path, &rust_png)
        .map_err(|e| format!("Failed to write Rust PNG: {}", e))?;

    // 2. Generate node-canvas output
    let node_script = generate_node_script(
        test.js_code,
        test.width,
        test.height,
        node_png_path.to_str().unwrap(),
    );
    std::fs::write(&script_path, &node_script)
        .map_err(|e| format!("Failed to write Node script: {}", e))?;

    // Set NODE_PATH to find canvas module in tests/node_baseline/node_modules
    let node_modules = get_node_modules_path();

    let node_output = Command::new("node")
        .arg(&script_path)
        .env("NODE_PATH", &node_modules)
        .output()
        .map_err(|e| format!("Failed to run node: {}", e))?;

    if !node_output.status.success() {
        return Err(format!(
            "Node script failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&node_output.stdout),
            String::from_utf8_lossy(&node_output.stderr)
        ));
    }

    // 3. Compare images using pixelmatch
    let rust_file = BufReader::new(
        File::open(&rust_png_path).map_err(|e| format!("Failed to open Rust PNG: {}", e))?,
    );
    let node_file = BufReader::new(
        File::open(&node_png_path).map_err(|e| format!("Failed to open Node PNG: {}", e))?,
    );
    let mut diff_file =
        File::create(&diff_png_path).map_err(|e| format!("Failed to create diff PNG: {}", e))?;

    let diff_count = pixelmatch(
        rust_file,
        node_file,
        Some(&mut diff_file),
        Some(test.width),
        Some(test.height),
        Some(pixelmatch::Options {
            threshold: test.threshold as f64 / 255.0,
            ..Default::default()
        }),
    )
    .map_err(|e| format!("Pixelmatch error: {:?}", e))?;

    let total = test.width * test.height;
    let diff_percent = (diff_count as f64 / total as f64) * 100.0;

    // Always save images for inspection
    let output_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_output")
        .join(test.name);
    let _ = std::fs::create_dir_all(&output_dir);
    let _ = std::fs::copy(&rust_png_path, output_dir.join("rust.png"));
    let _ = std::fs::copy(&node_png_path, output_dir.join("node.png"));
    let _ = std::fs::copy(&diff_png_path, output_dir.join("diff.png"));

    if diff_percent > test.max_diff_percent {
        return Err(format!(
            "Image difference too high: {:.2}% ({} of {} pixels differ). \
             Threshold: {}, Max allowed: {:.2}%. \
             Debug images saved to {:?}",
            diff_percent, diff_count, total, test.threshold, test.max_diff_percent, output_dir
        ));
    }

    Ok(())
}

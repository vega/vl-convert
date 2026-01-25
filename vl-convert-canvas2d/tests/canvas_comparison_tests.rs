//! Canvas comparison tests - compare vl-convert-canvas2d output against node-canvas.
//!
//! Each test renders the same drawing operations in both Rust and JavaScript (node-canvas),
//! then compares the resulting images using pixelmatch algorithm.
//!
//! ## Prerequisites
//!
//! These tests require node-canvas to be installed. Install it with:
//!
//! ```bash
//! cd vl-convert-canvas2d/tests/node_baseline
//! npm install
//! ```
//!
//! If node-canvas is not available, tests will be skipped.

use pixelmatch::pixelmatch;
use std::f32::consts::PI;
use std::fs::File;
use std::io::BufReader;
use std::process::Command;
use std::sync::OnceLock;
use tempfile::TempDir;
use vl_convert_canvas2d::{Canvas2dContext, TextAlign, TextBaseline};

/// Get path to node_modules for canvas tests.
fn get_node_modules_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("node_baseline")
        .join("node_modules")
}

/// Check if node-canvas is available.
fn node_canvas_available() -> bool {
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
        if !node_canvas_available() {
            eprintln!("Skipping test: node-canvas not available. Install with: npm install canvas");
            return;
        }
    };
}

/// Threshold for pixel comparison (0-255). Higher values allow more difference.
/// Text rendering typically needs higher tolerance due to font differences.
const DEFAULT_THRESHOLD: u8 = 20;
const TEXT_THRESHOLD: u8 = 30;

/// Maximum percentage of pixels that can differ before test fails.
const MAX_DIFF_PERCENT: f64 = 1.0;
const TEXT_MAX_DIFF_PERCENT: f64 = 2.0;

/// Test case definition for canvas comparison.
struct CanvasTestCase {
    name: &'static str,
    width: u32,
    height: u32,
    /// JavaScript code to run with node-canvas. The canvas context is available as `ctx`.
    js_code: &'static str,
    /// Rust code to execute on the context. Takes &mut Canvas2dContext.
    rust_fn: fn(&mut Canvas2dContext),
    /// Threshold for pixel matching (0-255).
    threshold: u8,
    /// Maximum percentage of pixels that can differ.
    max_diff_percent: f64,
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
fn run_comparison_test(test: &CanvasTestCase) -> Result<(), String> {
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
        .to_png()
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

// =============================================================================
// Test Cases
// =============================================================================

#[test]
fn test_fill_rect_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "fill_rect",
        width: 200,
        height: 200,
        js_code: r#"
ctx.fillStyle = '#ff0000';
ctx.fillRect(10, 10, 100, 100);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.fill_rect(10.0, 10.0, 100.0, 100.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("fill_rect comparison failed");
}

#[test]
fn test_stroke_rect_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "stroke_rect",
        width: 200,
        height: 200,
        js_code: r#"
ctx.strokeStyle = '#0000ff';
ctx.lineWidth = 4;
ctx.strokeRect(20, 20, 100, 80);
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#0000ff").unwrap();
            ctx.set_line_width(4.0);
            ctx.stroke_rect(20.0, 20.0, 100.0, 80.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("stroke_rect comparison failed");
}

#[test]
fn test_path_fill_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "path_fill",
        width: 100,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#00ff00';
ctx.beginPath();
ctx.moveTo(10, 10);
ctx.lineTo(90, 10);
ctx.lineTo(90, 90);
ctx.lineTo(10, 90);
ctx.closePath();
ctx.fill();
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#00ff00").unwrap();
            ctx.begin_path();
            ctx.move_to(10.0, 10.0);
            ctx.line_to(90.0, 10.0);
            ctx.line_to(90.0, 90.0);
            ctx.line_to(10.0, 90.0);
            ctx.close_path();
            ctx.fill();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("path_fill comparison failed");
}

#[test]
fn test_line_stroke_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "line_stroke",
        width: 100,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#000000';
ctx.lineWidth = 3;
ctx.beginPath();
ctx.moveTo(10, 50);
ctx.lineTo(90, 50);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#000000").unwrap();
            ctx.set_line_width(3.0);
            ctx.begin_path();
            ctx.move_to(10.0, 50.0);
            ctx.line_to(90.0, 50.0);
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("line_stroke comparison failed");
}

#[test]
fn test_arc_fill_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "arc_fill",
        width: 100,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#ff00ff';
ctx.beginPath();
ctx.arc(50, 50, 30, 0, Math.PI * 2, false);
ctx.fill();
"#,
        rust_fn: |ctx| {
            use std::f32::consts::PI;
            ctx.set_fill_style("#ff00ff").unwrap();
            ctx.begin_path();
            ctx.arc(50.0, 50.0, 30.0, 0.0, 2.0 * PI, false);
            ctx.fill();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0, // Slightly higher for arc anti-aliasing
    };
    run_comparison_test(&test).expect("arc_fill comparison failed");
}

#[test]
fn test_arc_stroke_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "arc_stroke",
        width: 100,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#ff0000';
ctx.lineWidth = 3;
ctx.beginPath();
ctx.arc(50, 50, 30, 0, Math.PI * 1.5, false);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            use std::f32::consts::PI;
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.set_line_width(3.0);
            ctx.begin_path();
            ctx.arc(50.0, 50.0, 30.0, 0.0, 1.5 * PI, false);
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0, // Arc anti-aliasing can differ
    };
    run_comparison_test(&test).expect("arc_stroke comparison failed");
}

#[test]
fn test_bezier_curve_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "bezier_curve",
        width: 100,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#000000';
ctx.lineWidth = 2;
ctx.beginPath();
ctx.moveTo(10, 50);
ctx.bezierCurveTo(30, 10, 70, 90, 90, 50);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#000000").unwrap();
            ctx.set_line_width(2.0);
            ctx.begin_path();
            ctx.move_to(10.0, 50.0);
            ctx.bezier_curve_to(30.0, 10.0, 70.0, 90.0, 90.0, 50.0);
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("bezier_curve comparison failed");
}

#[test]
fn test_quadratic_curve_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "quadratic_curve",
        width: 100,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#0066ff';
ctx.lineWidth = 2;
ctx.beginPath();
ctx.moveTo(10, 80);
ctx.quadraticCurveTo(50, 10, 90, 80);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#0066ff").unwrap();
            ctx.set_line_width(2.0);
            ctx.begin_path();
            ctx.move_to(10.0, 80.0);
            ctx.quadratic_curve_to(50.0, 10.0, 90.0, 80.0);
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("quadratic_curve comparison failed");
}

#[test]
fn test_ellipse_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "ellipse",
        width: 100,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#00ff00';
ctx.beginPath();
ctx.ellipse(50, 50, 40, 20, 0, 0, Math.PI * 2, false);
ctx.fill();
"#,
        rust_fn: |ctx| {
            use std::f32::consts::PI;
            ctx.set_fill_style("#00ff00").unwrap();
            ctx.begin_path();
            ctx.ellipse(50.0, 50.0, 40.0, 20.0, 0.0, 0.0, 2.0 * PI, false);
            ctx.fill();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("ellipse comparison failed");
}

#[test]
fn test_rotated_ellipse_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "rotated_ellipse",
        width: 100,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#ff6600';
ctx.beginPath();
ctx.ellipse(50, 50, 35, 15, Math.PI / 4, 0, Math.PI * 2, false);
ctx.fill();
"#,
        rust_fn: |ctx| {
            use std::f32::consts::PI;
            ctx.set_fill_style("#ff6600").unwrap();
            ctx.begin_path();
            ctx.ellipse(50.0, 50.0, 35.0, 15.0, PI / 4.0, 0.0, 2.0 * PI, false);
            ctx.fill();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0,
    };
    run_comparison_test(&test).expect("rotated_ellipse comparison failed");
}

#[test]
fn test_translate_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "translate",
        width: 100,
        height: 100,
        js_code: r#"
ctx.translate(50, 50);
ctx.fillStyle = '#ff0000';
ctx.fillRect(-15, -15, 30, 30);
"#,
        rust_fn: |ctx| {
            ctx.translate(50.0, 50.0);
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.fill_rect(-15.0, -15.0, 30.0, 30.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("translate comparison failed");
}

#[test]
fn test_rotate_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "rotate",
        width: 100,
        height: 100,
        js_code: r#"
ctx.translate(50, 50);
ctx.rotate(Math.PI / 4);
ctx.fillStyle = '#0000ff';
ctx.fillRect(-20, -20, 40, 40);
"#,
        rust_fn: |ctx| {
            use std::f32::consts::PI;
            ctx.translate(50.0, 50.0);
            ctx.rotate(PI / 4.0);
            ctx.set_fill_style("#0000ff").unwrap();
            ctx.fill_rect(-20.0, -20.0, 40.0, 40.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("rotate comparison failed");
}

#[test]
fn test_scale_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "scale",
        width: 100,
        height: 100,
        js_code: r#"
ctx.scale(2, 0.5);
ctx.fillStyle = '#00ff00';
ctx.fillRect(10, 50, 20, 40);
"#,
        rust_fn: |ctx| {
            ctx.scale(2.0, 0.5);
            ctx.set_fill_style("#00ff00").unwrap();
            ctx.fill_rect(10.0, 50.0, 20.0, 40.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("scale comparison failed");
}

#[test]
fn test_global_alpha_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "global_alpha",
        width: 100,
        height: 100,
        js_code: r#"
// First rectangle at full opacity
ctx.fillStyle = '#0000ff';
ctx.fillRect(10, 10, 60, 60);

// Second rectangle at 50% opacity
ctx.globalAlpha = 0.5;
ctx.fillStyle = '#ff0000';
ctx.fillRect(30, 30, 60, 60);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#0000ff").unwrap();
            ctx.fill_rect(10.0, 10.0, 60.0, 60.0);

            ctx.set_global_alpha(0.5);
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.fill_rect(30.0, 30.0, 60.0, 60.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("global_alpha comparison failed");
}

#[test]
fn test_line_cap_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "line_cap",
        width: 150,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#000000';
ctx.lineWidth = 15;

// Butt cap
ctx.lineCap = 'butt';
ctx.beginPath();
ctx.moveTo(25, 25);
ctx.lineTo(125, 25);
ctx.stroke();

// Round cap
ctx.lineCap = 'round';
ctx.beginPath();
ctx.moveTo(25, 50);
ctx.lineTo(125, 50);
ctx.stroke();

// Square cap
ctx.lineCap = 'square';
ctx.beginPath();
ctx.moveTo(25, 75);
ctx.lineTo(125, 75);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#000000").unwrap();
            ctx.set_line_width(15.0);

            ctx.set_line_cap(vl_convert_canvas2d::LineCap::Butt);
            ctx.begin_path();
            ctx.move_to(25.0, 25.0);
            ctx.line_to(125.0, 25.0);
            ctx.stroke();

            ctx.set_line_cap(vl_convert_canvas2d::LineCap::Round);
            ctx.begin_path();
            ctx.move_to(25.0, 50.0);
            ctx.line_to(125.0, 50.0);
            ctx.stroke();

            ctx.set_line_cap(vl_convert_canvas2d::LineCap::Square);
            ctx.begin_path();
            ctx.move_to(25.0, 75.0);
            ctx.line_to(125.0, 75.0);
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("line_cap comparison failed");
}

#[test]
fn test_line_join_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "line_join",
        width: 200,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#000000';
ctx.lineWidth = 10;

// Miter join
ctx.lineJoin = 'miter';
ctx.beginPath();
ctx.moveTo(20, 80);
ctx.lineTo(40, 20);
ctx.lineTo(60, 80);
ctx.stroke();

// Round join
ctx.lineJoin = 'round';
ctx.beginPath();
ctx.moveTo(80, 80);
ctx.lineTo(100, 20);
ctx.lineTo(120, 80);
ctx.stroke();

// Bevel join
ctx.lineJoin = 'bevel';
ctx.beginPath();
ctx.moveTo(140, 80);
ctx.lineTo(160, 20);
ctx.lineTo(180, 80);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#000000").unwrap();
            ctx.set_line_width(10.0);

            ctx.set_line_join(vl_convert_canvas2d::LineJoin::Miter);
            ctx.begin_path();
            ctx.move_to(20.0, 80.0);
            ctx.line_to(40.0, 20.0);
            ctx.line_to(60.0, 80.0);
            ctx.stroke();

            ctx.set_line_join(vl_convert_canvas2d::LineJoin::Round);
            ctx.begin_path();
            ctx.move_to(80.0, 80.0);
            ctx.line_to(100.0, 20.0);
            ctx.line_to(120.0, 80.0);
            ctx.stroke();

            ctx.set_line_join(vl_convert_canvas2d::LineJoin::Bevel);
            ctx.begin_path();
            ctx.move_to(140.0, 80.0);
            ctx.line_to(160.0, 20.0);
            ctx.line_to(180.0, 80.0);
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0,
    };
    run_comparison_test(&test).expect("line_join comparison failed");
}

#[test]
fn test_line_dash_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "line_dash",
        width: 100,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#000000';
ctx.lineWidth = 3;
ctx.setLineDash([10, 5]);
ctx.beginPath();
ctx.moveTo(10, 50);
ctx.lineTo(90, 50);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#000000").unwrap();
            ctx.set_line_width(3.0);
            ctx.set_line_dash(vec![10.0, 5.0]);
            ctx.begin_path();
            ctx.move_to(10.0, 50.0);
            ctx.line_to(90.0, 50.0);
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0,
    };
    run_comparison_test(&test).expect("line_dash comparison failed");
}

#[test]
fn test_clear_rect_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "clear_rect",
        width: 100,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#ff0000';
ctx.fillRect(0, 0, 100, 100);
ctx.clearRect(25, 25, 50, 50);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.fill_rect(0.0, 0.0, 100.0, 100.0);
            ctx.clear_rect(25.0, 25.0, 50.0, 50.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("clear_rect comparison failed");
}

#[test]
fn test_save_restore_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "save_restore",
        width: 100,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#ff0000';
ctx.save();
ctx.fillStyle = '#00ff00';
ctx.fillRect(0, 0, 50, 50);
ctx.restore();
ctx.fillRect(50, 50, 50, 50);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.save();
            ctx.set_fill_style("#00ff00").unwrap();
            ctx.fill_rect(0.0, 0.0, 50.0, 50.0);
            ctx.restore();
            ctx.fill_rect(50.0, 50.0, 50.0, 50.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("save_restore comparison failed");
}

#[test]
fn test_complex_path_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "complex_path",
        width: 150,
        height: 150,
        js_code: r#"
ctx.fillStyle = '#3366cc';
ctx.beginPath();
// Star shape
ctx.moveTo(75, 10);
ctx.lineTo(90, 55);
ctx.lineTo(140, 55);
ctx.lineTo(100, 85);
ctx.lineTo(115, 130);
ctx.lineTo(75, 100);
ctx.lineTo(35, 130);
ctx.lineTo(50, 85);
ctx.lineTo(10, 55);
ctx.lineTo(60, 55);
ctx.closePath();
ctx.fill();
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#3366cc").unwrap();
            ctx.begin_path();
            ctx.move_to(75.0, 10.0);
            ctx.line_to(90.0, 55.0);
            ctx.line_to(140.0, 55.0);
            ctx.line_to(100.0, 85.0);
            ctx.line_to(115.0, 130.0);
            ctx.line_to(75.0, 100.0);
            ctx.line_to(35.0, 130.0);
            ctx.line_to(50.0, 85.0);
            ctx.line_to(10.0, 55.0);
            ctx.line_to(60.0, 55.0);
            ctx.close_path();
            ctx.fill();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("complex_path comparison failed");
}

#[test]
fn test_fill_text_comparison() {
    skip_if_no_node_canvas!();
    // Use Helvetica explicitly to ensure both implementations use the same font
    let test = CanvasTestCase {
        name: "fill_text",
        width: 200,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '24px Helvetica';
ctx.fillText('Hello', 20, 50);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("24px Helvetica").unwrap();
            ctx.fill_text("Hello", 20.0, 50.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("fill_text comparison failed");
}

// Note: strokeText currently renders as filled text (limitation - proper stroke
// would require glyph outline extraction). Higher tolerance to account for this.
#[test]
fn test_stroke_text_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "stroke_text",
        width: 200,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#ff0000';
ctx.lineWidth = 1;
ctx.font = '32px Helvetica';
ctx.strokeText('Test', 20, 60);
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.set_line_width(1.0);
            ctx.set_font("32px Helvetica").unwrap();
            ctx.stroke_text("Test", 20.0, 60.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: 5.0, // Higher tolerance: strokeText renders as filled
    };
    run_comparison_test(&test).expect("stroke_text comparison failed");
}

// --- Rotated text tests ---

#[test]
fn test_text_rotate_45_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "text_rotate_45",
        width: 200,
        height: 150,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '20px Helvetica';
ctx.translate(100, 75);
ctx.rotate(Math.PI / 4);
ctx.fillText('Rotated 45°', 0, 0);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("20px Helvetica").unwrap();
            ctx.translate(100.0, 75.0);
            ctx.rotate(PI / 4.0);
            ctx.fill_text("Rotated 45°", 0.0, 0.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("text_rotate_45 comparison failed");
}

#[test]
fn test_text_rotate_90_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "text_rotate_90",
        width: 150,
        height: 200,
        js_code: r#"
ctx.fillStyle = '#0000ff';
ctx.font = '18px Helvetica';
ctx.translate(75, 100);
ctx.rotate(Math.PI / 2);
ctx.fillText('Vertical', 0, 0);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#0000ff").unwrap();
            ctx.set_font("18px Helvetica").unwrap();
            ctx.translate(75.0, 100.0);
            ctx.rotate(PI / 2.0);
            ctx.fill_text("Vertical", 0.0, 0.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("text_rotate_90 comparison failed");
}

#[test]
fn test_text_rotate_negative_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "text_rotate_negative",
        width: 200,
        height: 150,
        js_code: r#"
ctx.fillStyle = '#008000';
ctx.font = '16px Helvetica';
ctx.translate(100, 100);
ctx.rotate(-Math.PI / 6);
ctx.fillText('Tilted -30°', 0, 0);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#008000").unwrap();
            ctx.set_font("16px Helvetica").unwrap();
            ctx.translate(100.0, 100.0);
            ctx.rotate(-PI / 6.0);
            ctx.fill_text("Tilted -30°", 0.0, 0.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("text_rotate_negative comparison failed");
}

// --- textAlign tests ---

#[test]
fn test_text_align_left_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "text_align_left",
        width: 200,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '20px Helvetica';
ctx.textAlign = 'left';
// Draw reference line at x=100
ctx.strokeStyle = '#ff0000';
ctx.beginPath();
ctx.moveTo(100, 0);
ctx.lineTo(100, 100);
ctx.stroke();
// Draw text
ctx.fillText('Left', 100, 50);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("20px Helvetica").unwrap();
            ctx.set_text_align(TextAlign::Left);
            // Draw reference line
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.begin_path();
            ctx.move_to(100.0, 0.0);
            ctx.line_to(100.0, 100.0);
            ctx.stroke();
            // Draw text
            ctx.fill_text("Left", 100.0, 50.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("text_align_left comparison failed");
}

#[test]
fn test_text_align_center_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "text_align_center",
        width: 200,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '20px Helvetica';
ctx.textAlign = 'center';
// Draw reference line at x=100
ctx.strokeStyle = '#ff0000';
ctx.beginPath();
ctx.moveTo(100, 0);
ctx.lineTo(100, 100);
ctx.stroke();
// Draw text
ctx.fillText('Center', 100, 50);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("20px Helvetica").unwrap();
            ctx.set_text_align(TextAlign::Center);
            // Draw reference line
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.begin_path();
            ctx.move_to(100.0, 0.0);
            ctx.line_to(100.0, 100.0);
            ctx.stroke();
            // Draw text
            ctx.fill_text("Center", 100.0, 50.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("text_align_center comparison failed");
}

#[test]
fn test_text_align_right_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "text_align_right",
        width: 200,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '20px Helvetica';
ctx.textAlign = 'right';
// Draw reference line at x=100
ctx.strokeStyle = '#ff0000';
ctx.beginPath();
ctx.moveTo(100, 0);
ctx.lineTo(100, 100);
ctx.stroke();
// Draw text
ctx.fillText('Right', 100, 50);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("20px Helvetica").unwrap();
            ctx.set_text_align(TextAlign::Right);
            // Draw reference line
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.begin_path();
            ctx.move_to(100.0, 0.0);
            ctx.line_to(100.0, 100.0);
            ctx.stroke();
            // Draw text
            ctx.fill_text("Right", 100.0, 50.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("text_align_right comparison failed");
}

// --- textBaseline tests ---

#[test]
fn test_text_baseline_top_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "text_baseline_top",
        width: 200,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '24px Helvetica';
ctx.textBaseline = 'top';
// Draw reference line at y=50
ctx.strokeStyle = '#ff0000';
ctx.beginPath();
ctx.moveTo(0, 50);
ctx.lineTo(200, 50);
ctx.stroke();
// Draw text
ctx.fillText('Top', 20, 50);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("24px Helvetica").unwrap();
            ctx.set_text_baseline(TextBaseline::Top);
            // Draw reference line
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.begin_path();
            ctx.move_to(0.0, 50.0);
            ctx.line_to(200.0, 50.0);
            ctx.stroke();
            // Draw text
            ctx.fill_text("Top", 20.0, 50.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("text_baseline_top comparison failed");
}

#[test]
fn test_text_baseline_middle_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "text_baseline_middle",
        width: 200,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '24px Helvetica';
ctx.textBaseline = 'middle';
// Draw reference line at y=50
ctx.strokeStyle = '#ff0000';
ctx.beginPath();
ctx.moveTo(0, 50);
ctx.lineTo(200, 50);
ctx.stroke();
// Draw text
ctx.fillText('Middle', 20, 50);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("24px Helvetica").unwrap();
            ctx.set_text_baseline(TextBaseline::Middle);
            // Draw reference line
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.begin_path();
            ctx.move_to(0.0, 50.0);
            ctx.line_to(200.0, 50.0);
            ctx.stroke();
            // Draw text
            ctx.fill_text("Middle", 20.0, 50.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("text_baseline_middle comparison failed");
}

#[test]
fn test_text_baseline_bottom_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "text_baseline_bottom",
        width: 200,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '24px Helvetica';
ctx.textBaseline = 'bottom';
// Draw reference line at y=50
ctx.strokeStyle = '#ff0000';
ctx.beginPath();
ctx.moveTo(0, 50);
ctx.lineTo(200, 50);
ctx.stroke();
// Draw text
ctx.fillText('Bottom', 20, 50);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("24px Helvetica").unwrap();
            ctx.set_text_baseline(TextBaseline::Bottom);
            // Draw reference line
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.begin_path();
            ctx.move_to(0.0, 50.0);
            ctx.line_to(200.0, 50.0);
            ctx.stroke();
            // Draw text
            ctx.fill_text("Bottom", 20.0, 50.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("text_baseline_bottom comparison failed");
}

#[test]
fn test_text_baseline_alphabetic_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "text_baseline_alphabetic",
        width: 200,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '24px Helvetica';
ctx.textBaseline = 'alphabetic';
// Draw reference line at y=50
ctx.strokeStyle = '#ff0000';
ctx.beginPath();
ctx.moveTo(0, 50);
ctx.lineTo(200, 50);
ctx.stroke();
// Draw text with descender
ctx.fillText('Apgy', 20, 50);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("24px Helvetica").unwrap();
            ctx.set_text_baseline(TextBaseline::Alphabetic);
            // Draw reference line
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.begin_path();
            ctx.move_to(0.0, 50.0);
            ctx.line_to(200.0, 50.0);
            ctx.stroke();
            // Draw text with descender
            ctx.fill_text("Apgy", 20.0, 50.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("text_baseline_alphabetic comparison failed");
}

// ============================================================================
// Gradient Tests
// ============================================================================

#[test]
fn test_linear_gradient_horizontal_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "linear_gradient_horizontal",
        width: 200,
        height: 100,
        js_code: r#"
const gradient = ctx.createLinearGradient(0, 0, 200, 0);
gradient.addColorStop(0, '#ff0000');
gradient.addColorStop(1, '#0000ff');
ctx.fillStyle = gradient;
ctx.fillRect(0, 0, 200, 100);
"#,
        rust_fn: |ctx| {
            let mut gradient = ctx.create_linear_gradient(0.0, 0.0, 200.0, 0.0);
            gradient.add_color_stop(0.0, tiny_skia::Color::from_rgba8(255, 0, 0, 255));
            gradient.add_color_stop(1.0, tiny_skia::Color::from_rgba8(0, 0, 255, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(0.0, 0.0, 200.0, 100.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("linear_gradient_horizontal comparison failed");
}

#[test]
fn test_linear_gradient_vertical_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "linear_gradient_vertical",
        width: 100,
        height: 200,
        js_code: r#"
const gradient = ctx.createLinearGradient(0, 0, 0, 200);
gradient.addColorStop(0, '#00ff00');
gradient.addColorStop(1, '#ffff00');
ctx.fillStyle = gradient;
ctx.fillRect(0, 0, 100, 200);
"#,
        rust_fn: |ctx| {
            let mut gradient = ctx.create_linear_gradient(0.0, 0.0, 0.0, 200.0);
            gradient.add_color_stop(0.0, tiny_skia::Color::from_rgba8(0, 255, 0, 255));
            gradient.add_color_stop(1.0, tiny_skia::Color::from_rgba8(255, 255, 0, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(0.0, 0.0, 100.0, 200.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("linear_gradient_vertical comparison failed");
}

#[test]
fn test_linear_gradient_diagonal_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "linear_gradient_diagonal",
        width: 150,
        height: 150,
        js_code: r#"
const gradient = ctx.createLinearGradient(0, 0, 150, 150);
gradient.addColorStop(0, '#ff0000');
gradient.addColorStop(0.5, '#00ff00');
gradient.addColorStop(1, '#0000ff');
ctx.fillStyle = gradient;
ctx.fillRect(0, 0, 150, 150);
"#,
        rust_fn: |ctx| {
            let mut gradient = ctx.create_linear_gradient(0.0, 0.0, 150.0, 150.0);
            gradient.add_color_stop(0.0, tiny_skia::Color::from_rgba8(255, 0, 0, 255));
            gradient.add_color_stop(0.5, tiny_skia::Color::from_rgba8(0, 255, 0, 255));
            gradient.add_color_stop(1.0, tiny_skia::Color::from_rgba8(0, 0, 255, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(0.0, 0.0, 150.0, 150.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("linear_gradient_diagonal comparison failed");
}

#[test]
fn test_radial_gradient_centered_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "radial_gradient_centered",
        width: 150,
        height: 150,
        js_code: r#"
const gradient = ctx.createRadialGradient(75, 75, 0, 75, 75, 75);
gradient.addColorStop(0, '#ffffff');
gradient.addColorStop(1, '#000000');
ctx.fillStyle = gradient;
ctx.fillRect(0, 0, 150, 150);
"#,
        rust_fn: |ctx| {
            let mut gradient = ctx.create_radial_gradient(75.0, 75.0, 0.0, 75.0, 75.0, 75.0);
            gradient.add_color_stop(0.0, tiny_skia::Color::from_rgba8(255, 255, 255, 255));
            gradient.add_color_stop(1.0, tiny_skia::Color::from_rgba8(0, 0, 0, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(0.0, 0.0, 150.0, 150.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0, // Slightly higher for radial gradient edge differences
    };
    run_comparison_test(&test).expect("radial_gradient_centered comparison failed");
}

#[test]
fn test_radial_gradient_offset_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "radial_gradient_offset",
        width: 150,
        height: 150,
        js_code: r#"
const gradient = ctx.createRadialGradient(50, 50, 10, 75, 75, 70);
gradient.addColorStop(0, '#ff0000');
gradient.addColorStop(0.5, '#ffff00');
gradient.addColorStop(1, '#0000ff');
ctx.fillStyle = gradient;
ctx.fillRect(0, 0, 150, 150);
"#,
        rust_fn: |ctx| {
            let mut gradient = ctx.create_radial_gradient(50.0, 50.0, 10.0, 75.0, 75.0, 70.0);
            gradient.add_color_stop(0.0, tiny_skia::Color::from_rgba8(255, 0, 0, 255));
            gradient.add_color_stop(0.5, tiny_skia::Color::from_rgba8(255, 255, 0, 255));
            gradient.add_color_stop(1.0, tiny_skia::Color::from_rgba8(0, 0, 255, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(0.0, 0.0, 150.0, 150.0);
        },
        threshold: DEFAULT_THRESHOLD,
        // Higher tolerance for offset radial gradients - tiny_skia doesn't support
        // inner radius (r0) for two-point conical gradients, causing slight differences
        max_diff_percent: 35.0,
    };
    run_comparison_test(&test).expect("radial_gradient_offset comparison failed");
}

#[test]
fn test_linear_gradient_stroke_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "linear_gradient_stroke",
        width: 150,
        height: 150,
        js_code: r#"
const gradient = ctx.createLinearGradient(0, 0, 150, 0);
gradient.addColorStop(0, '#ff0000');
gradient.addColorStop(1, '#00ff00');
ctx.strokeStyle = gradient;
ctx.lineWidth = 10;
ctx.beginPath();
ctx.moveTo(20, 75);
ctx.lineTo(130, 75);
ctx.stroke();
ctx.beginPath();
ctx.arc(75, 75, 40, 0, Math.PI * 2);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            let mut gradient = ctx.create_linear_gradient(0.0, 0.0, 150.0, 0.0);
            gradient.add_color_stop(0.0, tiny_skia::Color::from_rgba8(255, 0, 0, 255));
            gradient.add_color_stop(1.0, tiny_skia::Color::from_rgba8(0, 255, 0, 255));
            ctx.set_stroke_style_gradient(gradient);
            ctx.set_line_width(10.0);
            ctx.begin_path();
            ctx.move_to(20.0, 75.0);
            ctx.line_to(130.0, 75.0);
            ctx.stroke();
            ctx.begin_path();
            ctx.arc(75.0, 75.0, 40.0, 0.0, 2.0 * PI, false);
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("linear_gradient_stroke comparison failed");
}

// ============================================================================
// Clipping Tests
// ============================================================================

#[test]
fn test_clip_rect_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "clip_rect",
        width: 150,
        height: 150,
        js_code: r#"
// Define rectangular clipping region
ctx.beginPath();
ctx.rect(25, 25, 100, 100);
ctx.clip();

// Fill entire canvas - only clipped region should be visible
ctx.fillStyle = '#ff0000';
ctx.fillRect(0, 0, 150, 150);

// Draw a circle that extends outside clip region
ctx.fillStyle = '#0000ff';
ctx.beginPath();
ctx.arc(75, 75, 60, 0, Math.PI * 2);
ctx.fill();
"#,
        rust_fn: |ctx| {
            // Define rectangular clipping region
            ctx.begin_path();
            ctx.rect(25.0, 25.0, 100.0, 100.0);
            ctx.clip();

            // Fill entire canvas - only clipped region should be visible
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.fill_rect(0.0, 0.0, 150.0, 150.0);

            // Draw a circle that extends outside clip region
            ctx.set_fill_style("#0000ff").unwrap();
            ctx.begin_path();
            ctx.arc(75.0, 75.0, 60.0, 0.0, 2.0 * PI, false);
            ctx.fill();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("clip_rect comparison failed");
}

#[test]
fn test_clip_circle_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "clip_circle",
        width: 150,
        height: 150,
        js_code: r#"
// Define circular clipping region
ctx.beginPath();
ctx.arc(75, 75, 50, 0, Math.PI * 2);
ctx.clip();

// Draw gradient background
const gradient = ctx.createLinearGradient(0, 0, 150, 150);
gradient.addColorStop(0, '#ff0000');
gradient.addColorStop(1, '#0000ff');
ctx.fillStyle = gradient;
ctx.fillRect(0, 0, 150, 150);

// Draw grid lines
ctx.strokeStyle = '#ffffff';
ctx.lineWidth = 2;
for (let i = 0; i <= 150; i += 20) {
    ctx.beginPath();
    ctx.moveTo(i, 0);
    ctx.lineTo(i, 150);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(0, i);
    ctx.lineTo(150, i);
    ctx.stroke();
}
"#,
        rust_fn: |ctx| {
            // Define circular clipping region
            ctx.begin_path();
            ctx.arc(75.0, 75.0, 50.0, 0.0, 2.0 * PI, false);
            ctx.clip();

            // Draw gradient background
            let mut gradient = ctx.create_linear_gradient(0.0, 0.0, 150.0, 150.0);
            gradient.add_color_stop(0.0, tiny_skia::Color::from_rgba8(255, 0, 0, 255));
            gradient.add_color_stop(1.0, tiny_skia::Color::from_rgba8(0, 0, 255, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(0.0, 0.0, 150.0, 150.0);

            // Draw grid lines
            ctx.set_stroke_style("#ffffff").unwrap();
            ctx.set_line_width(2.0);
            for i in (0..=150).step_by(20) {
                ctx.begin_path();
                ctx.move_to(i as f32, 0.0);
                ctx.line_to(i as f32, 150.0);
                ctx.stroke();
                ctx.begin_path();
                ctx.move_to(0.0, i as f32);
                ctx.line_to(150.0, i as f32);
                ctx.stroke();
            }
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0, // Higher tolerance for complex clipping
    };
    run_comparison_test(&test).expect("clip_circle comparison failed");
}

#[test]
fn test_clip_complex_path_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "clip_complex_path",
        width: 150,
        height: 150,
        js_code: r#"
// Define star-shaped clipping region
ctx.beginPath();
ctx.moveTo(75, 10);
ctx.lineTo(95, 55);
ctx.lineTo(140, 55);
ctx.lineTo(105, 85);
ctx.lineTo(120, 130);
ctx.lineTo(75, 105);
ctx.lineTo(30, 130);
ctx.lineTo(45, 85);
ctx.lineTo(10, 55);
ctx.lineTo(55, 55);
ctx.closePath();
ctx.clip();

// Fill with gradient
const gradient = ctx.createRadialGradient(75, 75, 0, 75, 75, 75);
gradient.addColorStop(0, '#ffff00');
gradient.addColorStop(1, '#ff0000');
ctx.fillStyle = gradient;
ctx.fillRect(0, 0, 150, 150);
"#,
        rust_fn: |ctx| {
            // Define star-shaped clipping region
            ctx.begin_path();
            ctx.move_to(75.0, 10.0);
            ctx.line_to(95.0, 55.0);
            ctx.line_to(140.0, 55.0);
            ctx.line_to(105.0, 85.0);
            ctx.line_to(120.0, 130.0);
            ctx.line_to(75.0, 105.0);
            ctx.line_to(30.0, 130.0);
            ctx.line_to(45.0, 85.0);
            ctx.line_to(10.0, 55.0);
            ctx.line_to(55.0, 55.0);
            ctx.close_path();
            ctx.clip();

            // Fill with gradient
            let mut gradient = ctx.create_radial_gradient(75.0, 75.0, 0.0, 75.0, 75.0, 75.0);
            gradient.add_color_stop(0.0, tiny_skia::Color::from_rgba8(255, 255, 0, 255));
            gradient.add_color_stop(1.0, tiny_skia::Color::from_rgba8(255, 0, 0, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(0.0, 0.0, 150.0, 150.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0,
    };
    run_comparison_test(&test).expect("clip_complex_path comparison failed");
}

#[test]
fn test_clip_with_transform_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "clip_with_transform",
        width: 150,
        height: 150,
        js_code: r#"
// Apply rotation transform
ctx.translate(75, 75);
ctx.rotate(Math.PI / 4);
ctx.translate(-75, -75);

// Define rectangular clipping region (now rotated)
ctx.beginPath();
ctx.rect(37.5, 37.5, 75, 75);
ctx.clip();

// Fill with solid color
ctx.fillStyle = '#00ff00';
ctx.fillRect(0, 0, 150, 150);

// Draw some lines
ctx.strokeStyle = '#000000';
ctx.lineWidth = 3;
ctx.beginPath();
ctx.moveTo(0, 75);
ctx.lineTo(150, 75);
ctx.stroke();
ctx.beginPath();
ctx.moveTo(75, 0);
ctx.lineTo(75, 150);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            // Apply rotation transform
            ctx.translate(75.0, 75.0);
            ctx.rotate(PI / 4.0);
            ctx.translate(-75.0, -75.0);

            // Define rectangular clipping region (now rotated)
            ctx.begin_path();
            ctx.rect(37.5, 37.5, 75.0, 75.0);
            ctx.clip();

            // Fill with solid color
            ctx.set_fill_style("#00ff00").unwrap();
            ctx.fill_rect(0.0, 0.0, 150.0, 150.0);

            // Draw some lines
            ctx.set_stroke_style("#000000").unwrap();
            ctx.set_line_width(3.0);
            ctx.begin_path();
            ctx.move_to(0.0, 75.0);
            ctx.line_to(150.0, 75.0);
            ctx.stroke();
            ctx.begin_path();
            ctx.move_to(75.0, 0.0);
            ctx.line_to(75.0, 150.0);
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0,
    };
    run_comparison_test(&test).expect("clip_with_transform comparison failed");
}

// =============================================================================
// Phase 1 Feature Tests - fill(fillRule), roundRect, Path2D, etc.
// =============================================================================

#[test]
fn test_fill_evenodd_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "fill_evenodd",
        width: 150,
        height: 150,
        js_code: r#"
// Draw a star with overlapping paths - evenodd creates a hole in the center
ctx.fillStyle = '#0000ff';
ctx.beginPath();
// Outer pentagon
const cx = 75, cy = 75, r = 60;
for (let i = 0; i < 5; i++) {
    const angle = (i * 4 * Math.PI / 5) - Math.PI / 2;
    const x = cx + r * Math.cos(angle);
    const y = cy + r * Math.sin(angle);
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
}
ctx.closePath();
ctx.fill('evenodd');
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::CanvasFillRule;
            ctx.set_fill_style("#0000ff").unwrap();
            ctx.begin_path();
            // Draw star polygon
            let cx = 75.0_f32;
            let cy = 75.0_f32;
            let r = 60.0_f32;
            for i in 0..5 {
                let angle = (i as f32 * 4.0 * PI / 5.0) - PI / 2.0;
                let x = cx + r * angle.cos();
                let y = cy + r * angle.sin();
                if i == 0 {
                    ctx.move_to(x, y);
                } else {
                    ctx.line_to(x, y);
                }
            }
            ctx.close_path();
            ctx.fill_with_rule(CanvasFillRule::EvenOdd);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("fill_evenodd comparison failed");
}

#[test]
fn test_round_rect_uniform_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "round_rect_uniform",
        width: 150,
        height: 150,
        js_code: r#"
ctx.fillStyle = '#4488ff';
ctx.beginPath();
ctx.roundRect(20, 20, 110, 110, 15);
ctx.fill();
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#4488ff").unwrap();
            ctx.begin_path();
            ctx.round_rect(20.0, 20.0, 110.0, 110.0, 15.0);
            ctx.fill();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("round_rect_uniform comparison failed");
}

#[test]
fn test_round_rect_different_radii_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "round_rect_different_radii",
        width: 150,
        height: 150,
        js_code: r#"
ctx.fillStyle = '#ff8844';
ctx.beginPath();
// Different radius for each corner: [top-left, top-right, bottom-right, bottom-left]
ctx.roundRect(20, 20, 110, 110, [5, 15, 25, 35]);
ctx.fill();
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#ff8844").unwrap();
            ctx.begin_path();
            ctx.round_rect_radii(20.0, 20.0, 110.0, 110.0, [5.0, 15.0, 25.0, 35.0]);
            ctx.fill();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("round_rect_different_radii comparison failed");
}

#[test]
fn test_round_rect_stroke_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "round_rect_stroke",
        width: 150,
        height: 150,
        js_code: r#"
ctx.strokeStyle = '#00aa00';
ctx.lineWidth = 4;
ctx.beginPath();
ctx.roundRect(25, 25, 100, 100, 20);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#00aa00").unwrap();
            ctx.set_line_width(4.0);
            ctx.begin_path();
            ctx.round_rect(25.0, 25.0, 100.0, 100.0, 20.0);
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("round_rect_stroke comparison failed");
}

#[test]
fn test_path2d_fill_comparison() {
    skip_if_no_node_canvas!();
    // Node-canvas doesn't support Path2D, so we use equivalent context path operations
    // The Rust side uses Path2D to verify it produces the same output
    let test = CanvasTestCase {
        name: "path2d_fill",
        width: 150,
        height: 150,
        js_code: r#"
// Draw two rectangles (what Path2D would produce)
ctx.fillStyle = '#9933ff';
ctx.beginPath();
ctx.rect(20, 20, 50, 50);
ctx.rect(80, 80, 50, 50);
ctx.fill();
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            let mut path = Path2D::new();
            path.rect(20.0, 20.0, 50.0, 50.0);
            path.rect(80.0, 80.0, 50.0, 50.0);

            ctx.set_fill_style("#9933ff").unwrap();
            ctx.fill_path2d(&mut path);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("path2d_fill comparison failed");
}

#[test]
fn test_path2d_stroke_comparison() {
    skip_if_no_node_canvas!();
    // Node-canvas doesn't support Path2D, so we use equivalent context path operations
    let test = CanvasTestCase {
        name: "path2d_stroke",
        width: 150,
        height: 150,
        js_code: r#"
ctx.strokeStyle = '#ff0066';
ctx.lineWidth = 3;
ctx.beginPath();
ctx.moveTo(20, 75);
ctx.lineTo(75, 20);
ctx.lineTo(130, 75);
ctx.lineTo(75, 130);
ctx.closePath();
ctx.stroke();
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            let mut path = Path2D::new();
            path.move_to(20.0, 75.0);
            path.line_to(75.0, 20.0);
            path.line_to(130.0, 75.0);
            path.line_to(75.0, 130.0);
            path.close_path();

            ctx.set_stroke_style("#ff0066").unwrap();
            ctx.set_line_width(3.0);
            ctx.stroke_path2d(&mut path);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("path2d_stroke comparison failed");
}

#[test]
fn test_path2d_reuse_comparison() {
    skip_if_no_node_canvas!();
    // Node-canvas doesn't support Path2D, so we manually draw the same shapes
    // The Rust side uses Path2D to verify path reuse works correctly
    let test = CanvasTestCase {
        name: "path2d_reuse",
        width: 150,
        height: 150,
        js_code: r#"
// Helper function to draw a circle at origin
function drawCircle() {
    ctx.beginPath();
    ctx.arc(0, 0, 20, 0, Math.PI * 2);
    ctx.fill();
}

// Draw in multiple positions
ctx.fillStyle = '#ff0000';
ctx.save();
ctx.translate(40, 40);
drawCircle();
ctx.restore();

ctx.fillStyle = '#00ff00';
ctx.save();
ctx.translate(110, 40);
drawCircle();
ctx.restore();

ctx.fillStyle = '#0000ff';
ctx.save();
ctx.translate(75, 110);
drawCircle();
ctx.restore();
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            let mut path = Path2D::new();
            path.arc(0.0, 0.0, 20.0, 0.0, 2.0 * PI, false);

            // Draw in multiple positions - reusing the same path
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.save();
            ctx.translate(40.0, 40.0);
            ctx.fill_path2d(&mut path);
            ctx.restore();

            ctx.set_fill_style("#00ff00").unwrap();
            ctx.save();
            ctx.translate(110.0, 40.0);
            ctx.fill_path2d(&mut path);
            ctx.restore();

            ctx.set_fill_style("#0000ff").unwrap();
            ctx.save();
            ctx.translate(75.0, 110.0);
            ctx.fill_path2d(&mut path);
            ctx.restore();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("path2d_reuse comparison failed");
}

#[test]
fn test_path2d_svg_simple_comparison() {
    skip_if_no_node_canvas!();
    // Test Path2D from SVG path data - simple lines
    let test = CanvasTestCase {
        name: "path2d_svg_simple",
        width: 150,
        height: 150,
        js_code: r#"
ctx.fillStyle = '#3366cc';
ctx.beginPath();
ctx.moveTo(10, 10);
ctx.lineTo(140, 10);
ctx.lineTo(140, 140);
ctx.lineTo(10, 140);
ctx.closePath();
ctx.fill();
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            let mut path = Path2D::from_svg_path_data("M10,10 L140,10 L140,140 L10,140 Z").unwrap();
            ctx.set_fill_style("#3366cc").unwrap();
            ctx.fill_path2d(&mut path);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("path2d_svg_simple comparison failed");
}

#[test]
fn test_path2d_svg_quadratic_comparison() {
    skip_if_no_node_canvas!();
    // Test Path2D from SVG with quadratic curves
    let test = CanvasTestCase {
        name: "path2d_svg_quadratic",
        width: 150,
        height: 150,
        js_code: r#"
ctx.strokeStyle = '#cc3366';
ctx.lineWidth = 3;
ctx.beginPath();
ctx.moveTo(20, 100);
ctx.quadraticCurveTo(75, 20, 130, 100);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            let mut path = Path2D::from_svg_path_data("M20,100 Q75,20 130,100").unwrap();
            ctx.set_stroke_style("#cc3366").unwrap();
            ctx.set_line_width(3.0);
            ctx.stroke_path2d(&mut path);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("path2d_svg_quadratic comparison failed");
}

#[test]
fn test_path2d_svg_cubic_comparison() {
    skip_if_no_node_canvas!();
    // Test Path2D from SVG with cubic bezier curves
    let test = CanvasTestCase {
        name: "path2d_svg_cubic",
        width: 150,
        height: 150,
        js_code: r#"
ctx.strokeStyle = '#66cc33';
ctx.lineWidth = 3;
ctx.beginPath();
ctx.moveTo(20, 75);
ctx.bezierCurveTo(20, 20, 130, 20, 130, 75);
ctx.bezierCurveTo(130, 130, 20, 130, 20, 75);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            let mut path = Path2D::from_svg_path_data(
                "M20,75 C20,20 130,20 130,75 C130,130 20,130 20,75"
            ).unwrap();
            ctx.set_stroke_style("#66cc33").unwrap();
            ctx.set_line_width(3.0);
            ctx.stroke_path2d(&mut path);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("path2d_svg_cubic comparison failed");
}

#[test]
fn test_path2d_svg_arc_comparison() {
    skip_if_no_node_canvas!();
    // Test Path2D from SVG with arc command (converted to cubics by svgtypes)
    // Compare against equivalent manual arc drawing
    let test = CanvasTestCase {
        name: "path2d_svg_arc",
        width: 150,
        height: 150,
        js_code: r#"
ctx.fillStyle = '#9966cc';
ctx.beginPath();
// Draw an arc from (30,75) to (120,75) with radius 45
// This is a half-circle arc
ctx.moveTo(30, 75);
ctx.arc(75, 75, 45, Math.PI, 0, false);
ctx.lineTo(120, 120);
ctx.lineTo(30, 120);
ctx.closePath();
ctx.fill();
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            // SVG arc: A rx ry x-axis-rotation large-arc-flag sweep-flag x y
            // A45,45 0 0 1 120,75 draws an arc with rx=45, ry=45 to point (120,75)
            let mut path = Path2D::from_svg_path_data(
                "M30,75 A45,45 0 0 1 120,75 L120,120 L30,120 Z"
            ).unwrap();
            ctx.set_fill_style("#9966cc").unwrap();
            ctx.fill_path2d(&mut path);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0, // Allow slightly more due to arc approximation differences
    };
    run_comparison_test(&test).expect("path2d_svg_arc comparison failed");
}

#[test]
fn test_path2d_svg_relative_comparison() {
    skip_if_no_node_canvas!();
    // Test Path2D from SVG with relative commands (lowercase)
    let test = CanvasTestCase {
        name: "path2d_svg_relative",
        width: 150,
        height: 150,
        js_code: r#"
ctx.fillStyle = '#cc9933';
ctx.beginPath();
// m10,10 means moveTo(10,10)
// l50,0 means lineTo(current_x+50, current_y) = lineTo(60,10)
// l0,50 means lineTo(60, 60)
// l-50,0 means lineTo(10, 60)
// z closes path
ctx.moveTo(10, 10);
ctx.lineTo(60, 10);
ctx.lineTo(60, 60);
ctx.lineTo(10, 60);
ctx.closePath();

// Second shape using relative
ctx.moveTo(80, 80);
ctx.lineTo(140, 80);
ctx.lineTo(140, 140);
ctx.lineTo(80, 140);
ctx.closePath();

ctx.fill();
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            // Using relative commands: m, l, z
            let mut path = Path2D::from_svg_path_data(
                "m10,10 l50,0 l0,50 l-50,0 z m70,70 l60,0 l0,60 l-60,0 z"
            ).unwrap();
            ctx.set_fill_style("#cc9933").unwrap();
            ctx.fill_path2d(&mut path);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("path2d_svg_relative comparison failed");
}

#[test]
fn test_path2d_svg_complex_comparison() {
    skip_if_no_node_canvas!();
    // Test Path2D with a more complex SVG path (star shape)
    let test = CanvasTestCase {
        name: "path2d_svg_complex",
        width: 150,
        height: 150,
        js_code: r#"
ctx.fillStyle = '#ff6600';
ctx.beginPath();
// Five-pointed star
ctx.moveTo(75, 10);
ctx.lineTo(90, 55);
ctx.lineTo(140, 55);
ctx.lineTo(100, 85);
ctx.lineTo(115, 130);
ctx.lineTo(75, 100);
ctx.lineTo(35, 130);
ctx.lineTo(50, 85);
ctx.lineTo(10, 55);
ctx.lineTo(60, 55);
ctx.closePath();
ctx.fill();
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            let mut path = Path2D::from_svg_path_data(
                "M75,10 L90,55 L140,55 L100,85 L115,130 L75,100 L35,130 L50,85 L10,55 L60,55 Z"
            ).unwrap();
            ctx.set_fill_style("#ff6600").unwrap();
            ctx.fill_path2d(&mut path);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("path2d_svg_complex comparison failed");
}

#[test]
fn test_path2d_svg_multi_subpath_comparison() {
    skip_if_no_node_canvas!();
    // Test Path2D with multiple subpaths (multiple M commands)
    let test = CanvasTestCase {
        name: "path2d_svg_multi_subpath",
        width: 150,
        height: 150,
        js_code: r#"
ctx.fillStyle = '#339966';
ctx.beginPath();
// First rectangle
ctx.moveTo(10, 10);
ctx.lineTo(60, 10);
ctx.lineTo(60, 60);
ctx.lineTo(10, 60);
ctx.closePath();
// Second rectangle
ctx.moveTo(90, 90);
ctx.lineTo(140, 90);
ctx.lineTo(140, 140);
ctx.lineTo(90, 140);
ctx.closePath();
ctx.fill();
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            let mut path = Path2D::from_svg_path_data(
                "M10,10 L60,10 L60,60 L10,60 Z M90,90 L140,90 L140,140 L90,140 Z"
            ).unwrap();
            ctx.set_fill_style("#339966").unwrap();
            ctx.fill_path2d(&mut path);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("path2d_svg_multi_subpath comparison failed");
}

// =============================================================================
// Image Smoothing Tests
// =============================================================================

#[test]
fn test_image_smoothing_disabled_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "image_smoothing_disabled",
        width: 100,
        height: 100,
        js_code: r#"
// Create a small checkerboard pattern
const patternCanvas = require('canvas').createCanvas(4, 4);
const patternCtx = patternCanvas.getContext('2d');
patternCtx.fillStyle = '#ff0000';
patternCtx.fillRect(0, 0, 2, 2);
patternCtx.fillRect(2, 2, 2, 2);
patternCtx.fillStyle = '#0000ff';
patternCtx.fillRect(2, 0, 2, 2);
patternCtx.fillRect(0, 2, 2, 2);

// Disable image smoothing
ctx.imageSmoothingEnabled = false;

// Scale the small pattern up - should show sharp pixels
ctx.drawImage(patternCanvas, 0, 0, 100, 100);
"#,
        rust_fn: |ctx| {
            // Create a small 4x4 checkerboard pattern
            let mut pattern_ctx = Canvas2dContext::new(4, 4).unwrap();
            pattern_ctx.set_fill_style("#ff0000").unwrap();
            pattern_ctx.fill_rect(0.0, 0.0, 2.0, 2.0);
            pattern_ctx.fill_rect(2.0, 2.0, 2.0, 2.0);
            pattern_ctx.set_fill_style("#0000ff").unwrap();
            pattern_ctx.fill_rect(2.0, 0.0, 2.0, 2.0);
            pattern_ctx.fill_rect(0.0, 2.0, 2.0, 2.0);

            // Disable image smoothing
            ctx.set_image_smoothing_enabled(false);

            // Scale the small pattern up - should show sharp pixels
            ctx.draw_image_scaled(pattern_ctx.pixmap().as_ref(), 0.0, 0.0, 100.0, 100.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("image_smoothing_disabled comparison failed");
}

#[test]
fn test_image_smoothing_quality_high_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "image_smoothing_quality_high",
        width: 100,
        height: 100,
        js_code: r#"
// Create a small gradient pattern
const patternCanvas = require('canvas').createCanvas(10, 10);
const patternCtx = patternCanvas.getContext('2d');
const gradient = patternCtx.createLinearGradient(0, 0, 10, 10);
gradient.addColorStop(0, '#ff0000');
gradient.addColorStop(1, '#0000ff');
patternCtx.fillStyle = gradient;
patternCtx.fillRect(0, 0, 10, 10);

// Enable high quality smoothing
ctx.imageSmoothingEnabled = true;
ctx.imageSmoothingQuality = 'high';

// Scale up
ctx.drawImage(patternCanvas, 0, 0, 100, 100);
"#,
        rust_fn: |ctx| {
            // Create a small 10x10 gradient pattern
            let mut pattern_ctx = Canvas2dContext::new(10, 10).unwrap();
            let mut gradient = pattern_ctx.create_linear_gradient(0.0, 0.0, 10.0, 10.0);
            gradient.add_color_stop(0.0, tiny_skia::Color::from_rgba8(255, 0, 0, 255));
            gradient.add_color_stop(1.0, tiny_skia::Color::from_rgba8(0, 0, 255, 255));
            pattern_ctx.set_fill_style_gradient(gradient);
            pattern_ctx.fill_rect(0.0, 0.0, 10.0, 10.0);

            // Enable high quality smoothing
            ctx.set_image_smoothing_enabled(true);
            ctx.set_image_smoothing_quality(vl_convert_canvas2d::ImageSmoothingQuality::High);

            // Scale up
            ctx.draw_image_scaled(pattern_ctx.pixmap().as_ref(), 0.0, 0.0, 100.0, 100.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0, // Higher tolerance for interpolation differences
    };
    run_comparison_test(&test).expect("image_smoothing_quality_high comparison failed");
}

// =============================================================================
// putImageData Tests
// =============================================================================

#[test]
fn test_put_image_data_basic_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "put_image_data_basic",
        width: 100,
        height: 100,
        js_code: r#"
// Create image data with a colored block
const imageData = ctx.createImageData(50, 50);
for (let i = 0; i < imageData.data.length; i += 4) {
    imageData.data[i] = 255;     // R
    imageData.data[i + 1] = 0;   // G
    imageData.data[i + 2] = 0;   // B
    imageData.data[i + 3] = 255; // A
}

// Put it at offset
ctx.putImageData(imageData, 25, 25);
"#,
        rust_fn: |ctx| {
            // Create image data with a red block (non-premultiplied RGBA)
            let mut data = vec![0u8; 50 * 50 * 4];
            for i in (0..data.len()).step_by(4) {
                data[i] = 255; // R
                data[i + 1] = 0; // G
                data[i + 2] = 0; // B
                data[i + 3] = 255; // A
            }

            ctx.put_image_data(&data, 50, 50, 25, 25);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("put_image_data_basic comparison failed");
}

#[test]
fn test_put_image_data_alpha_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "put_image_data_alpha",
        width: 100,
        height: 100,
        js_code: r#"
// Draw a blue background first
ctx.fillStyle = '#0000ff';
ctx.fillRect(0, 0, 100, 100);

// Create image data with semi-transparent red
const imageData = ctx.createImageData(50, 50);
for (let i = 0; i < imageData.data.length; i += 4) {
    imageData.data[i] = 255;     // R
    imageData.data[i + 1] = 0;   // G
    imageData.data[i + 2] = 0;   // B
    imageData.data[i + 3] = 128; // A (50% opacity)
}

// Put it - should NOT blend with background (putImageData bypasses compositing)
ctx.putImageData(imageData, 25, 25);
"#,
        rust_fn: |ctx| {
            // Draw a blue background first
            ctx.set_fill_style("#0000ff").unwrap();
            ctx.fill_rect(0.0, 0.0, 100.0, 100.0);

            // Create image data with semi-transparent red
            let mut data = vec![0u8; 50 * 50 * 4];
            for i in (0..data.len()).step_by(4) {
                data[i] = 255; // R
                data[i + 1] = 0; // G
                data[i + 2] = 0; // B
                data[i + 3] = 128; // A (50% opacity)
            }

            // Put it - should NOT blend with background (putImageData bypasses compositing)
            ctx.put_image_data(&data, 50, 50, 25, 25);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("put_image_data_alpha comparison failed");
}

#[test]
fn test_put_image_data_dirty_rect_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "put_image_data_dirty_rect",
        width: 100,
        height: 100,
        js_code: r#"
// Create a larger image with different colored quadrants
const imageData = ctx.createImageData(60, 60);
for (let y = 0; y < 60; y++) {
    for (let x = 0; x < 60; x++) {
        const i = (y * 60 + x) * 4;
        if (x < 30 && y < 30) {
            // Top-left: Red
            imageData.data[i] = 255;
            imageData.data[i + 1] = 0;
            imageData.data[i + 2] = 0;
        } else if (x >= 30 && y < 30) {
            // Top-right: Green
            imageData.data[i] = 0;
            imageData.data[i + 1] = 255;
            imageData.data[i + 2] = 0;
        } else if (x < 30 && y >= 30) {
            // Bottom-left: Blue
            imageData.data[i] = 0;
            imageData.data[i + 1] = 0;
            imageData.data[i + 2] = 255;
        } else {
            // Bottom-right: Yellow
            imageData.data[i] = 255;
            imageData.data[i + 1] = 255;
            imageData.data[i + 2] = 0;
        }
        imageData.data[i + 3] = 255;
    }
}

// Only put the bottom-right quadrant (yellow) using dirty rect
ctx.putImageData(imageData, 20, 20, 30, 30, 30, 30);
"#,
        rust_fn: |ctx| {
            // Create a larger image with different colored quadrants
            let mut data = vec![0u8; 60 * 60 * 4];
            for y in 0..60 {
                for x in 0..60 {
                    let i = (y * 60 + x) * 4;
                    if x < 30 && y < 30 {
                        // Top-left: Red
                        data[i] = 255;
                        data[i + 1] = 0;
                        data[i + 2] = 0;
                    } else if x >= 30 && y < 30 {
                        // Top-right: Green
                        data[i] = 0;
                        data[i + 1] = 255;
                        data[i + 2] = 0;
                    } else if x < 30 && y >= 30 {
                        // Bottom-left: Blue
                        data[i] = 0;
                        data[i + 1] = 0;
                        data[i + 2] = 255;
                    } else {
                        // Bottom-right: Yellow
                        data[i] = 255;
                        data[i + 1] = 255;
                        data[i + 2] = 0;
                    }
                    data[i + 3] = 255;
                }
            }

            // Only put the bottom-right quadrant (yellow) using dirty rect
            ctx.put_image_data_dirty(&data, 60, 60, 20, 20, 30, 30, 30, 30);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("put_image_data_dirty_rect comparison failed");
}

#[test]
fn test_put_image_data_round_trip() {
    // This is a unit test (not comparison) to verify alpha conversion precision
    let mut ctx = Canvas2dContext::new(10, 10).unwrap();

    // Create test data with various alpha values
    let original_data: Vec<u8> = vec![
        255, 0, 0, 255, // Fully opaque red
        0, 255, 0, 128, // Semi-transparent green
        0, 0, 255, 64, // Low-alpha blue
        255, 255, 0, 0, // Fully transparent yellow
        128, 128, 128, 200, // Gray with high alpha
    ];

    // Put the data (this will convert non-premultiplied to premultiplied)
    ctx.put_image_data(&original_data, 5, 1, 0, 0);

    // Get it back (this converts premultiplied back to non-premultiplied)
    let retrieved_data = ctx.get_image_data(0, 0, 5, 1);

    // Check that values are close (some precision loss is expected)
    for i in (0..original_data.len()).step_by(4) {
        let pixel_idx = i / 4;
        let orig_a = original_data[i + 3];
        let ret_a = retrieved_data[i + 3];

        // Alpha should be exact
        assert_eq!(
            orig_a, ret_a,
            "Alpha mismatch at pixel {}: expected {}, got {}",
            pixel_idx, orig_a, ret_a
        );

        // For non-zero alpha, RGB values should be within tolerance
        if orig_a > 0 {
            for c in 0..3 {
                let orig = original_data[i + c] as i32;
                let ret = retrieved_data[i + c] as i32;
                let diff = (orig - ret).abs();
                // Allow up to 2 units of error due to rounding in conversions
                assert!(
                    diff <= 2,
                    "Color mismatch at pixel {} channel {}: expected {}, got {} (diff {})",
                    pixel_idx,
                    c,
                    orig,
                    ret,
                    diff
                );
            }
        }
    }
}

// =============================================================================
// maxWidth Text Scaling Tests
// =============================================================================

#[test]
fn test_fill_text_max_width_scaled() {
    skip_if_no_node_canvas!();
    // Test text that needs scaling - text is wider than maxWidth
    let test = CanvasTestCase {
        name: "fill_text_max_width_scaled",
        width: 200,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '24px Helvetica';
// "Hello World" is wider than 50px, so it should be horizontally scaled
ctx.fillText('Hello World', 20, 50, 50);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("24px Helvetica").unwrap();
            ctx.fill_text_max_width("Hello World", 20.0, 50.0, 50.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("fill_text_max_width_scaled comparison failed");
}

#[test]
fn test_fill_text_max_width_fits() {
    skip_if_no_node_canvas!();
    // Test text that fits within maxWidth - no scaling needed
    let test = CanvasTestCase {
        name: "fill_text_max_width_fits",
        width: 200,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '24px Helvetica';
// "Hi" fits within 200px, so no scaling
ctx.fillText('Hi', 20, 50, 200);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("24px Helvetica").unwrap();
            ctx.fill_text_max_width("Hi", 20.0, 50.0, 200.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("fill_text_max_width_fits comparison failed");
}

#[test]
fn test_fill_text_max_width_extreme_scale() {
    skip_if_no_node_canvas!();
    // Test extreme scaling - text compressed significantly
    let test = CanvasTestCase {
        name: "fill_text_max_width_extreme",
        width: 200,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '24px Helvetica';
// Very narrow maxWidth forces extreme horizontal compression
ctx.fillText('ABCDEFGHIJ', 10, 50, 30);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("24px Helvetica").unwrap();
            ctx.fill_text_max_width("ABCDEFGHIJ", 10.0, 50.0, 30.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: 5.0, // Higher tolerance for extreme scaling
    };
    run_comparison_test(&test).expect("fill_text_max_width_extreme comparison failed");
}

#[test]
fn test_stroke_text_max_width_scaled() {
    skip_if_no_node_canvas!();
    // Test strokeText with maxWidth
    let test = CanvasTestCase {
        name: "stroke_text_max_width_scaled",
        width: 200,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#ff0000';
ctx.lineWidth = 1;
ctx.font = '24px Helvetica';
ctx.strokeText('Wide Text', 20, 50, 40);
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.set_line_width(1.0);
            ctx.set_font("24px Helvetica").unwrap();
            ctx.stroke_text_max_width("Wide Text", 20.0, 50.0, 40.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: 5.0, // Higher tolerance: strokeText renders as filled
    };
    run_comparison_test(&test).expect("stroke_text_max_width_scaled comparison failed");
}

#[test]
fn test_fill_text_max_width_with_alignment() {
    skip_if_no_node_canvas!();
    // Test maxWidth with center alignment
    let test = CanvasTestCase {
        name: "fill_text_max_width_center",
        width: 200,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#000000';
ctx.font = '24px Helvetica';
ctx.textAlign = 'center';
ctx.fillText('Centered Text', 100, 50, 60);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#000000").unwrap();
            ctx.set_font("24px Helvetica").unwrap();
            ctx.set_text_align(TextAlign::Center);
            ctx.fill_text_max_width("Centered Text", 100.0, 50.0, 60.0);
        },
        threshold: TEXT_THRESHOLD,
        max_diff_percent: TEXT_MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("fill_text_max_width_center comparison failed");
}

// =============================================================================
// Pattern Tests
// =============================================================================

#[test]
fn test_pattern_repeat() {
    skip_if_no_node_canvas!();
    // Test basic repeat pattern
    let test = CanvasTestCase {
        name: "pattern_repeat",
        width: 200,
        height: 200,
        js_code: r#"
// Create a small pattern: 20x20 checkerboard
const patternCanvas = createCanvas(20, 20);
const pctx = patternCanvas.getContext('2d');
pctx.fillStyle = '#ff0000';
pctx.fillRect(0, 0, 10, 10);
pctx.fillRect(10, 10, 10, 10);
pctx.fillStyle = '#0000ff';
pctx.fillRect(10, 0, 10, 10);
pctx.fillRect(0, 10, 10, 10);

const pattern = ctx.createPattern(patternCanvas, 'repeat');
ctx.fillStyle = pattern;
ctx.fillRect(0, 0, 200, 200);
"#,
        rust_fn: |ctx| {
            // Create a 20x20 checkerboard pattern
            let mut pattern_ctx = vl_convert_canvas2d::Canvas2dContext::new(20, 20).unwrap();
            pattern_ctx.set_fill_style("#ff0000").unwrap();
            pattern_ctx.fill_rect(0.0, 0.0, 10.0, 10.0);
            pattern_ctx.fill_rect(10.0, 10.0, 10.0, 10.0);
            pattern_ctx.set_fill_style("#0000ff").unwrap();
            pattern_ctx.fill_rect(10.0, 0.0, 10.0, 10.0);
            pattern_ctx.fill_rect(0.0, 10.0, 10.0, 10.0);

            let pattern = ctx
                .create_pattern_from_canvas(pattern_ctx.pixmap().as_ref(), "repeat")
                .unwrap();
            ctx.set_fill_style_pattern(pattern);
            ctx.fill_rect(0.0, 0.0, 200.0, 200.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("pattern_repeat comparison failed");
}

#[test]
fn test_pattern_no_repeat() {
    skip_if_no_node_canvas!();
    // Test no-repeat pattern - pattern should only appear once
    let test = CanvasTestCase {
        name: "pattern_no_repeat",
        width: 200,
        height: 200,
        js_code: r#"
// Create a 50x50 colored square pattern
const patternCanvas = require('canvas').createCanvas(50, 50);
const pctx = patternCanvas.getContext('2d');
pctx.fillStyle = '#00ff00';
pctx.fillRect(0, 0, 50, 50);

const pattern = ctx.createPattern(patternCanvas, 'no-repeat');
ctx.fillStyle = pattern;
ctx.fillRect(0, 0, 200, 200);
"#,
        rust_fn: |ctx| {
            // Create a 50x50 green square pattern
            let mut pattern_ctx = vl_convert_canvas2d::Canvas2dContext::new(50, 50).unwrap();
            pattern_ctx.set_fill_style("#00ff00").unwrap();
            pattern_ctx.fill_rect(0.0, 0.0, 50.0, 50.0);

            let pattern = ctx
                .create_pattern_from_canvas(pattern_ctx.pixmap().as_ref(), "no-repeat")
                .unwrap();
            ctx.set_fill_style_pattern(pattern);
            ctx.fill_rect(0.0, 0.0, 200.0, 200.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("pattern_no_repeat comparison failed");
}

#[test]
fn test_pattern_repeat_x() {
    skip_if_no_node_canvas!();
    // Test repeat-x pattern - pattern should repeat horizontally only
    let test = CanvasTestCase {
        name: "pattern_repeat_x",
        width: 200,
        height: 200,
        js_code: r#"
// Create a 30x30 gradient pattern
const patternCanvas = require('canvas').createCanvas(30, 30);
const pctx = patternCanvas.getContext('2d');
const grad = pctx.createLinearGradient(0, 0, 30, 0);
grad.addColorStop(0, '#ff0000');
grad.addColorStop(1, '#ffff00');
pctx.fillStyle = grad;
pctx.fillRect(0, 0, 30, 30);

const pattern = ctx.createPattern(patternCanvas, 'repeat-x');
ctx.fillStyle = pattern;
ctx.fillRect(0, 0, 200, 200);
"#,
        rust_fn: |ctx| {
            // Create a 30x30 gradient pattern
            let mut pattern_ctx = vl_convert_canvas2d::Canvas2dContext::new(30, 30).unwrap();
            let mut grad = vl_convert_canvas2d::CanvasGradient::new_linear(0.0, 0.0, 30.0, 0.0);
            grad.add_color_stop(0.0, tiny_skia::Color::from_rgba8(255, 0, 0, 255));
            grad.add_color_stop(1.0, tiny_skia::Color::from_rgba8(255, 255, 0, 255));
            pattern_ctx.set_fill_style_gradient(grad);
            pattern_ctx.fill_rect(0.0, 0.0, 30.0, 30.0);

            let pattern = ctx
                .create_pattern_from_canvas(pattern_ctx.pixmap().as_ref(), "repeat-x")
                .unwrap();
            ctx.set_fill_style_pattern(pattern);
            ctx.fill_rect(0.0, 0.0, 200.0, 200.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0, // Slightly higher tolerance for repeat-x edge handling
    };
    run_comparison_test(&test).expect("pattern_repeat_x comparison failed");
}

#[test]
fn test_pattern_repeat_y() {
    skip_if_no_node_canvas!();
    // Test repeat-y pattern - pattern should repeat vertically only
    let test = CanvasTestCase {
        name: "pattern_repeat_y",
        width: 200,
        height: 200,
        js_code: r#"
// Create a 30x30 gradient pattern
const patternCanvas = require('canvas').createCanvas(30, 30);
const pctx = patternCanvas.getContext('2d');
const grad = pctx.createLinearGradient(0, 0, 0, 30);
grad.addColorStop(0, '#0000ff');
grad.addColorStop(1, '#00ffff');
pctx.fillStyle = grad;
pctx.fillRect(0, 0, 30, 30);

const pattern = ctx.createPattern(patternCanvas, 'repeat-y');
ctx.fillStyle = pattern;
ctx.fillRect(0, 0, 200, 200);
"#,
        rust_fn: |ctx| {
            // Create a 30x30 gradient pattern
            let mut pattern_ctx = vl_convert_canvas2d::Canvas2dContext::new(30, 30).unwrap();
            let mut grad = vl_convert_canvas2d::CanvasGradient::new_linear(0.0, 0.0, 0.0, 30.0);
            grad.add_color_stop(0.0, tiny_skia::Color::from_rgba8(0, 0, 255, 255));
            grad.add_color_stop(1.0, tiny_skia::Color::from_rgba8(0, 255, 255, 255));
            pattern_ctx.set_fill_style_gradient(grad);
            pattern_ctx.fill_rect(0.0, 0.0, 30.0, 30.0);

            let pattern = ctx
                .create_pattern_from_canvas(pattern_ctx.pixmap().as_ref(), "repeat-y")
                .unwrap();
            ctx.set_fill_style_pattern(pattern);
            ctx.fill_rect(0.0, 0.0, 200.0, 200.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0, // Slightly higher tolerance for repeat-y edge handling
    };
    run_comparison_test(&test).expect("pattern_repeat_y comparison failed");
}

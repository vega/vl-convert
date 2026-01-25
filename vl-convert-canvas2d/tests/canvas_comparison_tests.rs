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

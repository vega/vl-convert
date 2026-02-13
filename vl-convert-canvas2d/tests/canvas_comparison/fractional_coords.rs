//\! Fractional coordinate / subpixel tests: fractional fill rect, subpixel line
//\! positioning, transforms with fractional coordinates, and rotated content.

use super::common::*;

#[test]
fn test_fractional_fill_rect_comparison() {
    skip_if_no_node_canvas!();
    // Test fillRect at fractional coordinates
    let test = CanvasTestCase {
        name: "fractional_fill_rect",
        width: 100,
        height: 100,
        js_code: r#"
ctx.fillStyle = '#ff0000';
ctx.fillRect(10.5, 10.5, 30.0, 30.0);

ctx.fillStyle = '#00ff00';
ctx.fillRect(50.25, 50.75, 25.5, 25.5);

ctx.fillStyle = '#0000ff';
ctx.fillRect(20.9, 60.1, 20.0, 20.0);
"#,
        rust_fn: |ctx| {
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.fill_rect(&RectParams {
                x: 10.5,
                y: 10.5,
                width: 30.0,
                height: 30.0,
            });

            ctx.set_fill_style("#00ff00").unwrap();
            ctx.fill_rect(&RectParams {
                x: 50.25,
                y: 50.75,
                width: 25.5,
                height: 25.5,
            });

            ctx.set_fill_style("#0000ff").unwrap();
            ctx.fill_rect(&RectParams {
                x: 20.9,
                y: 60.1,
                width: 20.0,
                height: 20.0,
            });
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("fractional_fill_rect comparison failed");
}

#[test]
fn test_subpixel_line_positioning_comparison() {
    skip_if_no_node_canvas!();
    // Test lines at fractional coordinates
    let test = CanvasTestCase {
        name: "subpixel_line_positioning",
        width: 100,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#000000';
ctx.lineWidth = 1;

// Line at integer coordinates
ctx.beginPath();
ctx.moveTo(10, 20);
ctx.lineTo(90, 20);
ctx.stroke();

// Line at .5 offset (should be crisp on most renderers)
ctx.beginPath();
ctx.moveTo(10.5, 40.5);
ctx.lineTo(90.5, 40.5);
ctx.stroke();

// Line at fractional coordinates
ctx.beginPath();
ctx.moveTo(10.25, 60.75);
ctx.lineTo(90.25, 60.75);
ctx.stroke();

// Vertical lines with subpixel positioning
ctx.beginPath();
ctx.moveTo(30, 10);
ctx.lineTo(30, 90);
ctx.stroke();

ctx.beginPath();
ctx.moveTo(50.5, 10.5);
ctx.lineTo(50.5, 90.5);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#000000").unwrap();
            ctx.set_line_width(1.0);

            // Line at integer coordinates
            ctx.begin_path();
            ctx.move_to(10.0, 20.0);
            ctx.line_to(90.0, 20.0);
            ctx.stroke();

            // Line at .5 offset (should be crisp on most renderers)
            ctx.begin_path();
            ctx.move_to(10.5, 40.5);
            ctx.line_to(90.5, 40.5);
            ctx.stroke();

            // Line at fractional coordinates
            ctx.begin_path();
            ctx.move_to(10.25, 60.75);
            ctx.line_to(90.25, 60.75);
            ctx.stroke();

            // Vertical lines with subpixel positioning
            ctx.begin_path();
            ctx.move_to(30.0, 10.0);
            ctx.line_to(30.0, 90.0);
            ctx.stroke();

            ctx.begin_path();
            ctx.move_to(50.5, 10.5);
            ctx.line_to(50.5, 90.5);
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0, // Subpixel rendering may differ between implementations
    };
    run_comparison_test(&test).expect("subpixel_line_positioning comparison failed");
}

#[test]
fn test_fill_rect_with_transform_comparison() {
    skip_if_no_node_canvas!();
    // Test fillRect behavior after various transforms
    let test = CanvasTestCase {
        name: "fill_rect_with_transform",
        width: 150,
        height: 150,
        js_code: r#"
// Original rectangle for reference
ctx.fillStyle = '#cccccc';
ctx.fillRect(10, 10, 30, 30);

// Translated rectangle
ctx.save();
ctx.translate(50, 0);
ctx.fillStyle = '#ff0000';
ctx.fillRect(10, 10, 30, 30);
ctx.restore();

// Scaled rectangle
ctx.save();
ctx.translate(0, 50);
ctx.scale(1.5, 1.5);
ctx.fillStyle = '#00ff00';
ctx.fillRect(10, 10, 20, 20);
ctx.restore();

// Rotated rectangle
ctx.save();
ctx.translate(100, 100);
ctx.rotate(Math.PI / 6);
ctx.fillStyle = '#0000ff';
ctx.fillRect(-15, -15, 30, 30);
ctx.restore();

// Combined transforms
ctx.save();
ctx.translate(50, 100);
ctx.scale(0.8, 1.2);
ctx.rotate(-Math.PI / 8);
ctx.fillStyle = '#ff00ff';
ctx.fillRect(-10, -10, 20, 20);
ctx.restore();
"#,
        rust_fn: |ctx| {
            // Original rectangle for reference
            ctx.set_fill_style("#cccccc").unwrap();
            ctx.fill_rect(&RectParams {
                x: 10.0,
                y: 10.0,
                width: 30.0,
                height: 30.0,
            });

            // Translated rectangle
            ctx.save();
            ctx.translate(50.0, 0.0);
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.fill_rect(&RectParams {
                x: 10.0,
                y: 10.0,
                width: 30.0,
                height: 30.0,
            });
            ctx.restore();

            // Scaled rectangle
            ctx.save();
            ctx.translate(0.0, 50.0);
            ctx.scale(1.5, 1.5);
            ctx.set_fill_style("#00ff00").unwrap();
            ctx.fill_rect(&RectParams {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 20.0,
            });
            ctx.restore();

            // Rotated rectangle
            ctx.save();
            ctx.translate(100.0, 100.0);
            ctx.rotate(PI / 6.0);
            ctx.set_fill_style("#0000ff").unwrap();
            ctx.fill_rect(&RectParams {
                x: -15.0,
                y: -15.0,
                width: 30.0,
                height: 30.0,
            });
            ctx.restore();

            // Combined transforms
            ctx.save();
            ctx.translate(50.0, 100.0);
            ctx.scale(0.8, 1.2);
            ctx.rotate(-PI / 8.0);
            ctx.set_fill_style("#ff00ff").unwrap();
            ctx.fill_rect(&RectParams {
                x: -10.0,
                y: -10.0,
                width: 20.0,
                height: 20.0,
            });
            ctx.restore();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("fill_rect_with_transform comparison failed");
}

#[test]
fn test_get_image_data_rotated_content_comparison() {
    skip_if_no_node_canvas!();
    // Test that rotation affects rendering but not getImageData coordinates
    let test = CanvasTestCase {
        name: "get_image_data_rotated_content",
        width: 100,
        height: 100,
        js_code: r#"
// Draw rotated content
ctx.translate(50, 50);
ctx.rotate(Math.PI / 4);
ctx.fillStyle = '#ff0000';
ctx.fillRect(-20, -20, 40, 40);

// Reset transform for getImageData
ctx.setTransform(1, 0, 0, 1, 0, 0);

// Get image data from a specific area (should get actual rendered pixels)
const imageData = ctx.getImageData(30, 30, 40, 40);

// Clear and show what we captured
ctx.clearRect(0, 0, 100, 100);
ctx.putImageData(imageData, 5, 5);
"#,
        rust_fn: |ctx| {
            // Draw rotated content
            ctx.translate(50.0, 50.0);
            ctx.rotate(PI / 4.0);
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.fill_rect(&RectParams {
                x: -20.0,
                y: -20.0,
                width: 40.0,
                height: 40.0,
            });

            // Reset transform for getImageData
            ctx.set_transform(DOMMatrix {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                e: 0.0,
                f: 0.0,
            });

            // Get image data from a specific area
            let image_data = ctx.get_image_data(30, 30, 40, 40);

            // Clear and show what we captured
            ctx.clear_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            });
            ctx.put_image_data(&image_data, 40, 40, 5, 5);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("get_image_data_rotated_content comparison failed");
}

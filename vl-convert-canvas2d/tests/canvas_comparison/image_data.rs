//\! getImageData / putImageData edge case tests: partial bounds, negative coords,
//\! transforms, alpha values, outside bounds, and roundtrip.

use super::common::*;

#[test]
fn test_get_image_data_partial_bounds_comparison() {
    skip_if_no_node_canvas!();
    // Test getImageData with a region that extends outside canvas bounds
    let test = CanvasTestCase {
        name: "get_image_data_partial_bounds",
        width: 100,
        height: 100,
        js_code: r#"
// Draw a red rectangle
ctx.fillStyle = '#ff0000';
ctx.fillRect(10, 10, 80, 80);

// Get image data that partially extends outside canvas
const imageData = ctx.getImageData(80, 80, 40, 40);

// Clear canvas
ctx.clearRect(0, 0, 100, 100);

// Put the image data at a new location
ctx.putImageData(imageData, 0, 0);
"#,
        rust_fn: |ctx| {
            // Draw a red rectangle
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.fill_rect(&RectParams {
                x: 10.0,
                y: 10.0,
                width: 80.0,
                height: 80.0,
            });

            // Get image data that partially extends outside canvas
            let image_data = ctx.get_image_data(80, 80, 40, 40);

            // Clear canvas
            ctx.clear_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            });

            // Put the image data at a new location
            ctx.put_image_data(&image_data, 40, 40, 0, 0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("get_image_data_partial_bounds comparison failed");
}

/// Note: This test doesn't compare against node-canvas because node-canvas has a bug
/// where getImageData with negative coords returns clipped dimensions instead of
/// the requested dimensions with transparent padding (as per HTML Canvas spec).
/// Our Rust implementation correctly follows the spec.
#[test]
fn test_get_image_data_negative_coords() {
    // Test getImageData with negative coordinates - Rust-only unit test
    let mut ctx = vl_convert_canvas2d::Canvas2dContext::new(100, 100).unwrap();

    // Draw a blue rectangle in corner
    ctx.set_fill_style("#0000ff").unwrap();
    ctx.fill_rect(&RectParams {
        x: 0.0,
        y: 0.0,
        width: 40.0,
        height: 40.0,
    });

    // Get image data starting at negative coordinates
    // This should return 30x30 with transparent pixels for out-of-bounds areas
    let image_data = ctx.get_image_data(-10, -10, 30, 30);

    // Verify dimensions: should be 30*30*4 = 3600 bytes
    assert_eq!(image_data.len(), 30 * 30 * 4);

    // Pixels at (0,0) to (9,9) should be transparent (out of canvas bounds)
    let idx_0_0 = 0;
    assert_eq!(
        &image_data[idx_0_0..idx_0_0 + 4],
        &[0, 0, 0, 0],
        "Pixel at (0,0) should be transparent"
    );

    // Pixels at (10,10) should be blue (maps to canvas (0,0))
    let idx_10_10 = (10 * 30 + 10) * 4;
    assert_eq!(
        image_data[idx_10_10 + 2],
        255,
        "Pixel at (10,10) should be blue"
    );
    assert_eq!(
        image_data[idx_10_10 + 3],
        255,
        "Pixel at (10,10) should be opaque"
    );

    // Pixels at (29,29) should be blue (maps to canvas (19,19))
    let idx_29_29 = (29 * 30 + 29) * 4;
    assert_eq!(
        image_data[idx_29_29 + 2],
        255,
        "Pixel at (29,29) should be blue"
    );
}

#[test]
fn test_get_image_data_after_transform_comparison() {
    skip_if_no_node_canvas!();
    // Test that transforms don't affect getImageData pixel coordinates
    let test = CanvasTestCase {
        name: "get_image_data_after_transform",
        width: 100,
        height: 100,
        js_code: r#"
// Draw a green rectangle
ctx.fillStyle = '#00ff00';
ctx.fillRect(20, 20, 40, 40);

// Apply a transform
ctx.translate(50, 0);
ctx.rotate(Math.PI / 4);

// getImageData should still use original pixel coordinates, not transformed ones
const imageData = ctx.getImageData(20, 20, 40, 40);

// Reset transform
ctx.setTransform(1, 0, 0, 1, 0, 0);

// Clear and put image data
ctx.clearRect(0, 0, 100, 100);
ctx.putImageData(imageData, 50, 50);
"#,
        rust_fn: |ctx| {
            // Draw a green rectangle
            ctx.set_fill_style("#00ff00").unwrap();
            ctx.fill_rect(&RectParams {
                x: 20.0,
                y: 20.0,
                width: 40.0,
                height: 40.0,
            });

            // Apply a transform
            ctx.translate(50.0, 0.0);
            ctx.rotate(PI / 4.0);

            // getImageData should still use original pixel coordinates
            let image_data = ctx.get_image_data(20, 20, 40, 40);

            // Reset transform
            ctx.set_transform(DOMMatrix {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                e: 0.0,
                f: 0.0,
            });

            // Clear and put image data
            ctx.clear_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            });
            ctx.put_image_data(&image_data, 40, 40, 50, 50);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("get_image_data_after_transform comparison failed");
}

#[test]
fn test_get_image_data_alpha_values_comparison() {
    skip_if_no_node_canvas!();
    // Test that alpha channel is correctly preserved
    let test = CanvasTestCase {
        name: "get_image_data_alpha_values",
        width: 100,
        height: 100,
        js_code: r#"
// Draw semi-transparent rectangles
ctx.fillStyle = 'rgba(255, 0, 0, 0.5)';
ctx.fillRect(10, 10, 40, 40);

ctx.fillStyle = 'rgba(0, 0, 255, 0.25)';
ctx.fillRect(30, 30, 40, 40);

// Get image data
const imageData = ctx.getImageData(10, 10, 60, 60);

// Clear and put at new position
ctx.clearRect(0, 0, 100, 100);
ctx.putImageData(imageData, 30, 30);
"#,
        rust_fn: |ctx| {
            // Draw semi-transparent rectangles
            ctx.set_fill_style("rgba(255, 0, 0, 0.5)").unwrap();
            ctx.fill_rect(&RectParams {
                x: 10.0,
                y: 10.0,
                width: 40.0,
                height: 40.0,
            });

            ctx.set_fill_style("rgba(0, 0, 255, 0.25)").unwrap();
            ctx.fill_rect(&RectParams {
                x: 30.0,
                y: 30.0,
                width: 40.0,
                height: 40.0,
            });

            // Get image data
            let image_data = ctx.get_image_data(10, 10, 60, 60);

            // Clear and put at new position
            ctx.clear_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            });
            ctx.put_image_data(&image_data, 60, 60, 30, 30);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0, // Slightly higher for alpha compositing differences
    };
    run_comparison_test(&test).expect("get_image_data_alpha_values comparison failed");
}

#[test]
fn test_put_image_data_outside_bounds_comparison() {
    skip_if_no_node_canvas!();
    // Test putImageData with destination partially outside canvas
    let test = CanvasTestCase {
        name: "put_image_data_outside_bounds",
        width: 100,
        height: 100,
        js_code: r#"
// Draw a red rectangle
ctx.fillStyle = '#ff0000';
ctx.fillRect(0, 0, 50, 50);

// Get image data
const imageData = ctx.getImageData(0, 0, 50, 50);

// Clear canvas
ctx.clearRect(0, 0, 100, 100);

// Put image data with part outside canvas bounds
ctx.putImageData(imageData, 70, 70);
"#,
        rust_fn: |ctx| {
            // Draw a red rectangle
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 50.0,
            });

            // Get image data
            let image_data = ctx.get_image_data(0, 0, 50, 50);

            // Clear canvas
            ctx.clear_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            });

            // Put image data with part outside canvas bounds
            ctx.put_image_data(&image_data, 50, 50, 70, 70);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("put_image_data_outside_bounds comparison failed");
}

#[test]
fn test_put_image_data_roundtrip_comparison() {
    skip_if_no_node_canvas!();
    // Test that get then put preserves pixels exactly
    let test = CanvasTestCase {
        name: "put_image_data_roundtrip",
        width: 100,
        height: 100,
        js_code: r#"
// Draw complex pattern
ctx.fillStyle = '#ff0000';
ctx.fillRect(10, 10, 30, 30);
ctx.fillStyle = '#00ff00';
ctx.fillRect(30, 30, 30, 30);
ctx.fillStyle = '#0000ff';
ctx.fillRect(50, 50, 30, 30);

// Draw a circle
ctx.fillStyle = '#ffff00';
ctx.beginPath();
ctx.arc(50, 50, 20, 0, Math.PI * 2);
ctx.fill();

// Get entire image data
const imageData = ctx.getImageData(0, 0, 100, 100);

// Clear and put back - should look identical
ctx.clearRect(0, 0, 100, 100);
ctx.putImageData(imageData, 0, 0);
"#,
        rust_fn: |ctx| {
            // Draw complex pattern
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.fill_rect(&RectParams {
                x: 10.0,
                y: 10.0,
                width: 30.0,
                height: 30.0,
            });
            ctx.set_fill_style("#00ff00").unwrap();
            ctx.fill_rect(&RectParams {
                x: 30.0,
                y: 30.0,
                width: 30.0,
                height: 30.0,
            });
            ctx.set_fill_style("#0000ff").unwrap();
            ctx.fill_rect(&RectParams {
                x: 50.0,
                y: 50.0,
                width: 30.0,
                height: 30.0,
            });

            // Draw a circle
            ctx.set_fill_style("#ffff00").unwrap();
            ctx.begin_path();
            ctx.arc(&ArcParams {
                x: 50.0,
                y: 50.0,
                radius: 20.0,
                start_angle: 0.0,
                end_angle: 2.0 * PI,
                anticlockwise: false,
            });
            ctx.fill();

            // Get entire image data
            let image_data = ctx.get_image_data(0, 0, 100, 100);

            // Clear and put back
            ctx.clear_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            });
            ctx.put_image_data(&image_data, 100, 100, 0, 0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("put_image_data_roundtrip comparison failed");
}


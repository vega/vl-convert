//\! putImageData tests: basic, alpha, dirty rect, and round trip.

use super::common::*;

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
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            });

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
            ctx.put_image_data_dirty(
                &data,
                60,
                60,
                20,
                20,
                &DirtyRect {
                    x: 30,
                    y: 30,
                    width: 30,
                    height: 30,
                },
            );
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


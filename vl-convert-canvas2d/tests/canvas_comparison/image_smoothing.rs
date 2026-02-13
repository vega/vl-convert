//\! Image smoothing tests: disabled and quality settings.

use super::common::*;

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
            pattern_ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 2.0,
                height: 2.0,
            });
            pattern_ctx.fill_rect(&RectParams {
                x: 2.0,
                y: 2.0,
                width: 2.0,
                height: 2.0,
            });
            pattern_ctx.set_fill_style("#0000ff").unwrap();
            pattern_ctx.fill_rect(&RectParams {
                x: 2.0,
                y: 0.0,
                width: 2.0,
                height: 2.0,
            });
            pattern_ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 2.0,
                width: 2.0,
                height: 2.0,
            });

            // Disable image smoothing
            ctx.set_image_smoothing_enabled(false);

            // Scale the small pattern up - should show sharp pixels
            ctx.draw_canvas_scaled(&pattern_ctx, 0.0, 0.0, 100.0, 100.0);
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
            gradient.add_color_stop(0.0, CanvasColor::from_rgba8(255, 0, 0, 255));
            gradient.add_color_stop(1.0, CanvasColor::from_rgba8(0, 0, 255, 255));
            pattern_ctx.set_fill_style_gradient(gradient);
            pattern_ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            });

            // Enable high quality smoothing
            ctx.set_image_smoothing_enabled(true);
            ctx.set_image_smoothing_quality(vl_convert_canvas2d::ImageSmoothingQuality::High);

            // Scale up
            ctx.draw_canvas_scaled(&pattern_ctx, 0.0, 0.0, 100.0, 100.0);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0, // Higher tolerance for interpolation differences
    };
    run_comparison_test(&test).expect("image_smoothing_quality_high comparison failed");
}


//\! Pattern tests: repeat, no-repeat, repeat-x, and repeat-y.

use super::common::*;

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
            pattern_ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            });
            pattern_ctx.fill_rect(&RectParams {
                x: 10.0,
                y: 10.0,
                width: 10.0,
                height: 10.0,
            });
            pattern_ctx.set_fill_style("#0000ff").unwrap();
            pattern_ctx.fill_rect(&RectParams {
                x: 10.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            });
            pattern_ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 10.0,
                width: 10.0,
                height: 10.0,
            });

            let pattern = ctx
                .create_pattern_from_canvas(&pattern_ctx, "repeat")
                .unwrap();
            ctx.set_fill_style_pattern(pattern);
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 200.0,
            });
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
            pattern_ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 50.0,
            });

            let pattern = ctx
                .create_pattern_from_canvas(&pattern_ctx, "no-repeat")
                .unwrap();
            ctx.set_fill_style_pattern(pattern);
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 200.0,
            });
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
            grad.add_color_stop(0.0, CanvasColor::from_rgba8(255, 0, 0, 255));
            grad.add_color_stop(1.0, CanvasColor::from_rgba8(255, 255, 0, 255));
            pattern_ctx.set_fill_style_gradient(grad);
            pattern_ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 30.0,
                height: 30.0,
            });

            let pattern = ctx
                .create_pattern_from_canvas(&pattern_ctx, "repeat-x")
                .unwrap();
            ctx.set_fill_style_pattern(pattern);
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 200.0,
            });
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
            grad.add_color_stop(0.0, CanvasColor::from_rgba8(0, 0, 255, 255));
            grad.add_color_stop(1.0, CanvasColor::from_rgba8(0, 255, 255, 255));
            pattern_ctx.set_fill_style_gradient(grad);
            pattern_ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 30.0,
                height: 30.0,
            });

            let pattern = ctx
                .create_pattern_from_canvas(&pattern_ctx, "repeat-y")
                .unwrap();
            ctx.set_fill_style_pattern(pattern);
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 200.0,
            });
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 3.0, // Slightly higher tolerance for repeat-y edge handling
    };
    run_comparison_test(&test).expect("pattern_repeat_y comparison failed");
}

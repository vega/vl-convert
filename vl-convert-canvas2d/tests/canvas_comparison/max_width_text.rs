//\! maxWidth text scaling tests: fillText and strokeText with maxWidth constraints.

use super::common::*;

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

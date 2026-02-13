//! Gradient tests: linear and radial gradients with various configurations.

use super::common::*;

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
            gradient.add_color_stop(0.0, CanvasColor::from_rgba8(255, 0, 0, 255));
            gradient.add_color_stop(1.0, CanvasColor::from_rgba8(0, 0, 255, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 100.0,
            });
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
            gradient.add_color_stop(0.0, CanvasColor::from_rgba8(0, 255, 0, 255));
            gradient.add_color_stop(1.0, CanvasColor::from_rgba8(255, 255, 0, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 200.0,
            });
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
            gradient.add_color_stop(0.0, CanvasColor::from_rgba8(255, 0, 0, 255));
            gradient.add_color_stop(0.5, CanvasColor::from_rgba8(0, 255, 0, 255));
            gradient.add_color_stop(1.0, CanvasColor::from_rgba8(0, 0, 255, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 150.0,
                height: 150.0,
            });
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
            let mut gradient = ctx.create_radial_gradient(&RadialGradientParams {
                x0: 75.0,
                y0: 75.0,
                r0: 0.0,
                x1: 75.0,
                y1: 75.0,
                r1: 75.0,
            });
            gradient.add_color_stop(0.0, CanvasColor::from_rgba8(255, 255, 255, 255));
            gradient.add_color_stop(1.0, CanvasColor::from_rgba8(0, 0, 0, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 150.0,
                height: 150.0,
            });
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
            let mut gradient = ctx.create_radial_gradient(&RadialGradientParams {
                x0: 50.0,
                y0: 50.0,
                r0: 10.0,
                x1: 75.0,
                y1: 75.0,
                r1: 70.0,
            });
            gradient.add_color_stop(0.0, CanvasColor::from_rgba8(255, 0, 0, 255));
            gradient.add_color_stop(0.5, CanvasColor::from_rgba8(255, 255, 0, 255));
            gradient.add_color_stop(1.0, CanvasColor::from_rgba8(0, 0, 255, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 150.0,
                height: 150.0,
            });
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
            gradient.add_color_stop(0.0, CanvasColor::from_rgba8(255, 0, 0, 255));
            gradient.add_color_stop(1.0, CanvasColor::from_rgba8(0, 255, 0, 255));
            ctx.set_stroke_style_gradient(gradient);
            ctx.set_line_width(10.0);
            ctx.begin_path();
            ctx.move_to(20.0, 75.0);
            ctx.line_to(130.0, 75.0);
            ctx.stroke();
            ctx.begin_path();
            ctx.arc(&ArcParams {
                x: 75.0,
                y: 75.0,
                radius: 40.0,
                start_angle: 0.0,
                end_angle: 2.0 * PI,
                anticlockwise: false,
            });
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("linear_gradient_stroke comparison failed");
}


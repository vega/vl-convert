//! Clipping tests: clip rect, circle, complex path, transform, evenodd, and Path2D clipping.

use super::common::*;

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
ctx.rect(&RectParams { x: 25, y: 25, width: 100, height: 100 });
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
            ctx.rect(&RectParams {
                x: 25.0,
                y: 25.0,
                width: 100.0,
                height: 100.0,
            });
            ctx.clip();

            // Fill entire canvas - only clipped region should be visible
            ctx.set_fill_style("#ff0000").unwrap();
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 150.0,
                height: 150.0,
            });

            // Draw a circle that extends outside clip region
            ctx.set_fill_style("#0000ff").unwrap();
            ctx.begin_path();
            ctx.arc(&ArcParams {
                x: 75.0,
                y: 75.0,
                radius: 60.0,
                start_angle: 0.0,
                end_angle: 2.0 * PI,
                anticlockwise: false,
            });
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
            ctx.arc(&ArcParams {
                x: 75.0,
                y: 75.0,
                radius: 50.0,
                start_angle: 0.0,
                end_angle: 2.0 * PI,
                anticlockwise: false,
            });
            ctx.clip();

            // Draw gradient background
            let mut gradient = ctx.create_linear_gradient(0.0, 0.0, 150.0, 150.0);
            gradient.add_color_stop(0.0, CanvasColor::from_rgba8(255, 0, 0, 255));
            gradient.add_color_stop(1.0, CanvasColor::from_rgba8(0, 0, 255, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 150.0,
                height: 150.0,
            });

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
            let mut gradient = ctx.create_radial_gradient(&RadialGradientParams {
                x0: 75.0,
                y0: 75.0,
                r0: 0.0,
                x1: 75.0,
                y1: 75.0,
                r1: 75.0,
            });
            gradient.add_color_stop(0.0, CanvasColor::from_rgba8(255, 255, 0, 255));
            gradient.add_color_stop(1.0, CanvasColor::from_rgba8(255, 0, 0, 255));
            ctx.set_fill_style_gradient(gradient);
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 150.0,
                height: 150.0,
            });
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
ctx.rect(&RectParams { x: 37.5, y: 37.5, width: 75, height: 75 });
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
            ctx.rect(&RectParams {
                x: 37.5,
                y: 37.5,
                width: 75.0,
                height: 75.0,
            });
            ctx.clip();

            // Fill with solid color
            ctx.set_fill_style("#00ff00").unwrap();
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 150.0,
                height: 150.0,
            });

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

#[test]
fn test_clip_evenodd_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "clip_evenodd",
        width: 160,
        height: 160,
        js_code: r#"
ctx.beginPath();
ctx.rect(20, 20, 120, 120);
ctx.rect(50, 50, 60, 60);
ctx.clip('evenodd');

ctx.fillStyle = '#ff8800';
ctx.fillRect(0, 0, 160, 160);
"#,
        rust_fn: |ctx| {
            ctx.begin_path();
            ctx.rect(&RectParams {
                x: 20.0,
                y: 20.0,
                width: 120.0,
                height: 120.0,
            });
            ctx.rect(&RectParams {
                x: 50.0,
                y: 50.0,
                width: 60.0,
                height: 60.0,
            });
            ctx.clip_with_rule(CanvasFillRule::EvenOdd);

            ctx.set_fill_style("#ff8800").unwrap();
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 160.0,
                height: 160.0,
            });
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("clip_evenodd comparison failed");
}

#[test]
fn test_clip_path2d_evenodd_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "clip_path2d_evenodd",
        width: 160,
        height: 160,
        // node-canvas Path2D coverage is inconsistent; compare against equivalent context-path clip.
        js_code: r#"
ctx.beginPath();
ctx.rect(20, 20, 120, 120);
ctx.rect(55, 55, 50, 50);
ctx.clip('evenodd');

ctx.fillStyle = '#0055ff';
ctx.fillRect(0, 0, 160, 160);
"#,
        rust_fn: |ctx| {
            let mut path = Path2D::new();
            path.rect(&RectParams {
                x: 20.0,
                y: 20.0,
                width: 120.0,
                height: 120.0,
            });
            path.rect(&RectParams {
                x: 55.0,
                y: 55.0,
                width: 50.0,
                height: 50.0,
            });
            ctx.clip_path2d_with_rule(&mut path, CanvasFillRule::EvenOdd);

            ctx.set_fill_style("#0055ff").unwrap();
            ctx.fill_rect(&RectParams {
                x: 0.0,
                y: 0.0,
                width: 160.0,
                height: 160.0,
            });
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("clip_path2d_evenodd comparison failed");
}


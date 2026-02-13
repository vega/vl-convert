//! Feature tests: fill(fillRule), roundRect, Path2D, SVG path data.

use super::common::*;

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
            ctx.round_rect(&RoundRectParams {
                x: 20.0,
                y: 20.0,
                width: 110.0,
                height: 110.0,
                radii: [
                    CornerRadius { x: 15.0, y: 15.0 },
                    CornerRadius { x: 15.0, y: 15.0 },
                    CornerRadius { x: 15.0, y: 15.0 },
                    CornerRadius { x: 15.0, y: 15.0 },
                ],
            });
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
            ctx.round_rect(&RoundRectParams {
                x: 20.0,
                y: 20.0,
                width: 110.0,
                height: 110.0,
                radii: [
                    CornerRadius { x: 5.0, y: 5.0 },
                    CornerRadius { x: 15.0, y: 15.0 },
                    CornerRadius { x: 25.0, y: 25.0 },
                    CornerRadius { x: 35.0, y: 35.0 },
                ],
            });
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
            ctx.round_rect(&RoundRectParams {
                x: 25.0,
                y: 25.0,
                width: 100.0,
                height: 100.0,
                radii: [
                    CornerRadius { x: 20.0, y: 20.0 },
                    CornerRadius { x: 20.0, y: 20.0 },
                    CornerRadius { x: 20.0, y: 20.0 },
                    CornerRadius { x: 20.0, y: 20.0 },
                ],
            });
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
ctx.rect(&RectParams { x: 20, y: 20, width: 50, height: 50 });
ctx.rect(&RectParams { x: 80, y: 80, width: 50, height: 50 });
ctx.fill();
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            let mut path = Path2D::new();
            path.rect(&RectParams {
                x: 20.0,
                y: 20.0,
                width: 50.0,
                height: 50.0,
            });
            path.rect(&RectParams {
                x: 80.0,
                y: 80.0,
                width: 50.0,
                height: 50.0,
            });

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
            path.arc(&ArcParams {
                x: 0.0,
                y: 0.0,
                radius: 20.0,
                start_angle: 0.0,
                end_angle: 2.0 * PI,
                anticlockwise: false,
            });

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
            let mut path =
                Path2D::from_svg_path_data("M20,75 C20,20 130,20 130,75 C130,130 20,130 20,75")
                    .unwrap();
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
            let mut path =
                Path2D::from_svg_path_data("M30,75 A45,45 0 0 1 120,75 L120,120 L30,120 Z")
                    .unwrap();
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
                "m10,10 l50,0 l0,50 l-50,0 z m70,70 l60,0 l0,60 l-60,0 z",
            )
            .unwrap();
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
                "M75,10 L90,55 L140,55 L100,85 L115,130 L75,100 L35,130 L50,85 L10,55 L60,55 Z",
            )
            .unwrap();
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
                "M10,10 L60,10 L60,60 L10,60 Z M90,90 L140,90 L140,140 L90,140 Z",
            )
            .unwrap();
            ctx.set_fill_style("#339966").unwrap();
            ctx.fill_path2d(&mut path);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("path2d_svg_multi_subpath comparison failed");
}

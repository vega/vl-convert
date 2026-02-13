//\! Transform timing tests: verify that transforms are applied at the correct time
//\! for path construction, stroke, fill, and Path2D operations.

use super::common::*;

#[test]
fn test_arc_non_uniform_scale_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "arc_non_uniform_scale",
        width: 220,
        height: 140,
        js_code: r#"
ctx.strokeStyle = '#cc0000';
ctx.lineWidth = 3;
ctx.scale(2, 0.5);
ctx.beginPath();
ctx.arc(50, 120, 35, 0, Math.PI * 1.75, false);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#cc0000").unwrap();
            ctx.set_line_width(3.0);
            ctx.scale(2.0, 0.5);
            ctx.begin_path();
            ctx.arc(&ArcParams {
                x: 50.0,
                y: 120.0,
                radius: 35.0,
                start_angle: 0.0,
                end_angle: PI * 1.75,
                anticlockwise: false,
            });
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("arc_non_uniform_scale comparison failed");
}

#[test]
fn test_arc_shear_transform_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "arc_shear_transform",
        width: 220,
        height: 180,
        js_code: r#"
ctx.strokeStyle = '#0066cc';
ctx.lineWidth = 2;
ctx.transform(1, 0.45, 0.2, 1, 15, 5);
ctx.beginPath();
ctx.arc(70, 60, 30, Math.PI * 0.25, Math.PI * 1.8, false);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#0066cc").unwrap();
            ctx.set_line_width(2.0);
            ctx.transform(DOMMatrix::new(1.0, 0.45, 0.2, 1.0, 15.0, 5.0));
            ctx.begin_path();
            ctx.arc(&ArcParams {
                x: 70.0,
                y: 60.0,
                radius: 30.0,
                start_angle: PI * 0.25,
                end_angle: PI * 1.8,
                anticlockwise: false,
            });
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("arc_shear_transform comparison failed");
}

#[test]
fn test_arc_to_non_uniform_scale_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "arc_to_non_uniform_scale",
        width: 220,
        height: 140,
        js_code: r#"
ctx.strokeStyle = '#009944';
ctx.lineWidth = 3;
ctx.scale(1.8, 0.6);
ctx.beginPath();
ctx.moveTo(20, 30);
ctx.arcTo(90, 30, 90, 90, 25);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#009944").unwrap();
            ctx.set_line_width(3.0);
            ctx.scale(1.8, 0.6);
            ctx.begin_path();
            ctx.move_to(20.0, 30.0);
            ctx.arc_to(&ArcToParams {
                x1: 90.0,
                y1: 30.0,
                x2: 90.0,
                y2: 90.0,
                radius: 25.0,
            });
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("arc_to_non_uniform_scale comparison failed");
}

#[test]
fn test_arc_to_after_transform_change_comparison() {
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "arc_to_after_transform_change",
        width: 220,
        height: 160,
        js_code: r#"
ctx.strokeStyle = '#aa00aa';
ctx.lineWidth = 3;
ctx.beginPath();
ctx.moveTo(30, 30);
ctx.scale(1.6, 0.75);
ctx.arcTo(110, 30, 110, 100, 28);
ctx.setTransform(1, 0, 0, 1, 0, 0);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#aa00aa").unwrap();
            ctx.set_line_width(3.0);
            ctx.begin_path();
            ctx.move_to(30.0, 30.0);
            ctx.scale(1.6, 0.75);
            ctx.arc_to(&ArcToParams {
                x1: 110.0,
                y1: 30.0,
                x2: 110.0,
                y2: 100.0,
                radius: 28.0,
            });
            ctx.set_transform(DOMMatrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0));
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: 2.0,
    };
    run_comparison_test(&test).expect("arc_to_after_transform_change comparison failed");
}

#[test]
fn test_transform_before_path_reset_before_stroke() {
    // Test: Scale applied before path construction, then reset before stroke.
    // Per spec, path coordinates ARE transformed when added, so the path
    // should remain scaled even after resetting the transform.
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "transform_before_path_reset_before_stroke",
        width: 200,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#ff0000';
ctx.lineWidth = 2;
ctx.scale(2, 2);
ctx.beginPath();
ctx.moveTo(10, 10);
ctx.lineTo(50, 40);
ctx.setTransform(1, 0, 0, 1, 0, 0);  // Reset transform
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.set_line_width(2.0);
            ctx.scale(2.0, 2.0);
            ctx.begin_path();
            ctx.move_to(10.0, 10.0);
            ctx.line_to(50.0, 40.0);
            ctx.set_transform(DOMMatrix {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                e: 0.0,
                f: 0.0,
            }); // Reset transform
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test)
        .expect("transform_before_path_reset_before_stroke comparison failed");
}

#[test]
fn test_transform_after_path_before_stroke() {
    // Test: Path constructed first, then transform applied before stroke.
    // Per spec, the path was built with identity transform, so it should
    // NOT be affected by the later scale.
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "transform_after_path_before_stroke",
        width: 200,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#0000ff';
ctx.lineWidth = 2;
ctx.beginPath();
ctx.moveTo(10, 10);
ctx.lineTo(50, 40);
ctx.scale(2, 2);  // Scale AFTER building path
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#0000ff").unwrap();
            ctx.set_line_width(2.0);
            ctx.begin_path();
            ctx.move_to(10.0, 10.0);
            ctx.line_to(50.0, 40.0);
            ctx.scale(2.0, 2.0); // Scale AFTER building path
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("transform_after_path_before_stroke comparison failed");
}

#[test]
fn test_line_width_uses_draw_time_transform() {
    // Test: Line width should be scaled by the transform at DRAW time.
    // Build path with scale(2,2), set lineWidth=5, reset transform, stroke.
    // The line width should be 5 (unscaled) because transform is identity at stroke time.
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "line_width_uses_draw_time_transform",
        width: 200,
        height: 100,
        js_code: r#"
ctx.strokeStyle = '#00ff00';
ctx.scale(2, 2);
ctx.lineWidth = 5;
ctx.beginPath();
ctx.moveTo(10, 10);
ctx.lineTo(50, 40);
ctx.setTransform(1, 0, 0, 1, 0, 0);  // Reset transform
ctx.stroke();  // Line width should be 5 (unscaled)
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#00ff00").unwrap();
            ctx.scale(2.0, 2.0);
            ctx.set_line_width(5.0);
            ctx.begin_path();
            ctx.move_to(10.0, 10.0);
            ctx.line_to(50.0, 40.0);
            ctx.set_transform(DOMMatrix {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                e: 0.0,
                f: 0.0,
            }); // Reset transform
            ctx.stroke(); // Line width should be 5 (unscaled)
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("line_width_uses_draw_time_transform comparison failed");
}

#[test]
fn test_transform_incremental_path_building() {
    // Test: Transform changes between path segments.
    // Each segment should be transformed by the CTM that was active when added.
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "transform_incremental_path_building",
        width: 200,
        height: 200,
        js_code: r#"
ctx.strokeStyle = '#ff00ff';
ctx.lineWidth = 2;
ctx.beginPath();
ctx.moveTo(10, 10);
ctx.lineTo(50, 10);  // Horizontal line at y=10
ctx.scale(1, 2);     // Scale Y by 2
ctx.lineTo(50, 50);  // This point becomes (50, 100)
ctx.lineTo(10, 50);  // This point becomes (10, 100)
ctx.setTransform(1, 0, 0, 1, 0, 0);
ctx.stroke();
"#,
        rust_fn: |ctx| {
            ctx.set_stroke_style("#ff00ff").unwrap();
            ctx.set_line_width(2.0);
            ctx.begin_path();
            ctx.move_to(10.0, 10.0);
            ctx.line_to(50.0, 10.0); // Horizontal line at y=10
            ctx.scale(1.0, 2.0); // Scale Y by 2
            ctx.line_to(50.0, 50.0); // This point becomes (50, 100)
            ctx.line_to(10.0, 50.0); // This point becomes (10, 100)
            ctx.set_transform(DOMMatrix {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                e: 0.0,
                f: 0.0,
            });
            ctx.stroke();
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("transform_incremental_path_building comparison failed");
}

#[test]
fn test_path2d_transform_at_draw_time() {
    // Test: Path2D stores untransformed coordinates.
    // Transform should be applied at draw time.
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "path2d_transform_at_draw_time",
        width: 200,
        height: 100,
        js_code: r#"
// Create path with no transform
let path = new Path2D();
path.moveTo(10, 10);
path.lineTo(50, 40);

// Draw with scale
ctx.strokeStyle = '#ff0000';
ctx.lineWidth = 2;
ctx.scale(2, 2);
ctx.stroke(path);  // Path should be scaled at draw time
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            // Create path with no transform
            let mut path = Path2D::new();
            path.move_to(10.0, 10.0);
            path.line_to(50.0, 40.0);

            // Draw with scale
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.set_line_width(2.0);
            ctx.scale(2.0, 2.0);
            ctx.stroke_path2d(&mut path); // Path should be scaled at draw time
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("path2d_transform_at_draw_time comparison failed");
}

#[test]
fn test_path2d_vs_default_path_same_transform() {
    // Test: Path2D and default path should produce same result
    // when transform is unchanged between construction and drawing.
    skip_if_no_node_canvas!();
    let test = CanvasTestCase {
        name: "path2d_vs_default_path_same_transform",
        width: 200,
        height: 200,
        js_code: r#"
ctx.scale(1.5, 1.5);

// Draw with default path (red)
ctx.strokeStyle = '#ff0000';
ctx.lineWidth = 2;
ctx.beginPath();
ctx.moveTo(10, 10);
ctx.lineTo(60, 10);
ctx.lineTo(60, 60);
ctx.stroke();

// Draw with Path2D (blue, offset)
let path = new Path2D();
path.moveTo(10, 70);
path.lineTo(60, 70);
path.lineTo(60, 120);
ctx.strokeStyle = '#0000ff';
ctx.stroke(path);
"#,
        rust_fn: |ctx| {
            use vl_convert_canvas2d::Path2D;
            ctx.scale(1.5, 1.5);

            // Draw with default path (red)
            ctx.set_stroke_style("#ff0000").unwrap();
            ctx.set_line_width(2.0);
            ctx.begin_path();
            ctx.move_to(10.0, 10.0);
            ctx.line_to(60.0, 10.0);
            ctx.line_to(60.0, 60.0);
            ctx.stroke();

            // Draw with Path2D (blue, offset)
            let mut path = Path2D::new();
            path.move_to(10.0, 70.0);
            path.line_to(60.0, 70.0);
            path.line_to(60.0, 120.0);
            ctx.set_stroke_style("#0000ff").unwrap();
            ctx.stroke_path2d(&mut path);
        },
        threshold: DEFAULT_THRESHOLD,
        max_diff_percent: MAX_DIFF_PERCENT,
    };
    run_comparison_test(&test).expect("path2d_vs_default_path_same_transform comparison failed");
}

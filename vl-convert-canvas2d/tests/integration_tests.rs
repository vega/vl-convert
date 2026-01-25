//! Integration tests for vl-convert-canvas2d.

use vl_convert_canvas2d::{Canvas2dContext, Canvas2dContextBuilder};

/// Test creating a canvas and drawing basic shapes.
#[test]
fn test_draw_rectangle() {
    let mut ctx = Canvas2dContext::new(200, 200).unwrap();

    // Set fill style and draw rectangle
    ctx.set_fill_style("#ff0000").unwrap();
    ctx.fill_rect(10.0, 10.0, 100.0, 100.0);

    // Verify the pixmap has non-transparent pixels
    let data = ctx.get_image_data(0, 0, 200, 200);
    assert!(!data.is_empty());

    // Check that the rectangle area contains red pixels
    // At position (50, 50) which should be inside the rectangle
    let idx = (50 * 200 + 50) * 4;
    assert_eq!(data[idx], 255); // R
    assert_eq!(data[idx + 1], 0); // G
    assert_eq!(data[idx + 2], 0); // B
    assert_eq!(data[idx + 3], 255); // A
}

/// Test path operations.
#[test]
fn test_path_operations() {
    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    ctx.set_fill_style("#00ff00").unwrap();
    ctx.begin_path();
    ctx.move_to(10.0, 10.0);
    ctx.line_to(90.0, 10.0);
    ctx.line_to(90.0, 90.0);
    ctx.line_to(10.0, 90.0);
    ctx.close_path();
    ctx.fill();

    // Check that the path was filled
    let data = ctx.get_image_data(0, 0, 100, 100);
    let idx = (50 * 100 + 50) * 4;
    assert_eq!(data[idx], 0); // R
    assert_eq!(data[idx + 1], 255); // G
    assert_eq!(data[idx + 2], 0); // B
}

/// Test stroke operations.
#[test]
fn test_stroke_operations() {
    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    ctx.set_stroke_style("#0000ff").unwrap();
    ctx.set_line_width(2.0);
    ctx.begin_path();
    ctx.move_to(10.0, 50.0);
    ctx.line_to(90.0, 50.0);
    ctx.stroke();

    // Check that the line was drawn (somewhere along y=50)
    let data = ctx.get_image_data(0, 0, 100, 100);
    let idx = (50 * 100 + 50) * 4;
    assert_eq!(data[idx], 0); // R
    assert_eq!(data[idx + 1], 0); // G
    assert_eq!(data[idx + 2], 255); // B
}

/// Test save/restore state.
#[test]
fn test_save_restore_state() {
    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    // Set initial state
    ctx.set_fill_style("#ff0000").unwrap();
    ctx.set_line_width(5.0);

    // Save state
    ctx.save();

    // Modify state
    ctx.set_fill_style("#00ff00").unwrap();
    ctx.set_line_width(10.0);

    // Draw with modified state
    ctx.fill_rect(0.0, 0.0, 50.0, 50.0);

    // Restore state
    ctx.restore();

    // Draw with original state
    ctx.fill_rect(50.0, 50.0, 50.0, 50.0);

    // Check colors - top-left should be green, bottom-right should be red
    let data = ctx.get_image_data(0, 0, 100, 100);

    // Check green area (25, 25)
    let idx_green = (25 * 100 + 25) * 4;
    assert_eq!(data[idx_green], 0); // R
    assert_eq!(data[idx_green + 1], 255); // G (green was used here)

    // Check red area (75, 75)
    let idx_red = (75 * 100 + 75) * 4;
    assert_eq!(data[idx_red], 255); // R (red was restored)
    assert_eq!(data[idx_red + 1], 0); // G
}

/// Test transforms.
#[test]
fn test_transforms() {
    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    // Translate and draw
    ctx.translate(50.0, 50.0);
    ctx.set_fill_style("#ff0000").unwrap();
    ctx.fill_rect(-10.0, -10.0, 20.0, 20.0);

    // Check that the rectangle is at the center
    let data = ctx.get_image_data(0, 0, 100, 100);
    let idx = (50 * 100 + 50) * 4;
    assert_eq!(data[idx], 255); // R
    assert_eq!(data[idx + 3], 255); // A (non-transparent)
}

/// Test clearRect.
#[test]
fn test_clear_rect() {
    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    // Fill entire canvas
    ctx.set_fill_style("#ff0000").unwrap();
    ctx.fill_rect(0.0, 0.0, 100.0, 100.0);

    // Clear center
    ctx.clear_rect(25.0, 25.0, 50.0, 50.0);

    // Check that center is cleared
    let data = ctx.get_image_data(0, 0, 100, 100);
    let idx_center = (50 * 100 + 50) * 4;
    assert_eq!(data[idx_center + 3], 0); // A should be 0 (transparent)

    // Check that corner is still filled
    let idx_corner = (10 * 100 + 10) * 4;
    assert_eq!(data[idx_corner], 255); // R
    assert_eq!(data[idx_corner + 3], 255); // A
}

/// Test PNG export.
#[test]
fn test_png_export() {
    let mut ctx = Canvas2dContext::new(50, 50).unwrap();

    ctx.set_fill_style("#0000ff").unwrap();
    ctx.fill_rect(0.0, 0.0, 50.0, 50.0);

    let png_data = ctx.to_png().unwrap();

    // Check PNG header
    assert_eq!(&png_data[0..8], b"\x89PNG\r\n\x1a\n");
    assert!(!png_data.is_empty());
}

/// Test text measurement.
#[test]
fn test_measure_text() {
    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    ctx.set_font("12px sans-serif").unwrap();
    let metrics = ctx.measure_text("Hello").unwrap();

    // Width should be positive for non-empty text
    assert!(metrics.width > 0.0);
}

/// Test arc drawing.
#[test]
fn test_arc() {
    use std::f32::consts::PI;

    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    ctx.set_fill_style("#ff00ff").unwrap();
    ctx.begin_path();
    ctx.arc(50.0, 50.0, 30.0, 0.0, 2.0 * PI, false);
    ctx.fill();

    // Check that the center has the fill color
    let data = ctx.get_image_data(0, 0, 100, 100);
    let idx = (50 * 100 + 50) * 4;
    assert_eq!(data[idx], 255); // R
    assert_eq!(data[idx + 1], 0); // G
    assert_eq!(data[idx + 2], 255); // B
}

/// Test bezier curves.
#[test]
fn test_bezier_curve() {
    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    ctx.set_stroke_style("#000000").unwrap();
    ctx.set_line_width(2.0);
    ctx.begin_path();
    ctx.move_to(10.0, 50.0);
    ctx.bezier_curve_to(30.0, 10.0, 70.0, 90.0, 90.0, 50.0);
    ctx.stroke();

    // Just verify it doesn't crash and produces some output
    let data = ctx.get_image_data(0, 0, 100, 100);
    let has_black = data
        .chunks(4)
        .any(|pixel| pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 && pixel[3] > 0);
    assert!(has_black);
}

/// Test quadratic curves.
#[test]
fn test_quadratic_curve() {
    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    ctx.set_stroke_style("#ff0000").unwrap();
    ctx.set_line_width(2.0);
    ctx.begin_path();
    ctx.move_to(10.0, 50.0);
    ctx.quadratic_curve_to(50.0, 10.0, 90.0, 50.0);
    ctx.stroke();

    // Just verify it doesn't crash and produces some output
    let data = ctx.get_image_data(0, 0, 100, 100);
    let has_red = data.chunks(4).any(|pixel| pixel[0] > 200 && pixel[3] > 0);
    assert!(has_red);
}

/// Test builder pattern with font database.
#[test]
fn test_builder_pattern() {
    let db = fontdb::Database::new();
    let ctx = Canvas2dContextBuilder::new(100, 100)
        .with_font_db(db)
        .build()
        .unwrap();

    assert_eq!(ctx.width(), 100);
    assert_eq!(ctx.height(), 100);
}

/// Test linear gradient.
#[test]
fn test_linear_gradient() {
    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    let mut gradient = ctx.create_linear_gradient(0.0, 0.0, 100.0, 0.0);
    gradient.add_color_stop(0.0, tiny_skia::Color::from_rgba8(255, 0, 0, 255));
    gradient.add_color_stop(1.0, tiny_skia::Color::from_rgba8(0, 0, 255, 255));

    ctx.set_fill_style_gradient(gradient);
    ctx.fill_rect(0.0, 0.0, 100.0, 100.0);

    let data = ctx.get_image_data(0, 0, 100, 100);

    // Left edge should be more red
    let idx_left = (50 * 100 + 5) * 4;
    assert!(data[idx_left] > 200); // R should be high

    // Right edge should be more blue
    let idx_right = (50 * 100 + 95) * 4;
    assert!(data[idx_right + 2] > 200); // B should be high
}

/// Test global alpha.
#[test]
fn test_global_alpha() {
    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    ctx.set_global_alpha(0.5);
    ctx.set_fill_style("#ff0000").unwrap();
    ctx.fill_rect(0.0, 0.0, 100.0, 100.0);

    let data = ctx.get_image_data(0, 0, 100, 100);
    let idx = (50 * 100 + 50) * 4;

    // Alpha should be approximately 127-128 (half of 255)
    assert!(data[idx + 3] > 100 && data[idx + 3] < 160);
}

/// Test line dash.
#[test]
fn test_line_dash() {
    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    ctx.set_stroke_style("#000000").unwrap();
    ctx.set_line_width(2.0);
    ctx.set_line_dash(vec![5.0, 5.0]);

    ctx.begin_path();
    ctx.move_to(10.0, 50.0);
    ctx.line_to(90.0, 50.0);
    ctx.stroke();

    // Check that we got the dash pattern
    let dash = ctx.get_line_dash();
    assert_eq!(dash, &[5.0, 5.0]);
}

/// Test ellipse.
#[test]
fn test_ellipse() {
    use std::f32::consts::PI;

    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    ctx.set_fill_style("#00ff00").unwrap();
    ctx.begin_path();
    ctx.ellipse(50.0, 50.0, 40.0, 20.0, 0.0, 0.0, 2.0 * PI, false);
    ctx.fill();

    // Check center is filled
    let data = ctx.get_image_data(0, 0, 100, 100);
    let idx = (50 * 100 + 50) * 4;
    assert_eq!(data[idx + 1], 255); // G
}

/// Test post_script_name fallback lookup for custom fonts like "Matter SemiBold".
/// When the font family name doesn't directly match (e.g., "Matter SemiBold" != "Matter"),
/// we should fall back to searching by post_script_name (e.g., "Matter-SemiBold").
#[test]
fn test_postscript_name_fallback() {
    // Load the Matter font from the test fonts directory
    let mut db = fontdb::Database::new();
    let matter_font_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("vl-convert-rs/tests/fonts/matter/Matter-SemiBold.ttf");

    if !matter_font_path.exists() {
        println!("Skipping test - Matter font not found at {:?}", matter_font_path);
        return;
    }

    db.load_font_file(&matter_font_path).expect("Failed to load Matter font");

    // Debug: Print all faces and their properties
    println!("\nFaces in fontdb after loading Matter font:");
    for face in db.faces() {
        println!("  - family: {:?}", face.families);
        println!("    post_script_name: {}", face.post_script_name);
        println!("    weight: {:?}, style: {:?}", face.weight, face.style);
    }

    // Create canvas with the font database
    let mut ctx = Canvas2dContextBuilder::new(100, 100)
        .with_font_db(db)
        .build()
        .unwrap();

    // Try setting font as "Matter SemiBold" (CSS-style name)
    // This should find the font via post_script_name lookup
    ctx.set_font("12px Matter SemiBold").unwrap();

    let metrics = ctx.measure_text("Test").unwrap();
    println!("Measured text 'Test' with 'Matter SemiBold': width = {}", metrics.width);

    // Also try with the actual family name + weight
    ctx.set_font("600 12px Matter").unwrap();
    let metrics2 = ctx.measure_text("Test").unwrap();
    println!("Measured text 'Test' with 'Matter' (weight 600): width = {}", metrics2.width);

    // The width should be positive and reasonable
    assert!(metrics.width > 0.0, "Width should be positive");
}

/// Test text measurement comparison with node-canvas.
/// node-canvas measurements (using system fonts on macOS):
///   "11px sans-serif" + "Count of Records" = 85.60546875
///   "bold 11px sans-serif" + "Count of Records" = 92.2822265625 (Vega legend title)
///   "11px sans-serif" + "IMDB Rating (binned)" = 105.775390625
///   "11px sans-serif" + "Rotten Tomatoes Rating (binned)" = 161.2431640625
#[test]
fn test_measure_text_vs_node_canvas() {
    let mut ctx = Canvas2dContext::new(100, 100).unwrap();

    // Test cases: (font, text, node_canvas_width)
    // Note: Vega uses "bold 11px sans-serif" for legend titles like "Count of Records"
    let test_cases = [
        ("11px sans-serif", "Count of Records", 85.60546875),
        ("bold 11px sans-serif", "Count of Records", 92.2822265625), // Vega legend title
        ("11px Arial", "Count of Records", 85.60546875),
        ("bold 11px Arial", "Count of Records", 92.2822265625),
        ("11px Helvetica", "Count of Records", 85.60546875),
        ("bold 11px Helvetica", "Count of Records", 92.2822265625),
        ("11px sans-serif", "IMDB Rating (binned)", 105.775390625),
        ("11px sans-serif", "Rotten Tomatoes Rating (binned)", 161.2431640625),
        ("10px sans-serif", "Count of Records", 77.8173828125),
        ("12px sans-serif", "Count of Records", 93.380859375),
    ];

    println!("\nText measurement comparison (cosmic_text vs node-canvas):");
    println!("=========================================================");

    for (font, text, node_canvas_width) in test_cases {
        ctx.set_font(font).unwrap();
        let metrics = ctx.measure_text(text).unwrap();
        let diff = metrics.width as f64 - node_canvas_width;
        let diff_pct = (diff / node_canvas_width) * 100.0;

        println!("font: \"{}\"", font);
        println!("  text: \"{}\"", text);
        println!("  cosmic_text: {:.4}", metrics.width);
        println!("  node-canvas: {:.4}", node_canvas_width);
        println!("  diff: {:.4} ({:.2}%)", diff, diff_pct);
        println!();
    }

    // For now, just ensure measurements are positive and in a reasonable range
    // The actual values will differ between cosmic_text and node-canvas
    ctx.set_font("11px sans-serif").unwrap();
    let metrics = ctx.measure_text("Count of Records").unwrap();
    assert!(metrics.width > 70.0, "Width should be > 70");
    assert!(metrics.width < 120.0, "Width should be < 120");
}

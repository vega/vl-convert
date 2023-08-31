use std::collections::HashSet;
use resvg::tiny_skia::Pixmap;
use std::fs;
use anyhow::bail;
use usvg::{fontdb, TreeParsing, TreeTextToPath};
use vl_convert_pdf::{FontMetricFonts, FontMetrics, get_text_width_height};

fn main() -> Result<(), anyhow::Error> {
    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();
    // Collect set of system font families
    let families: HashSet<String> = font_db
        .faces()
        .flat_map(|face| {
            face.families
                .iter()
                .map(|(fam, _lang)| fam.clone())
                .collect::<Vec<_>>()
        })
        .collect();

    let font_size = 12.0;
    let texts: Vec<_> = vec![
        "the quick",
        "brown",
        "fox",
        "jumped?",
        "over!",
        "the",
        "lazy",
        "dog",
        "-123",
        "+456",
        "—7890",
    ].into_iter().map(|s| s.to_string()).collect();

    let mut font_metric_fonts: Vec<FontMetricFonts> = Vec::new();
    for font_family in ["Helvetica", "Courier", "Times"] {
        if !families.contains(font_family) {
            bail!("{font_family} not found on system. Please install this font");
        }
        for font_weight in ["regular", "bold"] {
            if font_family == "Courier" && font_weight == "bold" {
                // Courier bold is the same width as Courier
                continue;
            }
            let font_name = format!(
                "{font_family}{}",
                match font_weight {
                    "bold" => "-Bold",
                    "regular" if font_family == "Times" => "-Roman",
                    _ => ""
                }
            );
            let mut widths: Vec<f64> = Vec::new();
            let mut heights: Vec<f64> = Vec::new();

            for text in &texts {
                let text_attrs = vec![
                    format!("font-size=\"{font_size}\""),
                    format!("font-family=\"{font_family}\""),
                    format!("font-weight=\"{font_weight}\""),
                ];
                let text_attrs_str = text_attrs.join(" ");

                let svg_width = 100;
                let svg_height = 100;
                let svg = format!(
                    r#"
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.1" width="{svg_width}" height="{svg_height}">
<text x="20" y="50" {text_attrs_str}>{text}</text>
</svg>"#
                );

                let mut tree = usvg::Tree::from_str(&svg, &Default::default())
                    .expect("Failed to parse text SVG");
                tree.convert_text(&font_db);

                let (width, height) = get_text_width_height(tree.root.clone())
                    .expect("Failed to extract text width and height");

                widths.push(width);
                heights.push(height);

                // println!("{font_name} - text: {text}, width: {width}, height: {height}");
                let rtree = resvg::Tree::from_usvg(&tree);
                let mut pixmap = Pixmap::new(svg_width as u32, svg_height as u32).unwrap();
                resvg::Tree::render(&rtree, Default::default(), &mut pixmap.as_mut());
                let png_bytes = pixmap.encode_png().expect("Failed to encode png");
                fs::write(
                    &format!("vl-convert-pdf/src/bin/images/{text}-{font_name}.png"),
                    png_bytes,
                )
                .expect("Failed to write PNG");
            }
            font_metric_fonts.push(FontMetricFonts {
                widths,
                heights,
                font_name,
            })
        }
    }

    let font_metrics = FontMetrics {
        texts,
        font_size,
        fonts: font_metric_fonts,
    };
    let json_str = serde_json::to_string_pretty(&font_metrics).expect("Failed to serialize metrics to JSON");
    println!("{json_str}");
    fs::write("vl-convert-pdf/font_metrics/metrics.json", json_str).expect("Failed to write font metrics");
    Ok(())
}

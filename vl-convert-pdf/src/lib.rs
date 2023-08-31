extern crate core;

use std::collections::{HashMap, HashSet};
use anyhow::{bail, Error as AnyError};
use pdf_writer::{Content, Finish, Name, PdfWriter, Rect, Ref, Str};
use serde::{Deserialize, Serialize};
use usvg::fontdb::Database;
use usvg::{Font, FontStretch, FontStyle, Node, NodeExt, NodeKind, Opacity, TextAnchor, TextToPath, Tree, TreeParsing, TreeTextToPath};
use serde_json;
use lazy_static::lazy_static;

const METRICS_JSON_STR: &str = include_str!("../font_metrics/metrics.json");

lazy_static!{
    static ref METRICS_JSON: FontMetrics = serde_json::from_str(METRICS_JSON_STR).expect("Failed to parse metrics.json");
}

pub fn svg_to_pdf(
    svg: &str,
    font_db: &Database,
    usvg_opts: &usvg::Options,
) -> Result<Vec<u8>, AnyError> {
    let mut converted_tree = usvg::Tree::from_str(&svg, usvg_opts)?;
    converted_tree.convert_text(&font_db);

    // Parse SVG again so that we have a copy that's not converted
    let unconverted_tree = usvg::Tree::from_str(&svg, usvg_opts)?;
    let fonts = collect_fonts(&unconverted_tree);

    // Extract SVGs size. We'll use this as the size of the resulting PDF docuemnt
    let width = converted_tree.size.width();
    let height = converted_tree.size.height();

    // Allocate the indirect reference IDs
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let page_id = Ref::new(3);
    let content_id = Ref::new(4);
    let ext_graphics_id = Ref::new(5);

    // Compute font pairs
    let next_ref = 6;
    let (font_mapping, next_ref) = compute_font_mapping(&fonts, &font_db, next_ref)?;
    let svg_id = Ref::new(next_ref);

    // Define names.
    let svg_name = Name(b"S1");
    let ext_graphics_name = Name(b"G1");

    // Start writing a PDF.
    let mut writer = PdfWriter::new();
    writer.catalog(catalog_id).pages(page_tree_id);
    writer.pages(page_tree_id).kids([page_id]).count(1);

    // Initialize page with size matching the SVG image
    let mut page = writer.page(page_id);
    page.media_box(Rect::new(0.0, 0.0, width as f32, height as f32));
    page.parent(page_tree_id);
    page.contents(content_id);

    // Setup the page's resources so these can be referenced in the page's content stream
    //      - The SVG XObject
    //      - The font(s)
    //      - The external graphics configuration used for overlay text
    let mut resources = page.resources();
    resources.x_objects().pair(svg_name, svg_id);
    {
        let mut resource_fonts = resources.fonts();
        for mapped_font in font_mapping.values() {
            resource_fonts.pair(
                Name(mapped_font.font_ref_name.as_slice()),
                mapped_font.font_ref
            );
        }
    }
    resources
        .ext_g_states()
        .pair(ext_graphics_name, ext_graphics_id);
    resources.finish();

    // Finish page configuration
    page.finish();

    // Write resources to the file with the writer
    // ## Xobject
    // This call allocates some indirect object reference IDs for itself. If we
    // wanted to write some more indirect objects afterwards, we could use the
    // return value as the next unused reference ID.
    svg2pdf::convert_tree_into(
        &converted_tree,
        svg2pdf::Options::default(),
        &mut writer,
        svg_id,
    );

    // ## Font
    // Set a predefined font, so we do not have to load anything extra.
    for mapped_font in font_mapping.values() {
        writer.type1_font(mapped_font.font_ref).base_font(Name(mapped_font.font_name.as_bytes()));
    }

    // ## External Graphics
    // Make extended graphic to set text to be transparent
    // (or semi-transparent for testing/debugging)
    writer.ext_graphics(ext_graphics_id).non_stroking_alpha(0.3);

    // Create a content stream with the SVG and overlay text
    let mut content = Content::new();

    // Add reference to the SVG XObject
    // It's re-scaled to the size of the document because convert_tree_into above
    // scales it to 1.0 x 1.0
    content
        .save_state()
        .transform([width as f32, 0.0, 0.0, height as f32, 0.0, 0.0])
        .x_object(svg_name)
        .restore_state();

    // Add Overlay Text
    content.save_state().set_parameters(ext_graphics_name);

    for node in unconverted_tree.root.children() {
        overlay_text(node, &mut content, &font_db, height as f32, &font_mapping)?;
    }

    content.restore_state();

    // Write the content stream
    writer.stream(content_id, &content.finish());

    // Generate the final PDF file's contents
    Ok(writer.finish())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontMetrics {
    pub texts: Vec<String>,
    pub font_size: f64,
    pub fonts: Vec<FontMetricFonts>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontMetricFonts {
    pub widths: Vec<f64>,
    pub heights: Vec<f64>,
    pub font_name: String,
}

#[derive(Debug, Clone)]
pub struct MappedFont {
    pub font_name: String,
    pub font_ref: Ref,
    pub font_ref_name: Vec<u8>,
    pub scale_factor: f64,
}

fn overlay_text(
    node: Node,
    content: &mut Content,
    font_db: &Database,
    height: f32,
    font_mapping: &HashMap<Font, MappedFont>,
) -> Result<(), AnyError> {
    // let font_name = Name(b"F1");
    match *node.borrow() {
        NodeKind::Text(ref text) if text.chunks.len() == 1 && text.chunks[0].spans.len() == 1 => {
            // For now, only overlay text with one chunk and one span.
            let Some(text_path_node) = text.convert(font_db, Default::default()) else {
                bail!("Failed to calculate text bounding box")
            };

            let Some((text_width, _)) = get_text_width_height(text_path_node) else {
                bail!("Failed to get text width from converted paths")
            };

            let chunk = &text.chunks[0];
            let span = chunk.spans[0].clone();
            let font_size = span.font_size.get() as f32;

            // Skip zero opacity text
            let span_opacity = span.fill.unwrap_or_default().opacity;
            if span_opacity == Opacity::ZERO || node_has_zero_opacity(&node) {
                return Ok(());
            }

            let Some(base_font) = font_mapping.get(&span.font) else { bail!("Mapped font not found") };
            let scaled_font_size = font_size / (base_font.scale_factor as f32);

            let x_offset = match chunk.anchor {
                TextAnchor::Start => 0.0,
                TextAnchor::Middle => -text_width / 2.0,
                TextAnchor::End => -text_width,
            };

            let tx = node.abs_transform();

            // Compute chunk x/y
            let chunk_x = chunk.x.unwrap_or(0.0) + x_offset as f32;
            let chunk_y = chunk.y.unwrap_or(0.0);

            content.save_state().transform([
                tx.sx as f32,
                tx.kx as f32,
                tx.ky as f32,
                tx.sy as f32,
                tx.tx as f32,
                height - tx.ty as f32,
            ]);

            content
                .begin_text()
                .set_font(Name(base_font.font_ref_name.as_slice()), scaled_font_size)
                .set_fill_rgb(0.0, 1.0, 1.0)
                .next_line(chunk_x as f32, chunk_y as f32)
                .show(Str(chunk.text.as_bytes()))
                .end_text()
                .restore_state();
        }
        NodeKind::Group(_) => {
            for child in node.children() {
                overlay_text(child, content, font_db, height, font_mapping)?;
            }
        }
        _ => {}
    }
    Ok(())
}

// Check if this node is a group node with zero opacity,
// or if it has an ancestor group node with zero opacity
fn node_has_zero_opacity(node: &Node) -> bool {
    if let NodeKind::Group(ref group) = *node.borrow() {
        if group.opacity == Opacity::ZERO {
            return true;
        }
    }
    if let Some(parent) = &node.parent() {
        node_has_zero_opacity(parent)
    } else {
        false
    }
}

/// TODO, unify with text module
pub fn get_text_width_height(node: Node) -> Option<(f64, f64)> {
    let bbox = node.calculate_bbox()?;
    match *node.borrow() {
        NodeKind::Group(_) => {
            for child in node.children() {
                if let Some(res) = get_text_width_height(child) {
                    return Some(res);
                }
            }
            None
        }
        NodeKind::Path(ref path) => {
            // Use text_box width and bounding box height
            return path
                .text_bbox
                .map(|p| (p.width() as f64, bbox.height() as f64));
        }
        NodeKind::Image(_) => None,
        NodeKind::Text(_) => None,
    }
}

pub fn collect_fonts(tree: &Tree) -> HashSet<Font> {
    let mut fonts: HashSet<Font> = HashSet::new();
    for node in tree.root.descendants() {
        match *node.borrow() {
            NodeKind::Text(ref text) if text.chunks.len() == 1 && text.chunks[0].spans.len() == 1  => {
                fonts.insert(text.chunks[0].spans[0].font.clone());
            }
            _ => {}
        }
    }
    fonts
}

pub fn compute_font_mapping(fonts: &HashSet<Font>, font_db: &Database, next_ref: i32) -> Result<(HashMap<Font, MappedFont>, i32), anyhow::Error> {
    let metrics = METRICS_JSON.clone();
    let mut next_ref = next_ref;
    let mut mapping: HashMap<Font, MappedFont> = Default::default();
    for font in fonts.iter() {
        // Compute widths/heights for reference text strings
        let mut widths = Vec::new();
        let mut heights = Vec::new();
        for text in &metrics.texts {
            let svg_for_text = svg_for_font(text, metrics.font_size, font);
            let mut tree = Tree::from_str(&svg_for_text, &Default::default())?;
            tree.convert_text(&font_db);
            let Some((width, height)) = get_text_width_height(tree.root ) else {
                bail!("Failed to locate text in svg node");
            };
            widths.push(width);
            heights.push(height)
        }
        let total_width: f64 = widths.iter().sum();

        // Find closest base font
        let mut min_residual = f64::MAX;
        let mut min_font_name = "".to_string();
        let mut min_total_width = 0.0;
        for base_font in &metrics.fonts {
            let residual: f64 = (0..widths.len()).map(|i| {
                ((widths[i] - base_font.widths[i]).powi(2) + (heights[i] - base_font.heights[i]).powi(2)).sqrt()
            }).sum();

            if residual < min_residual {
                min_residual = residual;
                min_font_name = base_font.font_name.clone();
                min_total_width = base_font.widths.iter().sum();
            }
        }

        // Compute font size scale factor for closes base font
        let scale_factor = min_total_width / total_width;

        // Update mapping
        mapping.insert(font.clone(), MappedFont {
            font_name: min_font_name.clone(),
            font_ref: Ref::new(next_ref),
            font_ref_name: min_font_name.replace(" ", "").into_bytes(),
            scale_factor
        });
        next_ref += 1;
    }

    Ok((mapping, next_ref))
}

pub fn svg_for_font(text: &str, font_size: f64, font: &Font) -> String {
    let text_attrs = vec![
        format!("font-size=\"{}\"", font_size),
        format!("font-family=\"{}\"", font.families.join(" ")),
        format!("font-weight=\"{}\"", font.weight),
        format!("font-stretch=\"{}\"", match font.stretch {
            FontStretch::UltraCondensed => "ultra-Condensed",
            FontStretch::ExtraCondensed => "extra-condensed",
            FontStretch::Condensed => "condensed",
            FontStretch::SemiCondensed => "semi-condensed",
            FontStretch::Normal => "normal",
            FontStretch::SemiExpanded => "semi-expanded",
            FontStretch::Expanded => "expanded",
            FontStretch::ExtraExpanded => "extra-expanded",
            FontStretch::UltraExpanded => "ultra-expanded",
        }),
        format!("font-style=\"{}\"", match font.style {
            FontStyle::Normal => "normal",
            FontStyle::Italic => "italic",
            FontStyle::Oblique => "oblique",
        })
    ];
    let text_attrs_str = text_attrs.join(" ");

    let svg_width = 200;
    let svg_height = 200;
    format!(
        r#"
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.1" width="{svg_width}" height="{svg_height}">
<text x="20" y="50" {text_attrs_str}>{text}</text>
</svg>"#
    )
}
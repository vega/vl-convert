pub mod encoding;

use crate::encoding::WIN_ANSI_ENCODING;
use anyhow::{bail, Error as AnyError};
use lazy_static::lazy_static;
use pdf_writer::{Content, Finish, Name, PdfWriter, Rect, Ref, Str};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::{HashMap, HashSet};
use usvg::fontdb::Database;
use usvg::{
    Font, FontStretch, FontStyle, Node, NodeExt, NodeKind, Opacity, TextAnchor, TextToPath, Tree,
    TreeParsing, TreeTextToPath,
};

const METRICS_JSON_STR: &str = include_str!("../font_metrics/metrics.json");

lazy_static! {
    static ref METRICS_JSON: FontMetrics =
        serde_json::from_str(METRICS_JSON_STR).expect("Failed to parse metrics.json");
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

    let mut ctx = PdfContext::new(width, height);
    let font_mapping = compute_font_mapping(&mut ctx, &fonts, &font_db)?;
    // Need to update svg_id to be last id before calling svg2pdf because it will allocate more ids
    ctx.svg_id = ctx.alloc.bump();
    construct_page(&mut ctx, &font_mapping);
    write_svg(&mut ctx, &converted_tree);
    write_fonts(&mut ctx, &font_mapping)?;
    write_ext_graphics(&mut ctx);
    write_content(&mut ctx, &unconverted_tree, &font_mapping, &font_db)?;
    Ok(ctx.writer.finish())
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

struct PdfContext {
    writer: PdfWriter,
    width: f32,
    height: f32,
    alloc: Ref,
    catalog_id: Ref,
    page_tree_id: Ref,
    page_id: Ref,
    content_id: Ref,
    svg_id: Ref,
    svg_name: Vec<u8>,
    ext_graphics_id: Ref,
    ext_graphics_name: Vec<u8>,
}

/// Additional methods for [`Ref`].
trait RefExt {
    /// Bump the reference up by one and return the previous one.
    fn bump(&mut self) -> Self;
}

impl RefExt for Ref {
    fn bump(&mut self) -> Self {
        let prev = *self;
        *self = Self::new(prev.get() + 1);
        prev
    }
}

impl PdfContext {
    fn new(width: f32, height: f32) -> Self {
        let mut alloc = Ref::new(1);
        let catalog_id = alloc.bump();
        let page_tree_id = alloc.bump();
        let page_id = alloc.bump();
        let content_id = alloc.bump();
        let ext_graphics_id = alloc.bump();

        // svg_id will be replaced later because it must be the last id before calling svg2pdf
        let svg_id = Ref::new(1);

        Self {
            writer: PdfWriter::new(),
            width,
            height,
            alloc,
            catalog_id,
            page_tree_id,
            page_id,
            content_id,
            svg_id,
            svg_name: Vec::from(b"S1".as_slice()),
            ext_graphics_id,
            ext_graphics_name: Vec::from(b"G1".as_slice()),
        }
    }
}

fn construct_page(ctx: &mut PdfContext, font_mapping: &HashMap<Font, MappedFont>) {
    ctx.writer.catalog(ctx.catalog_id).pages(ctx.page_tree_id);
    ctx.writer
        .pages(ctx.page_tree_id)
        .kids([ctx.page_id])
        .count(1);

    // Initialize page with size matching the SVG image
    let mut page = ctx.writer.page(ctx.page_id);
    page.media_box(Rect::new(0.0, 0.0, ctx.width, ctx.height));
    page.parent(ctx.page_tree_id);
    page.contents(ctx.content_id);

    let mut resources = page.resources();
    // SVG
    resources
        .x_objects()
        .pair(Name(ctx.svg_name.as_slice()), ctx.svg_id);

    // Fonts
    let mut resource_fonts = resources.fonts();
    for mapped_font in font_mapping.values() {
        resource_fonts.pair(
            Name(mapped_font.font_ref_name.as_slice()),
            mapped_font.font_ref,
        );
    }
    resource_fonts.finish();

    // Ext Graphics
    resources
        .ext_g_states()
        .pair(Name(ctx.ext_graphics_name.as_slice()), ctx.ext_graphics_id);

    resources.finish();

    // Finish page configuration
    page.finish();
}

fn write_svg(ctx: &mut PdfContext, tree: &Tree) {
    ctx.alloc = svg2pdf::convert_tree_into(
        &tree,
        svg2pdf::Options::default(),
        &mut ctx.writer,
        ctx.svg_id,
    );
}

fn write_fonts(
    ctx: &mut PdfContext,
    font_mapping: &HashMap<Font, MappedFont>,
) -> Result<(), AnyError> {
    // ## Font
    // Set a predefined font, so we do not have to load anything extra.
    for mapped_font in font_mapping.values() {
        ctx.writer
            .type1_font(mapped_font.font_ref)
            .base_font(Name(mapped_font.font_name.as_bytes()))
            .encoding_predefined(Name(encoding::WIN_ANSI_ENCODING.get_name().as_bytes()));
    }
    Ok(())
}

fn write_ext_graphics(ctx: &mut PdfContext) {
    ctx.writer
        .ext_graphics(ctx.ext_graphics_id)
        .non_stroking_alpha(0.3);
}

fn write_content(
    ctx: &mut PdfContext,
    unconverted_tree: &Tree,
    font_mapping: &HashMap<Font, MappedFont>,
    font_db: &Database,
) -> Result<(), AnyError> {
    // Create a content stream with the SVG and overlay text
    let mut content = Content::new();

    // Add reference to the SVG XObject
    // It's re-scaled to the size of the document because convert_tree_into above
    // scales it to 1.0 x 1.0
    content
        .save_state()
        .transform([ctx.width, 0.0, 0.0, ctx.height, 0.0, 0.0])
        .x_object(Name(ctx.svg_name.as_slice()))
        .restore_state();

    // Add Overlay Text
    content
        .save_state()
        .set_parameters(Name(ctx.ext_graphics_name.as_slice()));

    for node in unconverted_tree.root.children() {
        overlay_text(node, &mut content, &font_db, ctx.height, &font_mapping)?;
    }

    content.restore_state();

    // Write the content stream
    ctx.writer.stream(ctx.content_id, &content.finish());
    Ok(())
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

            // Encode text in the Windows-1252 format that we told the PDF we're using
            let encoded_text = WIN_ANSI_ENCODING.encode_string(&chunk.text);
            content
                .begin_text()
                .set_font(Name(base_font.font_ref_name.as_slice()), scaled_font_size)
                .set_fill_rgb(0.0, 1.0, 1.0)
                .next_line(chunk_x as f32, chunk_y as f32)
                .show(Str(encoded_text.as_slice()))
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
            NodeKind::Text(ref text)
                if text.chunks.len() == 1 && text.chunks[0].spans.len() == 1 =>
            {
                fonts.insert(text.chunks[0].spans[0].font.clone());
            }
            _ => {}
        }
    }
    fonts
}

fn compute_font_mapping(
    ctx: &mut PdfContext,
    fonts: &HashSet<Font>,
    font_db: &Database,
) -> Result<HashMap<Font, MappedFont>, anyhow::Error> {
    let metrics = METRICS_JSON.clone();
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
            let residual: f64 = (0..widths.len())
                .map(|i| {
                    ((widths[i] - base_font.widths[i]).powi(2)
                        + (heights[i] - base_font.heights[i]).powi(2))
                    .sqrt()
                })
                .sum();

            if residual < min_residual {
                min_residual = residual;
                min_font_name = base_font.font_name.clone();
                min_total_width = base_font.widths.iter().sum();
            }
        }

        // Compute font size scale factor for closes base font
        let scale_factor = min_total_width / total_width;

        // Update mapping
        mapping.insert(
            font.clone(),
            MappedFont {
                font_name: min_font_name.clone(),
                font_ref: ctx.alloc.bump(),
                font_ref_name: min_font_name.replace(" ", "").into_bytes(),
                scale_factor,
            },
        );
    }

    Ok(mapping)
}

pub fn svg_for_font(text: &str, font_size: f64, font: &Font) -> String {
    let text_attrs = vec![
        format!("font-size=\"{}\"", font_size),
        format!("font-family=\"{}\"", font.families.join(" ")),
        format!("font-weight=\"{}\"", font.weight),
        format!(
            "font-stretch=\"{}\"",
            match font.stretch {
                FontStretch::UltraCondensed => "ultra-Condensed",
                FontStretch::ExtraCondensed => "extra-condensed",
                FontStretch::Condensed => "condensed",
                FontStretch::SemiCondensed => "semi-condensed",
                FontStretch::Normal => "normal",
                FontStretch::SemiExpanded => "semi-expanded",
                FontStretch::Expanded => "expanded",
                FontStretch::ExtraExpanded => "extra-expanded",
                FontStretch::UltraExpanded => "ultra-expanded",
            }
        ),
        format!(
            "font-style=\"{}\"",
            match font.style {
                FontStyle::Normal => "normal",
                FontStyle::Italic => "italic",
                FontStyle::Oblique => "oblique",
            }
        ),
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

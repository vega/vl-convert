use pdf_writer::{Content, Finish, Name, PdfWriter, Rect, Ref, Str};
use usvg::fontdb::Database;
use usvg::{Node, NodeExt, NodeKind, Opacity, TextAnchor, TextToPath, TreeParsing, TreeTextToPath};
use anyhow::{bail, Error as AnyError};


pub fn svg_to_pdf(svg: &str, font_db: &Database, usvg_opts: &usvg::Options) -> Result<Vec<u8>, AnyError> {
    let mut converted_tree = usvg::Tree::from_str(&svg, usvg_opts)?;
    converted_tree.convert_text(&font_db);

    // Parse SVG again so that we have a copy that's not converted
    let unconverted_tree = usvg::Tree::from_str(&svg, usvg_opts)?;

    // Extract SVGs size. We'll use this as the size of the resulting PDF docuemnt
    let width = converted_tree.size.width();
    let height = converted_tree.size.height();

    // Allocate the indirect reference IDs
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let page_id = Ref::new(3);
    let font_id = Ref::new(4);
    let content_id = Ref::new(5);
    let ext_graphics_id = Ref::new(6);
    let svg_id = Ref::new(7);

    // Define names.
    let font_name = Name(b"F1");
    let svg_name = Name(b"S1");
    let ext_graphics_name = Name(b"G1");

    // Start writing a PDF.
    let mut writer = PdfWriter::new();
    writer.catalog(catalog_id).pages(page_tree_id);
    writer.pages(page_tree_id).kids([page_id]).count(1);

    // Initialize page with size matching the SVG image
    let mut page = writer.page(page_id);
    page.media_box(Rect::new(
        0.0,
        0.0,
        width as f32,
        height as f32,
    ));
    page.parent(page_tree_id);
    page.contents(content_id);

    // Setup the page's resources so these can be referenced in the page's content stream
    //      - The SVG XObject
    //      - The font(s)
    //      - The external graphics configuration used for overlay text
    let mut resources = page.resources();
    resources.x_objects().pair(svg_name, svg_id);
    resources.fonts().pair(font_name, font_id);
    resources.ext_g_states().pair(ext_graphics_name, ext_graphics_id);
    resources.finish();

    // Finish page configuration
    page.finish();

    // Write resources to the file with the writer
    // ## Xobject
    // This call allocates some indirect object reference IDs for itself. If we
    // wanted to write some more indirect objects afterwards, we could use the
    // return value as the next unused reference ID.
    svg2pdf::convert_tree_into(
        &converted_tree, svg2pdf::Options::default(), &mut writer, svg_id
    );

    // ## Font
    // Set a predefined font, so we do not have to load anything extra.
    writer.type1_font(font_id).base_font(Name(b"Helvetica"));

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
    content
        .save_state()
        .set_parameters(ext_graphics_name);

    for node in unconverted_tree.root.children() {
        overlay_text(node, &mut content, &font_db, height as f32)?;
    }

    content.restore_state();

    // Write the content stream
    writer.stream(content_id, &content.finish());

    // Generate the final PDF file's contents
    Ok(writer.finish())
}

fn overlay_text(node: Node, content: &mut Content, font_db: &Database, height: f32) -> Result<(), AnyError> {
    let font_name = Name(b"F1");
    match *node.borrow() {
        NodeKind::Text(ref text) if text.chunks.len() == 1 && text.chunks[0].spans.len() == 1 => {
            // For now, only overlay text with one chunk and one span.

            // println!("{text:#?}");
            let Some(text_path_node) = text.convert(font_db, Default::default()) else {
                bail!("Failed to calculate text bounding box")
            };

            let Some(text_width) = get_text_width(text_path_node) else {
                bail!("Failed to get text width from converted paths")
            };

            // println!("path bbox width: {}", path_bbox.width());
            // println!("{:?}", text);
            let chunk = &text.chunks[0];
            let span = chunk.spans[0].clone();
            let mut font_size = span.font_size.get() as f32;

            // Skip zero opacity text
            let mut span_opacity = span.fill.unwrap_or_default().opacity;
            if span_opacity == Opacity::ZERO || node_has_zero_opacity(&node) {
                return Ok(())
            }

            let x_offset = match chunk.anchor {
                TextAnchor::Start => 0.0,
                TextAnchor::Middle => -text_width / 2.0,
                TextAnchor::End => -text_width
            };

            let tx = node.abs_transform();
            // println!("Text node abs transform: {:?}", tx);

            // Compute chunk x/y
            let chunk_x = chunk.x.unwrap_or(0.0) + x_offset as f32;
            let chunk_y = chunk.y.unwrap_or(0.0);

            content.save_state()
                .transform([
                    tx.sx as f32,
                    tx.kx as f32,
                    tx.ky as f32,
                    tx.sy as f32,
                    tx.tx as f32,
                    height - tx.ty as f32
                ]);

            content
                .begin_text()
                .set_font(font_name, font_size)
                .set_fill_rgb(0.0, 1.0, 1.0)
                .next_line(chunk_x as f32, chunk_y as f32)
                .show(Str(chunk.text.as_bytes()))
                .end_text()
                .restore_state();
        }
        NodeKind::Group(ref group) => {
            // println!("group transform: {:?}", group.transform);
            for child in node.children() {
                overlay_text(child, content, font_db, height)?;
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
            return true
        }
    }
    if let Some(parent) = &node.parent() {
        node_has_zero_opacity(parent)
    } else {
        false
    }
}

/// TODO, unify with text module
fn get_text_width(node: Node) -> Option<f64> {
    match *node.borrow() {
        NodeKind::Group(ref group) => {
            for child in node.children() {
                if let Some(w) = get_text_width(child) {
                    return Some(w)
                }
            }
            None
        }
        NodeKind::Path(ref path) => {
            return path.text_bbox.map(|p| p.width() as f64)
        }
        NodeKind::Image(_) => None,
        NodeKind::Text(_) => None,
    }
}
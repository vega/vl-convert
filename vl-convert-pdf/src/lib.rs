use anyhow::{bail, Error as AnyError};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

use itertools::Itertools;
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use siphasher::sip128::{Hasher128, SipHasher13};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::hash::Hash;
use ttf_parser::{GlyphId, Tag};
use unicode_bidi::BidiInfo;
use usvg::fontdb::{Database, Family, Query, Source, Stretch, Style, Weight};
use usvg::{
    Font, FontStretch, FontStyle, Node, NodeExt, NodeKind, Opacity, Paint, Text, TextAnchor,
    TextToPath, Tree,
};

const CFF: Tag = Tag::from_bytes(b"CFF ");
const SYSTEM_INFO: SystemInfo = SystemInfo {
    registry: Str(b"Adobe"),
    ordering: Str(b"Identity"),
    supplement: 0,
};
const CMAP_NAME: Name = Name(b"Custom");

/// Convert a usvg::Tree into the bytes for a standalone PDF document
/// This function uses svg2pdf to perform non-text conversion and then overlays embedded
/// text on top.
pub fn svg_to_pdf(tree: &Tree, font_db: &Database, scale: f32) -> Result<Vec<u8>, AnyError> {
    // Extract SVGs size. We'll use this as the size of the resulting PDF document
    let width = tree.size.width();
    let height = tree.size.height();

    let font_chars = collect_font_to_chars_mapping(tree)?;

    let mut ctx = PdfContext::new(width, height, scale);

    // Create mapping from usvg::Font to FontMetrics, which contains the info needed to
    // build the embedded PDF font. Sort by Debug representation of Font for deterministic
    // ordering
    let mut font_metrics = HashMap::new();
    for (font, chars) in font_chars.iter().sorted_by_key(|(f, _)| format!("{f:?}")) {
        font_metrics.insert(
            font.clone(),
            compute_font_metrics(&mut ctx, font, chars, font_db)?,
        );
    }

    // Need to update svg_id to be last id before calling svg2pdf because it will allocate more ids
    ctx.svg_id = ctx.alloc.bump();
    construct_page(&mut ctx, &font_metrics);
    write_svg(&mut ctx, tree);
    write_fonts(&mut ctx, &font_metrics)?;
    write_content(&mut ctx, tree, &font_metrics, font_db)?;
    Ok(ctx.writer.finish())
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

struct PdfContext {
    writer: Pdf,
    width: f32,
    height: f32,
    scale: f32,
    alloc: Ref,
    info_id: Ref,
    catalog_id: Ref,
    page_tree_id: Ref,
    page_id: Ref,
    content_id: Ref,
    svg_id: Ref,
    svg_name: Vec<u8>,
    next_font_name_index: usize,
}

impl PdfContext {
    fn new(width: f32, height: f32, scale: f32) -> Self {
        let mut alloc = Ref::new(1);
        let info_id = alloc.bump();
        let catalog_id = alloc.bump();
        let page_tree_id = alloc.bump();
        let page_id = alloc.bump();
        let content_id = alloc.bump();

        // svg_id will be replaced later because it must be the last id before calling svg2pdf
        let svg_id = Ref::new(1);

        Self {
            writer: Pdf::new(),
            width,
            height,
            scale,
            alloc,
            info_id,
            catalog_id,
            page_tree_id,
            page_id,
            content_id,
            svg_id,
            svg_name: Vec::from(b"S1".as_slice()),
            next_font_name_index: 1,
        }
    }

    fn next_font_name(&mut self) -> String {
        let name = format!("F{}", self.next_font_name_index);
        self.next_font_name_index += 1;
        name
    }
}

/// Construct a single PDF page (with required parents)
fn construct_page(ctx: &mut PdfContext, font_metrics: &HashMap<Font, FontMetrics>) {
    let mut info = ctx.writer.document_info(ctx.info_id);
    info.creator(TextStr("VlConvert"));
    info.finish();

    ctx.writer.catalog(ctx.catalog_id).pages(ctx.page_tree_id);
    ctx.writer
        .pages(ctx.page_tree_id)
        .kids([ctx.page_id])
        .count(1);

    // Initialize page with size matching the SVG image
    let mut page = ctx.writer.page(ctx.page_id);
    page.media_box(Rect::new(
        0.0,
        0.0,
        ctx.width * ctx.scale,
        ctx.height * ctx.scale,
    ));
    page.parent(ctx.page_tree_id);
    page.contents(ctx.content_id);

    let mut resources = page.resources();
    // SVG
    resources
        .x_objects()
        .pair(Name(ctx.svg_name.as_slice()), ctx.svg_id);

    // Fonts
    let mut resource_fonts = resources.fonts();
    for mapped_font in font_metrics.values().sorted_by_key(|f| f.font_ref) {
        resource_fonts.pair(
            Name(mapped_font.font_ref_name.as_slice()),
            mapped_font.font_ref,
        );
    }
    resource_fonts.finish();
    resources.finish();

    // Finish page configuration
    page.finish();
}

/// Write the SVG to a PDF XObject using svg2pdf.
/// Note that svg2pdf currently ignores Text nodes, which is why we handle text
/// separately
fn write_svg(ctx: &mut PdfContext, tree: &Tree) {
    ctx.alloc = svg2pdf::convert_tree_into(
        tree,
        svg2pdf::Options::default(),
        &mut ctx.writer,
        ctx.svg_id,
    );
}

/// Write fonts to PDF resources
fn write_fonts(
    ctx: &mut PdfContext,
    font_metrics: &HashMap<Font, FontMetrics>,
) -> Result<(), AnyError> {
    for font_specs in font_metrics.values().sorted_by_key(|f| f.font_ref) {
        let cid_ref = ctx.alloc.bump();
        let descriptor_ref = ctx.alloc.bump();
        let cmap_ref = ctx.alloc.bump();
        let data_ref = ctx.alloc.bump();
        let is_cff = font_specs.is_cff;

        ctx.writer
            .type0_font(font_specs.font_ref)
            .base_font(Name(font_specs.base_font_type0.as_bytes()))
            .encoding_predefined(Name(b"Identity-H"))
            .descendant_font(cid_ref)
            .to_unicode(cmap_ref);

        // Write the CID font referencing the font descriptor.
        let mut cid = ctx.writer.cid_font(cid_ref);
        cid.subtype(CidFontType::Type2);
        cid.subtype(if is_cff {
            CidFontType::Type0
        } else {
            CidFontType::Type2
        });
        cid.base_font(Name(font_specs.base_font.as_bytes()));
        cid.system_info(SYSTEM_INFO);
        cid.font_descriptor(descriptor_ref);
        cid.default_width(0.0);
        if !is_cff {
            cid.cid_to_gid_map_predefined(Name(b"Identity"));
        }

        // Write all non-zero glyph widths.
        let mut width_writer = cid.widths();
        for (i, w) in font_specs.widths.iter().enumerate().skip(1) {
            if *w != 0.0 {
                width_writer.same(i as u16, i as u16, *w);
            }
        }

        width_writer.finish();
        cid.finish();

        // Write the font descriptor (contains metrics about the font).
        let mut font_descriptor = ctx.writer.font_descriptor(descriptor_ref);
        font_descriptor
            .name(Name(font_specs.base_font.as_bytes()))
            .flags(font_specs.flags)
            .bbox(font_specs.bbox)
            .italic_angle(font_specs.italic_angle)
            .ascent(font_specs.ascender)
            .descent(font_specs.descender)
            .cap_height(font_specs.cap_height)
            .stem_v(font_specs.stem_v);

        if is_cff {
            font_descriptor.font_file3(data_ref);
        } else {
            font_descriptor.font_file2(data_ref);
        }
        font_descriptor.finish();

        // Write the /ToUnicode character map, which maps character ids back to
        // unicode codepoints to enable copying out of the PDF.
        let cmap = create_cmap(&font_specs.cid_set);
        ctx.writer.cmap(cmap_ref, &cmap.finish());

        let glyphs: Vec<_> = font_specs.glyph_set.keys().copied().collect();
        let profile = subsetter::Profile::pdf(&glyphs);
        let subsetted = subsetter::subset(&font_specs.font_data, font_specs.face_index, profile);
        let subset_font_data = deflate(subsetted.as_deref().unwrap_or(&font_specs.font_data));

        let mut stream = ctx.writer.stream(data_ref, &subset_font_data);
        stream.filter(Filter::FlateDecode);
        if is_cff {
            stream.pair(Name(b"Subtype"), Name(b"CIDFontType0C"));
        }
        stream.finish();
    }
    Ok(())
}

fn write_content(
    ctx: &mut PdfContext,
    tree: &Tree,
    font_mapping: &HashMap<Font, FontMetrics>,
    font_db: &Database,
) -> Result<(), AnyError> {
    // Create a content stream with the SVG and text
    let mut content = Content::new();

    // Add reference to the SVG XObject
    // It's re-scaled to the size of the document because convert_tree_into above
    // scales it to 1.0 x 1.0
    content
        .save_state()
        .transform([
            ctx.width * ctx.scale,
            0.0,
            0.0,
            ctx.height * ctx.scale,
            0.0,
            0.0,
        ])
        .x_object(Name(ctx.svg_name.as_slice()))
        .restore_state();

    // Add Text
    content.save_state();

    for node in tree.root.children() {
        write_text(ctx, node, &mut content, font_db, font_mapping)?;
    }

    content.restore_state();

    // Write the content stream
    ctx.writer.stream(ctx.content_id, &content.finish());
    Ok(())
}

fn write_text(
    ctx: &PdfContext,
    node: Node,
    content: &mut Content,
    font_db: &Database,
    font_metrics: &HashMap<Font, FontMetrics>,
) -> Result<(), AnyError> {
    match *node.borrow() {
        NodeKind::Text(ref text) if text.chunks.len() == 1 => {
            let Some(text_width) = get_text_width(text, font_db) else {
                bail!("Failed to calculate text bounding box")
            };

            let chunk = &text.chunks[0];
            let x_offset = match chunk.anchor {
                TextAnchor::Start => 0.0,
                TextAnchor::Middle => -text_width / 2.0,
                TextAnchor::End => -text_width,
            };

            // Compute chunk x/y
            let chunk_x = chunk.x.unwrap_or(0.0) + x_offset as f32;
            let chunk_y = -chunk.y.unwrap_or(0.0);

            let tx = node.abs_transform();

            content.save_state().transform([
                tx.sx * ctx.scale,
                tx.kx * ctx.scale,
                tx.ky * ctx.scale,
                tx.sy * ctx.scale,
                tx.tx * ctx.scale,
                (ctx.height - tx.ty) * ctx.scale,
            ]);

            // Start text
            content.begin_text().next_line(chunk_x, chunk_y);

            for span in &chunk.spans {
                let font_size = span.font_size.get();

                // Skip zero opacity text, and text without a fill
                let span_opacity = span.fill.clone().unwrap_or_default().opacity;
                if span.fill.is_none()
                    || span_opacity == Opacity::ZERO
                    || node_has_zero_opacity(&node)
                {
                    continue;
                }

                let Some(font_specs) = font_metrics.get(&span.font) else {
                    bail!("Font metrics not found")
                };

                // Compute left-to-right ordering of characters
                let mut span_text = chunk.text[span.start..span.end].to_string();
                let bidi_info = BidiInfo::new(&span_text, None);
                if bidi_info.paragraphs.len() == 1 {
                    let para = &bidi_info.paragraphs[0];
                    let line = para.range.clone();
                    span_text = bidi_info.reorder_line(para, line).to_string();
                }

                // Encode 16-bit glyph index into two bytes
                let mut encoded_text = Vec::new();
                for ch in span_text.chars() {
                    if let Some(g) = font_specs.char_set.get(&ch) {
                        encoded_text.push((*g >> 8) as u8);
                        encoded_text.push((*g & 0xff) as u8);
                    }
                }

                // Extract fill color
                let (fill_r, fill_g, fill_b) = match &span.fill {
                    Some(fill) => {
                        if let Paint::Color(color) = fill.paint {
                            (
                                color.red as f32 / 255.0,
                                color.green as f32 / 255.0,
                                color.blue as f32 / 255.0,
                            )
                        } else {
                            // Use black for other pain modes
                            (0.0, 0.0, 0.0)
                        }
                    }
                    None => (0.0, 0.0, 0.0),
                };

                content
                    .set_font(Name(font_specs.font_ref_name.as_slice()), font_size)
                    .set_fill_rgb(fill_r, fill_g, fill_b)
                    .show(Str(encoded_text.as_slice()));
            }

            content.end_text().restore_state();
        }
        NodeKind::Group(_) => {
            for child in node.children() {
                write_text(ctx, child, content, font_db, font_metrics)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Check if this node is a group node with zero opacity,
/// or if it has an ancestor group node with zero opacity
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

fn get_text_width(text: &Text, font_db: &Database) -> Option<f64> {
    let Some(node) = text.convert(font_db, Default::default()) else {
        return None;
    };
    get_text_width_from_path(node)
}

fn get_text_width_from_path(node: Node) -> Option<f64> {
    match *node.borrow() {
        NodeKind::Group(_) => {
            for child in node.children() {
                if let Some(res) = get_text_width_from_path(child) {
                    return Some(res);
                }
            }
            None
        }
        NodeKind::Path(ref path) => {
            // Use text_box width and bounding box height
            path.text_bbox.map(|p| p.width() as f64)
        }
        _ => None,
    }
}

/// Collect mapping from usvg::Font to Unicode characters in that font
fn collect_font_to_chars_mapping(
    tree: &Tree,
) -> Result<HashMap<Font, HashSet<char>>, anyhow::Error> {
    let mut fonts: HashMap<Font, HashSet<char>> = HashMap::new();
    for node in tree.root.descendants() {
        if let NodeKind::Text(ref text) = *node.borrow() {
            match text.chunks.len() {
                // Ignore zero chunk text
                0 => {}
                1 => {
                    let chunk = &text.chunks[0];
                    let chunk_text = chunk.text.as_str();
                    for span in &chunk.spans {
                        let span_text = &chunk_text[span.start..span.end];
                        let font = &span.font;
                        fonts
                            .entry(font.clone())
                            .or_default()
                            .extend(span_text.chars());
                    }
                }
                _ => bail!("multi-chunk text not supported"),
            }
        }
    }
    Ok(fonts)
}

struct FontMetrics {
    font_ref: Ref,
    font_ref_name: Vec<u8>,
    font_data: Vec<u8>,
    face_index: u32,
    glyph_set: BTreeMap<u16, String>,
    char_set: BTreeMap<char, u16>,
    flags: FontFlags,
    bbox: Rect,
    widths: Vec<f32>,
    italic_angle: f32,
    ascender: f32,
    descender: f32,
    cap_height: f32,
    stem_v: f32,
    base_font: String,
    is_cff: bool,
    base_font_type0: String,
    cid_set: BTreeMap<u16, String>,
}

/// Compute the font metrics and references required by PDF embedding for a usvg::Font
fn compute_font_metrics(
    ctx: &mut PdfContext,
    font: &Font,
    chars: &HashSet<char>,
    font_db: &Database,
) -> Result<FontMetrics, anyhow::Error> {
    let families = font
        .families
        .iter()
        .map(|family| match family.as_str() {
            "serif" => Family::Serif,
            "sans-serif" | "sans serif" => Family::SansSerif,
            "monospace" => Family::Monospace,
            "cursive" => Family::Cursive,
            name => Family::Name(name),
        })
        .collect::<Vec<_>>();

    let stretch = match font.stretch {
        FontStretch::UltraCondensed => Stretch::UltraCondensed,
        FontStretch::ExtraCondensed => Stretch::ExtraCondensed,
        FontStretch::Condensed => Stretch::Condensed,
        FontStretch::SemiCondensed => Stretch::SemiCondensed,
        FontStretch::Normal => Stretch::Normal,
        FontStretch::SemiExpanded => Stretch::SemiExpanded,
        FontStretch::Expanded => Stretch::Expanded,
        FontStretch::ExtraExpanded => Stretch::ExtraExpanded,
        FontStretch::UltraExpanded => Stretch::UltraExpanded,
    };

    let style = match font.style {
        FontStyle::Normal => Style::Normal,
        FontStyle::Italic => Style::Italic,
        FontStyle::Oblique => Style::Oblique,
    };

    let Some(font_id) = font_db.query(&Query {
        families: &families,
        weight: Weight(font.weight),
        stretch,
        style,
    }) else {
        bail!("Unable to find installed font matching {font:?}")
    };

    let Some(face) = font_db.face(font_id) else {
        bail!("Unable to find installed font matching {font:?}")
    };

    let postscript_name = face.post_script_name.clone();

    let font_data = match &face.source {
        Source::Binary(d) => Vec::from(d.as_ref().as_ref()),
        Source::File(f) => fs::read(f)?,
        Source::SharedFile(_, d) => Vec::from(d.as_ref().as_ref()),
    };

    let ttf = ttf_parser::Face::parse(&font_data, face.index)?;

    let is_cff = ttf.raw_face().table(CFF).is_some();

    // Conversion function from ttf values in em to PDF's font units
    let to_font_units = |v: f32| (v / ttf.units_per_em() as f32) * 1000.0;

    // Font flags
    let mut flags = FontFlags::empty();
    flags.set(FontFlags::SERIF, postscript_name.contains("Serif"));
    flags.set(FontFlags::FIXED_PITCH, ttf.is_monospaced());
    flags.set(FontFlags::ITALIC, ttf.is_italic());
    flags.insert(FontFlags::SYMBOLIC);
    flags.insert(FontFlags::SMALL_CAP);

    // bounding box
    let global_bbox = ttf.global_bounding_box();
    let bbox = Rect::new(
        to_font_units(global_bbox.x_min.into()),
        to_font_units(global_bbox.y_min.into()),
        to_font_units(global_bbox.x_max.into()),
        to_font_units(global_bbox.y_max.into()),
    );

    // Compute glyph set and chart set
    let mut glyph_set: BTreeMap<u16, String> = BTreeMap::new();
    let mut cid_set: BTreeMap<u16, String> = BTreeMap::new();
    let mut char_set: BTreeMap<char, u16> = BTreeMap::new();
    for ch in chars {
        if let Some(g) = ttf.glyph_index(*ch) {
            let cid = glyph_cid(&ttf, g.0);
            glyph_set.entry(g.0).or_default().push(*ch);
            cid_set.entry(cid).or_default().push(*ch);
            char_set.insert(*ch, cid);
        }
    }

    // Compute widths
    let mut widths = vec![];
    for gid in std::iter::once(0).chain(glyph_set.keys().copied()) {
        let width = ttf.glyph_hor_advance(GlyphId(gid)).unwrap_or(0);
        let units = to_font_units(width as f32);
        let cid = glyph_cid(&ttf, gid);
        if usize::from(cid) >= widths.len() {
            widths.resize(usize::from(cid) + 1, 0.0);
            widths[usize::from(cid)] = units;
        }
    }

    // metrics
    let italic_angle = ttf.italic_angle().unwrap_or(0.0);
    let ascender = to_font_units(ttf.typographic_ascender().unwrap_or(ttf.ascender()).into());
    let descender = to_font_units(
        ttf.typographic_descender()
            .unwrap_or(ttf.descender())
            .into(),
    );
    let cap_height = to_font_units(ttf.capital_height().unwrap_or(ttf.ascender()).into());
    let stem_v = 10.0 + 0.244 * (f32::from(ttf.weight().to_number()) - 50.0);

    // Compute base_font name with subset tag
    let subset_tag = subset_tag(&glyph_set);
    let base_font = format!("{subset_tag}+{postscript_name}");
    let base_font_type0 = if is_cff {
        format!("{base_font}-Identity-H")
    } else {
        base_font.clone()
    };

    Ok(FontMetrics {
        base_font,
        base_font_type0,
        is_cff,
        font_ref: ctx.alloc.bump(),
        font_ref_name: Vec::from(ctx.next_font_name().as_bytes()),
        font_data,
        face_index: face.index,
        glyph_set,
        cid_set,
        char_set,
        flags,
        bbox,
        widths,
        italic_angle,
        ascender,
        descender,
        cap_height,
        stem_v,
    })
}

/// Produce a unique 6 letter tag for a glyph set.
fn subset_tag(glyphs: &BTreeMap<u16, String>) -> String {
    const LEN: usize = 6;
    const BASE: u128 = 26;
    let mut hash = hash128(glyphs);
    let mut letter = [b'A'; LEN];
    for l in letter.iter_mut() {
        *l = b'A' + (hash % BASE) as u8;
        hash /= BASE;
    }
    std::str::from_utf8(&letter).unwrap().to_string()
}

/// Calculate a 128-bit siphash of a value.
fn hash128<T: Hash + ?Sized>(value: &T) -> u128 {
    let mut state = SipHasher13::new();
    value.hash(&mut state);
    state.finish128().as_u128()
}

/// Create a /ToUnicode CMap.
fn create_cmap(cid_set: &BTreeMap<u16, String>) -> UnicodeCmap {
    // Produce a reverse mapping from glyphs to unicode strings.
    let mut cmap = UnicodeCmap::new(CMAP_NAME, SYSTEM_INFO);
    for (&g, text) in cid_set.iter() {
        if !text.is_empty() {
            cmap.pair_with_multiple(g, text.chars());
        }
    }

    cmap
}

fn deflate(data: &[u8]) -> Vec<u8> {
    const COMPRESSION_LEVEL: u8 = 6;
    miniz_oxide::deflate::compress_to_vec_zlib(data, COMPRESSION_LEVEL)
}

/// Get the CID for a glyph id.
///
/// jonmmease: function and docstring taken from Typst
///
/// When writing text into a PDF, we have to specify CIDs (character ids) not
/// GIDs (glyph IDs).
///
/// Most of the time, the mapping between these two is an identity mapping. In
/// particular, for TrueType fonts, the mapping is an identity mapping because
/// of this line above:
/// ```ignore
/// cid.cid_to_gid_map_predefined(Name(b"Identity"));
/// ```
///
/// However, CID-keyed CFF fonts may have a non-identity mapping defined in
/// their charset. For those, we must map the glyph IDs in a `TextItem` to CIDs.
/// The font defines the map through its charset. The charset usually maps
/// glyphs to SIDs (string ids) specifying the glyph's name. Not for CID-keyed
/// fonts though! For these, the SIDs are CIDs in disguise. Relevant quote from
/// the CFF spec:
///
/// > The charset data, although in the same format as non-CIDFonts, will
/// > represent CIDs rather than SIDs, [...]
///
/// This function performs the mapping from glyph ID to CID. It also works for
/// non CID-keyed fonts. Then, it will simply return the glyph ID.
fn glyph_cid(ttf: &ttf_parser::Face, glyph_id: u16) -> u16 {
    ttf.tables()
        .cff
        .and_then(|cff| cff.glyph_cid(ttf_parser::GlyphId(glyph_id)))
        .unwrap_or(glyph_id)
}

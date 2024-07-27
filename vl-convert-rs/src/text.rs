use crate::anyhow;
use crate::anyhow::{anyhow, bail};
use crate::image_loading::custom_string_resolver;
use deno_core::error::AnyError;
use deno_core::op2;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use usvg::fontdb::Database;
use usvg::{
    FallbackSelectionFn, FontFamily, FontResolver, FontSelectionFn, FontStretch, FontStyle,
    ImageHrefResolver,
};

deno_core::extension!(vl_convert_text_runtime, ops = [op_text_width]);

lazy_static! {
    pub static ref USVG_OPTIONS: Mutex<usvg::Options<'static>> = Mutex::new(init_usvg_options());
}

const LIBERATION_SANS_REGULAR: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-Regular.ttf");
const LIBERATION_SANS_BOLD: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-Bold.ttf");
const LIBERATION_SANS_ITALIC: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-Italic.ttf");
const LIBERATION_SANS_BOLDITALIC: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-BoldItalic.ttf");

fn init_usvg_options() -> usvg::Options<'static> {
    let image_href_resolver = ImageHrefResolver {
        resolve_string: custom_string_resolver(),
        ..Default::default()
    };

    let font_resolver = FontResolver {
        select_font: custom_font_selector(),
        select_fallback: custom_fallback_selector(),
    };

    usvg::Options {
        image_href_resolver,
        fontdb: Arc::new(init_font_db()),
        font_resolver,
        ..Default::default()
    }
}

fn init_font_db() -> Database {
    let mut font_database = Database::new();
    // Load fonts from the operating system
    font_database.load_system_fonts();

    // Set default sans-serif font family.
    // By default, Vega outputs SVGs with "sans-serif" as the font family, so
    // we vendor the "Liberation Sans" font so that there is always a fallback
    font_database.load_font_data(Vec::from(LIBERATION_SANS_REGULAR));
    font_database.load_font_data(Vec::from(LIBERATION_SANS_BOLD));
    font_database.load_font_data(Vec::from(LIBERATION_SANS_ITALIC));
    font_database.load_font_data(Vec::from(LIBERATION_SANS_BOLDITALIC));

    setup_default_fonts(&mut font_database);
    font_database
}

fn setup_default_fonts(fontdb: &mut Database) {
    // Collect set of system font families
    let families: HashSet<String> = fontdb
        .faces()
        .flat_map(|face| {
            face.families
                .iter()
                .map(|(fam, _lang)| fam.clone())
                .collect::<Vec<_>>()
        })
        .collect();

    for family in ["Arial", "Helvetica", "Liberation Sans"] {
        if families.contains(family) {
            fontdb.set_sans_serif_family(family);
            break;
        }
    }

    // Set default monospace font family
    for family in [
        "Courier New",
        "Courier",
        "Liberation Mono",
        "DejaVu Sans Mono",
    ] {
        if families.contains(family) {
            fontdb.set_monospace_family(family);
            break;
        }
    }

    // Set default serif font family
    for family in [
        "Times New Roman",
        "Times",
        "Liberation Serif",
        "DejaVu Serif",
    ] {
        if families.contains(family) {
            fontdb.set_serif_family(family);
            break;
        }
    }
}

pub fn custom_font_selector() -> FontSelectionFn<'static> {
    Box::new(move |font, fontdb| {
        // First, try for exact match using fontdb's default font lookup
        let mut name_list = Vec::new();
        for family in font.families() {
            name_list.push(match family {
                FontFamily::Serif => fontdb::Family::Serif,
                FontFamily::SansSerif => fontdb::Family::SansSerif,
                FontFamily::Cursive => fontdb::Family::Cursive,
                FontFamily::Fantasy => fontdb::Family::Fantasy,
                FontFamily::Monospace => fontdb::Family::Monospace,
                FontFamily::Named(s) => fontdb::Family::Name(s.as_str()),
            });
        }

        let stretch = match font.stretch() {
            FontStretch::UltraCondensed => fontdb::Stretch::UltraCondensed,
            FontStretch::ExtraCondensed => fontdb::Stretch::ExtraCondensed,
            FontStretch::Condensed => fontdb::Stretch::Condensed,
            FontStretch::SemiCondensed => fontdb::Stretch::SemiCondensed,
            FontStretch::Normal => fontdb::Stretch::Normal,
            FontStretch::SemiExpanded => fontdb::Stretch::SemiExpanded,
            FontStretch::Expanded => fontdb::Stretch::Expanded,
            FontStretch::ExtraExpanded => fontdb::Stretch::ExtraExpanded,
            FontStretch::UltraExpanded => fontdb::Stretch::UltraExpanded,
        };

        let style = match font.style() {
            FontStyle::Normal => fontdb::Style::Normal,
            FontStyle::Italic => fontdb::Style::Italic,
            FontStyle::Oblique => fontdb::Style::Oblique,
        };

        let query = fontdb::Query {
            families: &name_list,
            weight: fontdb::Weight(font.weight()),
            stretch,
            style,
        };

        if let Some(id) = fontdb.query(&query) {
            // fontdb found a match, use it
            return Some(id);
        }

        // Next, try matching the family name against the post_script_name of each font face.
        // For example, if the SVG font family is "Matter SemiBold", the logic above search for
        // a font family with this name, which will not be found (because the family is Matter).
        // The face's post_script_name for this face will be "Matter-SemiBold"
        for family in &name_list {
            let name = fontdb.family_name(family).replace('-', " ");
            for face in fontdb.faces() {
                if face.post_script_name.replace('-', " ") == name {
                    return Some(face.id);
                }
            }
        }

        log::warn!(
            "No match for '{}' font-family.",
            font.families()
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        None
    })
}

/// Creates a default font fallback selection resolver.
///
/// The default implementation searches through the entire `fontdb`
/// to find a font that has the correct style and supports the character.
pub fn custom_fallback_selector() -> FallbackSelectionFn<'static> {
    Box::new(|c, exclude_fonts, fontdb| {
        let base_font_id = exclude_fonts[0];

        // Prevent fallback to fonts that won't work, like LastResort on macOS
        let forbidden_fallback = ["LastResort"];

        // Iterate over fonts and check if any of them support the specified char.
        for face in fontdb.faces() {
            // Ignore fonts, that were used for shaping already.
            if exclude_fonts.contains(&face.id)
                || forbidden_fallback.contains(&face.post_script_name.as_str())
            {
                continue;
            }

            // Check that the new face has the same style.
            let base_face = fontdb.face(base_font_id)?;
            if base_face.style != face.style
                && base_face.weight != face.weight
                && base_face.stretch != face.stretch
            {
                continue;
            }

            // has_char is private in fontdb
            // if !fontdb.has_char(face.id, c) {
            //     continue;
            // }

            // Implement `fontdb.has_char`, which is not public in fontdb
            let res = fontdb.with_face_data(face.id, |font_data, face_index| -> Option<bool> {
                let font = ttf_parser::Face::parse(font_data, face_index).ok()?;

                font.glyph_index(c)?;
                Some(true)
            });
            if res != Some(Some(true)) {
                continue;
            }

            let base_family = base_face
                .families
                .iter()
                .find(|f| f.1 == fontdb::Language::English_UnitedStates)
                .unwrap_or(&base_face.families[0]);

            let new_family = face
                .families
                .iter()
                .find(|f| f.1 == fontdb::Language::English_UnitedStates)
                .unwrap_or(&base_face.families[0]);

            log::warn!("Fallback from {} to {}.", base_family.0, new_family.0);
            return Some(face.id);
        }

        None
    })
}

#[derive(Deserialize, Clone, Debug)]
struct TextInfo {
    style: Option<String>,
    variant: Option<String>,
    weight: Option<String>,
    family: Option<String>,
    size: f64,
    text: Option<Value>,
}

impl TextInfo {
    pub fn to_svg(&self) -> String {
        let mut text_attrs: Vec<String> = Vec::new();

        text_attrs.push(format!("font-size=\"{}\"", self.size));

        if let Some(family) = &self.family {
            // Remove quotes since usvg can't handle them
            let family = family.replace(['"', '\''], "");
            text_attrs.push(format!("font-family=\"{}\"", family));
        }

        if let Some(weight) = &self.weight {
            text_attrs.push(format!("font-weight=\"{}\"", weight));
        }

        if let Some(style) = &self.style {
            text_attrs.push(format!("font-style=\"{}\"", style));
        }

        if let Some(variant) = &self.variant {
            text_attrs.push(format!("font-variant=\"{}\"", variant));
        }

        let text_attrs_str = text_attrs.join(" ");
        let mut escaped_text = String::new();

        let text = match &self.text {
            Some(Value::String(s)) => s.to_string(),
            Some(text) => text.to_string(),
            None => "".to_string(),
        };

        for char in text.chars() {
            match char {
                '<' => escaped_text.push_str("&lt;"),
                '>' => escaped_text.push_str("&gt;"),
                '"' => escaped_text.push_str("&quot;"),
                '\'' => escaped_text.push_str("&apos;"),
                '&' => escaped_text.push_str("&amp;"),
                '\n' => escaped_text.push_str("&#xA;"),
                '\r' => escaped_text.push_str("&#xD;"),
                _ => escaped_text.push(char),
            }
        }

        format!(
            r#"
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.1" width="100" height="100">
    <text x="20" y="50" {text_attrs_str}>{escaped_text}</text>
</svg>"#,
            text_attrs_str = text_attrs_str,
            escaped_text = escaped_text
        )
    }

    /// Strip potentially unsupported font properties, replace with a supported font,
    /// replace text with zeros
    fn fallback(&self) -> Self {
        let mut new = self.clone();
        new.text = new
            .text
            .map(|t| Value::String(String::from_utf8(vec![b'0'; t.to_string().len()]).unwrap()));
        new.style = None;
        new.family = Some("sans-serif".to_string());
        new.variant = None;
        new.weight = None;
        new
    }
}

#[op2(fast)]
pub fn op_text_width(#[string] text_info_str: String) -> Result<f64, AnyError> {
    let text_info = match serde_json::from_str::<TextInfo>(&text_info_str) {
        Ok(text_info) => text_info,
        Err(err) => bail!("Failed to deserialize text info: {}", err.to_string()),
    };

    // Return width zero for text with non-positive size
    if text_info.size <= 0.0 {
        return Ok(0.0);
    }

    // Return width zero for empty strings and missing text
    match &text_info.text {
        Some(Value::String(text)) => {
            if text.trim().is_empty() {
                return Ok(0.0);
            }
        }
        None => {
            return Ok(0.0);
        }
        _ => {}
    }

    let svg = text_info.to_svg();
    if let Ok(width) = extract_text_width(&svg) {
        Ok(width)
    } else {
        // Try falling back to a supported text info
        let text_info = text_info.fallback();
        let svg = text_info.to_svg();
        extract_text_width(&svg)
    }
}

fn extract_text_width(svg: &String) -> Result<f64, AnyError> {
    let opts = USVG_OPTIONS
        .lock()
        .map_err(|err| anyhow!("Failed to acquire usvg options lock: {}", err.to_string()))?;

    let rtree = usvg::Tree::from_str(svg, &opts).expect("Failed to parse text SVG");

    // Children instead of descendents ok?
    for node in rtree.root().children() {
        // Text bboxes are different from path bboxes.
        if let usvg::Node::Text(ref text) = node {
            let bbox = text.bounding_box();
            let width = bbox.right() - bbox.left();
            let _height = bbox.bottom() - bbox.top();
            return Ok(width as f64);
        }
    }

    let node_strs: Vec<_> = rtree
        .root()
        .children()
        .iter()
        .map(|node| format!("{:?}", node))
        .collect();
    bail!("Failed to locate text in SVG:\n{}\n{:?}", svg, node_strs)
}

pub fn register_font_directory(dir: &str) -> Result<(), anyhow::Error> {
    let mut opts = USVG_OPTIONS
        .lock()
        .map_err(|err| anyhow!("Failed to acquire usvg options lock: {}", err.to_string()))?;

    // Get mutable reference to font_db. This should always be successful since
    // we're holding the mutex on USVG_OPTIONS
    let Some(font_db) = Arc::get_mut(&mut opts.fontdb) else {
        return Err(anyhow!("Could not acquire font_db reference"));
    };

    // Load fonts
    font_db.load_fonts_dir(dir);
    setup_default_fonts(font_db);

    Ok(())
}

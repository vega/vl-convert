use crate::anyhow;
use crate::anyhow::{anyhow, bail};
use crate::image_loading::custom_string_resolver;
use deno_core::error::AnyError;
use deno_core::op2;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Mutex;
use usvg::fontdb::Database;
use usvg::{ImageHrefResolver, TreeParsing, TreeTextToPath};

deno_core::extension!(vl_convert_text_runtime, ops = [op_text_width]);

lazy_static! {
    pub static ref USVG_OPTIONS: Mutex<usvg::Options> = Mutex::new(init_usvg_options());
    pub static ref FONT_DB: Mutex<Database> = Mutex::new(init_font_db());
}

const LIBERATION_SANS_REGULAR: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-Regular.ttf");
const LIBERATION_SANS_BOLD: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-Bold.ttf");
const LIBERATION_SANS_ITALIC: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-Italic.ttf");
const LIBERATION_SANS_BOLDITALIC: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-BoldItalic.ttf");

fn init_usvg_options() -> usvg::Options {
    let image_href_resolver = ImageHrefResolver {
        resolve_string: custom_string_resolver(),
        ..Default::default()
    };
    usvg::Options {
        image_href_resolver,
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
    let mut rtree = usvg::Tree::from_str(svg, &opts).expect("Failed to parse text SVG");

    let font_database = FONT_DB
        .lock()
        .map_err(|err| anyhow!("Failed to acquire fontdb lock: {}", err.to_string()))?;

    rtree.convert_text(&font_database);

    for node in rtree.root.descendants() {
        // Text bboxes are different from path bboxes.
        if let usvg::NodeKind::Path(ref path) = *node.borrow() {
            if let Some(ref bbox) = path.text_bbox {
                let width = bbox.right() - bbox.left();
                let _height = bbox.bottom() - bbox.top();
                return Ok(width as f64);
            }
        }
    }

    let node_strs: Vec<_> = rtree
        .root
        .descendants()
        .map(|node| format!("{:?}", node))
        .collect();
    bail!("Failed to locate text in SVG:\n{}\n{:?}", svg, node_strs)
}

pub fn register_font_directory(dir: &str) -> Result<(), anyhow::Error> {
    let mut font_database = FONT_DB
        .lock()
        .map_err(|err| anyhow!("Failed to acquire font_db lock: {}", err.to_string()))?;
    font_database.load_fonts_dir(dir);

    setup_default_fonts(&mut font_database);
    Ok(())
}

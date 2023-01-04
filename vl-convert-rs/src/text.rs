use crate::anyhow;
use crate::anyhow::{anyhow, bail};
use deno_core::error::AnyError;
use deno_core::op;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Mutex;
use usvg::fontdb::Database;

lazy_static! {
    pub static ref USVG_OPTIONS: Mutex<usvg::Options> = Mutex::new(init_usvg_options());
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
    let mut opt = usvg::Options::default();
    let fontdb = &mut opt.fontdb;

    // Load fonts from the operating system
    fontdb.load_system_fonts();

    // Set default sans-serif font family.
    // By default, Vega outputs SVGs with "sans-serif" as the font family, so
    // we vendor the "Liberation Sans" font so that there is always a fallback
    fontdb.load_font_data(Vec::from(LIBERATION_SANS_REGULAR));
    fontdb.load_font_data(Vec::from(LIBERATION_SANS_BOLD));
    fontdb.load_font_data(Vec::from(LIBERATION_SANS_ITALIC));
    fontdb.load_font_data(Vec::from(LIBERATION_SANS_BOLDITALIC));

    setup_default_fonts(fontdb);
    opt
}

fn setup_default_fonts(fontdb: &mut Database) {
    // Collect set of system font families
    let families: HashSet<String> = fontdb
        .faces()
        .iter()
        .map(|face| face.family.clone())
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
    size: i32,
    text: Value,
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
            Value::String(s) => s.to_string(),
            text => text.to_string(),
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

    /// Strip potentially unsupported font properties and replace with a supported font
    fn fallback(&self) -> Self {
        let mut new = self.clone();
        new.style = None;
        new.family = Some("sans-serif".to_string());
        new.variant = None;
        new.weight = None;
        new
    }
}

#[op]
pub fn op_text_width(text_info_str: String) -> Result<f64, AnyError> {
    let text_info = match serde_json::from_str::<TextInfo>(&text_info_str) {
        Ok(text_info) => text_info,
        Err(err) => bail!("Failed to deserialize text info: {}", err.to_string()),
    };

    if let Some(text) = text_info.text.as_str() {
        if text.trim().is_empty() {
            return Ok(0.0);
        }
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
    let rtree = usvg::Tree::from_str(svg, &opts.to_ref()).expect("Failed to parse text SVG");
    for node in rtree.root.descendants() {
        // Text bboxes are different from path bboxes.
        if let usvg::NodeKind::Path(ref path) = *node.borrow() {
            if let Some(ref bbox) = path.text_bbox {
                let width = bbox.right() - bbox.left();
                let _height = bbox.bottom() - bbox.top();
                return Ok(width);
            }
        }
    }

    let node_strs: Vec<_> = rtree
        .root
        .descendants()
        .into_iter()
        .map(|node| format!("{:?}", node))
        .collect();
    bail!("Failed to locate text in SVG:\n{}\n{:?}", svg, node_strs)
}

pub fn register_font_directory(dir: &str) -> Result<(), anyhow::Error> {
    let mut opts = USVG_OPTIONS
        .lock()
        .map_err(|err| anyhow!("Failed to acquire usvg options lock: {}", err.to_string()))?;
    opts.fontdb.load_fonts_dir(dir);

    setup_default_fonts(&mut opts.fontdb);
    Ok(())
}

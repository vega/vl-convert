use crate::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::op;
use serde::Deserialize;
use std::collections::HashSet;

lazy_static! {
    pub static ref USVG_OPTIONS: usvg::Options = init_usvg_options();
}

const SANS_SERIF_REGULAR: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-Regular.ttf");
const SANS_SERIF_BOLD: &[u8] = include_bytes!("../fonts/liberation-sans/LiberationSans-Bold.ttf");
const SANS_SERIF_ITALIC: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-Italic.ttf");
const SANS_SERIF_BOLDITALIC: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-BoldItalic.ttf");

fn init_usvg_options() -> usvg::Options {
    let mut opt = usvg::Options::default();

    // Load fonts from the operating system
    opt.fontdb.load_system_fonts();

    // Collect set of system font families
    let families: HashSet<String> = opt
        .fontdb
        .faces()
        .iter()
        .map(|face| face.family.clone())
        .collect();

    // Set default sans-serif font family.
    // By default, Vega outputs SVGs with "sans-serif" as the font family, so
    // we vendor the "Liberation Sans" font so that there is always a fallback
    opt.fontdb.load_font_data(Vec::from(SANS_SERIF_REGULAR));
    opt.fontdb.load_font_data(Vec::from(SANS_SERIF_BOLD));
    opt.fontdb.load_font_data(Vec::from(SANS_SERIF_ITALIC));
    opt.fontdb.load_font_data(Vec::from(SANS_SERIF_BOLDITALIC));

    for family in ["Arial", "Helvetica", "Liberation Sans"] {
        if families.contains(family) {
            opt.fontdb.set_sans_serif_family(family);
            break;
        }
    }

    // Set default monospace font family
    for family in ["Courier New", "Courier", "Liberation Mono"] {
        if families.contains(family) {
            opt.fontdb.set_monospace_family(family);
            break;
        }
    }

    // Set default serif font family
    for family in ["Times New Roman", "Times", "Liberation Serif"] {
        if families.contains(family) {
            opt.fontdb.set_serif_family(family);
            break;
        }
    }

    opt
}

#[derive(Deserialize, Clone, Debug)]
struct TextInfo {
    style: Option<String>,
    variant: Option<String>,
    weight: Option<String>,
    family: Option<String>,
    size: i32,
    text: String,
}

impl TextInfo {
    pub fn to_svg(&self) -> String {
        let mut text_attrs: Vec<String> = Vec::new();

        text_attrs.push(format!("font-size=\"{}\"", self.size));

        if let Some(family) = &self.family {
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

        format!(
            r#"
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.1" width="100" height="100">
    <text x="20" y="50" {text_attrs_str}>{text}</text>
</svg>"#,
            text_attrs_str = text_attrs_str,
            text = self.text
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
    let rtree =
        usvg::Tree::from_str(svg, &USVG_OPTIONS.to_ref()).expect("Failed to parse text SVG");
    for node in rtree.root().descendants() {
        if !rtree.is_in_defs(&node) {
            // Text bboxes are different from path bboxes.
            if let usvg::NodeKind::Path(ref path) = *node.borrow() {
                if let Some(ref bbox) = path.text_bbox {
                    let width = bbox.right() - bbox.left();
                    let _height = bbox.bottom() - bbox.top();
                    return Ok(width);
                }
            }
        }
    }

    let node_strs: Vec<_> = rtree
        .root()
        .descendants()
        .into_iter()
        .map(|node| format!("{:?}", node))
        .collect();
    bail!("Failed to locate text in SVG:\n{}\n{:?}", svg, node_strs)
}

use deno_core::error::AnyError;
use deno_core::op;
use serde::{Deserialize};
use crate::anyhow::bail;

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
            text_attrs_str=text_attrs_str,
            text=self.text
        )
    }
}

#[op]
pub fn op_text_width(text_info_str: String) -> Result<f64, AnyError> {
    let text_info = match serde_json::from_str::<TextInfo>(&text_info_str) {
        Ok(text_info) => text_info,
        Err(err) => bail!("Failed to deserialize text info: {}", err.to_string())
    };

    println!("{:?}", text_info);

    let svg = text_info.to_svg();
    let mut opt = usvg::Options::default();
    opt.fontdb.load_system_fonts();
    opt.fontdb.set_monospace_family("Courier New");
    opt.fontdb.set_sans_serif_family("Arial");
    opt.fontdb.set_serif_family("Times New Roman");

    let rtree = usvg::Tree::from_str(&svg, &opt.to_ref()).expect("Failed to parse text SVG");
    for node in rtree.root().descendants() {
        if !rtree.is_in_defs(&node) {
            // Text bboxes are different from path bboxes.
            if let usvg::NodeKind::Path(ref path) = *node.borrow() {
                if let Some(ref bbox) = path.text_bbox {
                    let width = bbox.right() - bbox.left();
                    let height = bbox.bottom() - bbox.top();
                    println!("width={} height={}", width, height);
                    return Ok(width)
                }
            }
        }
    }

    bail!("Failed to locate text in SVG")
}
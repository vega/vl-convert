use super::types::{parse_css_font_family, FontFamilyEntry, FontKey};
use deno_core::error::AnyError;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;

/// A non-data-URI `<image>` href found in the SVG, with the byte range of
/// the attribute value so it can be replaced in-place.
#[derive(Debug, Clone)]
pub struct ImageRef {
    /// The href string (URL or file path).
    pub href: String,
    /// Byte range of the href attribute *value* in the original SVG string.
    pub value_range: Range<usize>,
}

/// Result of analyzing a rendered SVG for font embedding and image inlining.
#[derive(Debug)]
pub struct SvgAnalysis {
    /// Characters used per (family, weight, style) -- extracted from `<text>` direct attributes.
    pub chars_by_key: HashMap<FontKey, BTreeSet<char>>,
    /// Unique font families found (for classification).
    pub families: HashSet<String>,
    /// Byte offset where `<defs><style>` should be inserted (before first child element).
    pub insert_pos: usize,
    /// Non-data-URI `<image>` hrefs with their attribute value byte ranges.
    pub image_refs: Vec<ImageRef>,
}

/// Extract all font-family CSS strings from an SVG document.
///
/// Parses the SVG as XML and collects `font-family` values from:
/// - Direct `font-family` attributes on any element
/// - `font-family:` declarations inside inline `style` attributes
/// - `font-family:` declarations inside `<style>` element text
///
/// Returns an empty set on parse error (usvg will provide a better error later).
pub fn extract_fonts_from_svg(svg: &str) -> HashSet<String> {
    let xml_opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };

    let doc = match roxmltree::Document::parse_with_options(svg, xml_opt) {
        Ok(doc) => doc,
        Err(_) => return HashSet::new(),
    };

    let mut fonts = HashSet::new();

    for node in doc.descendants() {
        if node.is_element() {
            // Direct font-family attribute
            if let Some(ff) = node.attribute("font-family") {
                fonts.insert(ff.to_string());
            }

            // Inline style attribute: tokenize declarations with simplecss
            if let Some(style) = node.attribute("style") {
                collect_font_family_from_declarations(
                    simplecss::DeclarationTokenizer::from(style),
                    &mut fonts,
                );
            }
        }

        // <style> element text content: parse as a full stylesheet
        if node.is_element() && node.tag_name().name() == "style" {
            if let Some(text_node) = node.first_child().filter(|c| c.is_text()) {
                if let Some(text) = text_node.text() {
                    let sheet = simplecss::StyleSheet::parse(text);
                    for rule in &sheet.rules {
                        collect_font_family_from_declarations(
                            rule.declarations.iter().copied(),
                            &mut fonts,
                        );
                    }
                }
            }
        }
    }

    fonts
}

/// Collect `font-family` values from an iterator of CSS declarations.
fn collect_font_family_from_declarations<'a>(
    declarations: impl Iterator<Item = simplecss::Declaration<'a>>,
    fonts: &mut HashSet<String>,
) {
    for decl in declarations {
        if decl.name == "font-family" {
            let value = decl.value.trim();
            if !value.is_empty() {
                fonts.insert(value.to_string());
            }
        }
    }
}

/// Normalize a font-weight string value to a numeric string.
fn normalize_weight_str(s: &str) -> String {
    match s.trim() {
        "normal" | "" => "400".to_string(),
        "bold" => "700".to_string(),
        "bolder" => "700".to_string(),
        "lighter" => "100".to_string(),
        other => other
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && *n > 0.0)
            .map(|n| format!("{}", n as i32))
            .unwrap_or_else(|| "400".to_string()),
    }
}

/// Normalize a font-style string value.
fn normalize_style_str(s: &str) -> String {
    let lower = s.trim().to_lowercase();
    if lower == "italic" || lower == "oblique" {
        "italic".to_string()
    } else {
        "normal".to_string()
    }
}

/// Parse an SVG once and extract all information needed for font embedding
/// and image inlining.
///
/// Extracts:
/// - Characters per (family, weight, style) from `<text>` element direct attributes
///   (Vega always emits font-family/weight/style as explicit attributes)
/// - Insertion point byte offset (before first child element of root `<svg>`)
/// - Non-data-URI `<image>` hrefs with byte ranges for replacement
pub fn analyze_svg(svg: &str) -> Result<SvgAnalysis, AnyError> {
    let xml_opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };

    let doc = roxmltree::Document::parse_with_options(svg, xml_opt)
        .map_err(|e| deno_core::anyhow::anyhow!("Failed to parse SVG: {e}"))?;

    let root = doc.root_element();

    // Insertion point: before the first child element of <svg>
    let insert_pos = root
        .children()
        .find(|n| n.is_element())
        .map(|n| n.range().start)
        .unwrap_or_else(|| {
            // No child elements -- insert before closing </svg>
            let r = root.range();
            // Find the start of </svg> by searching backwards
            svg[..r.end]
                .rfind("</svg>")
                .unwrap_or(r.end.saturating_sub("</svg>".len()))
        });

    let mut chars_by_key: HashMap<FontKey, BTreeSet<char>> = HashMap::new();
    let mut families: HashSet<String> = HashSet::new();
    let mut image_refs: Vec<ImageRef> = Vec::new();

    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }

        let tag = node.tag_name().name();

        if tag == "text" {
            // Extract font info from direct attributes
            let font_family_str = node.attribute("font-family").unwrap_or("sans-serif");
            let parsed = parse_css_font_family(font_family_str);
            let family = match parsed.first() {
                Some(FontFamilyEntry::Named(name)) => name.clone(),
                _ => continue,
            };

            let weight = normalize_weight_str(node.attribute("font-weight").unwrap_or(""));
            let style = normalize_style_str(node.attribute("font-style").unwrap_or(""));

            families.insert(family.clone());

            // Collect text content from this element and all text descendants
            let text_content = collect_text_content(&node);
            if !text_content.is_empty() {
                let key = FontKey {
                    family,
                    weight,
                    style,
                };
                let chars = chars_by_key.entry(key).or_default();
                for ch in text_content.chars() {
                    chars.insert(ch);
                }
            }
        } else if tag == "image" {
            // Extract image href for potential inlining
            // Try href first, then xlink:href
            let attr = node
                .attribute_node("href")
                .or_else(|| node.attribute_node(("http://www.w3.org/1999/xlink", "href")));
            let attr = match attr {
                Some(a) => a,
                None => continue,
            };

            let href = attr.value();
            if href.starts_with("data:") {
                continue; // already inlined
            }

            // Compute the byte range of the attribute value from the full attribute range.
            // The attribute range covers: name='value' or name="value"
            // We search for the first quote after '=' within the attribute range.
            let attr_range = attr.range();
            let attr_str = &svg[attr_range.clone()];
            let eq_pos = match attr_str.find('=') {
                Some(p) => p,
                None => continue,
            };
            let after_eq = &attr_str[eq_pos + 1..];
            let quote_char = after_eq.chars().find(|c| *c == '"' || *c == '\'');
            let quote_char = match quote_char {
                Some(q) => q,
                None => continue,
            };
            let value_start_in_attr = eq_pos + 1 + after_eq.find(quote_char).unwrap() + 1;
            let value_end_in_attr = attr_str.len() - 1; // before closing quote
            let value_range =
                (attr_range.start + value_start_in_attr)..(attr_range.start + value_end_in_attr);

            image_refs.push(ImageRef {
                href: href.to_string(),
                value_range,
            });
        }
    }

    Ok(SvgAnalysis {
        chars_by_key,
        families,
        insert_pos,
        image_refs,
    })
}

/// Collect all text content from a node and its text-node descendants.
fn collect_text_content(node: &roxmltree::Node) -> String {
    let mut text = String::new();
    for child in node.descendants() {
        if child.is_text() {
            if let Some(t) = child.text() {
                text.push_str(t);
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_svg_font_family_attribute() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <text font-family="Roboto, sans-serif">Hello</text>
        </svg>"#;
        let fonts = extract_fonts_from_svg(svg);
        assert!(fonts.contains("Roboto, sans-serif"));
        assert_eq!(fonts.len(), 1);
    }

    #[test]
    fn test_svg_inline_style() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <text style="font-family: Playfair Display; font-size: 14px;">Hello</text>
        </svg>"#;
        let fonts = extract_fonts_from_svg(svg);
        assert!(fonts.contains("Playfair Display"));
        assert_eq!(fonts.len(), 1);
    }

    #[test]
    fn test_svg_style_block() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <style>
                .title { font-family: Montserrat, sans-serif; font-size: 16px; }
                .label { font-family: Fira Code; }
            </style>
            <text class="title">Title</text>
        </svg>"#;
        let fonts = extract_fonts_from_svg(svg);
        assert!(fonts.contains("Montserrat, sans-serif"));
        assert!(fonts.contains("Fira Code"));
        assert_eq!(fonts.len(), 2);
    }

    #[test]
    fn test_svg_deduplication() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <text font-family="Roboto">One</text>
            <text font-family="Roboto">Two</text>
        </svg>"#;
        let fonts = extract_fonts_from_svg(svg);
        assert!(fonts.contains("Roboto"));
        assert_eq!(fonts.len(), 1);
    }

    #[test]
    fn test_svg_empty() {
        let fonts = extract_fonts_from_svg("");
        assert!(fonts.is_empty());
    }

    #[test]
    fn test_svg_invalid() {
        let fonts = extract_fonts_from_svg("<not valid xml");
        assert!(fonts.is_empty());
    }

    #[test]
    fn test_svg_no_fonts() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <rect width="100" height="100" fill="red"/>
        </svg>"#;
        let fonts = extract_fonts_from_svg(svg);
        assert!(fonts.is_empty());
    }

    #[test]
    fn test_svg_mixed_sources() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <style>.cls { font-family: Lato; }</style>
            <text font-family="Roboto">Attr</text>
            <text style="font-family: Open Sans;">Inline</text>
        </svg>"#;
        let fonts = extract_fonts_from_svg(svg);
        assert!(fonts.contains("Lato"));
        assert!(fonts.contains("Roboto"));
        assert!(fonts.contains("Open Sans"));
        assert_eq!(fonts.len(), 3);
    }

    #[test]
    fn test_analyze_svg_basic_text() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="Roboto" font-weight="bold">Hello</text></svg>"#;
        let result = analyze_svg(svg).unwrap();
        assert!(result.families.contains("Roboto"));
        let key = FontKey {
            family: "Roboto".into(),
            weight: "700".into(),
            style: "normal".into(),
        };
        let chars = result.chars_by_key.get(&key).unwrap();
        assert!(chars.contains(&'H'));
        assert!(chars.contains(&'o'));
    }

    #[test]
    fn test_analyze_svg_font_weight_normalization() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="Arial" font-weight="bold">X</text></svg>"#;
        let result = analyze_svg(svg).unwrap();
        let key = FontKey {
            family: "Arial".into(),
            weight: "700".into(),
            style: "normal".into(),
        };
        assert!(result.chars_by_key.contains_key(&key));
    }

    #[test]
    fn test_analyze_svg_default_weight_style() {
        let svg =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="Arial">X</text></svg>"#;
        let result = analyze_svg(svg).unwrap();
        let key = FontKey {
            family: "Arial".into(),
            weight: "400".into(),
            style: "normal".into(),
        };
        assert!(result.chars_by_key.contains_key(&key));
    }

    #[test]
    fn test_analyze_svg_italic_style() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="Arial" font-style="italic">X</text></svg>"#;
        let result = analyze_svg(svg).unwrap();
        let key = FontKey {
            family: "Arial".into(),
            weight: "400".into(),
            style: "italic".into(),
        };
        assert!(result.chars_by_key.contains_key(&key));
    }

    #[test]
    fn test_analyze_svg_insert_position() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="100"/></svg>"#;
        let result = analyze_svg(svg).unwrap();
        // Insert position should be at the start of <rect>
        assert_eq!(&svg[result.insert_pos..result.insert_pos + 5], "<rect");
    }

    #[test]
    fn test_analyze_svg_image_http() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="https://example.com/img.png"/></svg>"#;
        let result = analyze_svg(svg).unwrap();
        assert_eq!(result.image_refs.len(), 1);
        assert_eq!(result.image_refs[0].href, "https://example.com/img.png");
        // Verify the value range points to the correct bytes
        assert_eq!(
            &svg[result.image_refs[0].value_range.clone()],
            "https://example.com/img.png"
        );
    }

    #[test]
    fn test_analyze_svg_image_data_uri_skipped() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/png;base64,ABC"/></svg>"#;
        let result = analyze_svg(svg).unwrap();
        assert!(result.image_refs.is_empty());
    }

    #[test]
    fn test_analyze_svg_image_local_path() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="local.png"/></svg>"#;
        let result = analyze_svg(svg).unwrap();
        assert_eq!(result.image_refs.len(), 1);
        assert_eq!(result.image_refs[0].href, "local.png");
    }

    #[test]
    fn test_analyze_svg_generic_font_skipped() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="sans-serif">X</text></svg>"#;
        let result = analyze_svg(svg).unwrap();
        assert!(result.chars_by_key.is_empty());
        assert!(result.families.is_empty());
    }

    #[test]
    fn test_analyze_svg_no_text_content() {
        let svg =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="Arial"></text></svg>"#;
        let result = analyze_svg(svg).unwrap();
        assert!(result.chars_by_key.is_empty());
    }
}

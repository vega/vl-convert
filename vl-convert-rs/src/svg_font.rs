//! SVG font embedding and image inlining.
//!
//! Implements the `process_svg()` pipeline: single-parse extraction via
//! [`analyze_svg`], font CSS assembly, image inlining, and batch-edit
//! application in reverse byte-position order.

use crate::converter::{classify_scenegraph_fonts, MissingFontsPolicy, SvgOpts, VlConverterConfig};
use crate::extract::{analyze_svg, FontForHtml, FontKey, FontSource, SvgAnalysis};
use crate::font_embed::{generate_font_face_css, variants_by_family};
use crate::html::font_import_rule;
use crate::image_loading::{
    fetch_and_encode_image_http, resolve_and_read_local_image, ImageAccessPolicy,
};
use crate::text::GOOGLE_FONTS_CLIENT;
use deno_core::error::AnyError;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;
use std::path::Path;
use vl_convert_google_fonts::{FontStyle, VariantRequest};

/// A pending modification to the SVG string, identified by byte range.
#[derive(Debug)]
struct SvgEdit {
    /// Byte range in the original SVG string to replace.
    range: Range<usize>,
    /// Replacement text.
    replacement: String,
}

/// Apply a list of non-overlapping edits to an SVG string.
///
/// Edits are sorted by position descending and applied in reverse order so
/// that earlier edits don't shift the byte positions of later edits.
fn apply_edits(mut svg: String, mut edits: Vec<SvgEdit>) -> String {
    if edits.is_empty() {
        return svg;
    }

    // Sort descending by start position
    edits.sort_by(|a, b| b.range.start.cmp(&a.range.start));

    // Debug-assert non-overlapping
    debug_assert!(
        {
            let mut ok = true;
            for pair in edits.windows(2) {
                // pair[0].start >= pair[1].start (descending)
                // pair[1] should end before pair[0] starts
                if pair[1].range.end > pair[0].range.start {
                    ok = false;
                    break;
                }
            }
            ok
        },
        "SvgEdit ranges must be non-overlapping"
    );

    for edit in &edits {
        svg.replace_range(edit.range.clone(), &edit.replacement);
    }
    svg
}

/// Escape CSS for embedding inside SVG XML.
///
/// Google Fonts URLs contain `&` which must become `&amp;` in XML.
fn xml_escape_css(css: &str) -> String {
    css.replace('&', "&amp;")
}

/// Build the font CSS string for SVG embedding.
///
/// Returns the assembled CSS with `@import` rules before `@font-face` blocks.
/// Returns an empty string if there are no fonts to embed/reference.
#[allow(clippy::too_many_arguments)]
fn build_svg_font_css(
    analysis: &SvgAnalysis,
    html_fonts: &[FontForHtml],
    family_variants: &HashMap<String, BTreeSet<(String, String)>>,
    bundle: bool,
    subset_fonts: bool,
    missing_fonts: &MissingFontsPolicy,
    fontdb: &fontdb::Database,
    loaded_batches: &[vl_convert_google_fonts::LoadedFontBatch],
) -> Result<String, AnyError> {
    let mut css_parts: Vec<String> = Vec::new();

    // Compute per-family character sets for subsetting/CDN text param
    let family_chars: HashMap<String, BTreeSet<char>> = if subset_fonts {
        let mut map: HashMap<String, BTreeSet<char>> = HashMap::new();
        for (key, chars) in &analysis.chars_by_key {
            map.entry(key.family.clone()).or_default().extend(chars);
        }
        map
    } else {
        HashMap::new()
    };

    if bundle {
        // Embedded mode: all selected fonts as @font-face with base64 WOFF2
        if !html_fonts.is_empty() {
            let font_face_index = generate_font_face_css(
                &analysis.chars_by_key,
                html_fonts,
                missing_fonts,
                fontdb,
                loaded_batches,
                subset_fonts,
            )?;

            // Emit @font-face blocks in deterministic order
            let mut keys: Vec<&FontKey> = font_face_index.keys().collect();
            keys.sort_by(|a, b| {
                a.family
                    .cmp(&b.family)
                    .then(a.weight.cmp(&b.weight))
                    .then(a.style.cmp(&b.style))
            });
            for key in keys {
                if let Some(css) = font_face_index.get(key) {
                    css_parts.push(css.clone());
                }
            }
        }
    } else {
        // Reference mode: @import for Google fonts, @font-face for local fonts

        // @import rules first (Google fonts only)
        for f in html_fonts {
            if let FontSource::Google { .. } = &f.source {
                if let Some(cdn_set) = family_variants.get(&f.family) {
                    let text: Option<String> = family_chars
                        .get(&f.family)
                        .map(|chars| chars.iter().collect());
                    if let Some(rule) = font_import_rule(f, cdn_set, text.as_deref()) {
                        css_parts.push(rule);
                    }
                }
            }
        }

        // @font-face blocks for local fonts only
        let local_fonts: Vec<&FontForHtml> = html_fonts
            .iter()
            .filter(|f| matches!(f.source, FontSource::Local))
            .collect();

        if !local_fonts.is_empty() {
            let local_html_fonts: Vec<FontForHtml> =
                local_fonts.iter().map(|f| (*f).clone()).collect();
            let font_face_index = generate_font_face_css(
                &analysis.chars_by_key,
                &local_html_fonts,
                missing_fonts,
                fontdb,
                loaded_batches,
                subset_fonts,
            )?;

            let mut keys: Vec<&FontKey> = font_face_index.keys().collect();
            keys.sort_by(|a, b| {
                a.family
                    .cmp(&b.family)
                    .then(a.weight.cmp(&b.weight))
                    .then(a.style.cmp(&b.style))
            });
            for key in keys {
                if let Some(css) = font_face_index.get(key) {
                    css_parts.push(css.clone());
                }
            }
        }
    }

    Ok(css_parts.join("\n"))
}

/// Process a rendered SVG string to embed fonts and/or inline images.
///
/// This is the main entry point for SVG font bundling. It:
/// 1. Parses the SVG once with roxmltree
/// 2. Extracts font data and image references
/// 3. Classifies fonts (Google vs Local)
/// 4. Builds font CSS (@import or @font-face)
/// 5. Inlines images (if bundle=true)
/// 6. Applies all edits in reverse order
///
/// Returns the modified SVG string, or the original if no edits are needed.
pub(crate) async fn process_svg(
    svg: String,
    svg_opts: &SvgOpts,
    config: &VlConverterConfig,
    fontdb: &fontdb::Database,
    loaded_batches: &[vl_convert_google_fonts::LoadedFontBatch],
    image_policy: &ImageAccessPolicy,
    resources_dir: Option<&Path>,
) -> Result<String, AnyError> {
    // 1. Parse SVG and extract fonts + image refs
    let analysis = analyze_svg(&svg)?;

    // 2. Classify fonts
    let explicit_google_families: HashSet<String> = HashSet::new();
    let families: std::collections::BTreeSet<String> = analysis.families.iter().cloned().collect();
    let html_fonts = classify_scenegraph_fonts(
        &families,
        config.auto_google_fonts,
        svg_opts.embed_local_fonts,
        config.missing_fonts,
        &explicit_google_families,
    )
    .await?;

    // 3. Compute family variants
    let family_variants = variants_by_family(&analysis.chars_by_key);

    // 4. Resolve CDN variants for @import rules (non-bundle mode)
    let cdn_variants: HashMap<String, BTreeSet<(String, String)>> = if !svg_opts.bundle {
        let mut cdn_map = HashMap::new();
        for f in &html_fonts {
            if let FontSource::Google { .. } = &f.source {
                if let Some(vs) = family_variants.get(&f.family) {
                    let requested: Vec<VariantRequest> = vs
                        .iter()
                        .map(|(w, s)| VariantRequest {
                            weight: w.parse().unwrap_or(400),
                            style: s.parse().unwrap_or(FontStyle::Normal),
                        })
                        .collect();
                    match GOOGLE_FONTS_CLIENT
                        .resolve_available_variants(&f.family, &requested)
                        .await
                    {
                        Ok(resolved) => {
                            let set: BTreeSet<(String, String)> = resolved
                                .into_iter()
                                .map(|v| (v.weight.to_string(), v.style.as_str().to_string()))
                                .collect();
                            cdn_map.insert(f.family.clone(), set);
                        }
                        Err(e) => {
                            log::warn!(
                                "Failed to resolve variants for '{}': {e}, skipping CDN URL",
                                f.family
                            );
                        }
                    }
                }
            }
        }
        cdn_map
    } else {
        // In bundle mode, family_variants is used directly for @font-face generation
        family_variants.clone()
    };

    // Use resolved CDN variants for non-bundle mode, original for bundle mode
    let variants_for_css = if svg_opts.bundle {
        &family_variants
    } else {
        &cdn_variants
    };

    // 5. Build font CSS
    let css = build_svg_font_css(
        &analysis,
        &html_fonts,
        variants_for_css,
        svg_opts.bundle,
        svg_opts.subset_fonts,
        &config.missing_fonts,
        fontdb,
        loaded_batches,
    )?;

    // 6. Collect edits
    let mut edits: Vec<SvgEdit> = Vec::new();

    // Insert <defs><style> if we have CSS content
    if !css.is_empty() {
        let escaped_css = xml_escape_css(&css);
        edits.push(SvgEdit {
            range: analysis.insert_pos..analysis.insert_pos,
            replacement: format!("<defs><style>\n{escaped_css}\n</style></defs>\n"),
        });
    }

    // Inline images if bundle=true
    if svg_opts.bundle {
        for image_ref in &analysis.image_refs {
            let (mime, b64) = if image_ref.href.starts_with("http://")
                || image_ref.href.starts_with("https://")
            {
                fetch_and_encode_image_http(&image_ref.href, &image_policy.allowed_base_urls)?
            } else {
                resolve_and_read_local_image(
                    &image_ref.href,
                    image_policy.filesystem_root.as_deref(),
                    resources_dir,
                )?
            };

            let data_uri = format!("data:{mime};base64,{b64}");
            edits.push(SvgEdit {
                range: image_ref.value_range.clone(),
                replacement: data_uri,
            });
        }
    }

    // 7. Apply edits or return unchanged
    if edits.is_empty() {
        Ok(svg)
    } else {
        Ok(apply_edits(svg, edits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_edits_empty() {
        let svg = "<svg>hello</svg>".to_string();
        let result = apply_edits(svg.clone(), Vec::new());
        assert_eq!(result, svg);
    }

    #[test]
    fn test_apply_edits_insertion() {
        let svg = "<svg><rect/></svg>".to_string();
        // Insert before <rect>
        let edits = vec![SvgEdit {
            range: 5..5,
            replacement: "<defs/>".to_string(),
        }];
        let result = apply_edits(svg, edits);
        assert_eq!(result, "<svg><defs/><rect/></svg>");
    }

    #[test]
    fn test_apply_edits_replacement() {
        let svg = r#"<svg><image href="old.png"/></svg>"#.to_string();
        // Replace "old.png"
        let edits = vec![SvgEdit {
            range: 18..25,
            replacement: "data:image/png;base64,ABC".to_string(),
        }];
        let result = apply_edits(svg, edits);
        assert_eq!(
            result,
            r#"<svg><image href="data:image/png;base64,ABC"/></svg>"#
        );
    }

    #[test]
    fn test_apply_edits_multiple_non_overlapping() {
        let svg = r#"<svg><defs/><image href="a.png"/><image href="b.png"/></svg>"#.to_string();
        let edits = vec![
            SvgEdit {
                range: 25..30,
                replacement: "data:a".to_string(),
            },
            SvgEdit {
                range: 46..51,
                replacement: "data:b".to_string(),
            },
        ];
        let result = apply_edits(svg, edits);
        assert_eq!(
            result,
            r#"<svg><defs/><image href="data:a"/><image href="data:b"/></svg>"#
        );
    }

    #[test]
    fn test_xml_escape_css() {
        let css = r#"@import url("https://fonts.googleapis.com/css2?family=Roboto:wght@400&display=swap");"#;
        let escaped = xml_escape_css(css);
        assert!(escaped.contains("&amp;display"));
        assert!(!escaped.contains("&display"));
    }

    #[test]
    fn test_xml_escape_css_no_ampersand() {
        let css = "@font-face { font-family: 'Roboto'; }";
        let escaped = xml_escape_css(css);
        assert_eq!(escaped, css);
    }
}

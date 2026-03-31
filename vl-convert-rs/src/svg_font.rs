//! SVG font embedding and image inlining.
//!
//! Implements the `process_svg()` pipeline: font CSS assembly, image inlining,
//! and batch-edit application in reverse byte-position order.
//!
//! Callers provide pre-computed [`SvgAnalysis`] and classified font data so
//! that the SVG is parsed only once (in `postprocess_svg`).

use crate::converter::{MissingFontsPolicy, SvgOpts, VlcConfig};
use crate::extract::{ClassifiedFont, FontKey, FontSource, SvgAnalysis};
use crate::font_embed::{generate_font_face_css, resolve_cdn_variants};
use crate::html::font_import_rule;
use crate::image_loading::{
    fetch_and_encode_image_http, resolve_and_read_local_image, ImageAccessPolicy,
};
use deno_core::error::AnyError;
use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::path::Path;

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

    // Hard assert: overlapping edits would silently corrupt the SVG in
    // release builds if this were only a debug_assert.
    assert!(
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

/// Build the font CSS string for SVG embedding.
///
/// Returns the assembled CSS with `@import` rules before `@font-face` blocks.
/// Returns an empty string if there are no fonts to embed/reference.
#[allow(clippy::too_many_arguments)]
fn build_svg_font_css(
    analysis: &SvgAnalysis,
    classified_fonts: &[ClassifiedFont],
    cdn_variants: &HashMap<String, BTreeSet<(String, String)>>,
    bundle: bool,
    config: &VlcConfig,
    missing_fonts: &MissingFontsPolicy,
    fontdb: &fontdb::Database,
    loaded_batches: &[vl_convert_google_fonts::LoadedFontBatch],
) -> Result<String, AnyError> {
    let mut css_parts: Vec<String> = Vec::new();

    // Compute per-family character sets for subsetting/CDN text param
    let subset_fonts = config.subset_fonts;
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
        if !classified_fonts.is_empty() {
            let font_face_index = generate_font_face_css(
                &analysis.chars_by_key,
                classified_fonts,
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
        for f in classified_fonts {
            if let FontSource::Google { .. } = &f.source {
                if let Some(cdn_set) = cdn_variants.get(&f.family) {
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
        let local_fonts: Vec<ClassifiedFont> = classified_fonts
            .iter()
            .filter(|f| matches!(f.source, FontSource::Local))
            .cloned()
            .collect();

        if !local_fonts.is_empty() {
            let font_face_index = generate_font_face_css(
                &analysis.chars_by_key,
                &local_fonts,
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
/// Callers must provide pre-computed `analysis` and `classified_fonts` so the
/// SVG is not re-parsed. This function:
/// 1. Resolves CDN variants (non-bundle mode)
/// 2. Builds font CSS (@import or @font-face)
/// 3. Inlines images (if bundle=true)
/// 4. Applies all edits in reverse order
///
/// Returns the modified SVG string, or the original if no edits are needed.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn process_svg(
    svg: String,
    svg_opts: &SvgOpts,
    analysis: &SvgAnalysis,
    classified_fonts: &[ClassifiedFont],
    family_variants: &HashMap<String, BTreeSet<(String, String)>>,
    config: &VlcConfig,
    fontdb: &fontdb::Database,
    loaded_batches: &[vl_convert_google_fonts::LoadedFontBatch],
    image_policy: &ImageAccessPolicy,
    resources_dir: Option<&Path>,
) -> Result<String, AnyError> {
    // 1. Resolve CDN variants for @import rules (non-bundle mode)
    let cdn_variants = if !svg_opts.bundle {
        resolve_cdn_variants(classified_fonts, family_variants).await
    } else {
        HashMap::new()
    };

    // 2. Build font CSS
    let css = build_svg_font_css(
        analysis,
        classified_fonts,
        &cdn_variants,
        svg_opts.bundle,
        config,
        &config.missing_fonts,
        fontdb,
        loaded_batches,
    )?;

    // 3. Collect edits
    let mut edits: Vec<SvgEdit> = Vec::new();

    // Insert <defs><style> if we have CSS content
    if !css.is_empty() {
        // CDATA wrapping prevents XML parsing issues with CSS content
        // (e.g., `&` in Google Fonts URLs, `</style>` in font names)
        edits.push(SvgEdit {
            range: analysis.insert_pos..analysis.insert_pos,
            replacement: format!("<defs><style><![CDATA[\n{css}\n]]></style></defs>\n"),
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

    // 4. Apply edits or return unchanged
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
    #[should_panic(expected = "non-overlapping")]
    fn test_apply_edits_overlapping_panics() {
        let svg = "<svg>abcdefghij</svg>".to_string();
        let edits = vec![
            SvgEdit {
                range: 5..8,
                replacement: "X".to_string(),
            },
            SvgEdit {
                range: 7..10,
                replacement: "Y".to_string(),
            },
        ];
        apply_edits(svg, edits);
    }
}

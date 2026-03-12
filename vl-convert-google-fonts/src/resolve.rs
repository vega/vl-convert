use crate::error::GoogleFontsError;
use crate::types::{FontStyle, ResolvedFont, VariantRequest};
use std::collections::HashSet;
use urlencoding::encode;

/// A single TTF file to download, with its URL and variant metadata.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedTtfFile {
    pub url: String,
    pub weight: u16,
    pub style: FontStyle,
}

/// The result of resolving a CSS2 response into downloadable TTF files.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedDownloadPlan {
    pub files: Vec<ResolvedTtfFile>,
}

/// Build a CSS2 API URL requesting all standard weights (100-900) × both styles.
///
/// The CSS2 API silently omits unavailable variants, so we parse what comes back.
pub(crate) fn build_css2_url_all_variants(base_url: &str, family: &str) -> String {
    let mut tuples = Vec::with_capacity(18);
    for ital in [0u8, 1u8] {
        for weight in (100..=900).step_by(100) {
            tuples.push(format!("{ital},{weight}"));
        }
    }

    format!(
        "{}?family={}:ital,wght@{}&display=swap",
        base_url.trim_end_matches('/'),
        encode(family),
        tuples.join(";")
    )
}

/// Parse a CSS2 API response into resolved fonts.
///
/// Extracts `font-weight`, `font-style`, and `src: url(...)` from `@font-face` blocks.
/// Skips variable font blocks (weight ranges like `100 900`).
///
/// Deduplicates by (weight, style): when the request uses a non-browser User-Agent
/// (ours is `vl-convert`), the CSS2 API returns complete TTF files instead of
/// WOFF2 subsets. Each unicode-range block in the response points to the same
/// TTF URL, so we only need one download per (weight, style).
pub(crate) fn parse_css2_response(css: &str) -> Result<Vec<ResolvedFont>, GoogleFontsError> {
    let mut resolved = Vec::new();
    let mut seen: HashSet<(u16, FontStyle)> = HashSet::new();

    // Split on @font-face blocks
    for block in css.split("@font-face") {
        // Find the block body between { and }
        let Some(open) = block.find('{') else {
            continue;
        };
        let Some(close) = block[open..].find('}') else {
            continue;
        };
        let body = &block[open + 1..open + close];

        // Extract font-weight — skip variable font ranges (e.g., "100 900")
        let weight = match extract_font_weight(body) {
            Some(w) => w,
            None => continue,
        };

        // Extract font-style
        let style = extract_font_style(body).unwrap_or(FontStyle::Normal);

        // Extract TTF URL from src
        let Some(url) = extract_src_url(body) else {
            continue;
        };

        // Deduplicate by (weight, style) — same URL appears for different unicode-range blocks
        if seen.insert((weight, style)) {
            resolved.push(ResolvedFont { weight, style, url });
        }
    }

    Ok(resolved)
}

/// Extract a single integer font-weight value.
/// Returns `None` for variable font weight ranges (e.g., "100 900").
fn extract_font_weight(body: &str) -> Option<u16> {
    let idx = body.find("font-weight:")?;
    let after = &body[idx + "font-weight:".len()..];
    let after = after.trim_start();

    // Read the numeric value
    let end = after
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after.len());
    if end == 0 {
        return None;
    }
    let weight_str = &after[..end];
    let weight: u16 = weight_str.parse().ok()?;

    // Check if this is a weight range (variable font) — skip if so
    let rest = after[end..].trim_start();
    if rest.starts_with(|c: char| c.is_ascii_digit()) {
        // This is a range like "100 900" — variable font, skip
        return None;
    }

    Some(weight)
}

/// Extract `font-style: normal|italic` from a `@font-face` block body.
fn extract_font_style(body: &str) -> Option<FontStyle> {
    let idx = body.find("font-style:")?;
    let after = &body[idx + "font-style:".len()..];
    let after = after.trim_start();

    if after.starts_with("italic") {
        Some(FontStyle::Italic)
    } else if after.starts_with("normal") {
        Some(FontStyle::Normal)
    } else {
        None
    }
}

/// Extract the URL from `src: url(...)` in a `@font-face` block body.
fn extract_src_url(body: &str) -> Option<String> {
    let idx = body.find("src:")?;
    let after = &body[idx + 4..];
    let after = after.trim_start();

    let url_idx = after.find("url(")?;
    let after_url = &after[url_idx + 4..];
    let after_url = after_url.trim_start();

    let (url_content, _) = if after_url.starts_with('\'') || after_url.starts_with('"') {
        let quote = after_url.as_bytes()[0] as char;
        let rest = &after_url[1..];
        let end = rest.find(quote)?;
        (&rest[..end], &rest[end + 1..])
    } else {
        let end = after_url.find(|c: char| c == ')' || c.is_whitespace())?;
        (&after_url[..end], &after_url[end..])
    };

    if url_content.is_empty() {
        return None;
    }

    Some(url_content.to_string())
}

/// Deduplicate variant requests by (weight, style).
pub(crate) fn dedupe_variants(variants: &[VariantRequest]) -> Vec<VariantRequest> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::with_capacity(variants.len());

    for variant in variants {
        if seen.insert((variant.weight, variant.style)) {
            deduped.push(variant.clone());
        }
    }

    deduped
}

/// Resolve a CSS2 response into a download plan, validating requested variants.
pub(crate) fn resolve_from_css2(
    font_id: &str,
    css: &str,
    variants: Option<&[VariantRequest]>,
) -> Result<ResolvedDownloadPlan, GoogleFontsError> {
    let resolved = parse_css2_response(css)?;

    if resolved.is_empty() {
        return Err(GoogleFontsError::FontNotFound(font_id.to_string()));
    }

    match variants {
        Some(requested) => {
            let deduped = dedupe_variants(requested);
            let mut unavailable = Vec::new();
            let mut files = Vec::new();

            for req in &deduped {
                let found = resolved
                    .iter()
                    .find(|r| r.weight == req.weight && r.style == req.style);

                match found {
                    Some(font) => {
                        files.push(ResolvedTtfFile {
                            url: font.url.clone(),
                            weight: req.weight,
                            style: req.style,
                        });
                    }
                    None => {
                        unavailable.push(req.clone());
                    }
                }
            }

            if !unavailable.is_empty() {
                return Err(GoogleFontsError::VariantsNotAvailable {
                    font_id: font_id.to_string(),
                    unavailable,
                });
            }

            Ok(ResolvedDownloadPlan { files })
        }
        None => {
            // Load all available variants
            let files: Vec<ResolvedTtfFile> = resolved
                .iter()
                .map(|r| ResolvedTtfFile {
                    url: r.url.clone(),
                    weight: r.weight,
                    style: r.style,
                })
                .collect();

            Ok(ResolvedDownloadPlan { files })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CSS: &str = r#"/* latin */
@font-face {
  font-family: 'Roboto';
  font-style: normal;
  font-weight: 400;
  font-display: swap;
  src: url(https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1Mu4mxK.ttf) format('truetype');
  unicode-range: U+0000-00FF;
}
/* latin */
@font-face {
  font-family: 'Roboto';
  font-style: normal;
  font-weight: 700;
  font-display: swap;
  src: url(https://fonts.gstatic.com/s/roboto/v30/KFOlCnqEu92Fr1MmWUlfBBc4.ttf) format('truetype');
  unicode-range: U+0000-00FF;
}
/* latin */
@font-face {
  font-family: 'Roboto';
  font-style: italic;
  font-weight: 400;
  font-display: swap;
  src: url(https://fonts.gstatic.com/s/roboto/v30/KFOkCnqEu92Fr1Mu51xIIzI.ttf) format('truetype');
  unicode-range: U+0000-00FF;
}
"#;

    #[test]
    fn test_parse_css2_response_multi_weight() {
        let fonts = parse_css2_response(SAMPLE_CSS).unwrap();
        assert_eq!(fonts.len(), 3);
        assert_eq!(fonts[0].weight, 400);
        assert_eq!(fonts[0].style, FontStyle::Normal);
        assert!(fonts[0].url.contains("KFOmCnqEu92Fr1Mu4mxK.ttf"));
        assert_eq!(fonts[1].weight, 700);
        assert_eq!(fonts[1].style, FontStyle::Normal);
        assert_eq!(fonts[2].weight, 400);
        assert_eq!(fonts[2].style, FontStyle::Italic);
    }

    #[test]
    fn test_parse_css2_response_deduplicates_unicode_ranges() {
        let css = r#"
@font-face {
  font-family: 'Roboto';
  font-style: normal;
  font-weight: 400;
  src: url(https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1Mu4mxK.ttf) format('truetype');
  unicode-range: U+0000-00FF;
}
@font-face {
  font-family: 'Roboto';
  font-style: normal;
  font-weight: 400;
  src: url(https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1Mu4mxK.ttf) format('truetype');
  unicode-range: U+0100-024F;
}
"#;
        let fonts = parse_css2_response(css).unwrap();
        assert_eq!(fonts.len(), 1);
    }

    #[test]
    fn test_parse_css2_response_skips_variable_font_blocks() {
        let css = r#"
@font-face {
  font-family: 'Roboto Flex';
  font-style: normal;
  font-weight: 100 900;
  src: url(https://fonts.gstatic.com/s/robotoflex/v1/variable.ttf) format('truetype');
}
@font-face {
  font-family: 'Roboto';
  font-style: normal;
  font-weight: 400;
  src: url(https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1Mu4mxK.ttf) format('truetype');
}
"#;
        let fonts = parse_css2_response(css).unwrap();
        assert_eq!(fonts.len(), 1);
        assert_eq!(fonts[0].weight, 400);
    }

    #[test]
    fn test_parse_css2_response_empty() {
        let fonts = parse_css2_response("/* no fonts */").unwrap();
        assert!(fonts.is_empty());
    }

    #[test]
    fn test_parse_css2_minified() {
        let css = "@font-face{font-family:'Test';font-style:normal;font-weight:400;src:url(https://fonts.gstatic.com/s/test/v1/abc.ttf) format('truetype')}";
        let fonts = parse_css2_response(css).unwrap();
        assert_eq!(fonts.len(), 1);
        assert_eq!(fonts[0].weight, 400);
    }

    #[test]
    fn test_build_css2_url_all_variants() {
        let url = build_css2_url_all_variants("https://fonts.googleapis.com/css2", "Roboto");
        assert!(url.contains("0,100;"));
        assert!(url.contains("0,900;"));
        assert!(url.contains("1,100;"));
        assert!(url.ends_with("1,900&display=swap"));
    }

    #[test]
    fn test_parse_css2_with_src_in_font_family() {
        let css = r#"
@font-face {
  font-family: 'Nosrc Test';
  font-style: normal;
  font-weight: 400;
  src: url(https://fonts.gstatic.com/s/nosrctest/v1/abc.ttf) format('truetype');
}
"#;
        let fonts = parse_css2_response(css).unwrap();
        assert_eq!(fonts.len(), 1);
        assert_eq!(fonts[0].weight, 400);
        assert!(fonts[0].url.contains("abc.ttf"));
    }

    #[test]
    fn test_resolve_from_css2_font_not_found() {
        let result = resolve_from_css2("nonexistent", "/* no fonts */", None);
        assert!(matches!(result, Err(GoogleFontsError::FontNotFound(_))));
    }

    #[test]
    fn test_resolve_from_css2_variants_not_available() {
        let requested = vec![VariantRequest {
            weight: 900,
            style: FontStyle::Italic,
        }];
        let result = resolve_from_css2("roboto", SAMPLE_CSS, Some(&requested));
        assert!(matches!(
            result,
            Err(GoogleFontsError::VariantsNotAvailable { .. })
        ));
    }

    #[test]
    fn test_resolve_from_css2_specific_variants() {
        let requested = vec![
            VariantRequest {
                weight: 400,
                style: FontStyle::Normal,
            },
            VariantRequest {
                weight: 700,
                style: FontStyle::Normal,
            },
        ];
        let plan = resolve_from_css2("roboto", SAMPLE_CSS, Some(&requested)).unwrap();
        assert_eq!(plan.files.len(), 2);
    }

    #[test]
    fn test_resolve_from_css2_all_variants() {
        let plan = resolve_from_css2("roboto", SAMPLE_CSS, None).unwrap();
        assert_eq!(plan.files.len(), 3);
    }
}

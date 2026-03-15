use crate::converter::ValueOrString;
use crate::deno_emit::{bundle, BundleOptions, BundleType, EmitOptions, SourceMapOption};
use crate::extract::{FontForHtml, FontSource};
use crate::module_loader::import_map::{DEBOUNCE_PATH, JSDELIVR_URL, VEGA_EMBED_PATH, VEGA_PATH};
use crate::module_loader::VlConvertBundleLoader;
use crate::VlVersion;
use deno_core::error::AnyError;
use std::collections::BTreeSet;
use std::path::Path;

pub fn get_vega_or_vegalite_script(
    spec: impl Into<ValueOrString>,
    opts: serde_json::Value,
) -> Result<String, AnyError> {
    let chart_id = "vega-chart";
    let spec_json = match spec.into() {
        ValueOrString::JsonString(s) => s,
        ValueOrString::Value(v) => serde_json::to_string(&v)?,
    };

    // Setup embed opts
    let opts = format!("const opts = {}", serde_json::to_string(&opts)?);

    let index_js = format!(
        r##"
{{
    const spec = {spec_json};
    {opts}
    Promise.all([...document.fonts].map(f => f.load()))
        .then(() => vegaEmbed('#{chart_id}', spec, opts))
        .catch(console.error);
}}
"##,
    );
    Ok(index_js)
}

pub async fn bundle_script(script: String, vl_version: VlVersion) -> Result<String, AnyError> {
    // Bundle dependencies
    let bundle_entry_point =
        deno_core::resolve_path("vl-convert-index.js", Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let mut loader = VlConvertBundleLoader::new(script, vl_version);
    let bundled = bundle(
        bundle_entry_point,
        &mut loader,
        BundleOptions {
            bundle_type: BundleType::Module,
            transpile_options: Default::default(),
            emit_options: EmitOptions {
                source_map: SourceMapOption::None,
                ..Default::default()
            },
            emit_ignore_directives: false,
            minify: true,
        },
    )
    .await?;
    Ok(bundled.code)
}

/// Bundle a JavaScript snippet that may contain references to vegaEmbed, vegaLite, or vega
pub async fn bundle_vega_snippet(snippet: &str, vl_version: VlVersion) -> Result<String, AnyError> {
    let script = format!(
        r#"
import vegaEmbed from "{JSDELIVR_URL}{VEGA_EMBED_PATH}.js"
import vega from "{JSDELIVR_URL}{VEGA_PATH}.js"
import vegaLite from "{JSDELIVR_URL}{VEGA_LITE_PATH}.js"
import lodashDebounce from "{JSDELIVR_URL}{DEBOUNCE_PATH}.js"
{snippet}
"#,
        VEGA_LITE_PATH = vl_version.to_path()
    );

    bundle_script(script.to_string(), vl_version).await
}

/// All standard CSS font weights, enumerated for the Google Fonts CSS2 API.
/// Using enumerated values instead of ranges (100..900) because ranges require
/// the font to be a variable font, which single-weight fonts like Lugrasimo
/// are not. Google returns only the weights a font actually supports.
const ALL_WEIGHTS: &str = "100;200;300;400;500;600;700;800;900";

/// Format (weight, style) pairs as CSS2 API axis tuples.
///
/// When specific variants are provided, requests `ital,wght@{ital},{weight}`
/// tuples. Otherwise requests all weights via `wght@100;200;...;900`.
/// The fallback omits the ital axis to avoid errors on fonts without italic.
fn format_css2_axis(variants: Option<&BTreeSet<(String, String)>>) -> String {
    match variants {
        Some(vs) if !vs.is_empty() => {
            let has_italic = vs.iter().any(|(_, s)| s == "italic");
            if has_italic {
                let mut parts: Vec<String> = vs
                    .iter()
                    .map(|(weight, style)| {
                        let ital = if style == "italic" { "1" } else { "0" };
                        format!("{ital},{weight}")
                    })
                    .collect();
                parts.sort();
                parts.dedup();
                format!("ital,wght@{}", parts.join(";"))
            } else {
                let mut weights: Vec<String> = vs.iter().map(|(w, _)| w.clone()).collect();
                weights.sort();
                weights.dedup();
                format!("wght@{}", weights.join(";"))
            }
        }
        _ => format!("wght@{ALL_WEIGHTS}"),
    }
}

/// Return the CDN stylesheet URL for a font.
///
/// When `variants` is provided, the URL requests exactly those (weight, style)
/// tuples from the Google Fonts CSS2 API. Otherwise requests all standard
/// weights (Google returns only what the font supports).
///
/// When `text` is provided, appends `&text=` so Google returns only the
/// glyphs needed for the chart — significantly smaller than full unicode-range
/// responses.
///
/// Returns `None` for local fonts.
pub fn font_cdn_url(
    font: &FontForHtml,
    variants: Option<&BTreeSet<(String, String)>>,
    text: Option<&str>,
) -> Option<String> {
    match &font.source {
        FontSource::Google { .. } => {
            let name = font.family.replace(' ', "+");
            let axis = format_css2_axis(variants);
            let mut url =
                format!("https://fonts.googleapis.com/css2?family={name}:{axis}&display=swap");
            if let Some(t) = text {
                if !t.is_empty() {
                    url.push_str("&text=");
                    url.push_str(&urlencoding::encode(t));
                }
            }
            Some(url)
        }
        FontSource::Local => None,
    }
}

/// Return an HTML `<link rel="stylesheet">` tag for a font.
/// Returns `None` for local fonts.
pub fn font_link_tag(
    font: &FontForHtml,
    variants: Option<&BTreeSet<(String, String)>>,
    text: Option<&str>,
) -> Option<String> {
    let url = font_cdn_url(font, variants, text)?;
    Some(format!(r#"<link rel="stylesheet" href="{url}">"#))
}

/// Return a CSS `@import` rule for a font.
/// Returns `None` for local fonts.
pub fn font_import_rule(
    font: &FontForHtml,
    variants: Option<&BTreeSet<(String, String)>>,
    text: Option<&str>,
) -> Option<String> {
    let url = font_cdn_url(font, variants, text)?;
    Some(format!(r#"@import url("{url}");"#))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn google_font(family: &str) -> FontForHtml {
        FontForHtml {
            family: family.to_string(),
            source: FontSource::Google {
                font_id: family.to_lowercase().replace(' ', "-"),
            },
        }
    }

    fn local_font(family: &str) -> FontForHtml {
        FontForHtml {
            family: family.to_string(),
            source: FontSource::Local,
        }
    }

    // format_css2_axis tests

    #[test]
    fn test_format_css2_axis_none() {
        assert_eq!(format_css2_axis(None), format!("wght@{ALL_WEIGHTS}"));
    }

    #[test]
    fn test_format_css2_axis_normal_only() {
        let mut vs = BTreeSet::new();
        vs.insert(("300".to_string(), "normal".to_string()));
        assert_eq!(format_css2_axis(Some(&vs)), "wght@300");
    }

    #[test]
    fn test_format_css2_axis_mixed() {
        let mut vs = BTreeSet::new();
        vs.insert(("400".to_string(), "normal".to_string()));
        vs.insert(("700".to_string(), "italic".to_string()));
        vs.insert(("300".to_string(), "normal".to_string()));
        assert_eq!(format_css2_axis(Some(&vs)), "ital,wght@0,300;0,400;1,700");
    }

    // font_cdn_url tests

    #[test]
    fn test_cdn_url_default_variants() {
        let font = google_font("Roboto");
        let url = font_cdn_url(&font, None, None).unwrap();
        assert_eq!(
            url,
            format!(
                "https://fonts.googleapis.com/css2?family=Roboto:wght@{ALL_WEIGHTS}&display=swap"
            )
        );
    }

    #[test]
    fn test_cdn_url_specific_variants() {
        let font = google_font("Roboto");
        let mut vs = BTreeSet::new();
        vs.insert(("300".to_string(), "normal".to_string()));
        vs.insert(("600".to_string(), "italic".to_string()));
        let url = font_cdn_url(&font, Some(&vs), None).unwrap();
        assert_eq!(
            url,
            "https://fonts.googleapis.com/css2?family=Roboto:ital,wght@0,300;1,600&display=swap"
        );
    }

    #[test]
    fn test_cdn_url_with_text_subset() {
        let font = google_font("Roboto");
        let url = font_cdn_url(&font, None, Some("Hello World")).unwrap();
        assert!(url.ends_with("&text=Hello%20World"));
    }

    #[test]
    fn test_cdn_url_google_font_multi_word() {
        let font = google_font("Playfair Display");
        let url = font_cdn_url(&font, None, None).unwrap();
        assert!(url.contains("family=Playfair+Display:wght@"));
    }

    #[test]
    fn test_cdn_url_local_font() {
        assert!(font_cdn_url(&local_font("Arial"), None, None).is_none());
    }
}

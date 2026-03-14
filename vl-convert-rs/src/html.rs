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
    vegaEmbed('#{chart_id}', spec, opts).catch(console.error);
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

/// Default CSS2 API axis range requesting all available weights for both
/// normal and italic.  Browsers only download the variants actually used
/// by the page, so requesting the full range has no bandwidth cost.
const DEFAULT_VARIANT_TUPLES: &str = "0,100..900;1,100..900";

/// Format (weight, style) pairs as CSS2 API `ital,wght@...` tuples.
///
/// Each tuple is `{ital},{weight}` where ital is 1 for italic, 0 otherwise.
/// Falls back to a standard set (400/700 normal/italic) when no variants
/// are provided.
fn format_variant_tuples(variants: Option<&BTreeSet<(String, String)>>) -> String {
    match variants {
        Some(vs) if !vs.is_empty() => {
            let mut parts: Vec<String> = vs
                .iter()
                .map(|(weight, style)| {
                    let ital = if style == "italic" { "1" } else { "0" };
                    format!("{ital},{weight}")
                })
                .collect();
            parts.sort();
            parts.dedup();
            parts.join(";")
        }
        _ => DEFAULT_VARIANT_TUPLES.to_string(),
    }
}

/// Return the CDN stylesheet URL for a font.
///
/// When `variants` is provided, the URL requests exactly those (weight, style)
/// tuples from the Google Fonts CSS2 API. Otherwise falls back to a standard
/// set of common weight/style tuples (400/700 normal/italic).
///
/// Returns `None` for local fonts.
pub fn font_cdn_url(
    font: &FontForHtml,
    variants: Option<&BTreeSet<(String, String)>>,
) -> Option<String> {
    match &font.source {
        FontSource::Google { .. } => {
            let name = font.family.replace(' ', "+");
            let tuples = format_variant_tuples(variants);
            Some(format!(
                "https://fonts.googleapis.com/css2?family={name}:ital,wght@{tuples}&display=swap"
            ))
        }
        FontSource::Local => None,
    }
}

/// Return an HTML `<link rel="stylesheet">` tag for a font.
/// Returns `None` for local fonts.
pub fn font_link_tag(
    font: &FontForHtml,
    variants: Option<&BTreeSet<(String, String)>>,
) -> Option<String> {
    let url = font_cdn_url(font, variants)?;
    Some(format!(r#"<link rel="stylesheet" href="{url}">"#))
}

/// Return a CSS `@import` rule for a font.
/// Returns `None` for local fonts.
pub fn font_import_rule(
    font: &FontForHtml,
    variants: Option<&BTreeSet<(String, String)>>,
) -> Option<String> {
    let url = font_cdn_url(font, variants)?;
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

    // format_variant_tuples tests

    #[test]
    fn test_format_variant_tuples_none() {
        assert_eq!(format_variant_tuples(None), DEFAULT_VARIANT_TUPLES);
    }

    #[test]
    fn test_format_variant_tuples_single() {
        let mut vs = BTreeSet::new();
        vs.insert(("300".to_string(), "normal".to_string()));
        assert_eq!(format_variant_tuples(Some(&vs)), "0,300");
    }

    #[test]
    fn test_format_variant_tuples_mixed() {
        let mut vs = BTreeSet::new();
        vs.insert(("400".to_string(), "normal".to_string()));
        vs.insert(("700".to_string(), "italic".to_string()));
        vs.insert(("300".to_string(), "normal".to_string()));
        let result = format_variant_tuples(Some(&vs));
        assert_eq!(result, "0,300;0,400;1,700");
    }

    // font_cdn_url tests

    #[test]
    fn test_cdn_url_default_variants() {
        let font = google_font("Roboto");
        let url = font_cdn_url(&font, None).unwrap();
        assert_eq!(
            url,
            "https://fonts.googleapis.com/css2?family=Roboto:ital,wght@0,100..900;1,100..900&display=swap"
        );
    }

    #[test]
    fn test_cdn_url_specific_variants() {
        let font = google_font("Roboto");
        let mut vs = BTreeSet::new();
        vs.insert(("300".to_string(), "normal".to_string()));
        vs.insert(("600".to_string(), "italic".to_string()));
        let url = font_cdn_url(&font, Some(&vs)).unwrap();
        assert_eq!(
            url,
            "https://fonts.googleapis.com/css2?family=Roboto:ital,wght@0,300;1,600&display=swap"
        );
    }

    #[test]
    fn test_cdn_url_google_font_multi_word() {
        let font = google_font("Playfair Display");
        let url = font_cdn_url(&font, None).unwrap();
        assert!(url.contains("family=Playfair+Display:ital,wght@"));
    }

    #[test]
    fn test_cdn_url_local_font() {
        assert!(font_cdn_url(&local_font("Arial"), None).is_none());
    }
}

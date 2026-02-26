use crate::converter::ValueOrString;
use crate::deno_emit::{bundle, BundleOptions, BundleType, EmitOptions, SourceMapOption};
use crate::extract::{FontForHtml, FontSource};
use crate::module_loader::import_map::{DEBOUNCE_PATH, JSDELIVR_URL, VEGA_EMBED_PATH, VEGA_PATH};
use crate::module_loader::VlConvertBundleLoader;
use crate::VlVersion;
use deno_core::error::AnyError;
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

/// Return the CDN stylesheet URL for a font.
///
/// Google fonts use the Google Fonts CSS2 API. Non-Google Fontsource fonts
/// use the Fontsource jsDelivr CDN. Returns `None` for local fonts.
pub fn font_cdn_url(font: &FontForHtml) -> Option<String> {
    match &font.source {
        FontSource::Fontsource { font_id, font_type } => {
            if font_type == "google" {
                let name = font.family.replace(' ', "+");
                let tuples = "0,400;0,700;1,400;1,700";
                Some(format!(
                    "https://fonts.googleapis.com/css2?family={name}:ital,wght@{tuples}&display=swap"
                ))
            } else {
                Some(format!(
                    "https://cdn.jsdelivr.net/fontsource/fonts/{font_id}@latest/index.css"
                ))
            }
        }
        FontSource::Local => None,
    }
}

/// Return an HTML `<link rel="stylesheet">` tag for a font.
/// Returns `None` for local fonts.
pub fn font_link_tag(font: &FontForHtml) -> Option<String> {
    let url = font_cdn_url(font)?;
    Some(format!(r#"<link rel="stylesheet" href="{url}">"#))
}

/// Return a CSS `@import` rule for a font.
/// Returns `None` for local fonts.
pub fn font_import_rule(font: &FontForHtml) -> Option<String> {
    let url = font_cdn_url(font)?;
    Some(format!(r#"@import url("{url}");"#))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn google_font(family: &str, font_id: &str) -> FontForHtml {
        FontForHtml {
            family: family.to_string(),
            source: FontSource::Fontsource {
                font_id: font_id.to_string(),
                font_type: "google".to_string(),
            },
        }
    }

    fn other_font(family: &str, font_id: &str) -> FontForHtml {
        FontForHtml {
            family: family.to_string(),
            source: FontSource::Fontsource {
                font_id: font_id.to_string(),
                font_type: "other".to_string(),
            },
        }
    }

    fn local_font(family: &str) -> FontForHtml {
        FontForHtml {
            family: family.to_string(),
            source: FontSource::Local,
        }
    }

    // font_cdn_url tests

    #[test]
    fn test_cdn_url_google_font() {
        let font = google_font("Roboto", "roboto");
        let url = font_cdn_url(&font).unwrap();
        assert_eq!(
            url,
            "https://fonts.googleapis.com/css2?family=Roboto:ital,wght@0,400;0,700;1,400;1,700&display=swap"
        );
    }

    #[test]
    fn test_cdn_url_google_font_multi_word() {
        let font = google_font("Playfair Display", "playfair-display");
        let url = font_cdn_url(&font).unwrap();
        assert!(url.contains("family=Playfair+Display:ital,wght@"));
    }

    #[test]
    fn test_cdn_url_other_fontsource() {
        let font = other_font("Custom Font", "custom-font");
        let url = font_cdn_url(&font).unwrap();
        assert_eq!(
            url,
            "https://cdn.jsdelivr.net/fontsource/fonts/custom-font@latest/index.css"
        );
    }

    #[test]
    fn test_cdn_url_local_font() {
        assert!(font_cdn_url(&local_font("Arial")).is_none());
    }

    // font_link_tag tests

    #[test]
    fn test_link_tag_google_font() {
        let font = google_font("Roboto", "roboto");
        let tag = font_link_tag(&font).unwrap();
        assert_eq!(
            tag,
            r#"<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Roboto:ital,wght@0,400;0,700;1,400;1,700&display=swap">"#
        );
    }

    #[test]
    fn test_link_tag_local_font() {
        assert!(font_link_tag(&local_font("Arial")).is_none());
    }

    // font_import_rule tests

    #[test]
    fn test_import_rule_google_font() {
        let font = google_font("Roboto", "roboto");
        let rule = font_import_rule(&font).unwrap();
        assert!(rule.starts_with("@import url(\"https://fonts.googleapis.com/css2?"));
        assert!(rule.ends_with("\");"));
    }

    #[test]
    fn test_import_rule_other_fontsource() {
        let font = other_font("Custom Font", "custom-font");
        let rule = font_import_rule(&font).unwrap();
        assert_eq!(
            rule,
            r#"@import url("https://cdn.jsdelivr.net/fontsource/fonts/custom-font@latest/index.css");"#
        );
    }

    #[test]
    fn test_import_rule_local_font() {
        assert!(font_import_rule(&local_font("Arial")).is_none());
    }
}

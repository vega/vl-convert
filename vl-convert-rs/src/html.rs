use crate::converter::ValueOrString;
use crate::deno_emit::{bundle, BundleOptions, BundleType, EmitOptions, SourceMapOption};
use crate::extract::FontForHtml;
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

/// Generate HTML `<link>` tags for loading fonts from CDN.
///
/// Google fonts use the Google Fonts CSS2 API (automatic subsetting).
/// Non-Google Fontsource fonts use the Fontsource jsDelivr CDN.
pub fn generate_font_tags(fonts: &[FontForHtml]) -> String {
    if fonts.is_empty() {
        return String::new();
    }

    let (google_fonts, other_fonts): (Vec<_>, Vec<_>) =
        fonts.iter().partition(|f| f.font_type == "google");

    let has_google = !google_fonts.is_empty();

    let mut result = Vec::new();

    // Preconnect hints for Google Fonts
    if has_google {
        result
            .push(r#"    <link rel="preconnect" href="https://fonts.googleapis.com">"#.to_string());
        result.push(
            r#"    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>"#
                .to_string(),
        );
    }

    // Google Fonts: single <link> with multiple families
    if has_google {
        let families: Vec<String> = google_fonts
            .iter()
            .map(|f| format_google_font_family(f))
            .collect();
        let url = format!(
            "https://fonts.googleapis.com/css2?{}&display=swap",
            families.join("&")
        );
        result.push(format!(r#"    <link href="{url}" rel="stylesheet">"#));
    }

    // Fontsource CDN: one <link> per font
    for font in &other_fonts {
        let url = format!(
            "https://cdn.jsdelivr.net/fontsource/fonts/{}@latest/index.css",
            font.font_id
        );
        result.push(format!(r#"    <link rel="stylesheet" href="{url}">"#));
    }

    format!("{}\n", result.join("\n"))
}

/// Format a single font family for the Google Fonts CSS2 API.
///
/// Uses weights 400 and 700 (matching default_variants) with both
/// normal and italic styles.
fn format_google_font_family(font: &FontForHtml) -> String {
    let name = font.family.replace(' ', "+");
    let tuples = "0,400;0,700;1,400;1,700";
    format!("family={name}:ital,wght@{tuples}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn google_font(family: &str, font_id: &str) -> FontForHtml {
        FontForHtml {
            family: family.to_string(),
            font_id: font_id.to_string(),
            font_type: "google".to_string(),
        }
    }

    fn other_font(family: &str, font_id: &str) -> FontForHtml {
        FontForHtml {
            family: family.to_string(),
            font_id: font_id.to_string(),
            font_type: "other".to_string(),
        }
    }

    #[test]
    fn test_generate_font_tags_empty() {
        assert_eq!(generate_font_tags(&[]), "");
    }

    #[test]
    fn test_generate_font_tags_single_google() {
        let fonts = vec![google_font("Roboto", "roboto")];
        let result = generate_font_tags(&fonts);
        assert!(result.contains(r#"rel="preconnect" href="https://fonts.googleapis.com""#));
        assert!(result.contains(r#"rel="preconnect" href="https://fonts.gstatic.com""#));
        assert!(result.contains("fonts.googleapis.com/css2?family=Roboto:ital,wght@0,400;0,700;1,400;1,700&display=swap"));
    }

    #[test]
    fn test_generate_font_tags_single_other() {
        let fonts = vec![other_font("Custom Font", "custom-font")];
        let result = generate_font_tags(&fonts);
        // No preconnect for non-Google fonts
        assert!(!result.contains("preconnect"));
        assert!(result.contains("cdn.jsdelivr.net/fontsource/fonts/custom-font@latest/index.css"));
    }

    #[test]
    fn test_generate_font_tags_mixed() {
        let fonts = vec![
            google_font("Roboto", "roboto"),
            other_font("Custom Font", "custom-font"),
        ];
        let result = generate_font_tags(&fonts);
        assert!(result.contains("preconnect"));
        assert!(result.contains("fonts.googleapis.com/css2"));
        assert!(result.contains("cdn.jsdelivr.net/fontsource/fonts/custom-font@latest/index.css"));
    }

    #[test]
    fn test_generate_font_tags_multiple_google() {
        let fonts = vec![
            google_font("Roboto", "roboto"),
            google_font("Open Sans", "open-sans"),
        ];
        let result = generate_font_tags(&fonts);
        // Both families in a single <link> tag
        assert!(result.contains("family=Roboto:ital,wght@"));
        assert!(result.contains("family=Open+Sans:ital,wght@"));
        // Only one googleapis link tag
        assert_eq!(result.matches("fonts.googleapis.com/css2").count(), 1);
    }

    #[test]
    fn test_format_google_font_family_multi_word() {
        let font = google_font("Playfair Display", "playfair-display");
        let result = format_google_font_family(&font);
        assert_eq!(
            result,
            "family=Playfair+Display:ital,wght@0,400;0,700;1,400;1,700"
        );
    }

    #[test]
    fn test_format_google_font_family_single_word() {
        let font = google_font("Roboto", "roboto");
        let result = format_google_font_family(&font);
        assert_eq!(result, "family=Roboto:ital,wght@0,400;0,700;1,400;1,700");
    }
}

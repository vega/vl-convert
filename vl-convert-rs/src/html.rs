use crate::bundler::{bundle, BundleOptions, BundleType};
use crate::module_loader::import_map::{DEBOUNCE_PATH, JSDELIVR_URL, VEGA_EMBED_PATH, VEGA_PATH};
use crate::VlVersion;
use deno_core::error::AnyError;
use std::path::Path;

pub fn get_vega_or_vegalite_script(
    spec: serde_json::Value,
    opts: serde_json::Value,
) -> Result<String, AnyError> {
    let chart_id = "vega-chart";

    // Setup embed opts
    let opts = format!("const opts = {}", serde_json::to_string(&opts)?);

    let index_js = format!(
        r##"
{{
    const spec = {SPEC};
    {opts}
    vegaEmbed('#{chart_id}', spec, opts).catch(console.error);
}}
"##,
        SPEC = serde_json::to_string(&spec)?
    );
    Ok(index_js)
}

pub async fn bundle_script(script: String, vl_version: VlVersion) -> Result<String, AnyError> {
    // Create entry point specifier
    let entry_specifier =
        deno_core::resolve_path("vl-convert-index.js", Path::new(env!("CARGO_MANIFEST_DIR")))?;

    // Bundle dependencies using our bundler module
    let bundled = bundle(
        script,
        entry_specifier,
        vl_version,
        BundleOptions {
            bundle_type: BundleType::Module,
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

use crate::module_loader::import_map::{DEBOUNCE_PATH, SKYPACK_URL, VEGA_EMBED_PATH, VEGA_PATH};
use crate::module_loader::VlConvertBundleLoader;
use crate::VlVersion;
use deno_core::error::AnyError;
use deno_emit::{bundle, BundleOptions, BundleType, EmitOptions};
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
    // Bundle dependencies
    let bundle_entry_point =
        deno_core::resolve_path("vl-convert-index.js", Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let mut loader = VlConvertBundleLoader::new(script, vl_version);
    let bundled = bundle(
        bundle_entry_point,
        &mut loader,
        None,
        BundleOptions {
            bundle_type: BundleType::Module,
            emit_options: EmitOptions {
                source_map: false,
                inline_source_map: false,
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
import vegaEmbed from "{SKYPACK_URL}{VEGA_EMBED_PATH}"
import vega from "{SKYPACK_URL}{VEGA_PATH}"
import vegaLite from "{SKYPACK_URL}{VEGA_LITE_PATH}"
import lodashDebounce from "{SKYPACK_URL}{DEBOUNCE_PATH}"
{snippet}
"#,
        VEGA_LITE_PATH = vl_version.to_path()
    );

    bundle_script(script.to_string(), vl_version).await
}

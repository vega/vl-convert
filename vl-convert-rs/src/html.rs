use crate::converter::ValueOrString;
use crate::deno_emit::{bundle, BundleOptions, BundleType, EmitOptions, SourceMapOption};
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

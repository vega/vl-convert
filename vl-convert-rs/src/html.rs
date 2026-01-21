use crate::VlVersion;
use deno_core::anyhow;
use deno_core::error::AnyError;

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

pub async fn bundle_script(_script: String, _vl_version: VlVersion) -> Result<String, AnyError> {
    // TODO: Implement bundling without deno_emit
    // deno_emit was deprecated and incompatible with newer Deno versions.
    // For now, use bundle=false in HTML export to use CDN script tags instead.
    Err(anyhow::anyhow!(
        "JavaScript bundling is temporarily disabled. Use bundle=false for HTML export \
         to load Vega libraries from CDN instead."
    ))
}

/// Bundle a JavaScript snippet that may contain references to vegaEmbed, vegaLite, or vega
pub async fn bundle_vega_snippet(snippet: &str, vl_version: VlVersion) -> Result<String, AnyError> {
    bundle_script(snippet.to_string(), vl_version).await
}

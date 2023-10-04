use crate::module_loader::import_map::{SKYPACK_URL, VEGA_EMBED_PATH};
use crate::module_loader::VlConvertBundleLoader;
use crate::VlVersion;
use deno_core::error::AnyError;
use deno_emit::{bundle, BundleOptions, BundleType, EmitOptions};
use std::path::Path;

pub fn get_vegalite_index_js(
    spec: serde_json::Value,
    opts: serde_json::Value,
    bundle: bool,
) -> Result<String, AnyError> {
    let chart_id = "vega-chart";

    // import if bundled
    let import_str = if bundle {
        format!("import vegaEmbed from \"{SKYPACK_URL}{VEGA_EMBED_PATH}\"")
    } else {
        String::new()
    };

    // Setup embed opts
    let opts = format!("const opts = {}", serde_json::to_string(&opts)?);

    let index_js = format!(
        r##"
{import_str}
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

pub fn get_vega_index_js(spec: serde_json::Value, bundle: bool) -> Result<String, AnyError> {
    let chart_id = "vega-chart";
    // import if bundled
    let import_str = if bundle {
        format!("import vegaEmbed from \"{SKYPACK_URL}{VEGA_EMBED_PATH}\"")
    } else {
        String::new()
    };

    let index_js = format!(
        r##"
{import_str}
const spec = {SPEC};
vegaEmbed('#{chart_id}', spec);
"##,
        SPEC = serde_json::to_string(&spec)?
    );
    Ok(index_js)
}

pub async fn bundle_index_js(index_js: String, vl_version: VlVersion) -> Result<String, AnyError> {
    // Bundle dependencies
    let bundle_entry_point =
        deno_core::resolve_path("vl-convert-index.js", Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    let mut loader = VlConvertBundleLoader::new(index_js, vl_version);
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
        },
    )
    .await
    .unwrap();
    Ok(bundled.code)
}

pub fn build_bundled_html(code: String) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<script type=module>{code}</script>
<head>
    <style>
        vega-chart.vega-embed {{
          width: 100%;
          display: flex;
        }}
        vega-chart.vega-embed details,
        vega-chart.vega-embed details summary {{
          position: relative;
        }}
    </style>
    <meta charset="UTF-8">
    <title>Chart</title>
</head>
<body>
    <div id="vega-chart"></div>
</body>
</html>
        "#
    )
}

pub fn build_cdn_html(code: String, vl_version: VlVersion) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
  <head>
    <style>
        vega-chart.vega-embed {{
          width: 100%;
          display: flex;
        }}
        vega-chart.vega-embed details,
        vega-chart.vega-embed details summary {{
          position: relative;
        }}
    </style>
    <title>Chart</title>
    <script src="https://cdn.jsdelivr.net/npm/vega@5"></script>
    <script src="https://cdn.jsdelivr.net/npm/vega-lite@{vl_ver}"></script>
    <script src="https://cdn.jsdelivr.net/npm/vega-embed@6"></script>
  </head>
  <body>
    <div id="vega-chart"></div>
    <script type="text/javascript">
{code}
    </script>
  </body>
</html>
        "#,
        vl_ver = vl_version.to_semver()
    )
}

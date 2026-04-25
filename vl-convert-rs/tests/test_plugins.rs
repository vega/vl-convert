mod test_utils;

use futures::executor::block_on;
use serde_json::json;
use test_utils::{check_vg_png, initialize, load_vg_spec};
use vl_convert_rs::converter::{HtmlOpts, PngOpts, Renderer, SvgOpts, VgOpts, VlcConfig};
use vl_convert_rs::VlConverter;

/// Helper: build a minimal Vega spec that renders a text mark showing
/// the result of evaluating `expr`.
fn vega_spec_with_expression(expr: &str) -> serde_json::Value {
    json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 100,
        "height": 100,
        "marks": [{
            "type": "text",
            "encode": {
                "enter": {
                    "text": {"signal": expr},
                    "x": {"value": 50},
                    "y": {"value": 50}
                }
            }
        }]
    })
}

#[test]
fn test_plugin_custom_scheme_png() {
    initialize();

    let plugin_source =
        "export default function(vega) { vega.scheme('testscheme', ['#ff0000', '#00ff00', '#0000ff']); }";

    let converter = VlConverter::with_config(VlcConfig {
        vega_plugins: vec![plugin_source.to_string()],
        ..Default::default()
    })
    .unwrap();

    let vg_spec = load_vg_spec("plugin_custom_scheme");
    let output = block_on(converter.vega_to_png(
        vg_spec,
        VgOpts::default(),
        PngOpts {
            scale: Some(2.0),
            ppi: None,
        },
    ))
    .unwrap();

    check_vg_png("plugin_custom_scheme", output.data.as_slice());
}

#[tokio::test]
async fn test_plugin_registers_expression_function() {
    let plugin_source =
        "export default function(vega) { vega.expressionFunction('double', (x) => x * 2); }";
    let converter = VlConverter::with_config(VlcConfig {
        vega_plugins: vec![plugin_source.to_string()],
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("double(7)");
    let output = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await
        .unwrap();

    assert!(output.svg.contains("<svg"), "output should be valid SVG");
    assert!(
        output.svg.contains("14"),
        "SVG should contain the result of double(7) = 14"
    );
}

#[tokio::test]
async fn test_multiple_plugins_register_different_functions() {
    let plugin_a =
        "export default function(vega) { vega.expressionFunction('triple', (x) => x * 3); }";
    let plugin_b =
        "export default function(vega) { vega.expressionFunction('addTen', (x) => x + 10); }";
    let converter = VlConverter::with_config(VlcConfig {
        vega_plugins: vec![plugin_a.to_string(), plugin_b.to_string()],
        ..Default::default()
    })
    .unwrap();

    let spec_triple = vega_spec_with_expression("triple(4)");
    let output_triple = converter
        .vega_to_svg(spec_triple, VgOpts::default(), SvgOpts::default())
        .await
        .unwrap();
    assert!(
        output_triple.svg.contains("12"),
        "SVG should contain the result of triple(4) = 12"
    );

    let spec_add = vega_spec_with_expression("addTen(7)");
    let output_add = converter
        .vega_to_svg(spec_add, VgOpts::default(), SvgOpts::default())
        .await
        .unwrap();
    assert!(
        output_add.svg.contains("17"),
        "SVG should contain the result of addTen(7) = 17"
    );
}

#[tokio::test]
async fn test_plugin_with_syntax_error() {
    let bad_plugin = "export default function(vega) { vega.expressionFunction('bad', (x) =>; }";
    let converter = VlConverter::with_config(VlcConfig {
        vega_plugins: vec![bad_plugin.to_string()],
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("1 + 1");
    let err = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Vega plugin")
            && (msg.contains("bundling failed") || msg.contains("Failed to load")),
        "error should mention plugin failure, got: {msg}"
    );
}

#[tokio::test]
async fn test_plugin_without_default_export() {
    let no_default_plugin = "export const x = 1;";
    let converter = VlConverter::with_config(VlcConfig {
        vega_plugins: vec![no_default_plugin.to_string()],
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("1 + 1");
    let err = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("does not export a default function"),
        "error should mention missing default export, got: {msg}"
    );
}

#[tokio::test]
async fn test_plugin_poison_behavior() {
    // Use a plugin that bundles fine but throws at runtime (during default(vega)).
    // This tests the poison mechanism: the first call loads the plugin and it
    // throws, poisoning the worker. The second call should get the poison error.
    let good_plugin = "export default function(vega) { vega.expressionFunction('ok', () => 42); }";
    let bad_plugin = "export default function(vega) { throw new Error('plugin init boom'); }";

    let converter = VlConverter::with_config(VlcConfig {
        vega_plugins: vec![good_plugin.to_string(), bad_plugin.to_string()],
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("1 + 1");

    // First attempt: plugin throws during init_vega, poisoning the worker
    let err1 = converter
        .vega_to_svg(spec.clone(), VgOpts::default(), SvgOpts::default())
        .await
        .unwrap_err();
    let msg1 = err1.to_string();
    assert!(
        msg1.contains("plugin init boom"),
        "first error should contain the runtime throw message, got: {msg1}"
    );

    // Second attempt: worker is poisoned, returns stored error immediately
    let err2 = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await
        .unwrap_err();
    let msg2 = err2.to_string();
    assert!(
        msg2.contains("plugin init boom"),
        "second error should be the poison sentinel with original message, got: {msg2}"
    );
}

#[tokio::test]
async fn test_plugin_html_export_contains_module_script() {
    let plugin_source =
        "export default function(vega) { vega.expressionFunction('myFn', (x) => x); }";

    let converter = VlConverter::with_config(VlcConfig {
        vega_plugins: vec![plugin_source.to_string()],
        ..Default::default()
    })
    .unwrap();

    let spec = json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 100,
        "height": 100,
        "marks": [{
            "type": "rect",
            "encode": {
                "enter": {
                    "x": {"value": 0},
                    "y": {"value": 0},
                    "width": {"value": 50},
                    "height": {"value": 50},
                    "fill": {"value": "steelblue"}
                }
            }
        }]
    });

    let output = converter
        .vega_to_html(
            spec,
            VgOpts::default(),
            HtmlOpts {
                bundle: true,
                renderer: Renderer::Svg,
            },
        )
        .await
        .unwrap();

    assert!(
        output.html.contains(r#"type="module""#),
        "HTML should use <script type=\"module\"> when plugins are present"
    );
    assert!(
        output.html.contains("__vlcLoadPlugin"),
        "HTML should contain the __vlcLoadPlugin helper for inline plugin loading"
    );
}

#[tokio::test]
async fn test_file_path_plugin() {
    // Write a plugin to a temp file and pass its path as the plugin entry.
    // Exercises the normalize_converter_config() file-reading path.
    let dir = tempfile::tempdir().unwrap();
    let plugin_path = dir.path().join("my_plugin.js");
    std::fs::write(
        &plugin_path,
        "export default function(vega) { vega.expressionFunction('fromFile', (x) => x + 100); }",
    )
    .unwrap();

    let converter = VlConverter::with_config(VlcConfig {
        vega_plugins: vec![plugin_path.to_str().unwrap().to_string()],
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("fromFile(7)");
    let output = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await
        .unwrap();

    assert!(
        output.svg.contains("107"),
        "SVG should contain fromFile(7) = 107, got: {}",
        &output.svg[..output.svg.len().min(400)]
    );
}

// These tests require internet access (esm.sh).

#[tokio::test]
async fn test_url_plugin_end_to_end() {
    // End-to-end: inline plugin with HTTP import → bundled by deno_emit
    // via PluginBundleLoader → loaded into V8 → expression function works
    // in a real Vega conversion.
    let plugin_source = r#"
import { scaleLinear } from 'https://esm.sh/d3-scale@4';

export default function(vega) {
    const s = scaleLinear().domain([0, 1]).range([0, 100]);
    vega.expressionFunction('d3scaled', (x) => s(x));
}
"#;
    let converter = VlConverter::with_config(VlcConfig {
        vega_plugins: vec![plugin_source.to_string()],
        plugin_import_domains: vec!["esm.sh".to_string()],
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("d3scaled(0.5)");
    let output = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await
        .expect("URL plugin conversion must succeed (requires network)");

    assert!(
        output.svg.contains("50"),
        "d3scaled(0.5) should produce 50 in SVG output. Got: {}",
        &output.svg[..output.svg.len().min(500)]
    );
}

#[tokio::test]
async fn test_inline_plugin_with_http_import() {
    // Tests that an inline plugin with `import ... from 'https://...'`
    // is detected as needing bundling, bundled via deno_emit with the
    // PluginBundleLoader, and then loaded+executed in V8.
    let plugin_source = r#"
import { scaleLinear } from 'https://esm.sh/d3-scale@4';

export default function(vega) {
    const s = scaleLinear().domain([0, 10]).range([0, 1000]);
    vega.expressionFunction('d3scaled10', (x) => s(x));
}
"#;
    let converter = VlConverter::with_config(VlcConfig {
        vega_plugins: vec![plugin_source.to_string()],
        plugin_import_domains: vec!["esm.sh".to_string()],
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("d3scaled10(5)");
    let output = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await
        .expect("Inline plugin with HTTP import must succeed (requires network)");

    assert!(
        output.svg.contains("500"),
        "d3scaled10(5) should produce 500 in SVG output. Got: {}",
        &output.svg[..output.svg.len().min(500)]
    );
}

#[tokio::test]
async fn test_per_request_plugin_works() {
    let plugin_source =
        "export default function(vega) { vega.expressionFunction('perReq', (x) => x + 99); }";

    let converter = VlConverter::with_config(VlcConfig {
        allow_per_request_plugins: true,
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("perReq(1)");
    let output = converter
        .vega_to_svg(
            spec,
            VgOpts {
                vega_plugin: Some(plugin_source.to_string()),
                ..Default::default()
            },
            SvgOpts::default(),
        )
        .await
        .unwrap();

    assert!(
        output.svg.contains("100"),
        "SVG should contain perReq(1) = 100. Got: {}",
        &output.svg[..output.svg.len().min(400)]
    );
}

#[tokio::test]
async fn test_per_request_plugin_isolation() {
    // First request uses a per-request plugin that registers 'ephExpr'
    let plugin_source =
        "export default function(vega) { vega.expressionFunction('ephExpr', () => 777); }";

    let converter = VlConverter::with_config(VlcConfig {
        allow_per_request_plugins: true,
        ..Default::default()
    })
    .unwrap();

    // First conversion with plugin — should succeed
    let spec = vega_spec_with_expression("ephExpr()");
    let output = converter
        .vega_to_svg(
            spec.clone(),
            VgOpts {
                vega_plugin: Some(plugin_source.to_string()),
                ..Default::default()
            },
            SvgOpts::default(),
        )
        .await
        .unwrap();
    assert!(output.svg.contains("777"), "ephExpr() should produce 777");

    // Second conversion WITHOUT plugin — 'ephExpr' should NOT be available
    // (true isolation: the ephemeral worker was dropped)
    let result = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await;
    // The conversion should either fail (unknown expression) or produce no "777"
    match result {
        Ok(output) => {
            assert!(
                !output.svg.contains("777"),
                "ephExpr should NOT be available on the main pool worker"
            );
        }
        Err(_) => {
            // Error is also acceptable — the expression function doesn't exist
        }
    }
}

#[tokio::test]
async fn test_per_request_plugin_disabled_by_default() {
    let converter = VlConverter::with_config(VlcConfig::default()).unwrap();

    let spec = vega_spec_with_expression("1 + 1");
    let err = converter
        .vega_to_svg(
            spec,
            VgOpts {
                vega_plugin: Some("export default function(vega) {}".to_string()),
                ..Default::default()
            },
            SvgOpts::default(),
        )
        .await
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("allow_per_request_plugins"),
        "Error should mention allow_per_request_plugins, got: {msg}"
    );
}

#[tokio::test]
async fn test_per_request_plugin_with_config_level_plugins() {
    // Config-level plugin registers 'configFn', per-request registers 'reqFn'
    let config_plugin =
        "export default function(vega) { vega.expressionFunction('configFn', (x) => x * 3); }";
    let request_plugin =
        "export default function(vega) { vega.expressionFunction('reqFn', (x) => x + 50); }";

    let converter = VlConverter::with_config(VlcConfig {
        vega_plugins: vec![config_plugin.to_string()],
        allow_per_request_plugins: true,
        ..Default::default()
    })
    .unwrap();

    // Use reqFn in the spec — both config and request plugins should be active
    let spec = vega_spec_with_expression("reqFn(configFn(4))");
    let output = converter
        .vega_to_svg(
            spec,
            VgOpts {
                vega_plugin: Some(request_plugin.to_string()),
                ..Default::default()
            },
            SvgOpts::default(),
        )
        .await
        .unwrap();

    // configFn(4) = 12, reqFn(12) = 62
    assert!(
        output.svg.contains("62"),
        "SVG should contain reqFn(configFn(4)) = 62. Got: {}",
        &output.svg[..output.svg.len().min(400)]
    );
}

#[tokio::test]
async fn test_per_request_plugin_html_export() {
    let plugin_source =
        "export default function(vega) { vega.expressionFunction('htmlReq', (x) => x); }";

    let converter = VlConverter::with_config(VlcConfig {
        allow_per_request_plugins: true,
        ..Default::default()
    })
    .unwrap();

    let spec = json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 50, "height": 50,
        "marks": [{"type": "rect", "encode": {"enter": {
            "x": {"value": 0}, "y": {"value": 0},
            "width": {"value": 10}, "height": {"value": 10},
            "fill": {"value": "red"}
        }}}]
    });

    let output = converter
        .vega_to_html(
            spec,
            VgOpts {
                vega_plugin: Some(plugin_source.to_string()),
                ..Default::default()
            },
            HtmlOpts {
                bundle: true,
                renderer: Renderer::Svg,
            },
        )
        .await
        .unwrap();

    assert!(
        output.html.contains("htmlReq"),
        "HTML should contain the per-request plugin source"
    );
    assert!(
        output.html.contains(r#"type="module""#),
        "HTML should use module script when plugins present"
    );
}

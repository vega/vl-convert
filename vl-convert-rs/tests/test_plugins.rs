mod test_utils;

use futures::executor::block_on;
use serde_json::json;
use test_utils::{check_vg_png, initialize, load_vg_spec};
use vl_convert_rs::converter::{Renderer, VgOpts, VlConverterConfig};
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

// --- PNG baseline test ---

#[test]
fn test_plugin_custom_scheme_png() {
    initialize();

    let plugin_source =
        "export default function(vega) { vega.scheme('testscheme', ['#ff0000', '#00ff00', '#0000ff']); }";

    let converter = VlConverter::with_config(VlConverterConfig {
        vega_plugins: Some(vec![plugin_source.to_string()]),
        ..Default::default()
    })
    .unwrap();

    let vg_spec = load_vg_spec("plugin_custom_scheme");
    let png_data =
        block_on(converter.vega_to_png(vg_spec, VgOpts::default(), Some(2.0), None)).unwrap();

    check_vg_png("plugin_custom_scheme", png_data.as_slice());
}

// --- Expression function tests ---

#[tokio::test]
async fn test_plugin_registers_expression_function() {
    let plugin_source =
        "export default function(vega) { vega.expressionFunction('double', (x) => x * 2); }";
    let converter = VlConverter::with_config(VlConverterConfig {
        vega_plugins: Some(vec![plugin_source.to_string()]),
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("double(5)");
    let svg = converter
        .vega_to_svg(spec, VgOpts::default())
        .await
        .unwrap();

    assert!(svg.contains("<svg"), "output should be valid SVG");
    assert!(
        svg.contains("10"),
        "SVG should contain the result of double(5) = 10"
    );
}

#[tokio::test]
async fn test_multiple_plugins_register_different_functions() {
    let plugin_a =
        "export default function(vega) { vega.expressionFunction('triple', (x) => x * 3); }";
    let plugin_b =
        "export default function(vega) { vega.expressionFunction('addTen', (x) => x + 10); }";
    let converter = VlConverter::with_config(VlConverterConfig {
        vega_plugins: Some(vec![plugin_a.to_string(), plugin_b.to_string()]),
        ..Default::default()
    })
    .unwrap();

    let spec_triple = vega_spec_with_expression("triple(4)");
    let svg_triple = converter
        .vega_to_svg(spec_triple, VgOpts::default())
        .await
        .unwrap();
    assert!(
        svg_triple.contains("12"),
        "SVG should contain the result of triple(4) = 12"
    );

    let spec_add = vega_spec_with_expression("addTen(7)");
    let svg_add = converter
        .vega_to_svg(spec_add, VgOpts::default())
        .await
        .unwrap();
    assert!(
        svg_add.contains("17"),
        "SVG should contain the result of addTen(7) = 17"
    );
}

// --- Error handling tests ---

#[tokio::test]
async fn test_plugin_with_syntax_error() {
    let bad_plugin = "export default function(vega) { vega.expressionFunction('bad', (x) =>; }";
    let converter = VlConverter::with_config(VlConverterConfig {
        vega_plugins: Some(vec![bad_plugin.to_string()]),
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("1 + 1");
    let err = converter
        .vega_to_svg(spec, VgOpts::default())
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Failed to load Vega plugin"),
        "error should mention plugin loading failure, got: {msg}"
    );
}

#[tokio::test]
async fn test_plugin_without_default_export() {
    let no_default_plugin = "export const x = 1;";
    let converter = VlConverter::with_config(VlConverterConfig {
        vega_plugins: Some(vec![no_default_plugin.to_string()]),
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("1 + 1");
    let err = converter
        .vega_to_svg(spec, VgOpts::default())
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
    let good_plugin = "export default function(vega) { vega.expressionFunction('ok', () => 42); }";
    let bad_plugin = "export default function(vega) { vega.expressionFunction('bad', (x) =>; }";

    let converter = VlConverter::with_config(VlConverterConfig {
        vega_plugins: Some(vec![good_plugin.to_string(), bad_plugin.to_string()]),
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("1 + 1");

    // First attempt should fail due to the bad plugin
    let err1 = converter
        .vega_to_svg(spec.clone(), VgOpts::default())
        .await
        .unwrap_err();
    let msg1 = err1.to_string();
    assert!(
        msg1.contains("Failed to load Vega plugin"),
        "first error should be plugin loading failure, got: {msg1}"
    );

    // Second attempt should return the poison error, not retry
    let err2 = converter
        .vega_to_svg(spec, VgOpts::default())
        .await
        .unwrap_err();
    let msg2 = err2.to_string();
    assert!(
        msg2.contains("poisoned") || msg2.contains("Failed to load Vega plugin"),
        "second error should be poison or plugin failure, got: {msg2}"
    );
}

// --- HTML export tests ---

#[tokio::test]
async fn test_plugin_html_export_contains_module_script() {
    let plugin_source =
        "export default function(vega) { vega.expressionFunction('myFn', (x) => x); }";

    let converter = VlConverter::with_config(VlConverterConfig {
        vega_plugins: Some(vec![plugin_source.to_string()]),
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

    let html = converter
        .vega_to_html(spec, VgOpts::default(), true, false, true, Renderer::Svg)
        .await
        .unwrap();

    assert!(
        html.contains(r#"type="module""#),
        "HTML should use <script type=\"module\"> when plugins are present"
    );
    assert!(
        html.contains("__vlcLoadPlugin"),
        "HTML should contain the __vlcLoadPlugin helper for inline plugin loading"
    );
}

// --- File-path plugin tests ---

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

    let converter = VlConverter::with_config(VlConverterConfig {
        vega_plugins: Some(vec![plugin_path.to_str().unwrap().to_string()]),
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("fromFile(7)");
    let svg = converter
        .vega_to_svg(spec, VgOpts::default())
        .await
        .unwrap();

    assert!(
        svg.contains("107"),
        "SVG should contain fromFile(7) = 107, got: {}",
        &svg[..svg.len().min(400)]
    );
}

// --- Network-dependent tests ---
//
// These tests require internet access (esm.sh). They run in CI.

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
    let converter = VlConverter::with_config(VlConverterConfig {
        vega_plugins: Some(vec![plugin_source.to_string()]),
        allowed_plugin_import_domains: vec!["esm.sh".to_string()],
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("d3scaled(0.5)");
    let svg = converter
        .vega_to_svg(spec, VgOpts::default())
        .await
        .expect("URL plugin conversion must succeed (requires network)");

    assert!(
        svg.contains("50"),
        "d3scaled(0.5) should produce 50 in SVG output. Got: {}",
        &svg[..svg.len().min(500)]
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
    let converter = VlConverter::with_config(VlConverterConfig {
        vega_plugins: Some(vec![plugin_source.to_string()]),
        allowed_plugin_import_domains: vec!["esm.sh".to_string()],
        ..Default::default()
    })
    .unwrap();

    let spec = vega_spec_with_expression("d3scaled10(5)");
    let svg = converter
        .vega_to_svg(spec, VgOpts::default())
        .await
        .expect("Inline plugin with HTTP import must succeed (requires network)");

    assert!(
        svg.contains("500"),
        "d3scaled10(5) should produce 500 in SVG output. Got: {}",
        &svg[..svg.len().min(500)]
    );
}

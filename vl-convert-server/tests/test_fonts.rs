mod common;

use common::*;
use once_cell::sync::Lazy;
use serde_json::json;
use vl_convert_rs::converter::VlcConfig;

static FONT_SERVER: Lazy<ServerHandle> = Lazy::new(|| {
    let config = VlcConfig {
        embed_local_fonts: true,
        ..VlcConfig::default()
    };
    start_server_sync(config, default_serve_config())
});

fn font_vl_spec() -> serde_json::Value {
    json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "title": {"text": "Font Probe", "font": "Liberation Sans"},
        "data": {"values": [{"label": "A"}]},
        "mark": {"type": "text", "font": "Liberation Sans"},
        "encoding": {"text": {"field": "label"}}
    })
}

fn font_vg_spec() -> serde_json::Value {
    json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 100,
        "height": 100,
        "title": {"text": "Font Probe", "font": "Liberation Sans"},
        "data": [{"name": "values", "values": [{"label": "A"}]}],
        "marks": [{
            "type": "text",
            "from": {"data": "values"},
            "encode": {"enter": {
                "text": {"field": "label"},
                "font": {"value": "Liberation Sans"},
                "x": {"value": 10},
                "y": {"value": 20}
            }}
        }]
    })
}

fn assert_liberation_sans(body: serde_json::Value) {
    let fonts = body.as_array().expect("fonts response must be an array");
    let info = fonts
        .iter()
        .find(|font| font["name"] == "Liberation Sans")
        .unwrap_or_else(|| panic!("expected Liberation Sans in fonts response: {body}"));

    assert_eq!(info["source"]["type"], "local");
    let variants = info["variants"]
        .as_array()
        .expect("FontInfo.variants must be an array");
    assert!(
        !variants.is_empty(),
        "Liberation Sans should report at least one used variant: {info}"
    );
}

#[tokio::test]
async fn test_vegalite_fonts() {
    let server = &*FONT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/fonts", server.base_url))
        .json(&json!({"spec": font_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_liberation_sans(body);
}

#[tokio::test]
async fn test_vega_fonts() {
    let server = &*FONT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/fonts", server.base_url))
        .json(&json!({"spec": font_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_liberation_sans(body);
}

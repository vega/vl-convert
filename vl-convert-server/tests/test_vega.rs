mod common;

use common::*;
use serde_json;

#[tokio::test]
async fn test_vg_to_svg() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec(), "bundle": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.starts_with("<svg"),
        "Expected SVG, got: {}",
        &body[..body.len().min(100)]
    );
}

#[tokio::test]
async fn test_vg_to_png() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/png", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G'], "bad PNG magic");
}

#[tokio::test]
async fn test_vg_to_jpeg() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/jpeg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(&bytes[..2], &[0xFF, 0xD8], "bad JPEG magic");
}

#[tokio::test]
async fn test_vg_to_pdf() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/pdf", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.starts_with(b"%PDF"), "expected PDF magic");
}

#[tokio::test]
async fn test_vg_to_html() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/html", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("<html") || body.contains("<!DOCTYPE"),
        "expected HTML content"
    );
}

#[tokio::test]
async fn test_vg_to_url() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/url", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.starts_with("https://vega.github.io/editor/"),
        "expected Vega Editor URL, got: {}",
        &body[..body.len().min(80)]
    );
}

#[tokio::test]
async fn test_vg_to_scenegraph_json() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/scenegraph", server.base_url))
        .header("Accept", "application/json")
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("application/json"));
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_object(), "expected JSON scenegraph object");
}

#[tokio::test]
async fn test_vg_to_scenegraph_msgpack() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/scenegraph", server.base_url))
        .header("Accept", "application/msgpack")
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/msgpack"
    );
    let body = resp.bytes().await.unwrap();
    assert!(!body.is_empty(), "expected non-empty msgpack bytes");
}

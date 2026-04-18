mod common;

use common::*;
use serde_json;

#[tokio::test]
async fn test_vl_to_vega() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/vega", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body.get("$schema").is_some() || body.get("marks").is_some(),
        "expected Vega spec output"
    );
}

#[tokio::test]
async fn test_vl_to_svg() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "bundle": false}))
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
async fn test_vl_to_png() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/png", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("image/png"), "expected image/png, got: {ct}");
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.len() > 100, "PNG too small: {} bytes", bytes.len());
    assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G'], "bad PNG magic");
}

#[tokio::test]
async fn test_vl_to_jpeg() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/jpeg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("image/jpeg"), "expected image/jpeg, got: {ct}");
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.len() > 100, "JPEG too small: {} bytes", bytes.len());
    assert_eq!(&bytes[..2], &[0xFF, 0xD8], "bad JPEG magic");
}

#[tokio::test]
async fn test_vl_to_pdf() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/pdf", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("application/pdf"),
        "expected application/pdf, got: {ct}"
    );
    let bytes = resp.bytes().await.unwrap();
    assert!(
        bytes.starts_with(b"%PDF"),
        "expected PDF magic, got: {:?}",
        &bytes[..bytes.len().min(10)]
    );
}

#[tokio::test]
async fn test_vl_to_html() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/html", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
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
async fn test_vl_to_url() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/url", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
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
async fn test_vl_to_scenegraph_json() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/scenegraph", server.base_url))
        .header("Accept", "application/json")
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
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
async fn test_vl_to_scenegraph_msgpack() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/scenegraph", server.base_url))
        .header("Accept", "application/msgpack")
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
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

#[tokio::test]
async fn test_vl_to_scenegraph_prefers_json_when_msgpack_has_lower_quality() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/scenegraph", server.base_url))
        .header("Accept", "application/json, application/msgpack;q=0.1")
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
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
}

#[tokio::test]
async fn test_vl_to_scenegraph_prefers_msgpack_when_json_has_lower_quality() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/scenegraph", server.base_url))
        .header("Accept", "application/json;q=0.1, application/msgpack")
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
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
}

#[tokio::test]
async fn test_vl_to_scenegraph_x_msgpack_accept() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/scenegraph", server.base_url))
        .header("Accept", "application/x-msgpack")
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
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
}

#[tokio::test]
async fn test_vl_to_scenegraph_default_is_json() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/scenegraph", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
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
}

mod common;

use common::*;
#[tokio::test]
async fn test_png_scale() {
    let server = &*DEFAULT_SERVER;
    // Default scale (1.0)
    let resp1 = server
        .client
        .post(format!("{}/vegalite/png", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    let bytes1 = resp1.bytes().await.unwrap();

    // Scale 2.0 should produce larger PNG
    let resp2 = server
        .client
        .post(format!("{}/vegalite/png", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "scale": 2.0}))
        .send()
        .await
        .unwrap();
    let bytes2 = resp2.bytes().await.unwrap();

    assert!(
        bytes2.len() > bytes1.len(),
        "scale 2.0 should produce larger PNG: {} vs {}",
        bytes2.len(),
        bytes1.len()
    );
}

#[tokio::test]
async fn test_jpeg_quality() {
    let server = &*DEFAULT_SERVER;
    let resp_low = server
        .client
        .post(format!("{}/vegalite/jpeg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "quality": 1}))
        .send()
        .await
        .unwrap();
    let bytes_low = resp_low.bytes().await.unwrap();

    let resp_high = server
        .client
        .post(format!("{}/vegalite/jpeg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "quality": 100}))
        .send()
        .await
        .unwrap();
    let bytes_high = resp_high.bytes().await.unwrap();

    assert!(
        bytes_high.len() > bytes_low.len(),
        "quality 100 should be larger than quality 1: {} vs {}",
        bytes_high.len(),
        bytes_low.len()
    );
}

#[tokio::test]
async fn test_html_renderer_canvas() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/html", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "renderer": "canvas"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("canvas"),
        "expected 'canvas' renderer in HTML output"
    );
}

#[tokio::test]
async fn test_url_fullscreen() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/url", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "fullscreen": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("/view"),
        "expected /view in fullscreen URL, got: {}",
        &body[..body.len().min(100)]
    );
}

#[tokio::test]
async fn test_width_override() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "width": 500}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.starts_with("<svg"), "expected SVG output");
    // Width override should affect the SVG dimensions
    assert!(body.contains("500"), "expected width 500 in SVG output");
}

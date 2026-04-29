mod common;

use common::*;

#[tokio::test]
async fn test_bundle_default_version() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/bundling/bundle", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        ct.contains("javascript"),
        "expected javascript content type, got {ct}"
    );
    let body = resp.text().await.unwrap();
    assert!(body.contains("vega"), "expected vega in bundle");
    assert!(
        body.len() > 1000,
        "bundle seems too small: {} bytes",
        body.len()
    );
}

#[tokio::test]
async fn test_bundle_explicit_version() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!(
            "{}/bundling/bundle?vl_version=5.8",
            server.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("vega"), "expected vega in bundle");
}

#[tokio::test]
async fn test_bundle_invalid_version() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!(
            "{}/bundling/bundle?vl_version=99.99",
            server.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_bundle_snippet() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/bundling/bundle-snippet", server.base_url))
        .json(&serde_json::json!({
            "snippet": "window.vega = vega;"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        ct.contains("javascript"),
        "expected javascript content type, got {ct}"
    );
    let body = resp.text().await.unwrap();
    assert!(!body.is_empty(), "expected non-empty bundled snippet");
}

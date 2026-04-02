mod common;

use common::*;
use serde_json;

#[tokio::test]
async fn test_invalid_vl_version() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "vl_version": "99.99"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "expected 400 for invalid vl_version, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_missing_spec() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"bundle": false}))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "expected client error for missing spec, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_unknown_field_rejected() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "bogus_field": 123}))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "expected client error for unknown field, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_invalid_renderer() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/html", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "renderer": "invalid_renderer"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_google_fonts_blocked() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "google_fonts": ["Roboto"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "google_fonts should be rejected when allow_google_fonts is false"
    );
}

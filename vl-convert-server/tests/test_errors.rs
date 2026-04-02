mod common;

use common::*;
use serde_json;

#[tokio::test]
async fn test_vl_invalid_spec_422() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": {"invalid": true}}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 422);
}

#[tokio::test]
async fn test_vg_invalid_spec_422() {
    let server = &*DEFAULT_SERVER;
    // Use a non-object spec value to trigger a conversion error
    let resp = server
        .client
        .post(format!("{}/vega/svg", server.base_url))
        .json(&serde_json::json!({"spec": "not an object"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 422);
}

#[tokio::test]
async fn test_svg_invalid_svg_422() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/svg/png", server.base_url))
        .json(&serde_json::json!({"svg": "not valid svg at all"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 422);
}

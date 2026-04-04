mod common;

use common::*;

#[tokio::test]
async fn test_healthz() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/healthz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_readyz() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/readyz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn test_infoz() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/infoz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["version"].is_string(), "expected version string");
    assert!(
        body["vegalite_versions"].is_array(),
        "expected vegalite_versions array"
    );
}

mod common;

use common::*;
use serde_json;

#[tokio::test]
async fn test_auth_healthz_no_key_needed() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .get(format!("{}/healthz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "health endpoints should be accessible without auth"
    );
}

#[tokio::test]
async fn test_auth_readyz_no_key_needed() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .get(format!("{}/readyz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "readyz should be accessible without auth"
    );
}

#[tokio::test]
async fn test_auth_api_rejected_without_key() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401, "expected 401 without API key");
    let www_auth = resp.headers().get("www-authenticate");
    assert!(
        www_auth.is_some(),
        "expected WWW-Authenticate header on 401"
    );
}

#[tokio::test]
async fn test_auth_api_rejected_wrong_key() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes", server.base_url))
        .header("authorization", "Bearer wrong-key")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401, "expected 401 with wrong API key");
}

#[tokio::test]
async fn test_auth_api_accepted_with_correct_key() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes", server.base_url))
        .header("authorization", "Bearer test-secret")
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "expected 200 with correct API key, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_auth_post_endpoint_with_key() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .header("authorization", "Bearer test-secret")
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "expected 200 for authenticated POST, got: {}",
        resp.status()
    );
}

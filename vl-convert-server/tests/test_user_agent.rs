mod common;

use common::*;
use serde_json;

#[tokio::test]
async fn test_ua_healthz_no_ua_needed() {
    let server = &*UA_SERVER;
    let resp = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("{}/healthz", server.base_url))
        .header("user-agent", "")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "health endpoints should not require UA");
}

#[tokio::test]
async fn test_ua_api_rejected_without_ua() {
    let server = &*UA_SERVER;
    let no_ua_client = reqwest::Client::builder().no_proxy().build().unwrap();
    let resp = no_ua_client
        .get(format!("{}/themes", server.base_url))
        .header("user-agent", "")
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "expected 400 without User-Agent, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_ua_api_accepted_with_ua() {
    let server = &*UA_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes", server.base_url))
        .header("user-agent", "my-test-agent/1.0")
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "expected 200 with User-Agent, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_ua_post_accepted_with_ua() {
    let server = &*UA_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .header("user-agent", "my-test-agent/1.0")
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "expected 200 for POST with User-Agent, got: {}",
        resp.status()
    );
}

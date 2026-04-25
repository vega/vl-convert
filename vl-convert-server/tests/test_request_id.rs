mod common;

use common::*;

#[tokio::test]
async fn test_request_id_generated() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/healthz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let req_id = resp.headers().get("x-request-id");
    assert!(
        req_id.is_some(),
        "expected x-request-id header to be generated"
    );
    let id_str = req_id.unwrap().to_str().unwrap();
    assert!(!id_str.is_empty(), "x-request-id should not be empty");
}

#[tokio::test]
async fn test_request_id_propagated() {
    let server = &*DEFAULT_SERVER;
    let custom_id = "my-custom-trace-id-12345";
    let resp = server
        .client
        .get(format!("{}/healthz", server.base_url))
        .header("x-request-id", custom_id)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let req_id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        req_id, custom_id,
        "expected propagated x-request-id to match"
    );
}

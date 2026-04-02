mod common;

use common::*;
use serde_json;

#[tokio::test]
async fn test_opaque_errors_hide_details() {
    let server = &*OPAQUE_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "vl_version": "99.99"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body = resp.text().await.unwrap();
    assert!(
        body.is_empty(),
        "expected empty body for opaque error, got: {body}"
    );
}

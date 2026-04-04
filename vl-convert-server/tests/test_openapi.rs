mod common;

use common::*;

#[tokio::test]
async fn test_openapi_json() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/api-doc/openapi.json", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["openapi"].is_string(),
        "expected openapi version field"
    );
    assert!(body["paths"].is_object(), "expected paths object");
}

#[tokio::test]
async fn test_swagger_ui() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/docs/", server.base_url))
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
        ct.contains("text/html"),
        "expected HTML content type for swagger UI, got: {ct}"
    );
}

mod common;

use common::*;
use once_cell::sync::Lazy;

static SMALL_BODY_SERVER: Lazy<TestServer> = Lazy::new(|| {
    let config = vl_convert_rs::converter::VlcConfig::default();
    let mut serve_config = default_serve_config();
    serve_config.max_body_size_mb = 1; // 1MB limit
    start_server_sync(config, serve_config)
});

#[tokio::test]
async fn test_body_too_large() {
    let server = &*SMALL_BODY_SERVER;
    // Create a body > 1MB
    let big_data: String = "x".repeat(1_100_000);
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .header("content-type", "application/json")
        .body(format!(r#"{{"spec": {{"data": "{big_data}"}}}}"#))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "expected client error for oversized body, got: {}",
        resp.status()
    );
}

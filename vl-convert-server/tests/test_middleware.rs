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

    let big_values: Vec<serde_json::Value> = (0..50_000)
        .map(|i| serde_json::json!({"a": i, "b": format!("padding_{:0>20}", i)}))
        .collect();
    let payload = serde_json::json!({
        "spec": {
            "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
            "data": {"values": big_values},
            "mark": "bar",
            "encoding": {"x": {"field": "a"}, "y": {"field": "b"}}
        }
    });

    let serialized = serde_json::to_string(&payload).unwrap();
    assert!(
        serialized.len() > 1_048_576,
        "payload must exceed 1MB limit"
    );

    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .header("content-type", "application/json")
        .body(serialized)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        413,
        "expected 413 Payload Too Large, got: {}",
        resp.status()
    );
}

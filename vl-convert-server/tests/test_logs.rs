mod common;

use common::*;
use serde_json;

#[tokio::test]
async fn test_vlc_logs_header_on_conversion() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let logs_header = resp.headers().get("x-vlc-logs");
    assert!(
        logs_header.is_some(),
        "expected x-vlc-logs header on conversion response"
    );
    let logs_str = logs_header.unwrap().to_str().unwrap();
    let _logs: Vec<String> = serde_json::from_str(logs_str)
        .unwrap_or_else(|e| panic!("x-vlc-logs should be valid JSON array: {e}, got: {logs_str}"));
}

#[tokio::test]
async fn test_vlc_logs_header_on_themes() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let logs_header = resp.headers().get("x-vlc-logs");
    assert!(
        logs_header.is_some(),
        "expected x-vlc-logs header on themes response"
    );
}

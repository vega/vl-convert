mod common;

use common::*;
use serde_json;

#[tokio::test]
async fn test_background_override() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({
            "spec": simple_vl_spec(),
            "background": "#ff0000"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.starts_with("<svg"), "expected SVG output");
    assert!(
        body.contains("fill=\"#ff0000\""),
        "expected background fill color #ff0000 in SVG output"
    );
}

mod common;

use common::*;

#[tokio::test]
async fn test_vegalite_fonts() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/fonts", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_array(), "expected array of FontInfo");
}

#[tokio::test]
async fn test_vega_fonts() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/fonts", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_array(), "expected array of FontInfo");
}

#[tokio::test]
async fn test_vegalite_fonts_with_font_face() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/fonts", server.base_url))
        .json(&serde_json::json!({
            "spec": simple_vl_spec(),
            "include_font_face": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_array(), "expected array of FontInfo");
}

mod common;

use common::*;

#[tokio::test]
async fn test_list_themes() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let arr = body.as_array().expect("expected array of theme names");
    assert!(!arr.is_empty(), "expected at least one theme");
    assert!(
        arr.iter().any(|v| v.as_str() == Some("dark")),
        "expected 'dark' theme in list"
    );
}

#[tokio::test]
async fn test_get_theme_dark() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes/dark", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_object(), "expected theme config object");
}

#[tokio::test]
async fn test_get_theme_not_found() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes/nonexistent_theme_xyz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

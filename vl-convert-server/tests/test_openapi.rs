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

#[tokio::test]
async fn test_main_openapi_excludes_admin_paths() {
    // The main `/api-doc/openapi.json` must NOT leak any /admin/* path.
    // Admin is a separate surface with its own spec at `/admin/api-doc/openapi.json`.
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/api-doc/openapi.json", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let paths = body["paths"].as_object().expect("paths must be an object");
    let admin_paths: Vec<&String> = paths.keys().filter(|k| k.starts_with("/admin")).collect();
    assert!(
        admin_paths.is_empty(),
        "main /api-doc/openapi.json leaked admin paths: {admin_paths:?}"
    );
}

#[tokio::test]
async fn test_admin_openapi_includes_admin_paths() {
    // The admin spec at `/admin/api-doc/openapi.json` must include every
    // admin endpoint. Spawns a server with an admin listener so the spec is
    // reachable.
    let server = start_admin_config_server(
        vl_convert_rs::converter::VlcConfig::default(),
        default_serve_config(),
    );
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/admin/api-doc/openapi.json", server.admin_base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let paths = body["paths"].as_object().expect("paths must be an object");

    // Assert each of the expected admin paths appears. We don't assert
    // method counts because utoipa-axum groups GET/PATCH/PUT/DELETE on
    // the same path under one object with method-keyed operations.
    let expected = [
        "/admin/budget",
        "/admin/config",
        "/admin/config/fonts/directories",
    ];
    for path in expected {
        assert!(
            paths.contains_key(path),
            "admin spec missing expected path {path}; got keys: {:?}",
            paths.keys().collect::<Vec<_>>()
        );
    }

    // Negative: admin spec should NOT include main-surface paths.
    let has_vegalite = paths.keys().any(|k| k.starts_with("/vegalite"));
    assert!(
        !has_vegalite,
        "admin spec leaked /vegalite/* paths — should only contain /admin/*"
    );
}

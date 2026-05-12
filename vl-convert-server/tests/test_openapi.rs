mod common;

use common::*;
use serde_json::Value;

#[tokio::test]
async fn test_public_docs_available() {
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
async fn test_public_openapi_json_responses_have_schemas_and_resolved_refs() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/api-doc/openapi.json", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();

    let paths = body["paths"].as_object().expect("paths must be an object");
    let mut missing_response_schemas = Vec::new();
    for (path, path_item) in paths {
        let methods = path_item.as_object().expect("path item must be an object");
        for (method, operation) in methods {
            let responses = operation["responses"]
                .as_object()
                .expect("responses must be an object");
            for (status, response) in responses {
                let Some(content) = response.get("content").and_then(Value::as_object) else {
                    continue;
                };
                for (content_type, media_type) in content {
                    if content_type.starts_with("application/json")
                        && media_type.get("schema").is_none()
                    {
                        missing_response_schemas.push(format!(
                            "{} {} response {} {}",
                            method.to_uppercase(),
                            path,
                            status,
                            content_type
                        ));
                    }
                }
            }
        }
    }
    assert!(
        missing_response_schemas.is_empty(),
        "JSON responses missing schemas: {missing_response_schemas:?}"
    );

    let schemas = body
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .expect("components.schemas must be an object");
    let mut unresolved_refs = Vec::new();
    collect_schema_refs(&body, &mut unresolved_refs);
    unresolved_refs.retain(|schema_name| !schemas.contains_key(schema_name));
    unresolved_refs.sort();
    unresolved_refs.dedup();
    assert!(
        unresolved_refs.is_empty(),
        "OpenAPI schema refs missing from components.schemas: {unresolved_refs:?}"
    );
}

fn collect_schema_refs(value: &Value, refs: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(schema_name) = reference.strip_prefix("#/components/schemas/") {
                    refs.push(schema_name.to_string());
                }
            }
            for nested in map.values() {
                collect_schema_refs(nested, refs);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_schema_refs(item, refs);
            }
        }
        _ => {}
    }
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
        .get(format!(
            "{}/admin/api-doc/openapi.json",
            server.admin_base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let paths = body["paths"].as_object().expect("paths must be an object");

    // Expected admin paths should appear; method counts are grouped under
    // one path object by utoipa-axum.
    let expected = [
        "/admin/budget",
        "/admin/config",
        "/admin/config/fonts/directories",
        "/admin/diagnostics/workers",
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

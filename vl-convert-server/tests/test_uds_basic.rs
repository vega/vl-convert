#![cfg(unix)]
//! Basic UDS smoke tests — healthz + a conversion round-trip over a
//! pathname AF_UNIX SOCK_STREAM socket.

mod common;

use common::*;

#[tokio::test]
async fn test_uds_healthz() {
    let server = start_uds_server_sync(default_serve_config(), "main.sock", None);
    let path = uds_path(&server);
    let (status, body, _headers) = uds_get(&path, "/healthz").await;
    assert_eq!(status, 200, "healthz should return 200 over UDS");
    assert!(
        !body.is_empty(),
        "healthz body should not be empty, got {} bytes",
        body.len()
    );
}

#[tokio::test]
async fn test_uds_post_conversion() {
    let server = start_uds_server_sync(default_serve_config(), "main.sock", None);
    let path = uds_path(&server);
    let (status, body, _headers) = uds_post_json(
        &path,
        "/vegalite/svg",
        &serde_json::json!({"spec": simple_vl_spec()}),
    )
    .await;
    assert_eq!(status, 200, "conversion should succeed over UDS");
    let body_str = std::str::from_utf8(&body).unwrap_or("");
    assert!(
        body_str.contains("<svg"),
        "response should be an SVG document, got first 100 bytes: {}",
        &body_str[..body_str.len().min(100)]
    );
}

#[tokio::test]
async fn test_uds_both_listeners_on_uds() {
    // Main + admin both on UDS, in the same tempdir.
    let mut sc = default_serve_config();
    sc.per_ip_budget_ms = Some(1000);
    sc.global_budget_ms = Some(10_000);
    let server = start_uds_server_sync(sc, "main.sock", Some("admin.sock"));
    let main_path = uds_path(&server);
    // Main healthz still works.
    let (status, _, _) = uds_get(&main_path, "/healthz").await;
    assert_eq!(status, 200);
    // Admin socket sits alongside main in the tempdir.
    let admin_path = main_path.parent().unwrap().join("admin.sock");
    let (admin_status, admin_body, _) = uds_get(&admin_path, "/admin/budget").await;
    assert_eq!(
        admin_status, 200,
        "admin /admin/budget should respond over UDS"
    );
    let body_json: serde_json::Value = serde_json::from_slice(&admin_body).unwrap();
    assert_eq!(body_json["per_ip_budget_ms"], 1000);
    assert_eq!(body_json["global_budget_ms"], 10_000);
}

/// Helper to pull the socket path out of the `base_url` returned by
/// `start_uds_server_sync`. The URL form is `unix:///abs/path`.
fn uds_path(server: &ServerHandle) -> std::path::PathBuf {
    let s = server
        .base_url
        .strip_prefix("unix://")
        .expect("UDS ServerHandle.base_url should start with unix://");
    std::path::PathBuf::from(s)
}

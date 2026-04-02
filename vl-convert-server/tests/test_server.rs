use std::net::SocketAddr;

use once_cell::sync::Lazy;
use vl_convert_rs::converter::VlcConfig;
use vl_convert_server::{LogFormat, ServeConfig};

struct TestServer {
    base_url: String,
    client: reqwest::Client,
}

fn default_serve_config() -> ServeConfig {
    ServeConfig {
        host: "127.0.0.1".to_string(),
        port: 0,
        api_key: None,
        cors_origin: None,
        max_concurrent_requests: None,
        request_timeout_secs: 30,
        drain_timeout_secs: 30,
        max_body_size_mb: 50,
        opaque_errors: false,
        require_user_agent: false,
        log_format: LogFormat::Text,
        per_ip_budget_ms: None,
        global_budget_ms: None,
        budget_estimate_ms: 2000,
        admin_port: None,
        trust_proxy: false,
    }
}

fn find_free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn start_server_sync(config: VlcConfig, serve_config: ServeConfig) -> TestServer {
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let (app, _converter) = vl_convert_server::build_app(config, &serve_config).unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            tx.send(port).unwrap();
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .ok();
        });
    });

    let port = rx.recv_timeout(std::time::Duration::from_secs(60)).unwrap();
    let base_url = format!("http://127.0.0.1:{port}");

    // Poll for readiness using raw TCP connect (avoids reqwest::blocking inside
    // an async context, which panics due to nested runtimes).
    for _ in 0..150 {
        if std::net::TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    TestServer {
        base_url,
        client: reqwest::Client::new(),
    }
}

static DEFAULT_SERVER: Lazy<TestServer> = Lazy::new(|| {
    let config = VlcConfig::default();
    let serve_config = default_serve_config();
    start_server_sync(config, serve_config)
});

static AUTH_SERVER: Lazy<TestServer> = Lazy::new(|| {
    let config = VlcConfig::default();
    let mut serve_config = default_serve_config();
    serve_config.api_key = Some("test-secret".to_string());
    start_server_sync(config, serve_config)
});

static UA_SERVER: Lazy<TestServer> = Lazy::new(|| {
    let config = VlcConfig::default();
    let mut serve_config = default_serve_config();
    serve_config.require_user_agent = true;
    start_server_sync(config, serve_config)
});

static OPAQUE_SERVER: Lazy<TestServer> = Lazy::new(|| {
    let config = VlcConfig::default();
    let mut serve_config = default_serve_config();
    serve_config.opaque_errors = true;
    start_server_sync(config, serve_config)
});

static BUDGET_SERVER: Lazy<(TestServer, u16)> = Lazy::new(|| {
    let config = VlcConfig::default();
    let admin_port = find_free_port();
    let mut serve_config = default_serve_config();
    serve_config.per_ip_budget_ms = Some(1);
    serve_config.budget_estimate_ms = 2000;
    serve_config.admin_port = Some(admin_port);
    let server = start_server_sync(config, serve_config);
    (server, admin_port)
});

fn simple_vl_spec() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": [{"a": 1, "b": 2}]},
        "mark": "bar",
        "encoding": {"x": {"field": "a"}, "y": {"field": "b"}}
    })
}

fn simple_vg_spec() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 100, "height": 100,
        "data": [{"name": "values", "values": [{"x": 0, "y": 0}, {"x": 1, "y": 1}]}],
        "marks": [{"type": "rect", "from": {"data": "values"},
            "encode": {"enter": {
                "x": {"field": "x", "type": "quantitative"},
                "y": {"field": "y", "type": "quantitative"},
                "width": {"value": 10}, "height": {"value": 10},
                "fill": {"value": "steelblue"}
            }}}]
    })
}

fn simple_svg() -> &'static str {
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect width="100" height="100" fill="red"/></svg>"#
}

// ---------------------------------------------------------------------------
// 1. Health (3 tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_healthz() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/healthz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_readyz() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/readyz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn test_infoz() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/infoz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["version"].is_string(), "expected version string");
    assert!(
        body["vegalite_versions"].is_array(),
        "expected vegalite_versions array"
    );
}

// ---------------------------------------------------------------------------
// 2. Themes (3 tests)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 3. OpenAPI (2 tests)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 4. Vega-Lite conversions (7 tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_vl_to_vega() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/vega", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body.get("$schema").is_some() || body.get("marks").is_some(),
        "expected Vega spec output"
    );
}

#[tokio::test]
async fn test_vl_to_svg() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "bundle": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.starts_with("<svg"),
        "Expected SVG, got: {}",
        &body[..body.len().min(100)]
    );
}

#[tokio::test]
async fn test_vl_to_png() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/png", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("image/png"), "expected image/png, got: {ct}");
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.len() > 100, "PNG too small: {} bytes", bytes.len());
    assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G'], "bad PNG magic");
}

#[tokio::test]
async fn test_vl_to_jpeg() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/jpeg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("image/jpeg"), "expected image/jpeg, got: {ct}");
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.len() > 100, "JPEG too small: {} bytes", bytes.len());
    assert_eq!(&bytes[..2], &[0xFF, 0xD8], "bad JPEG magic");
}

#[tokio::test]
async fn test_vl_to_pdf() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/pdf", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
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
        ct.contains("application/pdf"),
        "expected application/pdf, got: {ct}"
    );
    let bytes = resp.bytes().await.unwrap();
    assert!(
        bytes.starts_with(b"%PDF"),
        "expected PDF magic, got: {:?}",
        &bytes[..bytes.len().min(10)]
    );
}

#[tokio::test]
async fn test_vl_to_html() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/html", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("<html") || body.contains("<!DOCTYPE"),
        "expected HTML content"
    );
}

#[tokio::test]
async fn test_vl_to_url() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/url", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.starts_with("https://vega.github.io/editor/"),
        "expected Vega Editor URL, got: {}",
        &body[..body.len().min(80)]
    );
}

// ---------------------------------------------------------------------------
// 5. Vega conversions (6 tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_vg_to_svg() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec(), "bundle": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.starts_with("<svg"),
        "Expected SVG, got: {}",
        &body[..body.len().min(100)]
    );
}

#[tokio::test]
async fn test_vg_to_png() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/png", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G'], "bad PNG magic");
}

#[tokio::test]
async fn test_vg_to_jpeg() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/jpeg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(&bytes[..2], &[0xFF, 0xD8], "bad JPEG magic");
}

#[tokio::test]
async fn test_vg_to_pdf() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/pdf", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.starts_with(b"%PDF"), "expected PDF magic");
}

#[tokio::test]
async fn test_vg_to_html() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/html", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("<html") || body.contains("<!DOCTYPE"),
        "expected HTML content"
    );
}

#[tokio::test]
async fn test_vg_to_url() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vega/url", server.base_url))
        .json(&serde_json::json!({"spec": simple_vg_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.starts_with("https://vega.github.io/editor/"),
        "expected Vega Editor URL, got: {}",
        &body[..body.len().min(80)]
    );
}

// ---------------------------------------------------------------------------
// 6. SVG conversions (3 tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_svg_to_png() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/svg/png", server.base_url))
        .json(&serde_json::json!({"svg": simple_svg()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G'], "bad PNG magic");
}

#[tokio::test]
async fn test_svg_to_jpeg() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/svg/jpeg", server.base_url))
        .json(&serde_json::json!({"svg": simple_svg()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(&bytes[..2], &[0xFF, 0xD8], "bad JPEG magic");
}

#[tokio::test]
async fn test_svg_to_pdf() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/svg/pdf", server.base_url))
        .json(&serde_json::json!({"svg": simple_svg()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.starts_with(b"%PDF"), "expected PDF magic");
}

// ---------------------------------------------------------------------------
// 7. X-VLC-Logs (2 tests)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 8. X-Request-Id (2 tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_request_id_generated() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/healthz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let req_id = resp.headers().get("x-request-id");
    assert!(
        req_id.is_some(),
        "expected x-request-id header to be generated"
    );
    let id_str = req_id.unwrap().to_str().unwrap();
    assert!(!id_str.is_empty(), "x-request-id should not be empty");
}

#[tokio::test]
async fn test_request_id_propagated() {
    let server = &*DEFAULT_SERVER;
    let custom_id = "my-custom-trace-id-12345";
    let resp = server
        .client
        .get(format!("{}/healthz", server.base_url))
        .header("x-request-id", custom_id)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let req_id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        req_id, custom_id,
        "expected propagated x-request-id to match"
    );
}

// ---------------------------------------------------------------------------
// 9. Request validation (3 tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_invalid_vl_version() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "vl_version": "99.99"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "expected 400 for invalid vl_version, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_missing_spec() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"bundle": false}))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "expected client error for missing spec, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_unknown_field_rejected() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "bogus_field": 123}))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "expected client error for unknown field, got: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// 10. Background override (1 test)
// ---------------------------------------------------------------------------

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
}

// ---------------------------------------------------------------------------
// 11. Auth (6 tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_auth_healthz_no_key_needed() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .get(format!("{}/healthz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "health endpoints should be accessible without auth"
    );
}

#[tokio::test]
async fn test_auth_readyz_no_key_needed() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .get(format!("{}/readyz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "readyz should be accessible without auth"
    );
}

#[tokio::test]
async fn test_auth_api_rejected_without_key() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401, "expected 401 without API key");
    let www_auth = resp.headers().get("www-authenticate");
    assert!(
        www_auth.is_some(),
        "expected WWW-Authenticate header on 401"
    );
}

#[tokio::test]
async fn test_auth_api_rejected_wrong_key() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes", server.base_url))
        .header("authorization", "Bearer wrong-key")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401, "expected 401 with wrong API key");
}

#[tokio::test]
async fn test_auth_api_accepted_with_correct_key() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes", server.base_url))
        .header("authorization", "Bearer test-secret")
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "expected 200 with correct API key, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_auth_post_endpoint_with_key() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .header("authorization", "Bearer test-secret")
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "expected 200 for authenticated POST, got: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// 12. User-Agent (4 tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_ua_healthz_no_ua_needed() {
    let server = &*UA_SERVER;
    let resp = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("{}/healthz", server.base_url))
        .header("user-agent", "")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "health endpoints should not require UA");
}

#[tokio::test]
async fn test_ua_api_rejected_without_ua() {
    let server = &*UA_SERVER;
    let no_ua_client = reqwest::Client::builder().no_proxy().build().unwrap();
    let resp = no_ua_client
        .get(format!("{}/themes", server.base_url))
        .header("user-agent", "")
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "expected 400 without User-Agent, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_ua_api_accepted_with_ua() {
    let server = &*UA_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes", server.base_url))
        .header("user-agent", "my-test-agent/1.0")
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "expected 200 with User-Agent, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_ua_post_accepted_with_ua() {
    let server = &*UA_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .header("user-agent", "my-test-agent/1.0")
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "expected 200 for POST with User-Agent, got: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// 13. Opaque errors (1 test)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_opaque_errors_hide_details() {
    let server = &*OPAQUE_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "vl_version": "99.99"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body = resp.text().await.unwrap();
    assert!(
        body.is_empty(),
        "expected empty body for opaque error, got: {body}"
    );
}

// ---------------------------------------------------------------------------
// 14. Budget (4 tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_budget_rate_limit_triggered() {
    let (server, _admin_port) = &*BUDGET_SERVER;
    // With per_ip_budget_ms=1 and budget_estimate_ms=2000, the very first request
    // should exhaust the budget. Send a request to trigger reservation.
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    // The first request may succeed or fail depending on timing; the second
    // should definitely be rate-limited.
    let resp2 = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    // At least one of them should be 429
    assert!(
        resp.status() == 429 || resp2.status() == 429,
        "expected at least one 429 response, got: {} and {}",
        resp.status(),
        resp2.status()
    );
}

#[tokio::test]
async fn test_budget_health_not_rate_limited() {
    let (server, _admin_port) = &*BUDGET_SERVER;
    // Health endpoints should bypass budget tracking
    for _ in 0..5 {
        let resp = server
            .client
            .get(format!("{}/healthz", server.base_url))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "health should never be rate limited");
    }
}

#[tokio::test]
async fn test_budget_admin_get() {
    let (_server, admin_port) = &*BUDGET_SERVER;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{admin_port}/admin/budget"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["per_ip_budget_ms"], 1);
    assert_eq!(body["estimate_ms"], 2000);
}

#[tokio::test]
async fn test_budget_admin_update() {
    let (_server, admin_port) = &*BUDGET_SERVER;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{admin_port}/admin/budget"))
        .json(&serde_json::json!({"estimate_ms": 500}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["estimate_ms"], 500);
}

// ---------------------------------------------------------------------------
// 15. CORS (3 tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cors_localhost_origin_allowed() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/themes", server.base_url),
        )
        .header("origin", "http://localhost:3000")
        .header("access-control-request-method", "GET")
        .send()
        .await
        .unwrap();
    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        acao, "http://localhost:3000",
        "expected localhost origin to be reflected"
    );
}

#[tokio::test]
async fn test_cors_127_origin_allowed() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/themes", server.base_url),
        )
        .header("origin", "http://127.0.0.1:8080")
        .header("access-control-request-method", "GET")
        .send()
        .await
        .unwrap();
    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        acao, "http://127.0.0.1:8080",
        "expected 127.0.0.1 origin to be reflected"
    );
}

#[tokio::test]
async fn test_cors_remote_origin_rejected() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/themes", server.base_url),
        )
        .header("origin", "https://evil.example.com")
        .header("access-control-request-method", "GET")
        .send()
        .await
        .unwrap();
    let acao = resp.headers().get("access-control-allow-origin");
    assert!(
        acao.is_none(),
        "expected remote origin to be rejected (no ACAO header), got: {:?}",
        acao.map(|v| v.to_str().ok())
    );
}

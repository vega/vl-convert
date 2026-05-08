mod common;

use common::*;
use serde_json::Value;

#[tokio::test]
async fn test_health_endpoints() {
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

    let resp = server
        .client
        .get(format!("{}/readyz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ready");

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

/// Locks the public `/infoz` surface: the exact set of keys must be
/// `{version, vega_version, vega_themes_version, vega_embed_version,
/// vegalite_versions}`. Anything else (notably `generation`) would
/// leak admin-scope observability to unauthenticated callers. Design §2.8.
#[tokio::test]
async fn test_infoz_surface_unchanged() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .get(format!("{}/infoz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let obj = body.as_object().expect("infoz must be a JSON object");

    let expected: std::collections::BTreeSet<&str> = [
        "version",
        "vega_version",
        "vega_themes_version",
        "vega_embed_version",
        "vegalite_versions",
        "google_fonts_cache_dir",
    ]
    .into_iter()
    .collect();
    let actual: std::collections::BTreeSet<&str> = obj.keys().map(|s| s.as_str()).collect();

    assert_eq!(
        actual, expected,
        "/infoz key set drifted; expected {expected:?}, got {actual:?}"
    );
    // Especially: generation must never be on /infoz (admin-only).
    assert!(
        !obj.contains_key("generation"),
        "/infoz must not expose `generation`"
    );
}

/// `/readyz` must return 503 while an admin reconfig is draining/rebuilding.
/// The test opens a main-listener TCP server with an admin listener, starts a
/// request whose body is deliberately held open, fires a rebuild PATCH, and
/// probes `/readyz` during the drain.
#[tokio::test]
async fn test_readyz_503_during_reconfig_in_progress() {
    use serde_json::json;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;

    let mut serve_config = default_serve_config();
    serve_config.reconfig_drain_timeout_secs = 10;
    let server =
        start_admin_config_server(vl_convert_rs::converter::VlcConfig::default(), serve_config);
    let main_url = server.handle.base_url.clone();
    let admin_url = server.admin_base_url.clone();

    let main_addr = main_url
        .strip_prefix("http://")
        .expect("test server must use http URL");
    let mut held_body = tokio::net::TcpStream::connect(main_addr)
        .await
        .expect("connect held request");
    held_body
        .write_all(
            b"POST /vegalite/svg HTTP/1.1\r\n\
              Host: localhost\r\n\
              Content-Type: application/json\r\n\
              Content-Length: 1000000\r\n\
              \r\n\
              {\"spec\":",
        )
        .await
        .expect("write held request headers");
    held_body.flush().await.expect("flush held request");
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Fire the PATCH.
    let admin_patch = admin_url.clone();
    let patch_task = tokio::spawn(async move {
        reqwest::Client::new()
            .patch(format!("{admin_patch}/admin/config"))
            .json(&json!({"default_theme": "dark"}))
            .send()
            .await
    });

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut saw_reconfiguring = false;
    while std::time::Instant::now() < deadline {
        let probe = reqwest::Client::new()
            .get(format!("{main_url}/readyz"))
            .send()
            .await
            .expect("readyz probe failed at transport layer");
        if probe.status() == 503 {
            saw_reconfiguring = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    drop(held_body);
    let _ = tokio::time::timeout(Duration::from_secs(30), patch_task).await;

    assert!(
        saw_reconfiguring,
        "/readyz must return 503 while reconfig waits for an admitted request to finish"
    );

    // After the patch completes, /readyz should return 200 again.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut ready_again = false;
    while std::time::Instant::now() < deadline {
        if let Ok(resp) = reqwest::Client::new()
            .get(format!("{main_url}/readyz"))
            .send()
            .await
        {
            if resp.status() == 200 {
                ready_again = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        ready_again,
        "/readyz must return 200 again after reconfig completes"
    );
}

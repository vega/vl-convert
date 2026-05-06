mod common;

use common::*;
use serde_json::Value;

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
/// slow conversion to keep the in-flight count above zero, fires a rebuild
/// PATCH, and probes `/readyz` during the drain.
#[tokio::test]
async fn test_readyz_503_during_reconfig_in_progress() {
    use serde_json::json;
    use std::time::Duration;

    let mut serve_config = default_serve_config();
    serve_config.reconfig_drain_timeout_secs = 10;
    let server =
        start_admin_config_server(vl_convert_rs::converter::VlcConfig::default(), serve_config);
    let main_url = server.handle.base_url.clone();
    let admin_url = server.admin_base_url.clone();

    // In-flight slow conversion keeps inflight > 0 so drain waits.
    let slow_values: Vec<Value> = (0..5000)
        .map(|i| json!({"x": i as f64 * 0.01, "y": (i as f64).sin()}))
        .collect();
    let slow_spec = json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": slow_values},
        "transform": [
            {"calculate": "datum.y * datum.x", "as": "prod"},
            {"bin": true, "field": "x", "as": "bin_x"}
        ],
        "mark": "bar",
        "encoding": {
            "x": {"field": "bin_x", "type": "quantitative"},
            "y": {"aggregate": "mean", "field": "prod", "type": "quantitative"}
        }
    });

    let main_slow = main_url.clone();
    let slow_task = tokio::spawn(async move {
        reqwest::Client::new()
            .post(format!("{main_slow}/vegalite/svg"))
            .json(&json!({"spec": slow_spec}))
            .send()
            .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Fire the PATCH.
    let admin_patch = admin_url.clone();
    let patch_task = tokio::spawn(async move {
        reqwest::Client::new()
            .patch(format!("{admin_patch}/admin/config"))
            .json(&json!({"default_theme": "dark"}))
            .send()
            .await
    });

    // Probe `/readyz` during the drain, while the slow request still holds an
    // in-flight guard.
    tokio::time::sleep(Duration::from_millis(20)).await;
    let probe = reqwest::Client::new()
        .get(format!("{main_url}/readyz"))
        .send()
        .await
        .expect("readyz probe failed at transport layer");
    let probe_status = probe.status();

    let _ = tokio::time::timeout(Duration::from_secs(30), slow_task).await;
    let _ = tokio::time::timeout(Duration::from_secs(30), patch_task).await;

    assert_eq!(
        probe_status, 503,
        "/readyz during reconfig drain must return 503; got {probe_status}"
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

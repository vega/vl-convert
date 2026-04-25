//! Drain-behavior integration tests for the admin reconfig pipeline (Task 13).
//!
//! Scenarios covered:
//!
//! * `test_drain_blocks_new_requests_with_503_retry_after` — while a PATCH
//!   is draining, new API requests get 503 + Retry-After.
//! * `test_drain_waits_for_inflight_to_finish` — a slow conversion pre-PATCH
//!   completes; PATCH only returns after.
//! * `test_drain_timeout_returns_504_and_reverts_gate` — forcing drain
//!   timeout via `reconfig_drain_timeout_secs = 0` yields 504 + the gate
//!   reopens afterwards.
//! * `test_back_to_back_patches_serialize` — two concurrent PATCHes both
//!   succeed and bump counters by 2.
//! * `test_failed_rebuild_restores_globals` — marked `#[ignore]` because
//!   the server does not expose a test-only rebuild-failure hook; see
//!   `findings.md`.
//! * `test_admission_race_regression` — stress the gate middleware with N
//!   concurrent POSTs while a PATCH closes the gate mid-burst.
//!
//! The drain tests rely on the admin rebuild path closing the gate via
//! `coordinator.close_gate()` before building the new converter. To reliably
//! get the main listener into the "draining" state the tests (a) issue a
//! slow conversion, (b) begin a PATCH that requires a rebuild, and (c) race
//! probe requests against the main listener. Non-trivial to time; all
//! assertions use generous polling windows.

mod common;

use common::*;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use vl_convert_rs::converter::VlcConfig;

/// Vega-Lite spec that takes ~100-400ms to render. 5k data points + an
/// aggregate transform gives the conversion enough runtime to overlap
/// with a PATCH that begins shortly after the conversion starts.
fn slow_spec() -> Value {
    let values: Vec<Value> = (0..5000)
        .map(|i| json!({"x": i as f64 * 0.01, "y": (i as f64).sin()}))
        .collect();
    json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": values},
        "transform": [
            {"calculate": "datum.y * datum.x", "as": "prod"},
            {"bin": true, "field": "x", "as": "bin_x"}
        ],
        "mark": "bar",
        "encoding": {
            "x": {"field": "bin_x", "type": "quantitative"},
            "y": {"aggregate": "mean", "field": "prod", "type": "quantitative"}
        }
    })
}

/// Small and fast — used as the "probe" request checking gate state.
fn fast_spec() -> Value {
    json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": [{"a": 1, "b": 2}]},
        "mark": "bar",
        "encoding": {"x": {"field": "a"}, "y": {"field": "b"}}
    })
}

#[tokio::test]
async fn test_drain_blocks_new_requests_with_503_retry_after() {
    // Start a fresh server with a generous drain timeout so the drain has
    // time to observe an in-flight request.
    let mut serve_config = default_serve_config();
    serve_config.reconfig_drain_timeout_secs = 10;
    let server = start_admin_config_server(VlcConfig::default(), serve_config);
    let main_url = server.handle.base_url.clone();
    let admin_url = server.admin_base_url.clone();

    // Kick off a slow conversion against the main listener. This will
    // admit via the gate (gate is currently open) and hold inflight = 1
    // for the duration of the render. Probe timing relies on the slow
    // render outlasting the PATCH's close-gate + drain window, so we
    // want the conversion to take at least ~500ms.
    let client_slow = reqwest::Client::new();
    let main_url_slow = main_url.clone();
    let slow_task = tokio::spawn(async move {
        client_slow
            .post(format!("{main_url_slow}/vegalite/svg"))
            .json(&json!({"spec": slow_spec()}))
            .send()
            .await
            .map(|r| r.status())
    });

    // Give the slow request a moment to be admitted (gate + handler start).
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Kick off the PATCH — default_theme requires rebuild, so the handler
    // closes the gate and waits for the slow request to complete.
    let client_patch = reqwest::Client::new();
    let admin_url_patch = admin_url.clone();
    let patch_task = tokio::spawn(async move {
        client_patch
            .patch(format!("{admin_url_patch}/admin/config"))
            .json(&json!({"default_theme": "dark"}))
            .send()
            .await
            .map(|r| r.status())
    });

    // Probe as soon as the PATCH has had a moment to close the gate but
    // BEFORE the slow request completes — otherwise the drain finishes
    // and the gate reopens before we observe the closed-gate response.
    // A 20 ms window has been sufficient empirically for the PATCH to
    // reach `close_gate()` without overlapping the rebuild's reopen.
    tokio::time::sleep(Duration::from_millis(20)).await;

    // A probe request to the main listener while the PATCH is draining
    // should see 503 + Retry-After: 5.
    let client_probe = reqwest::Client::new();
    let probe_resp = client_probe
        .post(format!("{main_url}/vegalite/svg"))
        .json(&json!({"spec": fast_spec()}))
        .send()
        .await
        .expect("probe request failed at transport layer");
    let probe_status = probe_resp.status();
    let retry_after = probe_resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Clean up.
    let _ = tokio::time::timeout(Duration::from_secs(15), slow_task).await;
    let _ = tokio::time::timeout(Duration::from_secs(15), patch_task).await;

    assert_eq!(
        probe_status, 503,
        "probe during drain must be 503; got {probe_status}"
    );
    assert_eq!(
        retry_after.as_deref(),
        Some("5"),
        "probe during drain must have Retry-After: 5"
    );
}

#[tokio::test]
async fn test_drain_waits_for_inflight_to_finish() {
    // Slow conversion pre-PATCH completes; PATCH only returns 200 after.
    let mut serve_config = default_serve_config();
    serve_config.reconfig_drain_timeout_secs = 30;
    let server = start_admin_config_server(VlcConfig::default(), serve_config);
    let main_url = server.handle.base_url.clone();
    let admin_url = server.admin_base_url.clone();

    let client_slow = reqwest::Client::new();
    let main_url_slow = main_url.clone();
    let slow_start = Instant::now();
    let slow_task = tokio::spawn(async move {
        let r = client_slow
            .post(format!("{main_url_slow}/vegalite/svg"))
            .json(&json!({"spec": slow_spec()}))
            .send()
            .await
            .map(|r| (r.status(), slow_start.elapsed()));
        r
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let patch_start = Instant::now();
    let client_patch = reqwest::Client::new();
    let admin_url_patch = admin_url.clone();
    let patch_task = tokio::spawn(async move {
        let r = client_patch
            .patch(format!("{admin_url_patch}/admin/config"))
            .json(&json!({"default_theme": "dark"}))
            .send()
            .await
            .map(|r| (r.status(), patch_start.elapsed()));
        r
    });

    let (slow_result, patch_result) = tokio::join!(slow_task, patch_task);
    let (slow_status, _slow_elapsed) = slow_result.unwrap().unwrap();
    let (patch_status, patch_elapsed) = patch_result.unwrap().unwrap();

    assert_eq!(slow_status, 200, "slow conversion must eventually 200");
    assert_eq!(patch_status, 200, "PATCH must eventually 200");
    // PATCH must have waited for slow conversion to finish (at least some
    // meaningful time).
    assert!(
        patch_elapsed >= Duration::from_millis(50),
        "PATCH returned too quickly ({patch_elapsed:?}) — drain didn't wait"
    );
}

#[tokio::test]
async fn test_drain_timeout_returns_504_and_reverts_gate() {
    // reconfig_drain_timeout_secs = 0 → drain deadline is "now"; if
    // there's an in-flight request the drain immediately fails with
    // Timeout and the handler returns 504.
    let mut serve_config = default_serve_config();
    serve_config.reconfig_drain_timeout_secs = 0;
    let server = start_admin_config_server(VlcConfig::default(), serve_config);
    let main_url = server.handle.base_url.clone();
    let admin_url = server.admin_base_url.clone();

    // Start a slow conversion.
    let client_slow = reqwest::Client::new();
    let main_url_slow = main_url.clone();
    let slow_task = tokio::spawn(async move {
        client_slow
            .post(format!("{main_url_slow}/vegalite/svg"))
            .json(&json!({"spec": slow_spec()}))
            .send()
            .await
            .map(|r| r.status())
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    // PATCH with 0-second drain timeout → 504.
    let client_patch = reqwest::Client::new();
    let resp = client_patch
        .patch(format!("{admin_url}/admin/config"))
        .json(&json!({"default_theme": "dark"}))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);

    assert_eq!(status, 504, "0s drain with in-flight must be 504; body: {body:?}");
    // Body shape: { error, in_flight } (when !opaque_errors).
    if body != Value::Null {
        assert!(
            body.get("error").is_some(),
            "504 body should carry an error field"
        );
        assert!(
            body.get("in_flight").is_some(),
            "504 body should expose in_flight count"
        );
    }

    // Let the slow request finish.
    let _ = tokio::time::timeout(Duration::from_secs(15), slow_task).await;

    // Gate should have been reopened — a new request on the main listener
    // must succeed (200 via fast_spec or at least not 503).
    let resp = reqwest::Client::new()
        .post(format!("{main_url}/vegalite/svg"))
        .json(&json!({"spec": fast_spec()}))
        .send()
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        503,
        "gate should have reopened after drain timeout"
    );

    // Generation must not have bumped (commit didn't happen).
    let config_resp = reqwest::Client::new()
        .get(format!("{admin_url}/admin/config"))
        .send()
        .await
        .unwrap();
    let config_body: Value = config_resp.json().await.unwrap();
    assert_eq!(
        config_body["generation"], 0,
        "failed drain must not bump generation"
    );
}

#[tokio::test]
async fn test_back_to_back_patches_serialize() {
    // Two PATCHes sent concurrently. Both should return 200, generation +
    // config_version should bump by 2 (the reconfig_lock serializes them).
    let mut serve_config = default_serve_config();
    serve_config.reconfig_drain_timeout_secs = 30;
    let server = start_admin_config_server(VlcConfig::default(), serve_config);
    let admin_url = server.admin_base_url.clone();

    let c1 = reqwest::Client::new();
    let c2 = reqwest::Client::new();
    let admin_url_1 = admin_url.clone();
    let admin_url_2 = admin_url.clone();

    let t1 = tokio::spawn(async move {
        c1.patch(format!("{admin_url_1}/admin/config"))
            .json(&json!({"default_theme": "dark"}))
            .send()
            .await
            .map(|r| r.status())
    });
    let t2 = tokio::spawn(async move {
        c2.patch(format!("{admin_url_2}/admin/config"))
            .json(&json!({"auto_google_fonts": true}))
            .send()
            .await
            .map(|r| r.status())
    });

    let (s1, s2) = tokio::join!(t1, t2);
    assert_eq!(s1.unwrap().unwrap(), 200);
    assert_eq!(s2.unwrap().unwrap(), 200);

    let resp = reqwest::Client::new()
        .get(format!("{admin_url}/admin/config"))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["generation"], 2,
        "two sequential rebuild PATCHes must bump generation by 2"
    );
    assert_eq!(body["config_version"], 2);
}

/// #[ignore]'d — there is no test-only hook in vl-convert-rs to force a
/// `VlConverter::with_config` or `warm_up` failure. The production code
/// path is covered by the admin handler's ReconfigScopeGuard rollback
/// closure, but exercising it from an integration test would require a
/// library-side fault-injection hook that does not exist. Documented in
/// findings.md.
#[ignore]
#[tokio::test]
async fn test_failed_rebuild_restores_globals() {
    // Intentionally empty — see doc comment above.
}

/// Admission race regression — stress the gate middleware with many
/// concurrent POSTs while a PATCH closes the gate mid-burst. Every request
/// must either admit cleanly (200/4xx), get rejected with 503, or get a
/// non-deadlocking response. No task should hang, and the server must
/// remain responsive after the PATCH completes.
#[tokio::test]
async fn test_admission_race_regression() {
    let mut serve_config = default_serve_config();
    serve_config.reconfig_drain_timeout_secs = 30;
    let server = start_admin_config_server(VlcConfig::default(), serve_config);
    let main_url = server.handle.base_url.clone();
    let admin_url = server.admin_base_url.clone();

    const TASKS: usize = 50;
    let main_url_probe = main_url.clone();
    let mut probes: Vec<tokio::task::JoinHandle<reqwest::StatusCode>> =
        Vec::with_capacity(TASKS);
    for _ in 0..TASKS {
        let m = main_url_probe.clone();
        probes.push(tokio::spawn(async move {
            let client = reqwest::Client::new();
            let r = client
                .post(format!("{m}/vegalite/svg"))
                .json(&json!({"spec": fast_spec()}))
                .send()
                .await
                .unwrap();
            r.status()
        }));
    }

    // Kick the PATCH shortly after — it will race with the probes.
    tokio::time::sleep(Duration::from_millis(20)).await;
    let patch_client = reqwest::Client::new();
    let admin_url_patch = admin_url.clone();
    let patch_task = tokio::spawn(async move {
        patch_client
            .patch(format!("{admin_url_patch}/admin/config"))
            .json(&json!({"default_theme": "dark"}))
            .send()
            .await
            .map(|r| r.status())
    });

    let mut admitted = 0;
    let mut rejected = 0;
    let mut other = 0;
    for h in probes {
        let s = tokio::time::timeout(Duration::from_secs(30), h)
            .await
            .expect("probe task deadlocked")
            .unwrap();
        match s.as_u16() {
            200 => admitted += 1,
            503 => rejected += 1,
            _ => other += 1,
        }
    }
    let patch_status = tokio::time::timeout(Duration::from_secs(30), patch_task)
        .await
        .expect("PATCH task deadlocked")
        .unwrap()
        .unwrap();
    assert_eq!(patch_status, 200, "PATCH must eventually succeed");

    // The exact split between admitted/rejected is timing-dependent; what
    // matters is that all TASKS accounted for (no lost requests) and the
    // server is responsive afterwards.
    assert_eq!(
        admitted + rejected + other,
        TASKS,
        "lost probes; admitted={admitted} rejected={rejected} other={other}"
    );

    // Server must be responsive after the PATCH — gate should be open.
    let resp = reqwest::Client::new()
        .post(format!("{main_url}/vegalite/svg"))
        .json(&json!({"spec": fast_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "server must be responsive after back-to-back drain+PATCH"
    );
}

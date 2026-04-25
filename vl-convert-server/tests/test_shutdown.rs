//! Verifies that `serve` drains its background tasks (budget refill,
//! admin listener) when the caller-supplied shutdown future fires.
//!
//! If the refill loop weren't wired to the cancellation token, its
//! `JoinSet` entry would never complete and `serve` would hang past the
//! 5s deadline instead of returning.

use serde_json::json;
use std::time::{Duration, Instant};
use vl_convert_rs::converter::VlcConfig;
use vl_convert_server::{BoundListener, ListenAddr, ServeConfig};

#[tokio::test]
async fn test_serve_drains_background_tasks_on_shutdown() {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std_listener.set_nonblocking(true).unwrap();
    let admin_port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };

    let serve_config = ServeConfig {
        per_ip_budget_ms: Some(1000),
        global_budget_ms: Some(10_000),
        budget_hold_ms: 100,
        admin: Some(ListenAddr::Tcp {
            host: "127.0.0.1".to_string(),
            port: admin_port,
        }),
        ..ServeConfig::default()
    };
    let built = vl_convert_server::build_app(VlcConfig::default(), &serve_config)
        .await
        .unwrap();
    let listener = BoundListener::Tcp(tokio::net::TcpListener::from_std(std_listener).unwrap());

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown = async move {
        let _ = rx.await;
    };

    let serve_handle = tokio::spawn(vl_convert_server::serve(listener, built, shutdown));

    tokio::time::sleep(Duration::from_millis(200)).await;
    tx.send(()).unwrap();

    let result = tokio::time::timeout(Duration::from_secs(5), serve_handle).await;
    let serve_result = result
        .expect("serve did not return within 5s — background task likely leaked past shutdown");
    serve_result
        .expect("serve task panicked")
        .expect("serve returned an error");
}

/// SIGTERM (shutdown signal) arriving mid-reconfig must win: the PATCH
/// should return 503 "server shutting down" (DrainError::Cancelled) and
/// the server must exit within `drain_timeout_secs + slack`.
///
/// This test uses the external shutdown future (the `shutdown: impl
/// Future` argument `serve()` accepts) rather than a real SIGTERM —
/// `serve()` converts that future into a `CancellationToken::cancel()`
/// call on the shared token, which is the exact path a SIGTERM handler
/// in `main.rs` would take. See `src/lib.rs::serve` for the cancellation
/// bridge.
#[tokio::test]
async fn test_sigterm_mid_reconfig_wins() {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std_listener.set_nonblocking(true).unwrap();
    let addr = std_listener.local_addr().unwrap();
    let admin_port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };

    let serve_config = ServeConfig {
        admin: Some(ListenAddr::Tcp {
            host: "127.0.0.1".to_string(),
            port: admin_port,
        }),
        // Long reconfig drain window — gives the PATCH time to enter the
        // drain loop before we cancel.
        reconfig_drain_timeout_secs: 60,
        ..ServeConfig::default()
    };
    let built = vl_convert_server::build_app(VlcConfig::default(), &serve_config)
        .await
        .unwrap();
    let listener = BoundListener::Tcp(tokio::net::TcpListener::from_std(std_listener).unwrap());

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown = async move {
        let _ = rx.await;
    };

    let serve_handle = tokio::spawn(vl_convert_server::serve(listener, built, shutdown));
    // Give the server a moment to start.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let main_url = format!("http://{addr}");
    let admin_url = format!("http://127.0.0.1:{admin_port}");

    // Start a slow conversion to hold inflight > 0 so PATCH enters the
    // drain loop.
    let slow_values: Vec<serde_json::Value> = (0..5000)
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

    // Fire the PATCH — it enters the drain loop with the slow request still
    // in-flight.
    let patch_start = Instant::now();
    let admin_patch = admin_url.clone();
    let patch_task = tokio::spawn(async move {
        reqwest::Client::new()
            .patch(format!("{admin_patch}/admin/config"))
            .json(&json!({"default_theme": "dark"}))
            .send()
            .await
    });
    // Give the PATCH a chance to close the gate and enter drain, but fire
    // SIGTERM quickly enough that we hit the `biased` select arm in
    // `ReconfigCoordinator::drain` while it's still awaiting the slow
    // request's inflight guard to drop — rebuild + warm_up on a fast
    // machine completes in ~200 ms.
    tokio::time::sleep(Duration::from_millis(30)).await;

    // Fire SIGTERM-equivalent: serve()'s `shutdown` future resolves, and
    // internally it calls `shutdown_token.cancel()` on the shared token
    // which the reconfig coordinator is observing via select!.
    let _ = tx.send(());

    // PATCH should complete with a 503 "server shutting down" — drain was
    // cancelled before it could drain. Accept any non-200 status because
    // the exact status depends on whether the admin listener responded
    // before or after the cancel propagated (503 is the contract, but the
    // admin listener may also get torn down and the client may see a
    // transport error).
    let patch_result = tokio::time::timeout(Duration::from_secs(10), patch_task).await;
    let patch_elapsed = patch_start.elapsed();
    match patch_result {
        Ok(Ok(Ok(resp))) => {
            let status = resp.status();
            assert!(
                status == 503 || status.is_server_error(),
                "PATCH mid-shutdown should return 503 or other 5xx; got {status}"
            );
        }
        Ok(Ok(Err(_transport_err))) => {
            // Admin listener was torn down before the PATCH got its
            // response — also acceptable (the server won the race).
        }
        Ok(Err(join_err)) => panic!("PATCH task panicked: {join_err}"),
        Err(_) => panic!("PATCH did not complete within 10s after shutdown"),
    }
    assert!(
        patch_elapsed < Duration::from_secs(30),
        "PATCH should not have waited the full 60s drain timeout"
    );

    // Server must have exited within a reasonable slack of the cancel.
    let serve_timeout = Duration::from_secs(10);
    let result = tokio::time::timeout(serve_timeout, serve_handle).await;
    result.expect("serve did not exit in time after shutdown")
        .expect("serve task panicked")
        .expect("serve returned err");

    // Slow task may have completed with an error or succeeded; either is
    // acceptable — we just drain it.
    let _ = tokio::time::timeout(Duration::from_secs(2), slow_task).await;
}

//! Verifies that `serve` drains its background tasks (budget refill,
//! admin listener) when the caller-supplied shutdown future fires.
//!
//! If the refill loop weren't wired to the cancellation token, its
//! `JoinSet` entry would never complete and `serve` would hang past the
//! 5s deadline instead of returning.

use std::time::Duration;
use vl_convert_rs::converter::VlcConfig;
use vl_convert_server::ServeConfig;

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
        admin_port: Some(admin_port),
        ..ServeConfig::default()
    };
    let built = vl_convert_server::build_app(VlcConfig::default(), &serve_config)
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::from_std(std_listener).unwrap();

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

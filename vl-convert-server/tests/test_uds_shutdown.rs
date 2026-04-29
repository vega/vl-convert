#![cfg(unix)]
//! UDS analogue of `test_shutdown.rs` — verifies `serve()` drains its
//! background tasks when given a `BoundListener::Uds` + a caller-driven
//! shutdown future. If the drain wiring regressed on the UDS arm, the
//! serve task would hang past the 5s deadline instead of returning.

use std::time::Duration;
use vl_convert_rs::converter::VlcConfig;
use vl_convert_server::{ListenAddr, ServeConfig};

#[tokio::test]
async fn test_serve_drains_background_tasks_on_shutdown_over_uds() {
    let tmp = tempfile::tempdir().unwrap();
    let main_sock = tmp.path().join("main.sock");
    let admin_sock = tmp.path().join("admin.sock");

    let serve_config = ServeConfig {
        main: ListenAddr::Uds {
            path: main_sock.clone(),
        },
        admin: Some(ListenAddr::Uds {
            path: admin_sock.clone(),
        }),
        per_ip_budget_ms: Some(1000),
        global_budget_ms: Some(10_000),
        budget_hold_ms: 100,
        ..ServeConfig::default()
    };

    let built = vl_convert_server::build_app(VlcConfig::default(), &serve_config)
        .await
        .unwrap();

    let listener = vl_convert_server::bind_listener(
        &ListenAddr::Uds {
            path: main_sock.clone(),
        },
        0o600,
    )
    .await
    .unwrap();

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown = async move {
        let _ = rx.await;
    };

    let serve_handle = tokio::spawn(vl_convert_server::serve(listener, built, shutdown));

    tokio::time::sleep(Duration::from_millis(200)).await;
    tx.send(()).unwrap();

    let result = tokio::time::timeout(Duration::from_secs(5), serve_handle).await;
    let serve_result = result.expect(
        "UDS serve did not return within 5s — drain wiring on the UDS arm likely regressed",
    );
    serve_result
        .expect("serve task panicked")
        .expect("serve returned an error");

    // After drain, the main socket file must be gone (UdsCleanup).
    assert!(
        !main_sock.exists(),
        "main UDS socket should be unlinked after drain"
    );
    assert!(
        !admin_sock.exists(),
        "admin UDS socket should be unlinked after drain"
    );
}

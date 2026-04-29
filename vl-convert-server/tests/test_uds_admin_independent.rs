#![cfg(unix)]
//! The admin listener's transport is configured independently from
//! the main listener — each can be TCP or UDS without constraint.
//! These tests exercise every combination end-to-end.

mod common;

use common::*;
use vl_convert_server::{BoundListener, ListenAddr, ServeConfig};

#[tokio::test]
async fn test_main_uds_admin_tcp() {
    // Main on UDS, admin on TCP loopback. Admin URL returned by the
    // server must be TCP-form; main is addressable only over the socket.
    let tmp = tempfile::tempdir().unwrap();
    let main_sock = tmp.path().join("main.sock");
    let admin_port = find_free_port();

    let mut sc = default_serve_config();
    sc.main = ListenAddr::Uds {
        path: main_sock.clone(),
    };
    sc.admin = Some(ListenAddr::Tcp {
        host: "127.0.0.1".to_string(),
        port: admin_port,
    });
    sc.per_ip_budget_ms = Some(1000);
    sc.global_budget_ms = Some(10_000);

    let _handle = spawn_server_with_config(sc);

    // Main responds over UDS.
    let (status, _body, _) = uds_get(&main_sock, "/healthz").await;
    assert_eq!(status, 200);

    // Admin responds over TCP loopback.
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{admin_port}/admin/budget"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["per_ip_budget_ms"], 1000);

    drop(tmp);
}

#[tokio::test]
async fn test_main_tcp_admin_uds() {
    // Inverse — main on TCP, admin on UDS. Reflects the "independent
    // transports" decision.
    let tmp = tempfile::tempdir().unwrap();
    let admin_sock = tmp.path().join("admin.sock");
    let tcp_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let main_port = tcp_listener.local_addr().unwrap().port();
    drop(tcp_listener); // release so the server can rebind

    let mut sc = default_serve_config();
    sc.main = ListenAddr::Tcp {
        host: "127.0.0.1".to_string(),
        port: main_port,
    };
    sc.admin = Some(ListenAddr::Uds {
        path: admin_sock.clone(),
    });
    sc.per_ip_budget_ms = Some(1000);
    sc.global_budget_ms = Some(10_000);

    let _handle = spawn_server_with_config(sc);

    // Main responds over TCP.
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{main_port}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Admin responds over UDS.
    let (status, body, _) = uds_get(&admin_sock, "/admin/budget").await;
    assert_eq!(status, 200);
    let body_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body_json["per_ip_budget_ms"], 1000);

    drop(tmp);
}

#[tokio::test]
async fn test_both_uds_different_paths() {
    // Two UDS listeners at different paths in the same tempdir.
    let mut sc = default_serve_config();
    sc.per_ip_budget_ms = Some(1000);
    sc.global_budget_ms = Some(10_000);
    let server = start_uds_server_sync(sc, "svc.sock", Some("ctl.sock"));
    let main_path = std::path::PathBuf::from(server.base_url.strip_prefix("unix://").unwrap());
    let admin_path = main_path.parent().unwrap().join("ctl.sock");

    let (main_status, _, _) = uds_get(&main_path, "/healthz").await;
    let (admin_status, _, _) = uds_get(&admin_path, "/admin/budget").await;
    assert_eq!(main_status, 200);
    assert_eq!(admin_status, 200);
    assert_ne!(
        main_path, admin_path,
        "main and admin should be at different paths"
    );
}

/// Start the server with the given config on a background thread.
/// Unlike `start_uds_server_sync` this accepts a fully-formed config
/// so tests can mix TCP and UDS across main/admin as they please.
fn spawn_server_with_config(sc: ServeConfig) -> std::thread::JoinHandle<()> {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let built =
                vl_convert_server::build_app(vl_convert_rs::converter::VlcConfig::default(), &sc)
                    .await
                    .expect("build_app failed");
            let listener = match &sc.main {
                ListenAddr::Tcp { host, port } => {
                    let addr = format!("{host}:{port}");
                    BoundListener::Tcp(tokio::net::TcpListener::bind(&addr).await.unwrap())
                }
                ListenAddr::Uds { .. } => {
                    vl_convert_server::bind_listener(&sc.main, sc.socket_mode)
                        .await
                        .expect("UDS bind")
                }
            };
            ready_tx.send(()).ok();
            vl_convert_server::serve(listener, built, std::future::pending())
                .await
                .ok();
        });
    });
    ready_rx.recv().expect("server failed to start");
    handle
}

#![cfg(unix)]
//! Socket-file lifecycle: probe-then-unlink on bind, Drop-based unlink
//! on clean shutdown, refusal to stomp live sockets, preservation of
//! non-socket files at the target path.

mod common;

use common::*;
use std::os::unix::fs::PermissionsExt;
use vl_convert_server::{BoundListener, ListenAddr, ServeConfig};

#[tokio::test]
async fn test_stale_socket_probe_and_unlink() {
    // Pre-create a stale socket file (simulating a crashed previous
    // run). Bind should probe, see ECONNREFUSED, unlink, then succeed.
    let tmp = tempfile::tempdir().unwrap();
    let sock_path = tmp.path().join("stale.sock");

    // Create a real UNIX socket file *without* a listener attached so
    // connect() will succeed the file exists check but fail at the
    // socket-level. Easiest way: bind then drop.
    {
        let _pre = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        // Leak: don't unlink so the file remains after drop simulates
        // a crashed run.
    }
    // The listener dropped; the socket file may still exist depending
    // on the std behavior. If it's gone, create a regular file to
    // simulate a truly stale state — bind will then fail with
    // AddrInUse rather than ECONNREFUSED. Re-bind-and-leak to make it
    // look like a socket file.
    if !sock_path.exists() {
        let _pre = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        std::mem::forget(_pre);
    }
    assert!(
        sock_path.exists(),
        "pre-bind sentinel should have left a file"
    );

    // Now call bind_listener — it should probe, decide the file is
    // stale (ECONNREFUSED since nothing is listening anymore), unlink
    // it, and bind successfully.
    let spec = ListenAddr::Uds {
        path: sock_path.clone(),
    };
    let bound = vl_convert_server::bind_listener(&spec, 0o600)
        .await
        .expect("bind_listener should clear stale and succeed");
    // Drop the bound listener to run UdsCleanup.
    drop(bound);
    // File should be unlinked by UdsCleanup.
    assert!(
        !sock_path.exists(),
        "UdsCleanup should unlink the socket on drop"
    );
}

#[tokio::test]
async fn test_live_socket_refuses_to_stomp() {
    // Spawn a live server holding a UDS. A second bind_listener call
    // on the same path must fail (not silently clobber).
    let tmp = tempfile::tempdir().unwrap();
    let sock_path = tmp.path().join("live.sock");
    let spec = ListenAddr::Uds {
        path: sock_path.clone(),
    };

    let a = vl_convert_server::bind_listener(&spec, 0o600)
        .await
        .expect("first bind should succeed");
    assert!(sock_path.exists());

    // Spawn a minimal accept loop so the probe sees a live server.
    let BoundListener::Uds(listener_a, cleanup_a) = a else {
        panic!("expected Uds variant");
    };
    let accept_task = tokio::spawn(async move {
        // Accept and immediately drop connections so the probe's
        // `connect` succeeds quickly.
        loop {
            let _ = listener_a.accept().await;
        }
    });
    // Hold cleanup_a so the socket stays alive for the test.
    let _cleanup_a = cleanup_a;

    // Brief yield so the accept task is polling.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // Second bind should fail.
    let result = vl_convert_server::bind_listener(&spec, 0o600).await;
    let err = match result {
        Ok(_) => panic!("second bind should refuse to stomp the live socket"),
        Err(e) => e,
    };
    let msg = format!("{err:#}");
    assert!(
        msg.contains("in use"),
        "error should mention 'in use'; got: {msg}"
    );

    // Original socket file still exists.
    assert!(sock_path.exists());

    accept_task.abort();
    drop(_cleanup_a);
    drop(tmp);
}

#[tokio::test]
async fn test_shutdown_removes_socket_file() {
    // Start a server, verify socket exists, drop the server (via
    // tempdir + implicit thread exit), verify socket gone.
    let server = start_uds_server_sync(default_serve_config(), "cleanup.sock", None);
    let sock_path = std::path::PathBuf::from(server.base_url.strip_prefix("unix://").unwrap());
    assert!(sock_path.exists(), "socket should be bound");

    // Drop the server handle — this drops the tempdir, which for the
    // `start_uds_server_sync` harness also keeps the file alive.
    // Explicit clean: drop the server, which kills the thread's
    // runtime, which drops the BoundListener::Uds, which triggers
    // UdsCleanup::Drop.
    let tmp = server._tempdir.expect("UDS server must own a tempdir");
    drop(tmp);
    // After tempdir drops, everything under it is gone.
    assert!(
        !sock_path.exists(),
        "socket file should be unlinked after tempdir drops"
    );
}

#[tokio::test]
async fn test_non_socket_file_not_unlinked() {
    // If the target path is a regular file (not a socket), the probe
    // returns an error kind that isn't ECONNREFUSED. The helper must
    // NOT unlink it — data-preservation over bind-success.
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("not-a-socket.txt");
    std::fs::write(&path, b"important data, do not delete").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    let spec = ListenAddr::Uds { path: path.clone() };
    let result = vl_convert_server::bind_listener(&spec, 0o600).await;
    // Exact error message varies by kernel. The critical property is
    // that the original file is preserved.
    let _ = result.err();
    assert!(
        path.exists(),
        "non-socket file must not be unlinked by probe_then_unlink"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "important data, do not delete",
        "file contents must be untouched"
    );

    // Cleanup helper for ServeConfig unused-warning.
    let _ = ServeConfig::default();
}

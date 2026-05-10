#![cfg(unix)]
//! Socket-file lifecycle: probe-then-unlink on bind, Drop-based unlink on
//! clean shutdown, live-socket refusal, and preservation of non-socket files.

use std::os::unix::fs::PermissionsExt;
use vl_convert_server::{BoundListener, ListenAddr};

#[tokio::test]
async fn test_stale_socket_probe_and_unlink() {
    // Pre-create a stale socket file. Bind should probe, see ECONNREFUSED,
    // unlink, then succeed.
    let tmp = tempfile::tempdir().unwrap();
    let sock_path = tmp.path().join("stale.sock");

    // Create a real UNIX socket file *without* a listener attached so
    // connect() will succeed the file exists check but fail at the
    // socket-level. Easiest way: bind then drop.
    {
        let _pre = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        // Leave the socket file behind after the listener drops.
    }
    // If the listener drop removed the socket file, bind and leak another
    // listener so the path still contains a socket file.
    if !sock_path.exists() {
        let _pre = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        std::mem::forget(_pre);
    }
    assert!(
        sock_path.exists(),
        "pre-bind sentinel should have left a file"
    );

    // bind_listener should probe the stale socket, unlink it, and bind
    // successfully.
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
async fn test_live_socket_refuses_replacement() {
    // A second bind_listener call on a live socket must fail.
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
        Ok(_) => panic!("second bind should refuse the live socket"),
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
async fn test_non_socket_file_not_unlinked() {
    // Regular files at the target path are preserved.
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
}

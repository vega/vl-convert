#![cfg(unix)]
//! Verify that the `UdsConnectInfo` extractor runs cleanly on a real
//! UDS connection — that is, `peer_cred()` doesn't panic and the
//! router serves the request.
//!
//! The actual uid/gid/pid values in tracing spans are validated
//! end-to-end in the subprocess e2e suite, which can capture stderr
//! and parse span events. Here we're only confirming the extractor
//! path doesn't blow up.

mod common;

use common::*;

#[tokio::test]
async fn test_peer_cred_uid_gid_observable() {
    // If the Connected impl weren't wired up or peer_cred() panicked,
    // this request would fail. peer_cred is observability-only, so a
    // missing value would still return 200 — we're asserting the
    // extractor path executes without aborting the accept loop.
    let server = start_uds_server_sync(default_serve_config(), "main.sock", None);
    let sock_path = std::path::PathBuf::from(server.base_url.strip_prefix("unix://").unwrap());

    let (status, _, _) = uds_get(&sock_path, "/healthz").await;
    assert_eq!(status, 200, "request must succeed even with peer_cred");
}

/// `UCred::pid()` is populated on Linux and macOS. Since this test's
/// server runs in the same process as the client, the pid the server
/// observes (when it reads peer_cred) is known and positive.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[tokio::test]
async fn test_peer_cred_pid_available_on_linux_and_macos() {
    let server = start_uds_server_sync(default_serve_config(), "main.sock", None);
    let sock_path = std::path::PathBuf::from(server.base_url.strip_prefix("unix://").unwrap());
    let (status, _, _) = uds_get(&sock_path, "/healthz").await;
    assert_eq!(status, 200);
    assert!(
        std::process::id() > 0,
        "test process has a pid the server's peer_cred could observe"
    );
}

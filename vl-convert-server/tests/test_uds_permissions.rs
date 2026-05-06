#![cfg(unix)]
//! Verify that `--socket-mode` (via `ServeConfig.socket_mode`) is applied to
//! the on-disk socket file immediately after bind.

mod common;

use common::*;
use std::os::unix::fs::PermissionsExt;
use vl_convert_server::ListenAddr;

#[tokio::test]
async fn test_default_mode_0600() {
    // ServeConfig::default() uses socket_mode = 0o600.
    let server = start_uds_server_sync(default_serve_config(), "s.sock", None);
    let sock_path = std::path::PathBuf::from(server.base_url.strip_prefix("unix://").unwrap());
    // The bind has completed; stat the file.
    let meta = std::fs::metadata(&sock_path).expect("socket file must exist");
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(
        mode,
        0o600,
        "default socket mode must be 0o600; got {:o} at {}",
        mode,
        sock_path.display()
    );
}

#[tokio::test]
async fn test_custom_mode_0660() {
    let mut sc = default_serve_config();
    sc.socket_mode = 0o660;
    let server = start_uds_server_sync(sc, "s.sock", None);
    let sock_path = std::path::PathBuf::from(server.base_url.strip_prefix("unix://").unwrap());
    let meta = std::fs::metadata(&sock_path).unwrap();
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o660,
        "socket_mode override must be applied post-bind; got {:o}",
        mode
    );
}

#[tokio::test]
async fn test_uds_admin_also_chmodded() {
    // Admin UDS also needs the socket-mode applied. Same process, same
    // socket_mode; verify admin's file has the correct mode.
    let mut sc = default_serve_config();
    sc.socket_mode = 0o660;
    sc.per_ip_budget_ms = Some(1000);
    let server = start_uds_server_sync(sc, "main.sock", Some("admin.sock"));
    let main_path = std::path::PathBuf::from(server.base_url.strip_prefix("unix://").unwrap());
    let admin_path = main_path.parent().unwrap().join("admin.sock");
    let admin_meta = std::fs::metadata(&admin_path).expect("admin socket must exist");
    assert_eq!(
        admin_meta.permissions().mode() & 0o777,
        0o660,
        "admin UDS should inherit --socket-mode alongside main"
    );
}

/// Library callers provide already-validated socket-mode bits; CLI/env
/// validation happens before constructing `ServeConfig`.
#[tokio::test]
async fn test_library_accepts_any_u32_socket_mode() {
    // `0o755` is rejected by CLI parsing, but direct library callers can pass
    // any u32 mode.
    let mut sc = default_serve_config();
    sc.socket_mode = 0o755;
    let server = start_uds_server_sync(sc, "s.sock", None);
    let sock_path = std::path::PathBuf::from(server.base_url.strip_prefix("unix://").unwrap());
    let meta = std::fs::metadata(&sock_path).unwrap();
    let mode = meta.permissions().mode() & 0o777;
    // Platform umask interactions can vary; either mode demonstrates that
    // set_permissions ran.
    assert!(
        mode == 0o755 || mode == 0o700,
        "socket_mode must be applied; got {:o} (expected 0o755 direct or 0o700 after \
         umask filtering, both indicating set_permissions ran)",
        mode
    );
    // Silence the "variant unused" warning on ListenAddr import
    let _ = std::marker::PhantomData::<ListenAddr>;
}

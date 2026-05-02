//! UDS HTTP helpers + subprocess lifecycle helpers for the
//! `test_serve_subprocess.rs` end-to-end suite.
//!
//! `uds_get` / `uds_post_json` / `uds_request` are duplicated verbatim
//! from `vl-convert-server/tests/common/mod.rs:255-338`. The duplicate
//! placement is deliberate per the design doc (option a-i): keep
//! `vl-convert-server`'s public API SemVer-clean, and use the
//! `CARGO_BIN_EXE_vl_convert` env var that's auto-set when compiling
//! tests under `vl-convert/tests/`.
//!
//! `spawn_serve_piped` / `read_ready_json` / `send_sigterm` /
//! `wait_with_timeout` are subprocess-lifecycle helpers ported from
//! v3's `vl-convert-server/tests/test_uds_e2e.rs` with `spawn_server_piped`
//! → `spawn_serve_piped` (binary is now `vl-convert`, subcommand is
//! `serve`).

#![allow(dead_code)]

use assert_cmd::prelude::*;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::Duration;

/// Wall-clock cap for "should become ready quickly" polls. The
/// binary warms up Deno workers during build_app which dominates the
/// startup time budget. Matches v3's `READY_TIMEOUT`.
pub const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// UDS HTTP GET helper. Returns `(status, body_bytes, headers)`.
///
/// reqwest has no UDS transport at workspace pin 0.11 — this uses raw
/// hyper + hyper-util + `tokio::net::UnixStream` instead.
pub async fn uds_get(
    sock_path: &std::path::Path,
    path: &str,
) -> (
    hyper::StatusCode,
    bytes::Bytes,
    hyper::HeaderMap<hyper::header::HeaderValue>,
) {
    uds_request(sock_path, hyper::Method::GET, path, None, &[]).await
}

/// UDS HTTP POST-JSON helper.
pub async fn uds_post_json(
    sock_path: &std::path::Path,
    path: &str,
    body: &serde_json::Value,
) -> (
    hyper::StatusCode,
    bytes::Bytes,
    hyper::HeaderMap<hyper::header::HeaderValue>,
) {
    let body_bytes = serde_json::to_vec(body).unwrap();
    uds_request(
        sock_path,
        hyper::Method::POST,
        path,
        Some(body_bytes),
        &[(
            hyper::header::CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("application/json"),
        )],
    )
    .await
}

/// Build and drive a single HTTP/1 request over a UDS connection.
pub async fn uds_request(
    sock_path: &std::path::Path,
    method: hyper::Method,
    path: &str,
    body: Option<Vec<u8>>,
    extra_headers: &[(hyper::header::HeaderName, hyper::header::HeaderValue)],
) -> (
    hyper::StatusCode,
    bytes::Bytes,
    hyper::HeaderMap<hyper::header::HeaderValue>,
) {
    use http_body_util::{BodyExt, Full};
    use hyper_util::rt::TokioIo;
    use tokio::net::UnixStream;

    let stream = UnixStream::connect(sock_path)
        .await
        .expect("UnixStream::connect failed");
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .expect("hyper handshake failed");
    tokio::spawn(async move {
        let _ = conn.await;
    });

    let mut req = hyper::Request::builder()
        .method(method)
        .uri(path)
        .header(hyper::header::HOST, "localhost");
    for (k, v) in extra_headers {
        req = req.header(k, v);
    }
    let req = req
        .body(Full::<bytes::Bytes>::from(body.unwrap_or_default()))
        .expect("request build failed");

    let resp = sender.send_request(req).await.expect("send_request failed");
    let (parts, incoming) = resp.into_parts();
    let body = incoming
        .collect()
        .await
        .expect("collect body failed")
        .to_bytes();
    (parts.status, body, parts.headers)
}

/// Spawn `vl-convert serve <args...>` with stdin/stdout/stderr piped
/// so we can both read the ready-JSON line and trigger EOF-based
/// shutdown later.
pub fn spawn_serve_piped(args: &[&str]) -> Child {
    let mut cmd = Command::cargo_bin("vl-convert").expect("vl-convert binary not built");
    cmd.arg("serve")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.spawn().expect("failed to spawn `vl-convert serve`")
}

/// Block until the server emits its ready-JSON line on stdout (or
/// until [`READY_TIMEOUT`] elapses). Returns the parsed JSON object.
pub fn read_ready_json(child: &mut Child) -> serde_json::Value {
    let stdout = child
        .stdout
        .take()
        .expect("spawned child must have stdout piped");
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        if reader.read_line(&mut line).is_ok() {
            let _ = tx.send(line);
        }
    });
    let line = rx
        .recv_timeout(READY_TIMEOUT)
        .expect("ready-JSON line did not appear within timeout");
    serde_json::from_str(line.trim())
        .unwrap_or_else(|e| panic!("ready-JSON was not valid JSON: {e} — raw: {line:?}"))
}

/// Send SIGTERM to a child process. Unix-only. Uses a minimal `kill(2)`
/// FFI to avoid pulling the full `libc` crate into dev-deps.
pub fn send_sigterm(child: &Child) {
    // SAFETY: `kill(2)` with a valid pid and a standard signal number
    // is a straightforward syscall with no memory-safety concerns.
    unsafe {
        signals::kill(child.id() as i32, signals::SIGTERM);
    }
}

/// Poll `try_wait` every 50ms because `Child::wait` is blocking.
/// Returns `Some(status)` on exit, or `None` if the timeout fires.
pub fn wait_with_timeout(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    child.try_wait().ok().flatten()
}

mod signals {
    // Minimal FFI: avoids pulling the full libc crate into dev-deps
    // just to send SIGTERM.
    pub const SIGTERM: i32 = 15;
    extern "C" {
        pub fn kill(pid: i32, sig: i32) -> i32;
    }
}

//! UDS HTTP helpers + subprocess lifecycle helpers for the
//! `test_serve_subprocess.rs` end-to-end suite.
//!
//! The UDS HTTP helpers live in this crate's tests so subprocess tests can
//! drive the `vl-convert` binary without exposing test-only helpers from
//! `vl-convert-server`.

#![allow(dead_code)]

use assert_cmd::prelude::*;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::Duration;

/// Wall-clock cap for subprocess readiness. Startup includes Deno worker
/// warm-up.
pub const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// UDS HTTP GET helper. Returns `(status, body_bytes, headers)`.
///
/// Uses raw hyper over `tokio::net::UnixStream` because reqwest 0.11 has no
/// UDS transport.
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

/// Spawn `vl-convert serve <args...>` with piped stdin/stdout/stderr for
/// ready-JSON reads and EOF-based shutdown.
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
        .unwrap_or_else(|e| panic!("ready-JSON was not valid JSON: {e}; raw: {line:?}"))
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
    // Minimal FFI for sending SIGTERM without adding a libc dev-dependency.
    pub const SIGTERM: i32 = 15;
    extern "C" {
        pub fn kill(pid: i32, sig: i32) -> i32;
    }
}

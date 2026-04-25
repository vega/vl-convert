//! Subprocess-level end-to-end tests. Each test spawns the actual
//! `vl-convert-server` binary via `assert_cmd` and exercises its
//! CLI + lifecycle contracts: `--ready-json` stdout schema, UDS
//! cleanup on signal, stdin-EOF parent-death, double-bind rejection,
//! bind-failure producing no stdout, and the Windows parse-time
//! rejection of `--unix-socket`.
//!
//! Conversion workload is not exercised here (the in-process
//! integration tests cover that). These tests focus on signals,
//! stdout hygiene, and filesystem-side cleanup — behaviors that only
//! appear when a real child process is running.

use assert_cmd::prelude::*;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

// Wall-clock cap for "should become ready quickly" polls. The
// binary warms up Deno workers during build_app which dominates the
// startup time budget.
const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Spawn the server with a piped stdin + stdout + stderr so we can
/// both read the ready-JSON line and trigger EOF-based shutdown later.
fn spawn_server_piped(args: &[&str]) -> std::process::Child {
    let mut cmd = Command::cargo_bin("vl-convert-server").unwrap();
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.spawn().expect("failed to spawn vl-convert-server")
}

/// Block until the server emits its ready-JSON line on stdout (or
/// until `READY_TIMEOUT` elapses). Returns the parsed JSON object.
fn read_ready_json(child: &mut std::process::Child) -> serde_json::Value {
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

#[cfg(unix)]
#[test]
fn test_ready_json_parseable() {
    // Spawn with UDS + --ready-json; parse the single-line JSON object
    // and assert every required field from the schema.
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("e2e.sock");

    let mut child = spawn_server_piped(&[
        "--unix-socket",
        sock.to_str().unwrap(),
        "--ready-json",
        "--drain-timeout-secs",
        "2",
    ]);

    let v = read_ready_json(&mut child);

    assert_eq!(
        v["ready"],
        serde_json::Value::Bool(true),
        "ready must be true"
    );
    assert!(
        v["version"].is_string() && !v["version"].as_str().unwrap().is_empty(),
        "version must be a non-empty string"
    );
    assert!(
        v["pid"].as_u64().is_some_and(|p| p > 0),
        "pid must be a positive integer"
    );
    assert_eq!(
        v["listen"]["transport"].as_str(),
        Some("unix"),
        "listen.transport must be \"unix\", got: {:?}",
        v["listen"]
    );
    assert!(
        v["listen"]["url"]
            .as_str()
            .is_some_and(|s| s.starts_with("unix:///")),
        "listen.url must be a unix:/// URL, got: {:?}",
        v["listen"]
    );
    assert!(
        v["listen"]["path"]
            .as_str()
            .is_some_and(|s| s.starts_with('/')),
        "listen.path must be an absolute filesystem path, got: {:?}",
        v["listen"]
    );
    assert!(
        v["admin_listen"].is_null(),
        "admin_listen must be null when admin is disabled, got: {:?}",
        v["admin_listen"]
    );

    // Clean up.
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(unix)]
#[test]
fn test_cleanup_on_clean_exit() {
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("cleanup.sock");

    let mut child = spawn_server_piped(&[
        "--unix-socket",
        sock.to_str().unwrap(),
        "--ready-json",
        "--drain-timeout-secs",
        "2",
    ]);
    let _ = read_ready_json(&mut child);
    assert!(sock.exists(), "socket must exist while server is running");

    // Close stdin so the auto-enabled parent-close watcher takes the
    // "first-read EOF + not explicit" branch and disables itself.
    // That leaves SIGTERM as the only active shutdown trigger, which
    // is the path this test exercises.
    drop(child.stdin.take());

    send_sigterm(&child);
    let _exit = wait_with_timeout(&mut child, Duration::from_secs(10))
        .expect("server did not exit within 10s after SIGTERM");

    // Drop runs synchronously, but give macOS's stat cache a moment
    // to surface the unlink.
    for _ in 0..20 {
        if !sock.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if sock.exists() {
        use std::io::Read as _;
        let mut stderr_buf = String::new();
        if let Some(mut s) = child.stderr.take() {
            let _ = s.read_to_string(&mut stderr_buf);
        }
        panic!(
            "socket file still at {} after exit (status {:?}); server stderr:\n{stderr_buf}",
            sock.display(),
            _exit
        );
    }
}

#[cfg(unix)]
#[test]
fn test_parent_death_stdin_eof() {
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("parent-death.sock");

    let mut child = spawn_server_piped(&[
        "--unix-socket",
        sock.to_str().unwrap(),
        "--ready-json",
        "--exit-on-parent-close=true",
    ]);
    let _ = read_ready_json(&mut child);
    assert!(sock.exists());

    // Drop the child's stdin handle → EOF on the watcher's read.
    drop(child.stdin.take());

    let exit = wait_with_timeout(&mut child, Duration::from_secs(10))
        .expect("child should have exited within 10s of stdin EOF");
    assert!(
        !sock.exists(),
        "socket must be unlinked after stdin-EOF-triggered shutdown"
    );
    let _ = exit;
}

#[cfg(unix)]
#[test]
fn test_double_bind_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("dup.sock");

    // First server: standard spawn.
    let mut a = spawn_server_piped(&["--unix-socket", sock.to_str().unwrap(), "--ready-json"]);
    let _ = read_ready_json(&mut a);
    assert!(sock.exists());

    // Second server on the same path: must exit non-zero.
    let b = Command::cargo_bin("vl-convert-server")
        .unwrap()
        .args(["--unix-socket", sock.to_str().unwrap(), "--ready-json"])
        .output()
        .expect("failed to run second server");
    assert!(
        !b.status.success(),
        "second server on a live UDS path should exit non-zero; got status {:?}",
        b.status
    );
    let stderr = String::from_utf8_lossy(&b.stderr);
    assert!(
        stderr.to_lowercase().contains("in use"),
        "stderr should mention 'in use'; got: {stderr}"
    );

    // First server still running and socket still present.
    assert!(sock.exists(), "first server's socket should be preserved");

    let _ = a.kill();
    let _ = a.wait();
}

#[cfg(unix)]
#[test]
fn test_bind_failure_emits_no_ready() {
    // Target a path whose parent directory doesn't exist: bind fails
    // at the syscall layer before the ready-JSON emitter runs, so
    // stdout must stay empty.
    let missing_dir =
        std::path::PathBuf::from("/var/this/directory/cannot/possibly/exist/for/vlc/e2e");
    let sock = missing_dir.join("x.sock");

    let out = Command::cargo_bin("vl-convert-server")
        .unwrap()
        .args(["--unix-socket", sock.to_str().unwrap(), "--ready-json"])
        .output()
        .expect("failed to run server");

    assert!(
        !out.status.success(),
        "server should exit non-zero when bind fails; got status {:?}",
        out.status
    );
    assert_eq!(
        out.stdout.len(),
        0,
        "no ready-JSON should be emitted when bind fails; stdout had {} bytes: {:?}",
        out.stdout.len(),
        String::from_utf8_lossy(&out.stdout)
    );
}

/// Subprocess-level round-trip: spawn the binary with UDS main + UDS
/// admin, read ready-JSON, PATCH via the admin socket, then GET via the
/// admin socket and confirm `generation` bumped. Also verifies the public
/// `/infoz` surface (on the main socket) does NOT expose generation.
///
/// Uses raw hyper + UnixStream (reqwest has no UDS transport at our
/// pinned version). The UDS helpers in `common::` aren't used because
/// this test file doesn't import `common` — it's a subprocess e2e test.
#[cfg(unix)]
#[test]
fn test_admin_config_roundtrip_via_uds() {
    use std::os::unix::net::UnixStream;
    use std::io::{Read, Write};

    let tmp = tempfile::tempdir().unwrap();
    let main_sock = tmp.path().join("main.sock");
    let admin_sock = tmp.path().join("admin.sock");

    let mut child = spawn_server_piped(&[
        "--unix-socket",
        main_sock.to_str().unwrap(),
        "--admin-unix-socket",
        admin_sock.to_str().unwrap(),
        "--ready-json",
        "--drain-timeout-secs",
        "5",
    ]);

    let ready = read_ready_json(&mut child);
    assert_eq!(ready["ready"], serde_json::Value::Bool(true));

    // Helper: blocking HTTP/1.1 over a UDS socket. Keeps things simple —
    // no hyper client runtime needed.
    fn uds_request(
        sock_path: &std::path::Path,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> (u16, String) {
        let mut stream = UnixStream::connect(sock_path).expect("connect to UDS failed");
        stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(30))).unwrap();

        let body = body.unwrap_or("");
        let req = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(req.as_bytes()).unwrap();

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).unwrap();
        let raw = String::from_utf8_lossy(&buf).to_string();
        let (headers, body_str) = raw
            .split_once("\r\n\r\n")
            .map(|(h, b)| (h.to_string(), b.to_string()))
            .unwrap_or_else(|| (raw.clone(), String::new()));
        let status_line = headers.lines().next().unwrap_or("");
        // "HTTP/1.1 200 OK" → 200
        let status: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        (status, body_str)
    }

    // GET /admin/config on admin socket — baseline generation = 0.
    let (status, body) = uds_request(&admin_sock, "GET", "/admin/config", None);
    assert_eq!(status, 200, "admin GET must 200; body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["generation"], 0);
    assert_eq!(parsed["config_version"], 0);

    // PATCH /admin/config via admin UDS.
    let (status, _) = uds_request(
        &admin_sock,
        "PATCH",
        "/admin/config",
        Some(r#"{"default_theme":"dark"}"#),
    );
    assert_eq!(status, 200, "admin PATCH must 200");

    // GET /admin/config on admin socket — generation should be 1.
    let (status, body) = uds_request(&admin_sock, "GET", "/admin/config", None);
    assert_eq!(status, 200);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        parsed["generation"], 1,
        "admin PATCH must bump generation to 1"
    );
    assert_eq!(parsed["effective"]["default_theme"], "dark");

    // GET /infoz on main socket — MUST NOT expose generation.
    let (status, body) = uds_request(&main_sock, "GET", "/infoz", None);
    assert_eq!(status, 200);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(
        parsed.get("generation").is_none(),
        "/infoz must not expose generation; got: {body}"
    );

    // Clean up.
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
#[test]
fn test_cli_rejects_uds_on_windows() {
    let out = Command::cargo_bin("vl-convert-server")
        .unwrap()
        .args(["--unix-socket", "C:\\tmp\\nope.sock"])
        .output()
        .expect("failed to run server");

    assert!(
        !out.status.success(),
        "Windows --unix-socket should exit non-zero at parse time"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not supported on Windows"),
        "stderr should mention 'not supported on Windows'; got: {stderr}"
    );
}

// =========================================================================
// Helpers
// =========================================================================

#[cfg(unix)]
fn send_sigterm(child: &std::process::Child) {
    // SAFETY: `kill(2)` with a valid pid and a standard signal number
    // is a straightforward syscall with no memory-safety concerns.
    unsafe {
        signals::kill(child.id() as i32, signals::SIGTERM);
    }
}

/// Poll `try_wait` every 50ms because `Child::wait` is blocking.
fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Option<std::process::ExitStatus> {
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

#[cfg(unix)]
mod signals {
    // Minimal FFI: avoids pulling the full libc crate into dev-deps
    // just to send SIGTERM.
    pub const SIGTERM: i32 = 15;
    extern "C" {
        pub fn kill(pid: i32, sig: i32) -> i32;
    }
}

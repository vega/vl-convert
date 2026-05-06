//! Subprocess-level end-to-end tests for `vl-convert serve`.
//!
//! Each test spawns the actual `vl-convert` binary via `assert_cmd`
//! and exercises the serve subcommand's CLI + lifecycle contracts:
//! `--ready-json` stdout schema, UDS cleanup on signal, stdin-EOF
//! parent-death, bind-failure producing no stdout, and the Windows
//! parse-time rejection of `--unix-socket`.
//!
mod common;

#[cfg(unix)]
use common::uds::*;

#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
#[test]
fn test_ready_json_parseable() {
    // Spawn with UDS + --ready-json; parse the single-line JSON object
    // and assert every required field from the schema.
    let tmp = tempfile::tempdir().unwrap();
    // Keep the path short for platform UDS path limits.
    let sock = tmp.path().join("m.sock");

    let mut child = spawn_serve_piped(&[
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
    let sock = tmp.path().join("m.sock");

    let mut child = spawn_serve_piped(&[
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
    let exit = wait_with_timeout(&mut child, Duration::from_secs(10))
        .expect("server did not exit within 10s after SIGTERM");

    // Drop runs synchronously, but give macOS's stat cache a moment
    // to surface the unlink.
    for _ in 0..200 {
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
            exit
        );
    }
}

#[cfg(unix)]
#[test]
fn test_parent_death_stdin_eof() {
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("m.sock");

    let mut child = spawn_serve_piped(&[
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
    let _ = exit;

    // Allow filesystem stat cache a moment to settle (matches the
    // SIGTERM cleanup test).
    for _ in 0..200 {
        if !sock.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !sock.exists(),
        "socket must be unlinked after stdin-EOF-triggered shutdown"
    );
}

#[cfg(unix)]
#[test]
fn test_bind_failure_emits_no_ready() {
    use assert_cmd::prelude::*;
    use std::process::Command;

    // Target a path whose parent directory doesn't exist: bind fails
    // at the syscall layer before the ready-JSON emitter runs, so
    // stdout must stay empty.
    let missing_dir =
        std::path::PathBuf::from("/var/this/directory/cannot/possibly/exist/for/vlc/e2e");
    let sock = missing_dir.join("x.sock");

    let out = Command::cargo_bin("vl-convert")
        .unwrap()
        .args([
            "serve",
            "--unix-socket",
            sock.to_str().unwrap(),
            "--ready-json",
        ])
        .output()
        .expect("failed to run `vl-convert serve`");

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

#[cfg(windows)]
#[test]
fn test_cli_rejects_uds_on_windows() {
    use assert_cmd::prelude::*;
    use std::process::Command;

    let out = Command::cargo_bin("vl-convert")
        .unwrap()
        .args(["serve", "--unix-socket", "C:\\tmp\\nope.sock"])
        .output()
        .expect("failed to run `vl-convert serve`");

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

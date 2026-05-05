//! Subprocess-level integration tests for the `VLC_*` env-var
//! fallback layer.
//!
//! Coverage is split between this file and the in-process
//! `cli_types::tests` module:
//!
//! * **Here (subprocess)**: the port-precedence ladder
//!   (`--port` > `VLC_PORT` > `PORT` (PaaS) > `3000`), env-driven
//!   `--log-level=error` actually filtering Vega warnings off stderr,
//!   and `VLC_AUTO_GOOGLE_FONTS=true` round-tripping through
//!   `parse_boolish_arg` end-to-end.
//! * **In-process** (`vl-convert/src/cli_types.rs::tests`): Vec
//!   value-delimiter splitting (`VLC_FONT_DIR` PATH-style; `;`-delimited
//!   for `VLC_PLUGIN_IMPORT_DOMAINS`, `VLC_VEGA_PLUGIN`,
//!   `VLC_GOOGLE_FONT`, and `VLC_ALLOWED_BASE_URLS`),
//!   `VLC_ALLOWED_BASE_URLS` reserved-literal expansion (`none` /
//!   `net` / `all`), and `VLC_DRAIN_TIMEOUT_SECS` overriding the
//!   `default_value_t = 30` on `serve`'s `u64` field.
//!
//! Subprocess tests are unix-only because they spawn `vl-convert serve`
//! and exercise SIGTERM-driven shutdown — matching the gating on
//! `tests/test_serve_subprocess.rs`.
//!
//! All tests must run single-threaded — `cargo test -p vl-convert
//! --test test_env_vars -- --test-threads=1` — to avoid port collisions
//! and to keep each subprocess's exit isolated from the next.

#![cfg(unix)]
#![allow(dead_code)]

mod common;

use assert_cmd::prelude::*;
use common::uds::{read_ready_json, send_sigterm, wait_with_timeout};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

// =============================================================================
// helpers
// =============================================================================

/// Build a `vl-convert` binary command with a clean environment plus
/// the supplied overrides. `env_clear()` is critical for the
/// port-precedence ladder: any inherited `VLC_PORT` or `PORT` from the
/// developer's shell would corrupt the assertion.
fn vl_convert_cmd_with_env(env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::cargo_bin("vl-convert").expect("vl-convert binary not built");
    cmd.env_clear();
    // Cargo, rustc, and the linker all need PATH (and on macOS we need
    // DYLD_FALLBACK_LIBRARY_PATH for some toolchains). Re-export the
    // load-bearing entries explicitly so `env_clear()` doesn't break
    // dynamic linking on the spawned binary.
    for key in &[
        "PATH",
        "HOME",
        "LANG",
        "LC_ALL",
        "DYLD_FALLBACK_LIBRARY_PATH",
    ] {
        if let Ok(v) = std::env::var(key) {
            cmd.env(key, v);
        }
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd
}

/// Spawn `vl-convert serve <args...>` with the supplied env overrides,
/// stdin/stdout/stderr piped, ready to read the ready-JSON line.
fn spawn_serve_with_env(env: &[(&str, &str)], args: &[&str]) -> Child {
    let mut cmd = vl_convert_cmd_with_env(env);
    cmd.arg("serve")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.spawn().expect("failed to spawn `vl-convert serve`")
}

/// Pick a free TCP port on `127.0.0.1` and return it. We bind, query
/// the OS-assigned port, and drop the listener — there is a brief
/// TOCTOU window before the spawned `vl-convert serve` re-binds it,
/// but for `--test-threads=1` runs on a developer machine that window
/// is empirically reliable.
fn pick_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind ephemeral TCP port");
    let port = listener
        .local_addr()
        .expect("local_addr on ephemeral listener")
        .port();
    drop(listener);
    port
}

/// Spawn `vl-convert serve` with the given env, parse its ready-JSON,
/// extract the listening TCP port, then signal a graceful shutdown
/// and wait for exit. Returns the bound port.
fn spawn_and_read_port(env: &[(&str, &str)], extra_args: &[&str]) -> u16 {
    let mut args = vec![
        "--host",
        "127.0.0.1",
        "--ready-json",
        "--drain-timeout-secs",
        "1",
    ];
    args.extend_from_slice(extra_args);
    let mut child = spawn_serve_with_env(env, &args);
    let v = read_ready_json(&mut child);

    let port = v["listen"]["port"]
        .as_u64()
        .unwrap_or_else(|| panic!("listen.port missing or non-numeric in ready-JSON: {v}"))
        as u16;

    // Triggering SIGTERM and dropping stdin both fire shutdown; one
    // is enough but doing both keeps the test resilient if the watcher
    // logic ever shifts.
    send_sigterm(&child);
    drop(child.stdin.take());
    let _ = wait_with_timeout(&mut child, Duration::from_secs(10));
    port
}

// =============================================================================
// 1. port precedence ladder (4 sub-tests)
//
// Documented ladder: --port > VLC_PORT > PORT (PaaS) > 3000.
// Each rung is exercised with picked-free ports per case so test runs
// don't collide with real services or each other.
// =============================================================================

#[test]
fn test_port_precedence_cli_flag_wins_over_vlc_port_and_port() {
    let cli_port = pick_free_port();
    let vlc_port = pick_free_port();
    let paas_port = pick_free_port();
    let port = spawn_and_read_port(
        &[
            ("VLC_PORT", &vlc_port.to_string()),
            ("PORT", &paas_port.to_string()),
        ],
        &["--port", &cli_port.to_string()],
    );
    assert_eq!(
        port, cli_port,
        "--port (CLI flag) must win over VLC_PORT and PORT — got {port}"
    );
}

#[test]
fn test_port_precedence_vlc_port_wins_over_port() {
    let vlc_port = pick_free_port();
    let paas_port = pick_free_port();
    let port = spawn_and_read_port(
        &[
            ("VLC_PORT", &vlc_port.to_string()),
            ("PORT", &paas_port.to_string()),
        ],
        &[],
    );
    assert_eq!(
        port, vlc_port,
        "VLC_PORT must win over PORT (PaaS) when --port is unset — got {port}"
    );
}

#[test]
fn test_port_precedence_port_paas_fallback() {
    // When neither --port nor VLC_PORT is set, the `From<&ServeArgs>
    // for ServeConfig` impl falls back to the `PORT` env var (the
    // PaaS convention from Heroku/Railway/Fly/Render).
    let paas_port = pick_free_port();
    let port = spawn_and_read_port(&[("PORT", &paas_port.to_string())], &[]);
    assert_eq!(
        port, paas_port,
        "PORT (PaaS) must be honored when neither --port nor VLC_PORT is set — got {port}"
    );
}

#[test]
fn test_port_precedence_default_3000_when_nothing_set() {
    // Final rung: with no `--port`, no `VLC_PORT`, no `PORT`, the
    // default is `3000` (baked into `From<&ServeArgs> for ServeConfig`).
    // We cannot avoid binding 3000 here without changing the contract,
    // so the test best-efforts skips when 3000 is already in use on
    // the host (e.g. the developer is running a local app on it).
    let probe = TcpListener::bind("127.0.0.1:3000");
    if probe.is_err() {
        eprintln!(
            "test_port_precedence_default_3000_when_nothing_set: \
             port 3000 already in use on this host; skipping."
        );
        return;
    }
    drop(probe);

    let port = spawn_and_read_port(&[], &[]);
    assert_eq!(
        port, 3000,
        "with no --port, no VLC_PORT, no PORT, the default must be 3000 — got {port}"
    );
}

// =============================================================================
// 6. VLC_LOG_LEVEL value-enum through env (mirrors test_log_level_error_hides_warnings)
// =============================================================================

/// Tiny Vega-Lite spec that triggers a "Log scale" warning on stderr
/// when run with `--log-level=warn` (the default). Reused from
/// `tests/test_logging.rs`.
fn log_scale_spec() -> &'static str {
    r#"{
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "data": {"values": [{"a": "A", "v": 100}, {"a": "B", "v": 200}]},
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "v", "type": "quantitative", "scale": {"type": "log"}}
        }
    }"#
}

#[test]
fn test_vlc_log_level_error_suppresses_warnings() {
    use std::io::Write as _;
    let mut cmd = vl_convert_cmd_with_env(&[("VLC_LOG_LEVEL", "error")]);
    let mut child = cmd
        .arg("vl2svg")
        .arg("-o")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn vl-convert");

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(log_scale_spec().as_bytes()).unwrap();
    drop(stdin);
    let output = child.wait_with_output().expect("wait_with_output failed");

    assert!(
        output.status.success(),
        "vl-convert vl2svg should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Log scale"),
        "VLC_LOG_LEVEL=error must suppress Vega 'Log scale' warning; stderr: {stderr}"
    );
}

// =============================================================================
// 7. VLC_AUTO_GOOGLE_FONTS=true — boolish parser running on env values
// =============================================================================

#[test]
fn test_vlc_auto_google_fonts_true_succeeds() {
    // Sanity check that `value_parser = parse_boolish_arg` runs on
    // env-resolved values (not just CLI flag values). The boolish
    // parser would reject any unrecognized value at parse time, so a
    // successful conversion is sufficient evidence that `=true` was
    // accepted on the env path.
    use std::io::Write as _;
    let mut cmd = vl_convert_cmd_with_env(&[("VLC_AUTO_GOOGLE_FONTS", "true")]);
    let mut child = cmd
        .arg("vl2svg")
        .arg("-o")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn vl-convert");

    // Use a minimal spec that doesn't require any Google Fonts so
    // we don't actually hit the network — we're only checking that
    // the env var parsed without rejection.
    let spec = r#"{
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "data": {"values": [{"a": 1}]},
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative"}}
    }"#;
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(spec.as_bytes()).unwrap();
    drop(stdin);
    let output = child.wait_with_output().expect("wait_with_output failed");

    assert!(
        output.status.success(),
        "VLC_AUTO_GOOGLE_FONTS=true must parse and the conversion must succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// =============================================================================
// 8. --vega-plugin inline-ESM rejection at parse time
//
// Phase 2 rationalization: making `;` a safe delimiter for
// `--vega-plugin` required dropping inline-ESM CLI support. The flag's
// `value_parser` rejects strings that don't look like a path or URL.
// =============================================================================

#[test]
fn test_vega_plugin_inline_esm_rejected_at_parse_time() {
    // Inline ESM is no longer accepted on the CLI flag; the
    // actionable error message points operators at `--vlc-config`
    // for inline plugins.
    let mut cmd = vl_convert_cmd_with_env(&[]);
    let output = cmd
        .arg("vl2svg")
        .arg("--vega-plugin")
        .arg("export default function(v){return v;}")
        .output()
        .expect("failed to run vl-convert vl2svg --vega-plugin <inline ESM>");

    assert!(
        !output.status.success(),
        "inline ESM on --vega-plugin must be rejected at parse time; \
         stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("must be a file path or URL"),
        "stderr should mention 'must be a file path or URL'; got: {stderr}"
    );
    assert!(
        stderr.contains("--vlc-config"),
        "stderr should redirect operators at --vlc-config for inline ESM; got: {stderr}"
    );
}

// =============================================================================
// `VLC_DRAIN_TIMEOUT_SECS=60` overriding `default_value_t = 30` is
// covered by an in-process parse test in `vl-convert/src/cli_types.rs::
// tests::vlc_drain_timeout_secs_overrides_default`. We verify that
// path in-process rather than via subprocess because the
// "Starting graceful drain" log line only fires if `serve()` is still
// running when the watchdog wakes up (it returns immediately when no
// requests are in-flight), which leaves no observable on stderr in
// the empty-server case.
// =============================================================================

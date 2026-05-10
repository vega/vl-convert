//! Subprocess-level integration tests for the `VLC_*` env-var
//! fallback layer.
//!
//! Coverage is split between this file and the in-process
//! `cli_types::tests` module:
//!
//! * **Here (subprocess)**: the port-precedence ladder
//!   (`--port` > `VLC_PORT` > `PORT` (PaaS) > `3000`), env-driven
//!   `--log-level=error` filtering Vega warnings off stderr,
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
//! Subprocess tests are Unix-only because they spawn `vl-convert serve`
//! and exercise SIGTERM-driven shutdown.
//!
//! Run with `--test-threads=1` to avoid port collisions and keep each
//! subprocess's exit isolated from the next.

#![cfg(unix)]
#![allow(dead_code)]

mod common;

use assert_cmd::prelude::*;
use common::uds::{read_ready_json, send_sigterm, wait_with_timeout};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Build a `vl-convert` binary command with a clean environment plus
/// the supplied overrides. `env_clear()` is critical for the
/// port-precedence ladder: any inherited `VLC_PORT` or `PORT` from the
/// developer's shell would corrupt the assertion.
fn vl_convert_cmd_with_env(env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::cargo_bin("vl-convert").expect("vl-convert binary not built");
    cmd.env_clear();
    // Preserve the environment entries required to spawn and dynamically link
    // the binary after `env_clear()`.
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

/// Pick a free TCP port on `127.0.0.1` and return it.
///
/// The caller immediately uses the port in a single-threaded subprocess test,
/// which keeps the bind/rebind race small enough for this suite.
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

// Port precedence ladder: --port > VLC_PORT > PORT (PaaS) > 3000.

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
    // The default-port contract requires binding 3000, so skip when the host
    // already has something listening there.
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

// VLC_LOG_LEVEL value-enum through env.

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

// VLC_AUTO_GOOGLE_FONTS=true exercises the env path through the boolish parser.

#[test]
fn test_vlc_auto_google_fonts_true_succeeds() {
    // `value_parser = parse_boolish_arg` must run on env-resolved values, not
    // only CLI flag values.
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

    // Use a minimal spec that does not require Google Fonts; the assertion is
    // that the env var parsed without rejection.
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

// --vega-plugin accepts paths and URLs; inline ESM is rejected at parse time.

#[test]
fn test_vega_plugin_inline_esm_rejected_at_parse_time() {
    // Inline ESM is rejected on the CLI flag; the error message points
    // operators at `--vlc-config` for inline plugins.
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

// `VLC_DRAIN_TIMEOUT_SECS=60` is covered by the in-process parse test because
// the empty subprocess case exits before the drain watchdog logs.

// `--admin-host` non-loopback success and failure paths. The clap-error
// case (missing --admin-port) is covered in the in-process
// `cli_types::tests` suite to keep the subprocess-spawn count small.

#[test]
fn test_admin_host_env_var_binds_non_loopback() {
    use std::io::Read;

    // `0.0.0.0` is bindable on every test platform and is non-loopback,
    // so it exercises both the propagation path and the validate_serve_config
    // "non-loopback admin requires admin_api_key" rule.
    let main_port = pick_free_port();
    let admin_port = pick_free_port();
    let mut child = spawn_serve_with_env(
        &[
            ("VLC_ADMIN_HOST", "0.0.0.0"),
            ("VLC_ADMIN_PORT", &admin_port.to_string()),
            ("VLC_ADMIN_API_KEY", "secret"),
        ],
        &[
            "--host",
            "127.0.0.1",
            "--port",
            &main_port.to_string(),
            "--ready-json",
            "--drain-timeout-secs",
            "1",
        ],
    );
    let v = read_ready_json(&mut child);

    let admin_host = v["admin_listen"]["host"]
        .as_str()
        .unwrap_or_else(|| panic!("admin_listen.host missing or non-string in ready-JSON: {v}"));
    assert_eq!(
        admin_host, "0.0.0.0",
        "VLC_ADMIN_HOST=0.0.0.0 must propagate to admin_listen.host"
    );

    send_sigterm(&child);
    drop(child.stdin.take());
    let _ = wait_with_timeout(&mut child, Duration::from_secs(10));
    // Drain stderr so the child's pipe doesn't block on close.
    let mut buf = Vec::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_end(&mut buf);
    }
}

#[test]
fn test_admin_host_without_admin_api_key_fails_fast() {
    let admin_port = pick_free_port();
    let output = vl_convert_cmd_with_env(&[
        ("VLC_ADMIN_HOST", "0.0.0.0"),
        ("VLC_ADMIN_PORT", &admin_port.to_string()),
    ])
    .arg("serve")
    .arg("--host")
    .arg("127.0.0.1")
    .arg("--port")
    .arg(pick_free_port().to_string())
    .arg("--drain-timeout-secs")
    .arg("1")
    .output()
    .expect("failed to run vl-convert serve");

    assert!(
        !output.status.success(),
        "non-loopback admin without admin_api_key must fail fast; \
         stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("admin listener bound to non-loopback"),
        "stderr should carry the validate_serve_config bail message; got: {stderr}"
    );
}

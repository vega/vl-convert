//! `vl-convert serve` subcommand: lifecycle wiring.
//!
//! This module defines [`ServeArgs`] (a [`clap::Args`] struct holding
//! all serve-local flags) and [`run_serve`], a near-verbatim port of
//! v3's `main.rs` wired through the conversion CLI's globals.
//!
//! The lifecycle ordering is load-bearing — see the comments inside
//! `run_serve` for the SIGTERM-pre-spawn invariant, the ready-JSON
//! single-writer invariant, and the drain-watchdog escalation path.
//! Notes for downstream binary authors live at the bottom of
//! `vl-convert-server/CLAUDE.md`; this module is the canonical
//! reference implementation those notes point to.
use std::io::Write as _;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::PathBuf;
use std::time::Duration;

use clap::{ArgGroup, Args};
use vl_convert_rs::anyhow::{self};
use vl_convert_rs::converter::VlcConfig;
use vl_convert_server::{
    bind_listener, build_app, serve as serve_app, BoundListener, EndpointInfo, ListenAddr,
    ServeConfig,
};

use crate::cli_types::Cli;
use crate::io_utils::parse_boolish_arg;

/// Rejection message emitted by [`parse_socket_path_arg`] on Windows.
/// Inlined verbatim from v3's `vl-convert-server/src/listen.rs::WINDOWS_UDS_REJECTION`
/// (which is `pub(crate)` and `#[cfg(windows)]`-only, so it can't be
/// imported across crate boundaries today). Keep these strings in sync
/// if either changes.
#[cfg(windows)]
pub(crate) const WINDOWS_UDS_REJECTION: &str =
    "--unix-socket PATH listeners are not supported on Windows. \
     Use --port PORT instead.";

/// Value parser for `--unix-socket` / `--admin-unix-socket`.
///
/// Ported from v3 `parse_socket_path_arg`:
/// * Trims and tilde-expands the input.
/// * Rejects empty paths and relative paths (with an actionable message).
/// * On `cfg(windows)`, rejects every invocation with the v3 message
///   pointing the user at `--port`.
pub(crate) fn parse_socket_path_arg(raw: &str) -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let _ = raw;
        return Err(WINDOWS_UDS_REJECTION.to_string());
    }

    #[cfg(not(windows))]
    {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("socket path must not be empty".to_string());
        }
        let expanded = PathBuf::from(shellexpand::tilde(trimmed).to_string());
        if !expanded.is_absolute() {
            return Err(format!(
                "socket path '{raw}' must be absolute (use /abs/path or ~/path)"
            ));
        }
        Ok(expanded)
    }
}

/// Value parser for `--socket-mode`.
///
/// Ported from v3 `parse_socket_mode_arg`. Accepts a 3-or-4 digit
/// octal literal (with an optional `0o`/`0O` prefix), parses via
/// `u32::from_str_radix(_, 8)`, and rejects:
///
/// * the all-zero mode (`0o000`),
/// * any value with `other` bits set (`mode & 0o007 != 0`),
/// * any value outside the `0o777` permission range.
///
/// Group-bit warnings are emitted at bind time (loopback advisory),
/// not here.
pub(crate) fn parse_socket_mode_arg(raw: &str) -> Result<u32, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("socket mode must not be empty".to_string());
    }
    // Strip an optional leading `0o`/`0O` so users can write either
    // `600` (shell-friendly) or `0o600` (Rust-friendly).
    let body = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
        .unwrap_or(trimmed);
    let mode = u32::from_str_radix(body, 8)
        .map_err(|err| format!("invalid octal socket mode '{raw}': {err}"))?;
    if mode == 0 {
        return Err("socket mode must not be 0o000 (unusable permissions)".to_string());
    }
    if mode & 0o007 != 0 {
        return Err(format!(
            "socket mode {raw} grants access to 'other' users; \
             drop the last octal digit's lower three bits (e.g. 0600, 0660, 0770)"
        ));
    }
    if mode & !0o777 != 0 {
        return Err(format!(
            "socket mode {raw} sets bits outside the 0o777 permission range"
        ));
    }
    Ok(mode)
}

/// Value parser for `--workers`: positive-integer → `NonZeroUsize`.
/// Ported from v3 `parse_non_zero_usize_arg`. Rejects `0` at parse time.
pub(crate) fn parse_non_zero_usize_arg(raw: &str) -> Result<NonZeroUsize, String> {
    let parsed: usize = raw
        .trim()
        .parse()
        .map_err(|err| format!("invalid unsigned integer '{raw}': {err}"))?;
    NonZeroUsize::new(parsed).ok_or_else(|| "must be a positive integer (>= 1)".to_string())
}

/// Value parser for positive `i64` budget knobs (`--budget-hold-ms`).
/// Rejects zero and negative values at parse time.
pub(crate) fn parse_positive_i64_arg(raw: &str) -> Result<i64, String> {
    let parsed: i64 = raw
        .trim()
        .parse()
        .map_err(|err| format!("invalid integer '{raw}': {err}"))?;
    if parsed <= 0 {
        return Err("must be positive".to_string());
    }
    Ok(parsed)
}

/// Serve-local flags for the `vl-convert serve` subcommand.
///
/// Globals (logging, font dirs, allowed-base-urls, etc.) live on
/// [`Cli`](crate::cli_types::Cli). The flags here are
/// listener/auth/budget/lifecycle/per-request-gate concerns that have
/// no meaning outside an HTTP server.
///
/// Listener arg-group conflicts mirror v3:
///
/// * `--unix-socket` is in `ArgGroup("main_listener")` and conflicts
///   with `--host`/`--port`.
/// * `--admin-unix-socket` is in `ArgGroup("admin_listener")` and
///   conflicts with `--admin-port`.
#[derive(Debug, Args)]
#[command(group(ArgGroup::new("main_listener").multiple(true).required(false)))]
#[command(group(ArgGroup::new("admin_listener").multiple(true).required(false)))]
pub(crate) struct ServeArgs {
    /// Bind address for the main HTTP listener (TCP). Mutually
    /// exclusive with `--unix-socket`.
    #[arg(long, group = "main_listener", value_name = "HOST")]
    pub(crate) host: Option<String>,

    /// Port for the main HTTP listener (TCP). Mutually exclusive with
    /// `--unix-socket`.
    #[arg(long, group = "main_listener", value_name = "PORT")]
    pub(crate) port: Option<u16>,

    /// Bind the main HTTP listener on a UDS path instead of TCP
    /// (Unix only). Mutually exclusive with `--host` and `--port`.
    #[arg(
        long,
        value_name = "PATH",
        value_parser = parse_socket_path_arg,
        group = "main_listener",
        conflicts_with_all = ["host", "port"],
    )]
    pub(crate) unix_socket: Option<PathBuf>,

    /// Unix permission mode for UDS listeners (octal, e.g. 0600).
    /// Defaults to 0o600 when unset.
    #[arg(long, value_name = "OCTAL", value_parser = parse_socket_mode_arg)]
    pub(crate) socket_mode: Option<u32>,

    /// Bind an admin listener on `127.0.0.1:<port>` for runtime
    /// reconfiguration. Mutually exclusive with `--admin-unix-socket`.
    #[arg(long, group = "admin_listener", value_name = "PORT")]
    pub(crate) admin_port: Option<u16>,

    /// Bind the admin HTTP listener on a UDS path instead of TCP
    /// (Unix only). Mutually exclusive with `--admin-port`.
    #[arg(
        long,
        value_name = "PATH",
        value_parser = parse_socket_path_arg,
        group = "admin_listener",
        conflicts_with = "admin_port",
    )]
    pub(crate) admin_unix_socket: Option<PathBuf>,

    /// API key for Bearer-token authentication on the admin listener.
    /// Independent of `--api-key`. When unset, the admin surface is
    /// listener-gated only (UDS filesystem permissions, or TCP
    /// loopback). Non-loopback TCP admin without a key fails startup.
    #[arg(long, value_name = "KEY")]
    pub(crate) admin_api_key: Option<String>,

    /// API key for Bearer-token authentication on the main listener.
    #[arg(long, value_name = "KEY")]
    pub(crate) api_key: Option<String>,

    /// Number of converter worker threads (must be >= 1). Defaults to
    /// the value loaded from `--vlc-config`, which itself defaults to
    /// the library default (= 1).
    #[arg(long, value_parser = parse_non_zero_usize_arg, value_name = "N")]
    pub(crate) workers: Option<NonZeroUsize>,

    /// Maximum simultaneous in-flight requests.
    #[arg(long, value_name = "N")]
    pub(crate) max_concurrent_requests: Option<usize>,

    /// HTTP request timeout in seconds.
    #[arg(long, value_name = "SECS")]
    pub(crate) request_timeout_secs: Option<u64>,

    /// Graceful shutdown drain timeout in seconds (default: 30).
    #[arg(long, value_name = "SECS", default_value_t = 30)]
    pub(crate) drain_timeout_secs: u64,

    /// Per-reconfig drain timeout in seconds. Defaults to the same
    /// value as `--drain-timeout-secs`.
    #[arg(long, value_name = "SECS")]
    pub(crate) reconfig_drain_timeout_secs: Option<u64>,

    /// Maximum request body size in megabytes.
    #[arg(long, value_name = "MB")]
    pub(crate) max_body_size_mb: Option<usize>,

    /// Return only HTTP status codes on error (no message bodies).
    #[arg(
        long,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
    )]
    pub(crate) opaque_errors: Option<bool>,

    /// Reject requests without a User-Agent header.
    #[arg(
        long,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
    )]
    pub(crate) require_user_agent: Option<bool>,

    /// Trust X-Forwarded-For and X-Real-IP headers for client IP
    /// extraction. Only enable behind a trusted reverse proxy.
    #[arg(
        long,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
    )]
    pub(crate) trust_proxy: Option<bool>,

    /// Allowed CORS origin(s), comma-separated or `*`.
    #[arg(long, value_name = "ORIGIN")]
    pub(crate) cors_origin: Option<String>,

    /// Conversion-time budget per IP, in milliseconds per minute.
    #[arg(long, value_name = "MS")]
    pub(crate) per_ip_budget_ms: Option<u64>,

    /// Total conversion-time budget for the server, in milliseconds
    /// per minute.
    #[arg(long, value_name = "MS")]
    pub(crate) global_budget_ms: Option<u64>,

    /// Per-request budget hold in milliseconds. Must be positive.
    #[arg(long, value_parser = parse_positive_i64_arg, value_name = "MS")]
    pub(crate) budget_hold_ms: Option<i64>,

    /// Emit one JSON readiness line on stdout after all listeners
    /// bind. Useful for orchestrators / process supervisors.
    #[arg(
        long,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
        default_value_t = false,
    )]
    pub(crate) ready_json: bool,

    /// Exit when the parent process closes stdin (auto-enabled when a
    /// UDS listener is in use). Pass `=true`/`=false` to override the
    /// auto rule.
    #[arg(
        long,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
    )]
    pub(crate) exit_on_parent_close: Option<bool>,

    /// Allow per-request `google_fonts` overrides.
    #[arg(
        long,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
    )]
    pub(crate) allow_google_fonts: Option<bool>,

    /// Allow per-request Vega plugin overrides.
    #[arg(
        long,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
    )]
    pub(crate) allow_per_request_plugins: Option<bool>,

    /// Maximum concurrent ephemeral workers for per-request plugins.
    #[arg(long, value_name = "N")]
    pub(crate) max_ephemeral_workers: Option<u64>,

    /// Domains allowed for HTTP imports in per-request plugins.
    /// Comma-separated. May be specified multiple times.
    #[arg(long = "per-request-plugin-import-domains", value_name = "csv")]
    pub(crate) per_request_plugin_import_domains: Vec<String>,
}

/// Build a [`ServeConfig`] from parsed CLI args.
///
/// Listener resolution rules:
/// * `--unix-socket` → `ListenAddr::Uds`.
/// * Otherwise `ListenAddr::Tcp { host, port }` with `host` defaulting
///   to `127.0.0.1` and `port` defaulting to the PaaS `PORT` env var
///   (parsed as `u16`) or `3000` if `PORT` is unset / non-numeric.
/// * Admin listener mirrors the same rules; absent flags → `None`.
///
/// `socket_mode` defaults to `0o600` when unset.
/// `reconfig_drain_timeout_secs` defaults to the value of
/// `--drain-timeout-secs` when unset (so operators tuning only the
/// shutdown drain inherit a sensible default for reconfig drain).
///
/// `drain_timeout_secs` is a binary-author concern (governs the drain
/// watchdog around `serve()`) and lives on `ServeArgs`, not in the
/// produced `ServeConfig`. The watchdog reads it directly.
impl From<&ServeArgs> for ServeConfig {
    fn from(args: &ServeArgs) -> Self {
        // Main listener: UDS wins over TCP when set; otherwise build a
        // TCP target from --host / --port with PaaS PORT-env fallback.
        let main = match &args.unix_socket {
            #[cfg(unix)]
            Some(path) => ListenAddr::Uds { path: path.clone() },
            #[cfg(not(unix))]
            Some(_) => unreachable!("parse_socket_path_arg rejects on Windows"),
            None => {
                let host = args.host.clone().unwrap_or_else(|| "127.0.0.1".to_string());
                let port = args.port.unwrap_or_else(|| {
                    // PaaS convention: Railway/Heroku/Fly/Render/Cloud Run
                    // all inject PORT. Silently fall through to 3000 on a
                    // non-numeric value rather than failing startup on an
                    // unrelated env-var collision.
                    std::env::var("PORT")
                        .ok()
                        .and_then(|s| s.parse::<u16>().ok())
                        .unwrap_or(3000)
                });
                ListenAddr::Tcp { host, port }
            }
        };

        // Admin listener: --admin-unix-socket wins; otherwise a
        // loopback-TCP listener if --admin-port is set; otherwise None.
        let admin: Option<ListenAddr> = match &args.admin_unix_socket {
            #[cfg(unix)]
            Some(path) => Some(ListenAddr::Uds { path: path.clone() }),
            #[cfg(not(unix))]
            Some(_) => unreachable!("parse_socket_path_arg rejects on Windows"),
            None => args.admin_port.map(ListenAddr::loopback_tcp),
        };

        let socket_mode = args.socket_mode.unwrap_or(0o600);
        let reconfig_drain_timeout_secs = args
            .reconfig_drain_timeout_secs
            .unwrap_or(args.drain_timeout_secs);

        let mut cfg = ServeConfig {
            main,
            admin,
            api_key: args.api_key.clone(),
            admin_api_key: args.admin_api_key.clone(),
            cors_origin: args.cors_origin.clone(),
            max_concurrent_requests: args.max_concurrent_requests,
            // Numeric fields below have non-Option types on ServeConfig;
            // unset CLI flags fall through to ServeConfig::default().
            ..ServeConfig::default()
        };
        if let Some(v) = args.request_timeout_secs {
            cfg.request_timeout_secs = v;
        }
        if let Some(v) = args.max_body_size_mb {
            cfg.max_body_size_mb = v;
        }
        if let Some(v) = args.opaque_errors {
            cfg.opaque_errors = v;
        }
        if let Some(v) = args.require_user_agent {
            cfg.require_user_agent = v;
        }
        if let Some(v) = args.trust_proxy {
            cfg.trust_proxy = v;
        }
        // Budget knobs: CLI uses u64 for cleanliness; ServeConfig uses
        // i64 because budget_hold_ms can carry settlement deltas. The
        // `as i64` cast is lossless for any practical millisecond
        // budget — `i64::MAX ≈ 9.2 × 10¹⁸ ms ≈ 300 billion years`.
        if let Some(v) = args.per_ip_budget_ms {
            cfg.per_ip_budget_ms = Some(v as i64);
        }
        if let Some(v) = args.global_budget_ms {
            cfg.global_budget_ms = Some(v as i64);
        }
        if let Some(v) = args.budget_hold_ms {
            cfg.budget_hold_ms = v;
        }
        cfg.socket_mode = socket_mode;
        cfg.reconfig_drain_timeout_secs = reconfig_drain_timeout_secs;
        // `log_format` is a Cli global, not a serve-local flag;
        // `run_serve` injects it after this `From` impl returns.
        cfg
    }
}

/// Serialized into the single `--ready-json` line emitted to stdout.
///
/// Schema is byte-identical to v3 so subprocess parents written
/// against the v3 binary continue to parse correctly:
///
/// ```text
/// {
///   "ready":        true,
///   "version":      "<crate version>",
///   "pid":          <u32>,
///   "listen":       <endpoint-info>,
///   "admin_listen": <endpoint-info> | null
/// }
/// ```
///
/// `<endpoint-info>` is the internally-tagged
/// [`vl_convert_server::EndpointInfo`] form (`transport: "tcp" | "unix"`).
#[derive(serde::Serialize)]
struct ReadyJson<'a> {
    ready: bool,
    version: &'a str,
    pid: u32,
    listen: EndpointInfo,
    admin_listen: Option<EndpointInfo>,
}

/// Log a security advisory at startup if the listener configuration
/// looks risky:
///
/// - TCP non-loopback + no API key → warn (the listener is reachable
///   to any network client).
/// - UDS `0600` + no API key → silent (the intended safe default;
///   filesystem permissions are the trust boundary).
/// - UDS with any group-permission bit set + no API key → warn
///   recommending an API key, since the socket grants access beyond
///   the owning uid.
/// - Any configuration with an API key set → silent.
fn advise_listener_security(main: &BoundListener, serve_config: &ServeConfig) {
    if serve_config.api_key.is_some() {
        return;
    }

    match main {
        BoundListener::Tcp(_) => {
            if !main.is_loopback() {
                let endpoint = main.endpoint_label();
                log::warn!(
                    "Server binding to {endpoint} with no API key — accessible to any \
                     network client. Set --api-key to restrict access."
                );
            }
        }
        #[cfg(unix)]
        BoundListener::Uds(..) => {
            // Owner-only (0o600 / 0o700): no warning. Any group-permission
            // bit set (0o040 read, 0o020 write, 0o010 exec): warn about
            // unintended access, since group-exec on a socket grants
            // connect-directory traversal.
            if serve_config.socket_mode & 0o070 != 0 {
                log::warn!(
                    "UDS socket mode {:o} grants group access and no API key is set. \
                     Consider tightening --socket-mode or setting --api-key.",
                    serve_config.socket_mode
                );
            }
        }
    }
}

/// Security advisory for the admin listener:
///
/// - No admin listener configured → no-op.
/// - UDS admin without `--admin-api-key` → silent (filesystem 0o600 is the
///   trust boundary; matches the "UDS default is safe" rule from
///   [`advise_listener_security`]).
/// - TCP loopback admin without `--admin-api-key` → warn advisory (same
///   treatment as loopback main-listener without `--api-key`).
/// - Non-loopback TCP admin without `--admin-api-key` → soft warn here
///   only. The hard `bail!` lives in
///   [`vl_convert_server::build_app`]'s `validate_serve_config` and
///   already fired by the time we reach this advisory; the additional
///   log line gives operators a clearer message in the unlikely case
///   the validator's wording changes.
fn advise_admin_security(
    admin: Option<EndpointInfo>,
    serve_config: &ServeConfig,
) -> Result<(), anyhow::Error> {
    let Some(info) = admin else {
        return Ok(());
    };
    if serve_config.admin_api_key.is_some() {
        return Ok(());
    }
    match info {
        #[cfg(unix)]
        EndpointInfo::Unix { .. } => Ok(()),
        EndpointInfo::Tcp { host, url, .. } => {
            let parsed_ip = host.parse::<std::net::IpAddr>().ok();
            let is_loopback = matches!(host.as_str(), "localhost")
                || parsed_ip.is_some_and(|ip| ip.is_loopback());
            if is_loopback {
                log::warn!(
                    "Admin listener binding to {url} with no admin API key — \
                     loopback is still the trust boundary. Set --admin-api-key \
                     for defense-in-depth."
                );
            } else {
                // `validate_serve_config` already hard-bails on this
                // case before we reach here; treat the advisory as a
                // belt-and-braces log line.
                log::warn!(
                    "Admin listener at {url} is non-loopback and has no \
                     --admin-api-key set."
                );
            }
            Ok(())
        }
    }
}

/// Emit the readiness JSON signal on stdout. Exactly one line of
/// compact JSON terminated by `\n`, followed by an explicit flush so
/// parents blocked on `read_line()` unblock even through a
/// block-buffered pipe.
///
/// Emitted only when `--ready-json` was set; when disabled this
/// function is a no-op so stdout stays silent for the process
/// lifetime.
fn emit_ready_json_if_enabled(
    enabled: bool,
    main: &BoundListener,
    admin_endpoint: Option<EndpointInfo>,
) -> Result<(), anyhow::Error> {
    if !enabled {
        return Ok(());
    }
    let payload = ReadyJson {
        ready: true,
        version: env!("CARGO_PKG_VERSION"),
        pid: std::process::id(),
        listen: main.endpoint_info(),
        admin_listen: admin_endpoint,
    };
    let line = serde_json::to_string(&payload)
        .map_err(|e| anyhow::anyhow!("failed to serialize ready-JSON: {e}"))?;
    // The `vl-convert serve` subcommand reserves stdout for this
    // one-shot readiness line. Logs go to stderr (enforced by
    // `init_tracing`'s `.with_writer(std::io::stderr)`). Do not add
    // another stdout writer to this module.
    //
    // allow-ready-json-emitter
    println!("{line}");
    // Explicit flush — stdout is block-buffered when piped to a parent
    // (the subprocess use case). Without this, a parent blocked on
    // read_line() could wait for the buffer to fill before ever
    // seeing the readiness signal.
    std::io::stdout()
        .flush()
        .map_err(|e| anyhow::anyhow!("failed to flush ready-JSON: {e}"))?;
    Ok(())
}

/// Watch inherited stdin for EOF. Returns `Some(reason)` when the
/// parent closes stdin (cleanly or by dying), signaling the caller to
/// trigger graceful shutdown. Returns `None` to silently disable the
/// watcher (e.g., first-read-is-EOF on an auto-enabled invocation with
/// `/dev/null`-redirected stdin).
///
/// `explicit = true` means the user set `--exit-on-parent-close`. In
/// that case we honor their intent even on a first-read-EOF.
/// `explicit = false` means we auto-enabled because a listener is
/// UDS — there we degrade gracefully when stdin is already gone
/// (common under shell-backgrounded launches).
///
/// First-read EOF is interpreted by stdin's file type:
/// - **pipe (FIFO) or socket**: real IPC channel from a parent
///   process. EOF means the parent closed it (typically because it
///   exited) — fire shutdown so an orphaned UDS server doesn't
///   linger.
/// - **anything else** (tty, `/dev/null`, regular file, character
///   device): no parent is feeding bytes, so degrade silently when
///   `!explicit` to avoid the shell-backgrounded launch trap.
///
/// Once the watcher has read at least one byte, every subsequent EOF
/// is treated as parent-closed regardless of file type.
async fn watch_stdin_eof(explicit: bool) -> Option<&'static str> {
    use tokio::io::AsyncReadExt;

    // Probe stdin's file type once at startup. When `explicit` is true
    // the user has opted in to "EOF means shutdown" unconditionally,
    // so the probe is irrelevant.
    let stdin_is_parent_pipe = !explicit && stdin_is_pipe_or_socket();

    let mut stdin = tokio::io::stdin();
    let mut buf = [0u8; 256];
    let mut first_read = true;
    loop {
        match stdin.read(&mut buf).await {
            Ok(0) if first_read && !explicit && !stdin_is_parent_pipe => {
                log::warn!(
                    "stdin already closed at startup; auto-enabled \
                     --exit-on-parent-close disabled. Pass \
                     --exit-on-parent-close=true to force."
                );
                return None;
            }
            Ok(0) => return Some("parent closed stdin"),
            Ok(_) => {
                // Discard bytes — we don't interpret stdin content, only EOF.
                first_read = false;
            }
            Err(e) => {
                log::debug!("stdin watcher read error: {e}; watcher disabled");
                return None;
            }
        }
    }
}

/// On Unix, return `true` when fd 0 is a FIFO (pipe) or a socket —
/// the two file types that indicate "parent process is using stdin
/// as an IPC channel". A tty, regular file, character device, or
/// `/dev/null` returns `false`.
///
/// We probe via `/dev/stdin` rather than fstat(2) to avoid pulling
/// `libc` into the CLI dep graph for one call site. `/dev/stdin`
/// exists on Linux (symlink → `/proc/self/fd/0`) and macOS (symlink
/// → `fd/0`); `metadata()` follows the symlink and reports the
/// underlying file's type.
#[cfg(unix)]
fn stdin_is_pipe_or_socket() -> bool {
    use std::os::unix::fs::FileTypeExt;
    std::fs::metadata("/dev/stdin")
        .map(|m| {
            let ft = m.file_type();
            ft.is_fifo() || ft.is_socket()
        })
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn stdin_is_pipe_or_socket() -> bool {
    false
}

/// Entry point for `vl-convert serve`. Near-verbatim port of v3's
/// `main.rs`, adapted to take a pre-built `VlcConfig` from the global
/// flag pipeline and a parsed `ServeArgs` from clap.
///
/// Lifecycle:
///
/// 1. Apply per-request converter gates onto `base_config`.
/// 2. Apply the optional `--workers` override.
/// 3. Eagerly install the SIGTERM handler **before** any
///    `tokio::spawn` so an early signal can't take the process's
///    default disposition (terminate without cleanup). See
///    `vl-convert-server/CLAUDE.md` "Notes for downstream binary
///    authors".
/// 4. `build_app` validates `ServeConfig` (admin loopback) and warms
///    the worker pool.
/// 5. `bind_listener` applies the UDS lifecycle (probe-then-unlink,
///    chmod, cleanup-guard) for the main listener.
/// 6. Security advisories on stderr.
/// 7. `eprintln!` + `log::info!` the listening endpoint.
/// 8. Emit the optional `--ready-json` line on stdout (single
///    writer; followed by `flush()`).
/// 9. Spawn the signal task (Ctrl-C / SIGTERM) and the optional
///    stdin-EOF watcher; aggregate triggers via an mpsc channel into
///    a single `shutdown` future passed to `serve()`.
/// 10. Race `serve()` against the drain watchdog; the watchdog calls
///     `std::process::exit(1)` on timeout so it never returns.
pub(crate) async fn run_serve(
    cli: &Cli,
    mut base_config: VlcConfig,
    args: ServeArgs,
) -> anyhow::Result<()> {
    // Step 1 — per-request converter gates onto base_config. These four
    // mutate `VlcConfig` but are meaningless outside serve, so the main
    // CLI plumbing block leaves them alone and we apply them here.
    if let Some(v) = args.allow_google_fonts {
        base_config.allow_google_fonts = v;
    }
    if let Some(v) = args.allow_per_request_plugins {
        base_config.allow_per_request_plugins = v;
    }
    if let Some(v) = args.max_ephemeral_workers {
        // CLI shape is `Option<u64>`; library field is
        // `Option<NonZeroU64>`. `0` collapses to `None` (no cap).
        base_config.max_ephemeral_workers = NonZeroU64::new(v);
    }
    let per_request_domains =
        crate::io_utils::flatten_plugin_domains(&args.per_request_plugin_import_domains);
    if !per_request_domains.is_empty() {
        base_config.per_request_plugin_import_domains = per_request_domains;
    }

    // Step 2 — `--workers` override. The conversion path forces
    // `num_workers = 1` in main.rs but skips that line for `Serve`, so
    // here we either honor an explicit `--workers <N>` or leave the
    // value resolved from `--vlc-config` / library default in place.
    if let Some(n) = args.workers {
        // Library field is `NonZeroU64`; CLI parser produces
        // `NonZeroUsize`. Convert via `get()` because both are
        // documented to be >= 1.
        let n_u64 = u64::try_from(n.get())
            .map_err(|e| anyhow::anyhow!("--workers {} doesn't fit in u64: {e}", n.get()))?;
        base_config.num_workers =
            NonZeroU64::new(n_u64).expect("NonZeroUsize.get() is always >= 1");
    }

    // Step 3 — install SIGTERM handler eagerly, BEFORE any
    // `tokio::spawn`. `tokio::signal::unix::signal(...)` registers
    // the kernel handler synchronously at construction; deferring
    // it to a spawned task creates a window in which an early
    // SIGTERM takes the default disposition (terminate without
    // cleanup). The Signal binding is moved into the signal task
    // below.
    #[cfg(unix)]
    let mut sigterm_recv =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_err(|e| anyhow::anyhow!("failed to install SIGTERM handler: {e}"))?;

    // Step 4 — build the ServeConfig from args + globals (log_format
    // is a Cli global, not a serve-local flag).
    let mut serve_config: ServeConfig = (&args).into();
    serve_config.log_format = cli.log_format;

    // Step 5 — build the app (validates admin-loopback rule, warms
    // workers, binds admin listener if configured) then bind the main
    // listener.
    let built = build_app(base_config, &serve_config).await?;
    let listener: BoundListener =
        bind_listener(&serve_config.main, serve_config.socket_mode).await?;

    // Step 6 — security advisories. `advise_admin_security`'s hard
    // bail is already enforced inside `build_app` via
    // `validate_serve_config`, so here it's belt-and-braces logging.
    let endpoint = listener.endpoint_label();
    advise_listener_security(&listener, &serve_config);
    advise_admin_security(built.admin_endpoint_info(), &serve_config)?;

    // Step 7 — log the listening endpoint on both stderr (eprintln!
    // for unconditional human-visible output) and via tracing
    // (so structured-log consumers see it too).
    eprintln!("Listening on {endpoint}");
    log::info!("Listening on {endpoint}");

    // Step 8 — emit ready-JSON line on stdout if enabled. Must fire
    // BEFORE any subsequent await that could cancel (signal task
    // spawn, stdin watcher spawn) so parents blocked on `read_line()`
    // unblock promptly.
    emit_ready_json_if_enabled(args.ready_json, &listener, built.admin_endpoint_info())?;

    // Step 9 — shutdown plumbing.
    let drain_secs = args.drain_timeout_secs;
    let (signal_tx, signal_rx) = tokio::sync::oneshot::channel::<()>();

    // Decide whether to spawn the stdin-EOF watcher. Auto-enabled
    // when either listener is UDS (subprocess-style use case).
    // Explicit true/false from the user overrides auto-detection.
    #[allow(unused_mut, unused_assignments)]
    let mut main_is_uds = false;
    #[allow(unused_mut, unused_assignments)]
    let mut admin_is_uds = false;
    #[cfg(unix)]
    {
        main_is_uds = matches!(serve_config.main, ListenAddr::Uds { .. });
        admin_is_uds = matches!(serve_config.admin, Some(ListenAddr::Uds { .. }));
    }
    let watcher_explicit = args.exit_on_parent_close;
    let watcher_enabled = match watcher_explicit {
        Some(v) => v,
        None => main_is_uds || admin_is_uds,
    };

    // Shared shutdown channel. Up to three producers can fire it
    // (SIGINT, SIGTERM on Unix, stdin-EOF watcher); one consumer
    // (the aggregator below).
    let (shutdown_trigger_tx, mut shutdown_trigger_rx) =
        tokio::sync::mpsc::channel::<&'static str>(4);

    // Signal task: select between Ctrl-C and SIGTERM (Unix only).
    {
        let tx = shutdown_trigger_tx.clone();
        tokio::spawn(async move {
            let ctrl_c = tokio::signal::ctrl_c();
            #[cfg(unix)]
            {
                tokio::select! {
                    _ = ctrl_c => { let _ = tx.send("SIGINT").await; }
                    _ = sigterm_recv.recv() => { let _ = tx.send("SIGTERM").await; }
                }
            }
            #[cfg(not(unix))]
            {
                let _ = ctrl_c.await;
                let _ = tx.send("Ctrl-C").await;
            }
        });
    }

    // Stdin-EOF watcher (conditional). Preserves v3's first-read-EOF
    // auto-disable behavior (covers shell-backgrounded launches with
    // /dev/null redirect).
    if watcher_enabled {
        let tx = shutdown_trigger_tx.clone();
        let is_explicit = watcher_explicit == Some(true);
        tokio::spawn(async move {
            if let Some(reason) = watch_stdin_eof(is_explicit).await {
                let _ = tx.send(reason).await;
            }
        });
    }
    // Drop the original tx so the channel closes when all spawned
    // senders drop (otherwise the aggregator's `recv()` would hang
    // forever after every spawned sender exits).
    drop(shutdown_trigger_tx);

    // Aggregator: first trigger wins. Logs the reason, fires the
    // oneshot to wake the watchdog (which now starts its drain
    // sleep), then returns so `serve()` observes the shutdown
    // future resolving and begins its own graceful drain.
    let shutdown = async move {
        if let Some(reason) = shutdown_trigger_rx.recv().await {
            log::info!("{reason} — initiating graceful shutdown");
        }
        let _ = signal_tx.send(());
    };

    // Drain watchdog. Sleeps `drain_timeout_secs` after the first
    // shutdown trigger; if `serve()` hasn't returned by then, force
    // exit. `std::process::exit` skips Drop guards (UDS cleanup
    // etc.) — the next launch's probe-then-unlink in
    // `bind_listener` clears any stale file left behind.
    let watchdog = async move {
        if signal_rx.await.is_ok() {
            log::info!("Starting graceful drain ({drain_secs}s deadline)...");
            tokio::time::sleep(Duration::from_secs(drain_secs)).await;
            log::warn!("Drain timeout ({drain_secs}s) exceeded, forcing exit");
            std::process::exit(1);
        }
    };

    // Step 10 — race the serve future against the watchdog. The
    // watchdog branch is unreachable because the watchdog calls
    // `std::process::exit` before completing.
    tokio::select! {
        result = serve_app(listener, built, shutdown) => result,
        _ = watchdog => unreachable!("watchdog exits the process before returning"),
    }
}

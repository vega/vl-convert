//! `vl-convert serve` subcommand: lifecycle wiring.
//!
//! This module defines [`ServeArgs`] (a [`clap::Args`] struct holding
//! all serve-local flags) and [`run_serve`], which turns resolved CLI
//! config into a running `vl-convert-server` instance.
//!
//! `run_serve` owns process-level behavior that the library crate
//! intentionally leaves to callers: signal registration, the ready-JSON
//! stdout contract, parent-close detection, and drain-timeout escalation.
use std::io::Write as _;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::PathBuf;
use std::time::Duration;

use clap::{ArgGroup, Args, ValueEnum};
use vl_convert_rs::anyhow::{self};
use vl_convert_rs::converter::VlcConfig;
use vl_convert_server::{
    bind_listener, build_app, serve as serve_app, BoundListener, EndpointInfo, ListenAddr,
    ServeConfig,
};

use crate::cli_types::Cli;
use crate::io_utils::parse_boolish_arg;

/// Server OpenAPI surface to print for `--dump-openapi`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum OpenApiSurface {
    Public,
    Admin,
}

/// Rejection message emitted by [`parse_socket_path_arg`] on Windows.
#[cfg(windows)]
pub(crate) const WINDOWS_UDS_REJECTION: &str =
    "--unix-socket PATH listeners are not supported on Windows. \
     Use --port PORT instead.";

/// Value parser for `--unix-socket` / `--admin-unix-socket`.
///
/// * Trims and tilde-expands the input.
/// * Rejects empty and relative paths.
/// * Rejects every invocation on Windows.
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
/// Accepts a 3-or-4 digit octal literal with an optional `0o`/`0O`
/// prefix, then rejects:
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
/// Rejects `0` at parse time.
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

/// Value parser for non-negative `i64` budget caps.
/// Accepts zero to preserve the existing "disabled" budget dimension.
pub(crate) fn parse_budget_ms_arg(raw: &str) -> Result<i64, String> {
    let parsed: i64 = raw
        .trim()
        .parse()
        .map_err(|err| format!("invalid non-negative integer '{raw}': {err}"))?;
    if parsed < 0 {
        return Err("must be non-negative".to_string());
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
/// Listener arg groups make TCP and UDS binding mutually exclusive:
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
    #[arg(long, group = "main_listener", value_name = "HOST", env = "VLC_HOST")]
    pub(crate) host: Option<String>,

    /// Port for the main HTTP listener (TCP). Mutually exclusive with
    /// `--unix-socket`.
    #[arg(long, group = "main_listener", value_name = "PORT", env = "VLC_PORT")]
    pub(crate) port: Option<u16>,

    /// Bind the main HTTP listener on a UDS path instead of TCP
    /// (Unix only). Mutually exclusive with `--host` and `--port`.
    #[arg(
        long,
        value_name = "PATH",
        value_parser = parse_socket_path_arg,
        group = "main_listener",
        conflicts_with_all = ["host", "port"],
        env = "VLC_UNIX_SOCKET",
    )]
    pub(crate) unix_socket: Option<PathBuf>,

    /// Unix permission mode for UDS listeners (octal, e.g. 0600).
    /// Defaults to 0o600 when unset.
    #[arg(long, value_name = "OCTAL", value_parser = parse_socket_mode_arg, env = "VLC_SOCKET_MODE")]
    pub(crate) socket_mode: Option<u32>,

    /// Bind an admin listener on `127.0.0.1:<port>` for runtime
    /// reconfiguration. Mutually exclusive with `--admin-unix-socket`.
    #[arg(
        long,
        group = "admin_listener",
        value_name = "PORT",
        env = "VLC_ADMIN_PORT"
    )]
    pub(crate) admin_port: Option<u16>,

    /// Bind the admin HTTP listener on a UDS path instead of TCP
    /// (Unix only). Mutually exclusive with `--admin-port`.
    #[arg(
        long,
        value_name = "PATH",
        value_parser = parse_socket_path_arg,
        group = "admin_listener",
        conflicts_with = "admin_port",
        env = "VLC_ADMIN_UNIX_SOCKET",
    )]
    pub(crate) admin_unix_socket: Option<PathBuf>,

    /// API key for Bearer-token authentication on the admin listener.
    /// Independent of `--api-key`. When unset, the admin surface is
    /// listener-gated only (UDS filesystem permissions, or TCP
    /// loopback). Non-loopback TCP admin without a key fails startup.
    #[arg(long, value_name = "KEY", env = "VLC_ADMIN_API_KEY")]
    pub(crate) admin_api_key: Option<String>,

    /// Print an OpenAPI JSON document to stdout and exit. Defaults to
    /// the public API; pass `--dump-openapi=admin` for the admin API.
    #[arg(
        long,
        value_name = "SPEC",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "public",
        value_enum,
    )]
    pub(crate) dump_openapi: Option<OpenApiSurface>,

    /// API key for Bearer-token authentication on the main listener.
    #[arg(long, value_name = "KEY", env = "VLC_API_KEY")]
    pub(crate) api_key: Option<String>,

    /// Number of converter worker threads (must be >= 1). Defaults to
    /// the value loaded from `--vlc-config`, which itself defaults to
    /// the library default (= 1).
    #[arg(long, value_parser = parse_non_zero_usize_arg, value_name = "N", env = "VLC_WORKERS")]
    pub(crate) workers: Option<NonZeroUsize>,

    /// Maximum simultaneous in-flight requests.
    #[arg(long, value_name = "N", env = "VLC_MAX_CONCURRENT_REQUESTS")]
    pub(crate) max_concurrent_requests: Option<usize>,

    /// HTTP request timeout in seconds.
    #[arg(long, value_name = "SECS", env = "VLC_REQUEST_TIMEOUT_SECS")]
    pub(crate) request_timeout_secs: Option<u64>,

    /// Graceful shutdown drain timeout in seconds (default: 30).
    #[arg(
        long,
        value_name = "SECS",
        default_value_t = 30,
        env = "VLC_DRAIN_TIMEOUT_SECS"
    )]
    pub(crate) drain_timeout_secs: u64,

    /// Per-reconfig drain timeout in seconds. Defaults to the same
    /// value as `--drain-timeout-secs`.
    #[arg(long, value_name = "SECS", env = "VLC_RECONFIG_DRAIN_TIMEOUT_SECS")]
    pub(crate) reconfig_drain_timeout_secs: Option<u64>,

    /// Maximum request body size in megabytes.
    #[arg(long, value_name = "MB", env = "VLC_MAX_BODY_SIZE_MB")]
    pub(crate) max_body_size_mb: Option<usize>,

    /// Return only HTTP status codes on error (no message bodies).
    #[arg(
        long,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
        env = "VLC_OPAQUE_ERRORS",
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
        env = "VLC_REQUIRE_USER_AGENT",
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
        env = "VLC_TRUST_PROXY",
    )]
    pub(crate) trust_proxy: Option<bool>,

    /// Allowed CORS origin(s), comma-separated or `*`.
    #[arg(long, value_name = "ORIGIN", env = "VLC_CORS_ORIGIN")]
    pub(crate) cors_origin: Option<String>,

    /// Conversion-time budget per IP, in milliseconds per minute.
    #[arg(long, value_parser = parse_budget_ms_arg, value_name = "MS", env = "VLC_PER_IP_BUDGET_MS")]
    pub(crate) per_ip_budget_ms: Option<i64>,

    /// Total conversion-time budget for the server, in milliseconds
    /// per minute.
    #[arg(long, value_parser = parse_budget_ms_arg, value_name = "MS", env = "VLC_GLOBAL_BUDGET_MS")]
    pub(crate) global_budget_ms: Option<i64>,

    /// Per-request budget hold in milliseconds. Must be positive.
    #[arg(long, value_parser = parse_positive_i64_arg, value_name = "MS", env = "VLC_BUDGET_HOLD_MS")]
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
        env = "VLC_READY_JSON",
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
        env = "VLC_EXIT_ON_PARENT_CLOSE",
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
        env = "VLC_ALLOW_GOOGLE_FONTS",
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
        env = "VLC_ALLOW_PER_REQUEST_PLUGINS",
    )]
    pub(crate) allow_per_request_plugins: Option<bool>,

    /// Maximum concurrent ephemeral workers for per-request plugins.
    #[arg(long, value_name = "N", env = "VLC_MAX_EPHEMERAL_WORKERS")]
    pub(crate) max_ephemeral_workers: Option<u64>,

    /// Domains allowed for HTTP imports in per-request plugins.
    /// `;`-separated. May be specified multiple times.
    #[arg(
        long = "per-request-plugin-import-domains",
        value_name = "DOMAIN;DOMAIN;...",
        env = "VLC_PER_REQUEST_PLUGIN_IMPORT_DOMAINS",
        value_delimiter = ';'
    )]
    pub(crate) per_request_plugin_import_domains: Vec<String>,
}

impl ServeArgs {
    pub(crate) fn openapi_dump_surface(&self) -> Option<OpenApiSurface> {
        self.dump_openapi
    }
}

pub(crate) fn dump_openapi(surface: OpenApiSurface) -> anyhow::Result<()> {
    let spec = match surface {
        OpenApiSurface::Public => vl_convert_server::public_openapi(),
        OpenApiSurface::Admin => vl_convert_server::admin_openapi(),
    };
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer_pretty(&mut handle, &spec)?;
    writeln!(handle)?;
    Ok(())
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

        let socket_mode = args
            .socket_mode
            .unwrap_or_else(|| ServeConfig::default().socket_mode);
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
        if let Some(v) = args.per_ip_budget_ms {
            cfg.per_ip_budget_ms = Some(v);
        }
        if let Some(v) = args.global_budget_ms {
            cfg.global_budget_ms = Some(v);
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
/// - UDS `0600` + no API key → silent; filesystem permissions are the
///   trust boundary.
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
/// - UDS admin without `--admin-api-key` → silent; filesystem
///   permissions are the trust boundary.
/// - TCP loopback admin without `--admin-api-key` → warn.
/// - Non-loopback TCP admin without `--admin-api-key` → warn here if
///   reached; `build_app` validation rejects this configuration first.
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
                     loopback is still the trust boundary. Set \
                     --admin-api-key as a redundant guard."
                );
            } else {
                // `validate_serve_config` rejects this before bind.
                log::warn!(
                    "Admin listener at {url} is non-loopback and has no \
                     --admin-api-key set."
                );
            }
            Ok(())
        }
    }
}

/// Warn when the listener is non-loopback TCP and the operator hasn't
/// narrowed `allowed_base_urls` away from the library default.
fn advise_data_access_security(main: &BoundListener, allowed_base_urls_is_library_default: bool) {
    if !allowed_base_urls_is_library_default || main.is_loopback() {
        return;
    }
    let endpoint = main.endpoint_label();
    log::warn!(
        "Server binding to {endpoint} with the library-default \
         allowed_base_urls (any http: or https: URL is permitted, \
         including private-network targets). Pass \
         --allowed-base-urls=net to keep the same scheme allowlist \
         but make the choice explicit, or pass an explicit list / \
         --allowed-base-urls=none for production deployments."
    );
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
    // stdout is reserved for the ready-JSON line. Logs go to stderr.
    //
    // allow-ready-json-emitter
    println!("{line}");
    // Flush so a parent blocked on `read_line()` sees readiness
    // immediately through a block-buffered pipe.
    std::io::stdout()
        .flush()
        .map_err(|e| anyhow::anyhow!("failed to flush ready-JSON: {e}"))?;
    Ok(())
}

#[cfg(unix)]
fn spawn_signal_forwarder(
    tx: tokio::sync::mpsc::Sender<&'static str>,
) -> Result<(), anyhow::Error> {
    use signal_hook::consts::signal::{SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGINT, SIGTERM])
        .map_err(|e| anyhow::anyhow!("failed to install signal handlers: {e}"))?;
    std::thread::Builder::new()
        .name("vl-convert-signal-forwarder".to_string())
        .spawn(move || {
            if let Some(signal) = signals.forever().next() {
                let reason = match signal {
                    SIGINT => "SIGINT",
                    SIGTERM => "SIGTERM",
                    _ => "signal",
                };
                let _ = tx.blocking_send(reason);
            }
        })
        .map_err(|e| anyhow::anyhow!("failed to spawn signal forwarder: {e}"))?;
    Ok(())
}

#[cfg(not(unix))]
fn spawn_signal_forwarder(
    tx: tokio::sync::mpsc::Sender<&'static str>,
) -> Result<(), anyhow::Error> {
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = tx.send("Ctrl-C").await;
    });
    Ok(())
}

/// Watch inherited stdin for EOF. Returns `Some(reason)` when the
/// parent closes stdin (cleanly or by dying), signaling the caller to
/// trigger graceful shutdown. Returns `None` to silently disable the
/// watcher (e.g., first-read-is-EOF on an auto-enabled invocation with
/// `/dev/null`-redirected stdin).
///
/// `explicit = true` treats EOF as shutdown for every stdin file type.
/// In auto mode (`explicit = false`), a first-read EOF only shuts down
/// when stdin is a FIFO or socket.
///
/// First-read EOF is interpreted by stdin's file type:
/// - **pipe (FIFO) or socket**: real IPC channel from a parent
///   process. EOF means the parent closed it (typically because it
///   exited); fire shutdown so an orphaned UDS server doesn't
///   linger.
/// - **anything else** (tty, `/dev/null`, regular file, character
///   device): first-read EOF disables the auto watcher.
///
/// Once the watcher has read at least one byte, every subsequent EOF
/// is treated as parent-closed regardless of file type.
async fn watch_stdin_eof(explicit: bool) -> Option<&'static str> {
    use tokio::io::AsyncReadExt;

    // In explicit mode every EOF is a shutdown signal, so the file-type
    // probe is only needed for auto mode.
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
                // Discard bytes; stdin content is ignored and only EOF matters.
                first_read = false;
            }
            Err(e) => {
                log::debug!("stdin watcher read error: {e}; watcher disabled");
                return None;
            }
        }
    }
}

/// On Unix, return `true` when fd 0 is a FIFO (pipe) or a socket:
/// the two file types that indicate "parent process is using stdin
/// as an IPC channel". A tty, regular file, character device, or
/// `/dev/null` returns `false`.
///
/// `/dev/stdin` resolves to fd 0 on Linux and macOS; `metadata()`
/// follows that link and reports the underlying file type.
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

/// Entry point for `vl-convert serve`.
///
/// Lifecycle:
///
/// 1. Apply per-request converter gates onto `base_config`.
/// 2. Apply the optional `--workers` override.
/// 3. Install SIGINT/SIGTERM handling before spawning runtime tasks.
/// 4. Build and warm the server before binding the main listener.
/// 5. Bind the main listener.
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
    if let Some(surface) = args.openapi_dump_surface() {
        return dump_openapi(surface);
    }

    // Per-request gates only apply in serve mode, so they are injected here.
    if let Some(v) = args.allow_google_fonts {
        base_config.allow_google_fonts = v;
    }
    if let Some(v) = args.allow_per_request_plugins {
        base_config.allow_per_request_plugins = v;
    }
    if let Some(v) = args.max_ephemeral_workers {
        base_config.max_ephemeral_workers = NonZeroU64::new(v);
    }
    let per_request_domains =
        crate::io_utils::flatten_plugin_domains(&args.per_request_plugin_import_domains);
    if !per_request_domains.is_empty() {
        base_config.per_request_plugin_import_domains = per_request_domains;
    }

    if let Some(n) = args.workers {
        let n_u64 = u64::try_from(n.get())
            .map_err(|e| anyhow::anyhow!("--workers {} doesn't fit in u64: {e}", n.get()))?;
        base_config.num_workers =
            NonZeroU64::new(n_u64).expect("NonZeroUsize.get() is always >= 1");
    }

    let mut serve_config: ServeConfig = (&args).into();
    serve_config.log_format = cli.log_format;

    // Capture before `base_config` moves into `build_app`.
    let allowed_base_urls_is_library_default = base_config.allowed_base_urls == ["http:", "https:"];

    // Validate and warm before binding a public socket.
    let built = build_app(base_config, &serve_config).await?;
    let listener: BoundListener =
        bind_listener(&serve_config.main, serve_config.socket_mode).await?;

    let endpoint = listener.endpoint_label();
    advise_listener_security(&listener, &serve_config);
    advise_admin_security(built.admin_endpoint_info(), &serve_config)?;
    advise_data_access_security(&listener, allowed_base_urls_is_library_default);

    eprintln!("Listening on {endpoint}");
    log::info!("Listening on {endpoint}");

    // Shared shutdown channel. Up to three producers can fire it
    // (SIGINT, SIGTERM on Unix, stdin-EOF watcher); one consumer
    // (the aggregator below).
    let (shutdown_trigger_tx, mut shutdown_trigger_rx) =
        tokio::sync::mpsc::channel::<&'static str>(4);

    // Signal producer: forward OS signals into the same shutdown channel
    // used by stdin EOF. Install before ready JSON so parent processes can
    // signal immediately after readiness without losing cleanup.
    spawn_signal_forwarder(shutdown_trigger_tx.clone())?;

    // Must fire before any subsequent await that could cancel, so a
    // parent blocked on `read_line()` unblocks promptly.
    emit_ready_json_if_enabled(args.ready_json, &listener, built.admin_endpoint_info())?;
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

    // Stdin-EOF watcher (conditional).
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

    // Aggregator: first trigger wins. Logs the reason, fires the oneshot to
    // wake the watchdog, then returns so `serve()` observes the shutdown
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
    // etc.); the next launch's probe-then-unlink in
    // `bind_listener` clears any stale file left behind.
    let watchdog = async move {
        if signal_rx.await.is_ok() {
            log::info!("Starting graceful drain ({drain_secs}s deadline)...");
            tokio::time::sleep(Duration::from_secs(drain_secs)).await;
            log::warn!("Drain timeout ({drain_secs}s) exceeded, forcing exit");
            std::process::exit(1);
        }
    };

    // The watchdog exits the process on drain timeout. If its signal
    // channel closes without a shutdown trigger, return an error log
    // instead of panicking.
    tokio::select! {
        result = serve_app(listener, built, shutdown) => result,
        _ = watchdog => {
            log::error!(
                "watchdog returned without firing process::exit; \
                 signal sender dropped before the drain deadline."
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_args() -> ServeArgs {
        ServeArgs {
            host: None,
            port: None,
            unix_socket: None,
            socket_mode: None,
            admin_port: None,
            admin_unix_socket: None,
            admin_api_key: None,
            dump_openapi: None,
            api_key: None,
            workers: None,
            max_concurrent_requests: None,
            request_timeout_secs: None,
            drain_timeout_secs: 30,
            reconfig_drain_timeout_secs: None,
            max_body_size_mb: None,
            opaque_errors: None,
            require_user_agent: None,
            trust_proxy: None,
            cors_origin: None,
            per_ip_budget_ms: None,
            global_budget_ms: None,
            budget_hold_ms: None,
            ready_json: false,
            exit_on_parent_close: None,
            allow_google_fonts: None,
            allow_per_request_plugins: None,
            max_ephemeral_workers: None,
            per_request_plugin_import_domains: Vec::new(),
        }
    }

    #[test]
    fn parse_budget_ms_arg_accepts_non_negative_i64_values() {
        assert_eq!(parse_budget_ms_arg("0").unwrap(), 0);
        assert_eq!(parse_budget_ms_arg("1").unwrap(), 1);
        assert_eq!(
            parse_budget_ms_arg(&i64::MAX.to_string()).unwrap(),
            i64::MAX
        );
    }

    #[test]
    fn parse_budget_ms_arg_rejects_negative_and_overflow_values() {
        assert!(parse_budget_ms_arg("-1").is_err());

        let overflow = (i64::MAX as u128 + 1).to_string();
        assert!(parse_budget_ms_arg(&overflow).is_err());
    }

    #[test]
    fn serve_config_from_args_preserves_budget_values() {
        let mut args = default_args();
        args.per_ip_budget_ms = Some(0);
        args.global_budget_ms = Some(i64::MAX);

        let config = ServeConfig::from(&args);
        assert_eq!(config.per_ip_budget_ms, Some(0));
        assert_eq!(config.global_budget_ms, Some(i64::MAX));
    }
}

use axum::Router;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tracing_subscriber::EnvFilter;
use vl_convert_rs::anyhow;
use vl_convert_rs::converter::{VlConverter, VlcConfig};

use crate::budget::BudgetTracker;
use crate::listen::ListenAddr;
use crate::{health, json_fmt};

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default, PartialEq, Eq)]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}

pub fn init_tracing(filter: &str, format: LogFormat) {
    let filter: EnvFilter = filter.parse().expect("valid tracing filter directives");

    // Explicit stderr: tracing_subscriber::fmt() defaults to stdout,
    // which would collide with the --ready-json emitter's exclusive
    // ownership of stdout. Logs always go to stderr.
    match format {
        LogFormat::Json => {
            tracing_subscriber::fmt()
                .event_format(json_fmt::FlatJsonFormatter)
                .fmt_fields(tracing_subscriber::fmt::format::JsonFields::new())
                .with_env_filter(filter)
                .with_writer(std::io::stderr)
                .init();
        }
        LogFormat::Text => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(true)
                .with_writer(std::io::stderr)
                .init();
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// Main HTTP listener binding. TCP by default;
    /// `--unix-socket`/`VLC_UNIX_SOCKET` synthesizes the UDS variant.
    pub main: ListenAddr,
    /// Admin HTTP listener binding. `None` disables the admin
    /// router entirely. TCP (loopback) is the default when enabled;
    /// `--admin-unix-socket`/`VLC_ADMIN_UNIX_SOCKET` synthesizes UDS.
    pub admin: Option<ListenAddr>,
    /// Optional bearer token. When `Some`, `auth_middleware` rejects
    /// requests lacking `Authorization: Bearer <key>` with 401. `None`
    /// disables auth on the main listener; on UDS this is the intended
    /// default because filesystem permissions are the trust boundary.
    pub api_key: Option<String>,
    /// CORS `Access-Control-Allow-Origin` override. When `None`, the
    /// default predicate accepts only loopback origins (safe for local
    /// development). Ignored on UDS where browsers can't connect.
    pub cors_origin: Option<String>,
    /// Maximum number of requests in flight at once. Requests beyond
    /// the limit are rejected with 503 by the tower `ConcurrencyLimit`
    /// + `LoadShed` layers. `None` disables the cap.
    pub max_concurrent_requests: Option<usize>,
    /// Per-request deadline (seconds). Requests still running past
    /// this are aborted with 504 by the tower `TimeoutLayer`.
    pub request_timeout_secs: u64,
    /// Maximum request body size in MB. Larger bodies are rejected
    /// with 413. Applied via axum's `DefaultBodyLimit`.
    pub max_body_size_mb: usize,
    /// When `true`, error responses omit internal detail (exception
    /// messages, stack traces, internal paths) so they're safe to
    /// surface to untrusted callers. Intended for production
    /// deployments; development should keep this `false`.
    pub opaque_errors: bool,
    /// When `true`, `user_agent_middleware` rejects requests without
    /// a `User-Agent` header with 400. Useful for filtering unkeyed
    /// crawlers / health probes in shared environments.
    pub require_user_agent: bool,
    /// Log event format: human-readable `Text` or structured `Json`.
    /// Writer target is always stderr (stdout is reserved for the
    /// `--ready-json` emitter — see `init_tracing`).
    pub log_format: LogFormat,
    /// Per-peer-IP budget for rate limiting (milliseconds). `None`
    /// disables the per-IP dimension; the global dimension still
    /// applies if enabled. Each request reserves `budget_hold_ms` up
    /// front; the reservation settles against actual elapsed time at
    /// response time. See `budget::reserve` for the state machine.
    pub per_ip_budget_ms: Option<i64>,
    /// Shared-budget cap across all peers (milliseconds). `None`
    /// disables the global dimension. Useful as a host-CPU defense
    /// that applies regardless of transport.
    pub global_budget_ms: Option<i64>,
    /// Reservation size each request tentatively charges against both
    /// budget dimensions before settlement. Small values allow more
    /// concurrency; large values protect against request-duration
    /// outliers. Default 1000 (1s).
    pub budget_hold_ms: i64,
    /// When `true`, `extract_client_ip` honors `X-Forwarded-For` /
    /// `X-Envoy-External-Address` headers from upstream proxies
    /// (walking XFF right-to-left to land on the first trusted hop).
    /// When `false`, the socket peer IP is the authoritative client
    /// identity. Set to `true` only behind a trusted proxy that
    /// rewrites client IP.
    pub trust_proxy: bool,
    /// Permission bits applied to UDS sockets via `PermissionsExt::set_mode`
    /// immediately after bind. Ignored on TCP-only listeners and on
    /// Windows (where UDS is absent). Default `0o600`.
    pub socket_mode: u32,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            main: ListenAddr::Tcp {
                host: "127.0.0.1".to_string(),
                port: 3000,
            },
            admin: None,
            api_key: None,
            cors_origin: None,
            max_concurrent_requests: None,
            request_timeout_secs: 30,
            max_body_size_mb: 50,
            opaque_errors: false,
            require_user_agent: false,
            log_format: LogFormat::Text,
            per_ip_budget_ms: None,
            global_budget_ms: None,
            budget_hold_ms: 1000,
            trust_proxy: false,
            socket_mode: 0o600,
        }
    }
}

pub(crate) struct AppState {
    pub converter: VlConverter,
    pub config: VlcConfig,
    pub api_key: Option<ApiKey>,
    pub opaque_errors: bool,
    pub require_user_agent: bool,
    pub readiness: health::ReadinessState,
}

pub(crate) struct ApiKey(String);

impl ApiKey {
    pub fn new(key: String) -> Self {
        Self(key)
    }

    pub fn matches(&self, other: &[u8]) -> bool {
        let key_bytes = self.0.as_bytes();
        key_bytes.ct_eq(other).into()
    }
}

/// A fully-constructed server returned from [`crate::build_app`]. Pass to
/// [`crate::serve`] to run it, or call `router.oneshot(req)` directly for
/// `tower::ServiceExt`-style tests.
pub struct BuiltApp {
    /// The main app router with all middleware applied.
    pub router: Router,
    /// The underlying converter handle, for callers that want to drive
    /// conversions programmatically in addition to serving HTTP.
    pub converter: VlConverter,
    /// Budget tracker. Consumed by [`crate::serve`] to drive the refill loop.
    pub(crate) tracker: Option<Arc<BudgetTracker>>,
    /// Admin listener setup. Consumed by [`crate::serve`] to bind and serve.
    pub(crate) admin: Option<AdminConfig>,
}

impl BuiltApp {
    /// Endpoint label of the admin listener, if one was bound. Matches
    /// [`crate::BoundListener::endpoint_label`] output — canonical URL
    /// form for log lines.
    pub fn admin_endpoint(&self) -> Option<&str> {
        self.admin.as_ref().map(|a| a.addr.as_str())
    }

    /// Structured descriptor of the admin listener, if one was bound.
    /// Intended for readiness JSON emission so wrappers receive parsed
    /// `host`/`port` (TCP) or `path` (UDS) fields alongside the URL.
    pub fn admin_endpoint_info(&self) -> Option<crate::EndpointInfo> {
        self.admin.as_ref().map(|a| a.listener.endpoint_info())
    }
}

pub(crate) struct AdminConfig {
    pub listener: crate::listener::BoundListener,
    /// Human-readable label for the bound endpoint; equals
    /// `listener.endpoint_label()` at bind time. Stored separately so
    /// log lines don't re-query `local_addr()` on every spawn.
    pub addr: String,
    pub router: Router,
}

/// Apply server-safe defaults to a VlcConfig. Called from [`crate::build_app`]
/// so every entry into the server gets hardened defaults regardless of
/// whether the caller remembered to set them.
pub(crate) fn apply_server_defaults(config: &mut VlcConfig) {
    if config.allowed_base_urls.is_none() {
        config.allowed_base_urls = Some(vec![]);
        log::info!(
            "Data access disabled by default in server mode. \
             Use --data-access=allowlist with --allowed-base-urls to allow \
             specific URLs or file paths."
        );
    }
    if config.max_v8_heap_size_mb == 0 {
        config.max_v8_heap_size_mb = 512;
        log::info!(
            "Defaulting to 512MB V8 heap limit per worker \
             (override with --max-v8-heap-size-mb)"
        );
    }
    if config.allow_per_request_plugins && config.max_ephemeral_workers == 0 {
        config.max_ephemeral_workers = 2;
        log::info!(
            "Limiting ephemeral plugin workers to 2 \
             (override with --max-ephemeral-workers)"
        );
    }
}

pub(crate) fn validate_serve_config(serve_config: &ServeConfig) -> Result<(), anyhow::Error> {
    if serve_config.budget_hold_ms <= 0 {
        anyhow::bail!("budget_hold_ms must be positive");
    }

    Ok(())
}

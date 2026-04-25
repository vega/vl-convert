use arc_swap::ArcSwap;
use axum::Router;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;
use vl_convert_rs::anyhow;
use vl_convert_rs::converter::{VlConverter, VlcConfig};

use crate::budget::BudgetTracker;
use crate::listen::ListenAddr;
use crate::reconfig::ReconfigCoordinator;
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
    /// Optional bearer token for the admin listener. Independent of
    /// `api_key`. When `Some`, `admin_auth_middleware` rejects every
    /// admin request lacking the correct Bearer. When `None`, the admin
    /// surface is listener-gated only (UDS `0o600` or TCP loopback).
    /// Non-loopback TCP admin with no key bails at startup.
    pub admin_api_key: Option<String>,
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
    /// Per-reconfig drain timeout in seconds. Defaults to match the binary
    /// `--drain-timeout-secs` at resolve time. Distinct from the binary's
    /// shutdown drain because reconfig is admin-initiated and typically
    /// tolerates a longer wait than pod-eviction grace.
    pub reconfig_drain_timeout_secs: u64,
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
            admin_api_key: None,
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
            reconfig_drain_timeout_secs: 30,
        }
    }
}

/// Immutable point-in-time view of the converter + effective config.
///
/// Handlers call `state.runtime.load_full()` exactly once at entry and use the
/// returned `Arc<RuntimeSnapshot>` for the rest of the request. This gives each
/// request a stable view that cannot be swapped out from under it mid-flight,
/// which is important for multi-step conversions that might otherwise observe
/// two different `VlcConfig` values. The admin reconfig path replaces the
/// snapshot atomically via `ArcSwap::store`; in-flight requests keep their
/// old `Arc` alive until the next yield point.
pub(crate) struct RuntimeSnapshot {
    pub converter: VlConverter,
    pub config: Arc<VlcConfig>,
    /// Bumps on every rebuild-commit (drain+rebuild path). Exposed through
    /// admin endpoints; not used on the conversion hot path.
    //
    // Task 6 (admin.rs rewrite) and Task 8 (/admin/config surface) will read
    // this. Suppress the dead-code lint until then.
    #[allow(dead_code)]
    pub generation: u64,
    /// Bumps on every successful commit (including hot-apply paths that don't
    /// rebuild the converter). `generation <= config_version` always.
    #[allow(dead_code)]
    pub config_version: u64,
}

pub(crate) struct AppState {
    pub runtime: Arc<ArcSwap<RuntimeSnapshot>>,
    pub api_key: Option<ApiKey>,
    pub opaque_errors: bool,
    pub require_user_agent: bool,
    pub readiness: Arc<health::ReadinessState>,
    /// Shared with the gate middleware (main router) and admin handlers so
    /// every drain-participating actor works against the same coordinator.
    //
    // Task 4 (install gate middleware) and Task 6 (admin.rs rewrite) will be
    // the first consumers. Suppress the dead-code lint until they land.
    #[allow(dead_code)]
    pub coordinator: Arc<ReconfigCoordinator>,
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
    /// Atomic holder for the current converter + config. Cloned into
    /// `AppState` for the router; exposed through
    /// [`Self::current_converter`] / [`Self::current_config`] for callers
    /// that want to drive conversions programmatically alongside serving HTTP.
    pub(crate) runtime: Arc<ArcSwap<RuntimeSnapshot>>,
    /// Shared shutdown signal used by [`crate::serve`] and by the admin
    /// reconfig path (via `coordinator.shutdown_token()`). Populated in
    /// `build_app`; `serve()` clones it so cancellations observed by the
    /// coordinator also abort the main / admin listeners.
    pub(crate) shutdown_token: CancellationToken,
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

    /// The converter handle currently backing the server. Freshly loaded
    /// from the runtime snapshot on every call — callers that expect a
    /// stable view across multiple reads should retain the returned clone.
    pub fn current_converter(&self) -> VlConverter {
        self.runtime.load_full().converter.clone()
    }

    /// The `VlcConfig` currently backing the server. Returned as `Arc`
    /// because the runtime snapshot owns it; callers must not mutate.
    pub fn current_config(&self) -> Arc<VlcConfig> {
        self.runtime.load_full().config.clone()
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

pub(crate) fn validate_serve_config(serve_config: &ServeConfig) -> Result<(), anyhow::Error> {
    if serve_config.budget_hold_ms <= 0 {
        anyhow::bail!("budget_hold_ms must be positive");
    }

    Ok(())
}

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

/// Output format for server log events.
#[derive(Debug, Clone, Copy, clap::ValueEnum, Default, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable text logs.
    #[default]
    Text,
    /// Flat structured JSON logs.
    Json,
}

/// Install the crate's tracing subscriber.
///
/// `filter` is parsed as a `tracing_subscriber::EnvFilter` directive. Logs are
/// written to stderr in either [`LogFormat::Text`] or [`LogFormat::Json`]
/// form. Call this at most once per process, before serving requests.
pub fn init_tracing(filter: &str, format: LogFormat) {
    // `tracing-log` routes upstream `log::*` records through this
    // subscriber.
    let filter: EnvFilter = filter.parse().expect("valid tracing filter directives");

    // stdout is reserved by `vl-convert serve --ready-json`; logs use stderr.
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

/// HTTP server configuration.
///
/// This config controls listener addresses, auth, CORS, request limits,
/// request budgets, logging format, and admin-server behavior. It does not
/// parse environment variables or CLI flags; embedders map their own
/// configuration source onto this struct.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// Main HTTP listener binding.
    pub main: ListenAddr,
    /// Admin HTTP listener binding. `None` disables the admin
    /// router entirely.
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
    /// Non-loopback TCP admin with no key is rejected at startup.
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
    /// When `true`, error responses omit exception messages, stack traces,
    /// and internal paths.
    pub opaque_errors: bool,
    /// When `true`, `user_agent_middleware` rejects requests without
    /// a `User-Agent` header with 400. Useful for filtering unkeyed
    /// crawlers / health probes in shared environments.
    pub require_user_agent: bool,
    /// Log event format: human-readable `Text` or structured `Json`.
    /// Writer target is always stderr.
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
    /// Reservation size each request tentatively charges against enabled
    /// budget dimensions before settlement. Default 1000 (1s).
    pub budget_hold_ms: i64,
    /// Additional budget milliseconds charged per Google Fonts CSS or font-file
    /// cache miss. Default 0 disables the surcharge.
    pub google_font_cache_miss_penalty_ms: i64,
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
    /// Per-reconfiguration drain timeout in seconds. This bounds how long an
    /// admin config replacement waits for in-flight requests to finish before
    /// returning a 503.
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
            google_font_cache_miss_penalty_ms: 0,
            trust_proxy: false,
            socket_mode: 0o600,
            reconfig_drain_timeout_secs: 30,
        }
    }
}

/// Immutable point-in-time view of the converter and effective config.
///
/// Handlers load one snapshot at entry and keep it for the request. Admin
/// reconfiguration replaces future snapshots atomically via `ArcSwap::store`.
pub(crate) struct RuntimeSnapshot {
    pub converter: VlConverter,
    pub config: Arc<VlcConfig>,
    /// Bumps on every rebuild-commit (drain+rebuild path). Exposed through
    /// admin endpoints; not used on the conversion hot path.
    pub generation: u64,
}

pub(crate) struct AppState {
    pub runtime: Arc<ArcSwap<RuntimeSnapshot>>,
    pub api_key: Option<ApiKey>,
    pub opaque_errors: bool,
    pub require_user_agent: bool,
    pub readiness: Arc<health::ReadinessState>,
    pub local_tz: Option<String>,
    /// Shared with the gate middleware (main router) and admin handlers so
    /// every drain-participating actor works against the same coordinator.
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
/// [`crate::serve`] to run it.
pub struct BuiltApp {
    /// The main app router with all middleware applied. Kept crate-private so
    /// the public API exposes the server lifecycle rather than router internals.
    pub(crate) router: Router,
    /// Atomic holder for the current converter + config. Cloned into
    /// `AppState` for the router; exposed through
    /// [`Self::current_converter`] / [`Self::current_config`] for callers
    /// that want to drive conversions programmatically alongside serving HTTP.
    pub(crate) runtime: Arc<ArcSwap<RuntimeSnapshot>>,
    /// Shared shutdown signal used by [`crate::serve`] and by the
    /// reconfig coordinator (which holds its own clone). Populated in
    /// `build_app`; `serve()` clones it so cancellations observed by
    /// the coordinator also abort the main / admin listeners.
    pub(crate) shutdown_token: CancellationToken,
    /// Budget tracker. Consumed by [`crate::serve`] to drive the refill loop.
    pub(crate) tracker: Option<Arc<BudgetTracker>>,
    /// Admin listener setup. Consumed by [`crate::serve`] to bind and serve.
    pub(crate) admin: Option<AdminConfig>,
}

impl BuiltApp {
    /// Endpoint label of the admin listener, if one was bound.
    pub fn admin_endpoint(&self) -> Option<&str> {
        self.admin.as_ref().map(|a| a.addr.as_str())
    }

    /// Structured descriptor of the admin listener, if one was bound.
    pub fn admin_endpoint_info(&self) -> Option<crate::EndpointInfo> {
        self.admin.as_ref().map(|a| a.listener.endpoint_info())
    }

    /// The converter handle currently backing the server.
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
    if serve_config.google_font_cache_miss_penalty_ms < 0 {
        anyhow::bail!("google_font_cache_miss_penalty_ms must be non-negative");
    }

    if serve_config
        .api_key
        .as_deref()
        .is_some_and(|key| key.trim().is_empty())
    {
        anyhow::bail!("api_key must not be empty or whitespace-only");
    }
    if serve_config
        .admin_api_key
        .as_deref()
        .is_some_and(|key| key.trim().is_empty())
    {
        anyhow::bail!("admin_api_key must not be empty or whitespace-only");
    }

    // A non-loopback TCP admin listener requires bearer auth. UDS admin
    // may rely on filesystem permissions.
    if let Some(admin_addr) = &serve_config.admin {
        if serve_config.admin_api_key.is_none() && !admin_addr.is_loopback_or_uds() {
            anyhow::bail!(
                "admin listener bound to non-loopback address {admin_addr} requires \
                 a non-empty admin_api_key; either set admin_api_key or use a \
                 loopback / UDS bind"
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_serve_config_rejects_negative_google_font_penalty() {
        let serve_config = ServeConfig {
            google_font_cache_miss_penalty_ms: -1,
            ..ServeConfig::default()
        };

        let err = validate_serve_config(&serve_config).unwrap_err();
        assert!(
            err.to_string()
                .contains("google_font_cache_miss_penalty_ms must be non-negative"),
            "unexpected error: {err}"
        );
    }
}

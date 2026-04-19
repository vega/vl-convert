use axum::Router;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tracing_subscriber::EnvFilter;
use vl_convert_rs::anyhow;
use vl_convert_rs::converter::{VlConverter, VlcConfig};

use crate::budget::BudgetTracker;
use crate::{health, json_fmt};

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default, PartialEq, Eq)]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}

pub fn init_tracing(filter: &str, format: LogFormat) {
    let filter: EnvFilter = filter.parse().expect("valid tracing filter directives");

    match format {
        LogFormat::Json => {
            tracing_subscriber::fmt()
                .event_format(json_fmt::FlatJsonFormatter)
                .fmt_fields(tracing_subscriber::fmt::format::JsonFields::new())
                .with_env_filter(filter)
                .init();
        }
        LogFormat::Text => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(true)
                .init();
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServeConfig {
    pub host: String,
    pub port: u16,
    pub api_key: Option<String>,
    pub cors_origin: Option<String>,
    pub max_concurrent_requests: Option<usize>,
    pub request_timeout_secs: u64,
    pub max_body_size_mb: usize,
    pub opaque_errors: bool,
    pub require_user_agent: bool,
    pub log_format: LogFormat,
    pub per_ip_budget_ms: Option<i64>,
    pub global_budget_ms: Option<i64>,
    pub budget_hold_ms: i64,
    pub admin_port: Option<u16>,
    pub trust_proxy: bool,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
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
            admin_port: None,
            trust_proxy: false,
        }
    }
}

pub struct AppState {
    pub converter: VlConverter,
    pub config: VlcConfig,
    pub api_key: Option<ApiKey>,
    pub opaque_errors: bool,
    pub require_user_agent: bool,
    pub readiness: health::ReadinessState,
}

pub struct ApiKey(String);

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

pub(crate) struct AdminConfig {
    pub listener: tokio::net::TcpListener,
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

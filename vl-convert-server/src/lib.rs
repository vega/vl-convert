mod admin;
pub mod budget;
pub mod datadog_fmt;
mod health;
mod svg;
mod themes;
pub mod types;
mod vega;
mod vegalite;

use axum::error_handling::HandleErrorLayer;
use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use subtle::ConstantTimeEq;
use tower::limit::ConcurrencyLimitLayer;
use tower::load_shed::LoadShedLayer;
use tower::timeout::TimeoutLayer;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

use vl_convert_rs::anyhow;

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default, PartialEq, Eq)]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}
use vl_convert_rs::converter::{GoogleFontRequest, LogEntry, VlConverter, VlcConfig};

use types::ErrorResponse;

#[derive(OpenApi)]
#[openapi(tags(
    (name = "Health", description = "Health and info endpoints"),
    (name = "Themes", description = "Vega themes"),
    (name = "Vega-Lite", description = "Vega-Lite conversions"),
    (name = "Vega", description = "Vega conversions"),
    (name = "SVG", description = "SVG conversions"),
))]
struct ApiDoc;

pub fn format_log_entries(logs: &[LogEntry]) -> Vec<String> {
    logs.iter()
        .map(|e| format!("{}: {}", e.level, e.message))
        .collect()
}

pub fn init_tracing(level: &str, format: LogFormat) {
    let filter = EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| {
        format!("vl_convert={level},tower_http={level}")
            .parse()
            .expect("valid default filter directives")
    });

    match format {
        LogFormat::Json => {
            tracing_subscriber::fmt()
                .event_format(datadog_fmt::FlatJsonFormatter)
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

pub fn vegalite_versions() -> Vec<&'static str> {
    vl_convert_rs::module_loader::import_map::VL_VERSIONS
        .iter()
        .map(|v| v.to_semver())
        .collect()
}

pub struct ServeConfig {
    pub host: String,
    pub port: u16,
    pub api_key: Option<String>,
    pub cors_origin: Option<String>,
    pub max_concurrent_requests: Option<usize>,
    pub request_timeout_secs: u64,
    pub drain_timeout_secs: u64,
    pub max_body_size_mb: usize,
    pub opaque_errors: bool,
    pub require_user_agent: bool,
    pub log_format: LogFormat,
    pub per_ip_budget_ms: Option<i64>,
    pub global_budget_ms: Option<i64>,
    pub budget_estimate_ms: i64,
    pub admin_port: Option<u16>,
    pub trust_proxy: bool,
}

pub struct AppState {
    pub converter: VlConverter,
    pub config: VlcConfig,
    pub api_key: Option<ApiKey>,
    pub opaque_errors: bool,
    pub require_user_agent: bool,
}

pub struct ApiKey(String);

impl ApiKey {
    pub fn matches(&self, other: &[u8]) -> bool {
        let key_bytes = self.0.as_bytes();
        key_bytes.ct_eq(other).into()
    }
}

pub fn error_response(status: StatusCode, message: &str, opaque: bool) -> Response {
    if opaque {
        status.into_response()
    } else {
        (
            status,
            Json(ErrorResponse {
                error: message.to_string(),
            }),
        )
            .into_response()
    }
}

pub fn append_vlc_logs_header(headers: &mut HeaderMap, logs: &[String]) {
    let truncated: Vec<&str> = logs.iter().take(50).map(|s| s.as_str()).collect();
    let json = serde_json::to_string(&truncated).unwrap_or_else(|_| "[]".to_string());
    if let Ok(val) = HeaderValue::from_str(&json) {
        headers.insert("x-vlc-logs", val);
    } else {
        let safe: String = json
            .chars()
            .filter(|c| c.is_ascii_graphic() || *c == ' ')
            .collect();
        if let Ok(val) = HeaderValue::from_str(&safe) {
            headers.insert("x-vlc-logs", val);
        } else {
            headers.insert("x-vlc-logs", HeaderValue::from_static("[]"));
        }
    }
}

pub fn parse_google_font_args(fonts: &[String]) -> Result<Vec<GoogleFontRequest>, String> {
    fonts
        .iter()
        .map(|s| {
            let Some((family, variants_str)) = s.split_once(':') else {
                return Ok(GoogleFontRequest {
                    family: s.to_string(),
                    variants: None,
                });
            };
            let mut variants = Vec::new();
            for token in variants_str.split(',') {
                let token = token.trim();
                if token.is_empty() {
                    continue;
                }
                let (weight_str, style) = if let Some(w) = token.strip_suffix("italic") {
                    (w, vl_convert_google_fonts::FontStyle::Italic)
                } else {
                    (token, vl_convert_google_fonts::FontStyle::Normal)
                };
                let weight: u16 = weight_str
                    .parse()
                    .map_err(|_| format!("invalid font variant '{token}' in '{s}'"))?;
                variants.push(vl_convert_google_fonts::VariantRequest { weight, style });
            }
            Ok(GoogleFontRequest {
                family: family.to_string(),
                variants: if variants.is_empty() {
                    None
                } else {
                    Some(variants)
                },
            })
        })
        .collect()
}

use vl_convert_rs::converter::{FormatLocale, TimeFormatLocale};

pub(crate) struct CommonOpts {
    pub format_locale: Option<FormatLocale>,
    pub time_format_locale: Option<TimeFormatLocale>,
    pub google_fonts: Option<Vec<GoogleFontRequest>>,
    pub vega_plugin: Option<String>,
    pub config: Option<serde_json::Value>,
    pub background: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_common_opts(
    format_locale: &Option<serde_json::Value>,
    time_format_locale: &Option<serde_json::Value>,
    google_fonts: &Option<Vec<String>>,
    vega_plugin: &Option<String>,
    config: &Option<serde_json::Value>,
    background: &Option<String>,
    width: Option<f32>,
    height: Option<f32>,
    state: &AppState,
) -> Result<CommonOpts, String> {
    let format_locale = format_locale
        .as_ref()
        .map(|v| match v {
            serde_json::Value::String(s) => Ok(FormatLocale::Name(s.clone())),
            obj @ serde_json::Value::Object(_) => Ok(FormatLocale::Object(obj.clone())),
            _ => Err("format_locale must be a string or object".to_string()),
        })
        .transpose()?;

    let time_format_locale = time_format_locale
        .as_ref()
        .map(|v| match v {
            serde_json::Value::String(s) => Ok(TimeFormatLocale::Name(s.clone())),
            obj @ serde_json::Value::Object(_) => Ok(TimeFormatLocale::Object(obj.clone())),
            _ => Err("time_format_locale must be a string or object".to_string()),
        })
        .transpose()?;

    let google_fonts = google_fonts
        .as_ref()
        .map(|fonts| parse_google_font_args(fonts))
        .transpose()?;

    if google_fonts.is_some() && !state.config.allow_google_fonts {
        return Err("google_fonts requires allow_google_fonts: true in server config".to_string());
    }

    if vega_plugin.is_some() && !state.config.allow_per_request_plugins {
        return Err(
            "vega_plugin requires allow_per_request_plugins: true in server config".to_string(),
        );
    }

    Ok(CommonOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin: vega_plugin.clone(),
        config: config.clone(),
        background: background.clone(),
        width,
        height,
    })
}

fn build_cors_layer(cors_origin: &Option<String>) -> CorsLayer {
    let base = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .expose_headers(vec![
            header::HeaderName::from_static("x-request-id"),
            header::HeaderName::from_static("x-vlc-logs"),
        ])
        .max_age(Duration::from_secs(600));

    match cors_origin.as_deref() {
        Some("") => base.allow_origin(AllowOrigin::list(std::iter::empty::<HeaderValue>())),
        Some("*") => base.allow_origin(tower_http::cors::Any),
        Some(origins) => {
            let origins: Vec<HeaderValue> = origins
                .split(',')
                .filter_map(|o| HeaderValue::from_str(o.trim()).ok())
                .collect();
            base.allow_origin(AllowOrigin::list(origins))
        }
        None => base.allow_origin(AllowOrigin::predicate(|origin, _| {
            let origin = origin.as_bytes();
            let s = std::str::from_utf8(origin).unwrap_or("");
            is_loopback_origin(s)
        })),
    }
}

fn is_loopback_origin(origin: &str) -> bool {
    let Some(rest) = origin.strip_prefix("http://") else {
        return false;
    };
    for loopback in &["localhost", "127.0.0.1", "[::1]"] {
        if let Some(after) = rest.strip_prefix(loopback) {
            if after.is_empty() || after.starts_with(':') {
                return true;
            }
        }
    }
    false
}

fn build_router(
    state: Arc<AppState>,
    tracker: Option<Arc<budget::BudgetTracker>>,
    opaque_errors: bool,
    trust_proxy: bool,
) -> Router {
    // Health endpoints bypass budget tracking entirely
    let health_router = Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
        .route("/infoz", get(health::infoz));

    // API routes with OpenAPI documentation
    let (api_router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(themes::list_themes))
        .routes(routes!(themes::get_theme))
        .routes(routes!(vegalite::vegalite_to_vega))
        .routes(routes!(vegalite::vegalite_to_svg))
        .routes(routes!(vegalite::vegalite_to_png))
        .routes(routes!(vegalite::vegalite_to_jpeg))
        .routes(routes!(vegalite::vegalite_to_pdf))
        .routes(routes!(vegalite::vegalite_to_html))
        .routes(routes!(vegalite::vegalite_to_url))
        .routes(routes!(vega::vega_to_svg))
        .routes(routes!(vega::vega_to_png))
        .routes(routes!(vega::vega_to_jpeg))
        .routes(routes!(vega::vega_to_pdf))
        .routes(routes!(vega::vega_to_html))
        .routes(routes!(vega::vega_to_url))
        .routes(routes!(svg::svg_to_png))
        .routes(routes!(svg::svg_to_jpeg))
        .routes(routes!(svg::svg_to_pdf))
        .split_for_parts();

    // Serve Swagger UI and OpenAPI spec
    let mut api_router =
        api_router.merge(SwaggerUi::new("/docs").url("/api-doc/openapi.json", api));

    // Budget tracking middleware (optional)
    if let Some(tracker) = tracker {
        api_router =
            api_router.layer(axum::middleware::from_fn(
                move |req: axum::http::Request<axum::body::Body>, next: axum::middleware::Next| {
                    let tracker = tracker.clone();
                    async move {
                        budget_middleware(tracker, opaque_errors, trust_proxy, req, next).await
                    }
                },
            ));
    }

    // Auth and UA middleware only on API routes — health endpoints are exempt
    let api_router = api_router
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            user_agent_middleware,
        ));

    health_router.merge(api_router).with_state(state)
}

fn make_span_text(req: &axum::http::Request<axum::body::Body>) -> tracing::Span {
    tracing::info_span!(
        "request",
        method = %req.method(),
        uri = %req.uri(),
    )
}

fn make_span_json(req: &axum::http::Request<axum::body::Body>) -> tracing::Span {
    let ua = req
        .headers()
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let (trace_id, span_id) = extract_trace_context(req.headers());

    tracing::info_span!(
        "request",
        method = %req.method(),
        uri = %req.uri(),
        version = ?req.version(),
        user_agent = %ua,
        request_id = %request_id,
        trace_id = %trace_id,
        span_id = %span_id,
    )
}

struct InitResult {
    state: Arc<AppState>,
    tracker: Option<Arc<budget::BudgetTracker>>,
    converter: VlConverter,
}

/// Initialize converter, app state, budget tracker, and admin listener.
fn init_app_state(
    config: VlcConfig,
    serve_config: &ServeConfig,
) -> Result<InitResult, anyhow::Error> {
    let num_workers = config.num_workers;
    log::info!("Initializing converter with {num_workers} worker(s)...");
    let converter = VlConverter::with_config(config.clone())?;
    converter.warm_up()?;
    log::info!("Workers initialized");

    let api_key = serve_config.api_key.as_ref().map(|k| ApiKey(k.clone()));
    let state = Arc::new(AppState {
        converter: converter.clone(),
        config: config.clone(),
        api_key,
        opaque_errors: serve_config.opaque_errors,
        require_user_agent: serve_config.require_user_agent,
    });

    let tracker = if serve_config.per_ip_budget_ms.is_some()
        || serve_config.global_budget_ms.is_some()
        || serve_config.admin_port.is_some()
    {
        let t = budget::BudgetTracker::new(
            serve_config.per_ip_budget_ms.unwrap_or(0),
            serve_config.global_budget_ms.unwrap_or(0),
            serve_config.budget_estimate_ms,
        );
        let refill_tracker = t.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                refill_tracker.refill();
            }
        });
        Some(t)
    } else {
        None
    };

    if let (Some(admin_port), Some(ref tracker)) = (serve_config.admin_port, &tracker) {
        let admin_router = admin::admin_router(tracker.clone())
            .layer(PropagateRequestIdLayer::x_request_id())
            .layer(TraceLayer::new_for_http())
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(CatchPanicLayer::new());
        let admin_addr = format!("127.0.0.1:{admin_port}");
        let admin_addr_clone = admin_addr.clone();
        tokio::spawn(async move {
            match tokio::net::TcpListener::bind(&admin_addr_clone).await {
                Ok(listener) => {
                    log::info!("Admin API listening on http://{admin_addr_clone}");
                    let _ = axum::serve(listener, admin_router).await;
                }
                Err(e) => {
                    log::error!("Failed to bind admin port {admin_addr_clone}: {e}");
                }
            }
        });
    }

    Ok(InitResult {
        state,
        tracker,
        converter,
    })
}

/// Build the middleware stack that wraps the API router.
fn build_middleware_stack(router: Router, serve_config: &ServeConfig) -> Router {
    let cors = build_cors_layer(&serve_config.cors_origin);
    let mut app = router.layer(CompressionLayer::new());

    let opaque = serve_config.opaque_errors;
    if let Some(max) = serve_config.max_concurrent_requests {
        app = app.layer(
            tower::ServiceBuilder::new()
                .layer(HandleErrorLayer::new(
                    move |_: tower::BoxError| async move {
                        error_response(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "too many concurrent requests",
                            opaque,
                        )
                    },
                ))
                .layer(LoadShedLayer::new())
                .layer(ConcurrencyLimitLayer::new(max)),
        );
    }

    if serve_config.request_timeout_secs > 0 {
        app = app.layer(
            tower::ServiceBuilder::new()
                .layer(HandleErrorLayer::new(
                    move |_: tower::BoxError| async move {
                        error_response(StatusCode::SERVICE_UNAVAILABLE, "request timed out", opaque)
                    },
                ))
                .layer(TimeoutLayer::new(Duration::from_secs(
                    serve_config.request_timeout_secs,
                ))),
        );
    }

    let app = app
        .layer(DefaultBodyLimit::max(
            serve_config.max_body_size_mb * 1024 * 1024,
        ))
        .layer(cors)
        .layer(PropagateRequestIdLayer::x_request_id());

    let app = if serve_config.log_format == LogFormat::Json {
        app.layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    make_span_json as fn(&axum::http::Request<axum::body::Body>) -> tracing::Span,
                )
                .on_response(datadog_fmt::FlatJsonOnResponse),
        )
    } else {
        app.layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    make_span_text as fn(&axum::http::Request<axum::body::Body>) -> tracing::Span,
                )
                .on_response(
                    tower_http::trace::DefaultOnResponse::new().level(tracing::Level::INFO),
                ),
        )
    };

    app.layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(CatchPanicLayer::new())
}

pub async fn run(config: VlcConfig, serve_config: ServeConfig) -> Result<(), anyhow::Error> {
    let InitResult {
        state, tracker, ..
    } = init_app_state(config.clone(), &serve_config)?;

    let router = build_router(
        state.clone(),
        tracker,
        serve_config.opaque_errors,
        serve_config.trust_proxy,
    );
    let app = build_middleware_stack(router, &serve_config);

    // Graceful shutdown signal
    let drain_secs = serve_config.drain_timeout_secs;
    let shutdown_signal = async move {
        let ctrl_c = tokio::signal::ctrl_c();

        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to install SIGTERM handler");
            tokio::select! {
                _ = ctrl_c => log::info!("Received SIGINT, shutting down..."),
                _ = sigterm.recv() => log::info!("Received SIGTERM, shutting down..."),
            }
        }

        #[cfg(not(unix))]
        {
            ctrl_c.await.expect("failed to install Ctrl-C handler");
            log::info!("Received Ctrl-C, shutting down...");
        }

        log::info!("Starting graceful drain ({drain_secs}s deadline)...");
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(drain_secs)).await;
            log::warn!("Drain timeout ({drain_secs}s) exceeded, forcing exit");
            std::process::exit(1);
        });
    };

    // Bind and serve
    let addr = if serve_config.host.contains(':') {
        format!("[{}]:{}", serve_config.host, serve_config.port)
    } else {
        format!("{}:{}", serve_config.host, serve_config.port)
    };

    let host = &serve_config.host;
    if host != "127.0.0.1" && host != "localhost" && host != "::1" && state.api_key.is_none() {
        log::warn!(
            "Server binding to {addr} with no API key — accessible to any network client. \
             Set --api-key or VLC_API_KEY to restrict access."
        );
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    log::info!("Listening on http://{addr}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal)
    .await?;

    Ok(())
}

/// Build the fully-configured app (Router with all middleware) without binding.
/// Used by tests to bind to port 0 and discover the assigned port.
pub fn build_app(
    config: VlcConfig,
    serve_config: &ServeConfig,
) -> Result<(Router, VlConverter), anyhow::Error> {
    let InitResult {
        state,
        tracker,
        converter,
    } = init_app_state(config, serve_config)?;

    let router = build_router(
        state,
        tracker,
        serve_config.opaque_errors,
        serve_config.trust_proxy,
    );
    let app = build_middleware_stack(router, serve_config);

    Ok((app, converter))
}

/// Extract trace context from W3C traceparent or Datadog headers.
/// Returns (trace_id, span_id) as strings suitable for dd.trace_id / dd.span_id.
/// Returns empty strings if no trace context is found.
fn extract_trace_context(headers: &axum::http::HeaderMap) -> (String, String) {
    // W3C traceparent: 00-<32-hex-trace-id>-<16-hex-parent-id>-<2-hex-flags>
    if let Some(tp) = headers.get("traceparent").and_then(|v| v.to_str().ok()) {
        let parts: Vec<&str> = tp.split('-').collect();
        if parts.len() >= 3 {
            let trace_id = parts[1].to_string();
            let span_id = parts[2].to_string();
            if !trace_id.is_empty() && !span_id.is_empty() {
                return (trace_id, span_id);
            }
        }
    }

    // Datadog headers: x-datadog-trace-id (decimal), x-datadog-parent-id (decimal)
    // Convert decimal to hex for consistent output format
    let dd_trace = headers
        .get("x-datadog-trace-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|n| format!("{n:016x}"));
    let dd_span = headers
        .get("x-datadog-parent-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|n| format!("{n:016x}"));
    if let (Some(trace_id), Some(span_id)) = (dd_trace, dd_span) {
        return (trace_id, span_id);
    }

    (String::new(), String::new())
}

async fn auth_middleware(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    if let Some(ref key) = state.api_key {
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        let authorized = match auth_header {
            Some(val)
                if val
                    .get(..7)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("bearer ")) =>
            {
                key.matches(&val.as_bytes()[7..])
            }
            _ => false,
        };

        if !authorized {
            let mut resp = error_response(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                state.opaque_errors,
            );
            resp.headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
            return resp;
        }
    }
    next.run(req).await
}

async fn user_agent_middleware(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    if state.require_user_agent {
        let ua = req
            .headers()
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if ua.is_empty() {
            return error_response(
                StatusCode::BAD_REQUEST,
                "User-Agent header is required",
                state.opaque_errors,
            );
        }
    }
    next.run(req).await
}

/// Extract client IP. When `trust_proxy` is true, checks X-Forwarded-For and
/// X-Real-IP headers (only safe behind a reverse proxy that sets these).
/// Otherwise, always uses the peer socket address.
fn extract_client_ip(
    req: &axum::http::Request<axum::body::Body>,
    trust_proxy: bool,
) -> Option<std::net::IpAddr> {
    if trust_proxy {
        // X-Forwarded-For (first entry is the original client)
        if let Some(xff) = req.headers().get("x-forwarded-for") {
            if let Ok(xff_str) = xff.to_str() {
                if let Some(first_ip) = xff_str.split(',').next() {
                    if let Ok(ip) = first_ip.trim().parse::<std::net::IpAddr>() {
                        return Some(ip);
                    }
                }
            }
        }
        // X-Real-IP (set by nginx and some other proxies)
        if let Some(xri) = req.headers().get("x-real-ip") {
            if let Ok(ip_str) = xri.to_str() {
                if let Ok(ip) = ip_str.trim().parse::<std::net::IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }
    // Peer socket address (always available, always trustworthy)
    req.extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip())
}

async fn budget_middleware(
    tracker: Arc<budget::BudgetTracker>,
    opaque_errors: bool,
    trust_proxy: bool,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    if !tracker.is_enabled() {
        return next.run(req).await;
    }

    let ip = extract_client_ip(&req, trust_proxy)
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));

    if let Err(e) = tracker.reserve(ip) {
        return error_response(
            StatusCode::TOO_MANY_REQUESTS,
            &format!("{e}"),
            opaque_errors,
        );
    }

    let start = std::time::Instant::now();
    let response = next.run(req).await;
    let actual_ms = start.elapsed().as_millis() as i64;

    tracker.adjust(ip, actual_ms);

    response
}

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
use axum::routing::{get, post};
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

use crate::LogFormat;

use vl_convert_rs::anyhow;
use vl_convert_rs::converter::{GoogleFontRequest, VlConverter, VlcConfig};

use types::ErrorResponse;

pub fn init_tracing(level: &str, format: LogFormat) {
    let filter = EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| {
        format!("vl_convert={level},tower_http={level}")
            .parse()
            .expect("valid default filter directives")
    });

    match format {
        LogFormat::Json => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(filter)
                .with_target(true)
                .init();
        }
        LogFormat::Text => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(true)
                .init();
        }
        LogFormat::Datadog => {
            tracing_subscriber::fmt()
                .event_format(datadog_fmt::DatadogFormatter)
                .fmt_fields(tracing_subscriber::fmt::format::JsonFields::new())
                .with_env_filter(filter)
                .init();
        }
    }
}

pub const VEGALITE_VERSIONS: &[&str] = &[
    "5.8", "5.14", "5.15", "5.16", "5.17", "5.20", "5.21", "6.1", "6.4",
];

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
    pub rate_limit_per_second: Option<u64>,
    pub rate_limit_burst: u32,
}

pub struct AppState {
    pub converter: VlConverter,
    pub config: VlcConfig,
    pub api_key: Option<ApiKey>,
    pub opaque_errors: bool,
    pub require_user_agent: bool,
    pub num_workers: usize,
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

fn build_cors_layer(cors_origin: &Option<String>, _api_key_set: bool) -> CorsLayer {
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
    rate_limit_per_second: Option<u64>,
    rate_limit_burst: u32,
    opaque_errors: bool,
) -> Router {
    // Health endpoints bypass rate limiting entirely
    let health_router = Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
        .route("/infoz", get(health::infoz));

    // API routes with optional rate limiting
    let mut api_router = Router::new()
        .route("/themes", get(themes::list_themes))
        .route("/themes/{name}", get(themes::get_theme))
        .route("/vegalite/vega", post(vegalite::vegalite_to_vega))
        .route("/vegalite/svg", post(vegalite::vegalite_to_svg))
        .route("/vegalite/png", post(vegalite::vegalite_to_png))
        .route("/vegalite/jpeg", post(vegalite::vegalite_to_jpeg))
        .route("/vegalite/pdf", post(vegalite::vegalite_to_pdf))
        .route("/vegalite/html", post(vegalite::vegalite_to_html))
        .route("/vegalite/url", post(vegalite::vegalite_to_url))
        .route("/vega/svg", post(vega::vega_to_svg))
        .route("/vega/png", post(vega::vega_to_png))
        .route("/vega/jpeg", post(vega::vega_to_jpeg))
        .route("/vega/pdf", post(vega::vega_to_pdf))
        .route("/vega/html", post(vega::vega_to_html))
        .route("/vega/url", post(vega::vega_to_url))
        .route("/svg/png", post(svg::svg_to_png))
        .route("/svg/jpeg", post(svg::svg_to_jpeg))
        .route("/svg/pdf", post(svg::svg_to_pdf));

    // Per-IP rate limiting (optional)
    if let Some(per_second) = rate_limit_per_second {
        use tower_governor::governor::GovernorConfigBuilder;
        use tower_governor::key_extractor::SmartIpKeyExtractor;
        use tower_governor::GovernorLayer;

        let governor_conf = Arc::new(
            GovernorConfigBuilder::default()
                .per_second(per_second)
                .burst_size(rate_limit_burst)
                .key_extractor(SmartIpKeyExtractor)
                .error_handler(move |err| {
                    if opaque_errors {
                        StatusCode::TOO_MANY_REQUESTS.into_response()
                    } else {
                        (
                            StatusCode::TOO_MANY_REQUESTS,
                            Json(ErrorResponse {
                                error: format!("{err}"),
                            }),
                        )
                            .into_response()
                    }
                })
                .finish()
                .expect("valid governor config"),
        );

        // Background cleanup of expired rate limit entries
        let limiter = governor_conf.limiter().clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                limiter.retain_recent();
            }
        });

        api_router = api_router.layer(GovernorLayer {
            config: governor_conf,
        });
    }

    health_router.merge(api_router).with_state(state)
}

pub async fn run(config: VlcConfig, serve_config: ServeConfig) -> Result<(), anyhow::Error> {
    let num_workers = config.num_workers;

    log::info!("Initializing converter with {num_workers} worker(s)...");
    let converter = VlConverter::with_config(config.clone())?;

    // Warm up workers so /readyz is meaningful
    converter.warm_up()?;
    log::info!("Workers initialized");

    let api_key = serve_config.api_key.map(ApiKey);
    let state = Arc::new(AppState {
        converter,
        config: config.clone(),
        api_key,
        opaque_errors: serve_config.opaque_errors,
        require_user_agent: serve_config.require_user_agent,
        num_workers,
    });

    let router = build_router(
        state.clone(),
        serve_config.rate_limit_per_second,
        serve_config.rate_limit_burst,
        serve_config.opaque_errors,
    );

    // Build middleware stack (applied bottom-up, so first in list = outermost)
    let cors = build_cors_layer(&serve_config.cors_origin, state.api_key.is_some());

    // Build middleware stack (layers applied bottom-up: first listed = outermost)
    let mut app = router.layer(CompressionLayer::new());

    // Concurrency limit + load shedding (innermost operational layer)
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

    // Request timeout
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
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            user_agent_middleware,
        ))
        .layer(cors)
        .layer(PropagateRequestIdLayer::x_request_id());

    let app = if serve_config.log_format == crate::LogFormat::Datadog {
        app.layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    tower_http::trace::DefaultMakeSpan::new().level(tracing::Level::INFO),
                )
                .on_response(datadog_fmt::DatadogOnResponse),
        )
    } else {
        app.layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    tower_http::trace::DefaultMakeSpan::new().level(tracing::Level::INFO),
                )
                .on_response(
                    tower_http::trace::DefaultOnResponse::new().level(tracing::Level::INFO),
                ),
        )
    };

    let app = app
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(CatchPanicLayer::new());

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

    // Bind and serve (TCP only; Unix socket support planned for future release)
    let addr = if serve_config.host.contains(':') {
        // IPv6 addresses must be bracketed in socket addresses
        format!("[{}]:{}", serve_config.host, serve_config.port)
    } else {
        format!("{}:{}", serve_config.host, serve_config.port)
    };

    // Warn if non-loopback without API key
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
            Some(val) if val.starts_with("Bearer ") => {
                key.matches(&val.as_bytes()["Bearer ".len()..])
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

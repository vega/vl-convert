use axum::error_handling::HandleErrorLayer;
use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::Router;
use std::sync::Arc;
use std::time::Duration;
use tower::limit::ConcurrencyLimitLayer;
use tower::load_shed::LoadShedLayer;
use tower::timeout::TimeoutLayer;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

use crate::middleware::{auth_middleware, user_agent_middleware};
use crate::util::error_response;
use crate::{
    budget, bundling, health, json_fmt, svg, themes, vega, vegalite, AppState, LogFormat,
    ServeConfig,
};

#[derive(OpenApi)]
#[openapi(tags(
    (name = "Health", description = "Health and info endpoints"),
    (name = "Themes", description = "Vega themes"),
    (name = "Vega-Lite", description = "Vega-Lite conversions"),
    (name = "Vega", description = "Vega conversions"),
    (name = "SVG", description = "SVG conversions"),
    (name = "Bundling", description = "JavaScript bundling"),
))]
struct ApiDoc;

pub(crate) fn build_router(
    state: Arc<AppState>,
    tracker: Option<Arc<budget::BudgetTracker>>,
    opaque_errors: bool,
    trust_proxy: bool,
) -> Router {
    // Health endpoints: registered via OpenApiRouter for docs, but bypass auth/budget middleware
    let (health_router, health_api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health::healthz))
        .routes(routes!(health::readyz))
        .routes(routes!(health::infoz))
        .split_for_parts();

    // API routes with OpenAPI documentation
    let (api_router, mut api) = OpenApiRouter::new()
        .routes(routes!(themes::list_themes))
        .routes(routes!(themes::get_theme))
        .routes(routes!(vegalite::vegalite_to_vega))
        .routes(routes!(vegalite::vegalite_to_svg))
        .routes(routes!(vegalite::vegalite_to_png))
        .routes(routes!(vegalite::vegalite_to_jpeg))
        .routes(routes!(vegalite::vegalite_to_pdf))
        .routes(routes!(vegalite::vegalite_to_html))
        .routes(routes!(vegalite::vegalite_to_url))
        .routes(routes!(vegalite::vegalite_scenegraph))
        .routes(routes!(vegalite::vegalite_fonts))
        .routes(routes!(vega::vega_to_svg))
        .routes(routes!(vega::vega_to_png))
        .routes(routes!(vega::vega_to_jpeg))
        .routes(routes!(vega::vega_to_pdf))
        .routes(routes!(vega::vega_to_html))
        .routes(routes!(vega::vega_to_url))
        .routes(routes!(vega::vega_scenegraph))
        .routes(routes!(vega::vega_fonts))
        .routes(routes!(svg::svg_to_png))
        .routes(routes!(svg::svg_to_jpeg))
        .routes(routes!(svg::svg_to_pdf))
        .routes(routes!(bundling::bundle))
        .routes(routes!(bundling::bundle_snippet))
        .split_for_parts();

    // Merge health endpoint paths into the API OpenAPI spec
    for (path, item) in health_api.paths.paths {
        api.paths.paths.insert(path, item);
    }

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
                        budget::middleware(tracker, opaque_errors, trust_proxy, req, next).await
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
        budget_outcome = tracing::field::Empty,
        budget_charged_ms = tracing::field::Empty,
        budget_global_remaining_ms = tracing::field::Empty,
        budget_ip_remaining_ms = tracing::field::Empty,
        budget_client_ip = tracing::field::Empty,
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
        budget_outcome = tracing::field::Empty,
        budget_charged_ms = tracing::field::Empty,
        budget_global_remaining_ms = tracing::field::Empty,
        budget_ip_remaining_ms = tracing::field::Empty,
        budget_client_ip = tracing::field::Empty,
    )
}

/// Build the middleware stack that wraps the API router.
pub(crate) fn build_middleware_stack(router: Router, serve_config: &ServeConfig) -> Router {
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
                .on_response(json_fmt::FlatJsonOnResponse),
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

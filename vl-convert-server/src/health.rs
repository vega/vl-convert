use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use utoipa::ToSchema;
use vl_convert_rs::module_loader::import_map;

use crate::config::AppState;

const PROBE_INTERVAL: Duration = Duration::from_secs(1);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct StatusResponse {
    status: String,
}

impl StatusResponse {
    fn new(status: impl Into<String>) -> Self {
        Self {
            status: status.into(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct InfoResponse {
    version: String,
    vega_version: String,
    vega_themes_version: String,
    vega_embed_version: String,
    vegalite_versions: Vec<String>,
    google_fonts_cache_dir: Option<String>,
    local_tz: Option<String>,
}

pub struct ReadinessState {
    last_check: Mutex<Option<Instant>>,
    last_result: Mutex<bool>,
    /// Set `true` by the admin reconfig handler while a drain-then-rebuild is
    /// in flight; cleared on commit / rollback / abort. `/readyz` returns 503
    /// while this is set so orchestrator probes see the pod as temporarily
    /// unready and shed traffic. Cleared by `ReconfigScopeGuard` on drop.
    pub reconfig_in_progress: AtomicBool,
}

impl Default for ReadinessState {
    fn default() -> Self {
        Self {
            last_check: Mutex::new(None),
            last_result: Mutex::new(true),
            reconfig_in_progress: AtomicBool::new(false),
        }
    }
}

#[utoipa::path(
    get,
    path = "/healthz",
    responses(
        (
            status = 200,
            body = StatusResponse,
            content_type = "application/json",
            description = "Health check",
            example = json!({"status": "ok"})
        ),
    ),
    tag = "Health"
)]
pub async fn healthz() -> Json<StatusResponse> {
    Json(StatusResponse::new("ok"))
}

#[utoipa::path(
    get,
    path = "/readyz",
    responses(
        (
            status = 200,
            body = StatusResponse,
            content_type = "application/json",
            description = "Ready",
            example = json!({"status": "ready"})
        ),
        (
            status = 503,
            body = StatusResponse,
            content_type = "application/json",
            description = "Not ready",
            example = json!({"status": "not ready"})
        ),
    ),
    tag = "Health"
)]
pub async fn readyz(State(state): State<Arc<AppState>>) -> Response {
    // Reconfig drains return 503 so orchestrators shed traffic while the
    // rebuild runs. `ReconfigScopeGuard::drop` clears the flag.
    if state.readiness.reconfig_in_progress.load(Ordering::Acquire) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(StatusResponse::new("reconfig in progress")),
        )
            .into_response();
    }

    let should_probe = {
        let last = state.readiness.last_check.lock().await;
        match *last {
            None => true,
            Some(t) => t.elapsed() >= PROBE_INTERVAL,
        }
    };

    if should_probe {
        // `load_full()` returns an owned `Arc<RuntimeSnapshot>` that is safe
        // to hold across the `.await`.
        let snap = state.runtime.load_full();
        let ready = matches!(
            tokio::time::timeout(PROBE_TIMEOUT, snap.converter.health_check()).await,
            Ok(Ok(()))
        );

        *state.readiness.last_result.lock().await = ready;
        *state.readiness.last_check.lock().await = Some(Instant::now());
    }

    let ready = *state.readiness.last_result.lock().await;
    if ready {
        (StatusCode::OK, Json(StatusResponse::new("ready"))).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(StatusResponse::new("not ready")),
        )
            .into_response()
    }
}

#[utoipa::path(
    get,
    path = "/infoz",
    responses(
        (
            status = 200,
            body = InfoResponse,
            content_type = "application/json",
            description = "Server info",
            example = json!({
                "version": env!("CARGO_PKG_VERSION"),
                "vega_version": import_map::VEGA_VERSION,
                "vega_themes_version": import_map::VEGA_THEMES_VERSION,
                "vega_embed_version": import_map::VEGA_EMBED_VERSION,
                "vegalite_versions": crate::util::vegalite_versions(),
                "google_fonts_cache_dir": "/home/app/.cache/vl-convert/google-fonts",
                "local_tz": "America/New_York"
            })
        ),
    ),
    tag = "Health"
)]
pub async fn infoz(State(state): State<Arc<AppState>>) -> Json<InfoResponse> {
    Json(InfoResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        vega_version: import_map::VEGA_VERSION.to_string(),
        vega_themes_version: import_map::VEGA_THEMES_VERSION.to_string(),
        vega_embed_version: import_map::VEGA_EMBED_VERSION.to_string(),
        vegalite_versions: crate::util::vegalite_versions()
            .into_iter()
            .map(str::to_string)
            .collect(),
        google_fonts_cache_dir: vl_convert_rs::google_fonts_cache_dir()
            .map(|p| p.to_string_lossy().into_owned()),
        local_tz: state.local_tz.clone(),
    })
}

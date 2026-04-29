use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use vl_convert_rs::module_loader::import_map;

use crate::config::AppState;

const PROBE_INTERVAL: Duration = Duration::from_secs(1);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

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
        (status = 200, content_type = "application/json", description = "Health check"),
    ),
    tag = "Health"
)]
pub async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

#[utoipa::path(
    get,
    path = "/readyz",
    responses(
        (status = 200, content_type = "application/json", description = "Ready"),
        (status = 503, content_type = "application/json", description = "Not ready"),
    ),
    tag = "Health"
)]
pub async fn readyz(State(state): State<Arc<AppState>>) -> Response {
    // Admin-driven reconfig drain is in progress — return 503 so
    // orchestrators shed traffic while the drain + rebuild runs. Cleared
    // on any exit path by `ReconfigScopeGuard::drop`.
    if state.readiness.reconfig_in_progress.load(Ordering::Acquire) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "reconfig in progress" })),
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
        // `load_full()` returns an owned `Arc<RuntimeSnapshot>` safe to hold
        // across the `.await`. Do NOT use `load()` — its `Guard` binds to
        // the `ArcSwap` and is not Send-across-await-points.
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
        (StatusCode::OK, Json(json!({ "status": "ready" }))).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "not ready" })),
        )
            .into_response()
    }
}

#[utoipa::path(
    get,
    path = "/infoz",
    responses(
        (status = 200, content_type = "application/json", description = "Server info"),
    ),
    tag = "Health"
)]
pub async fn infoz() -> Json<Value> {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "vega_version": import_map::VEGA_VERSION,
        "vega_themes_version": import_map::VEGA_THEMES_VERSION,
        "vega_embed_version": import_map::VEGA_EMBED_VERSION,
        "vegalite_versions": crate::util::vegalite_versions(),
    }))
}

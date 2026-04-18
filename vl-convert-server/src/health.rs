use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde_json::{json, Value};
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
}

impl Default for ReadinessState {
    fn default() -> Self {
        Self {
            last_check: Mutex::new(None),
            last_result: Mutex::new(true),
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
    let should_probe = {
        let last = state.readiness.last_check.lock().await;
        match *last {
            None => true,
            Some(t) => t.elapsed() >= PROBE_INTERVAL,
        }
    };

    if should_probe {
        let ready = matches!(
            tokio::time::timeout(PROBE_TIMEOUT, state.converter.health_check()).await,
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

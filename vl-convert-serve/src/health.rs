use axum::extract::State;
use axum::response::Json;
use serde_json::{json, Value};
use std::sync::Arc;

use super::AppState;

pub async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn readyz() -> Json<Value> {
    Json(json!({ "status": "ready" }))
}

pub async fn infoz(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "vegalite_versions": super::VEGALITE_VERSIONS,
        "workers": state.config.num_workers,
    }))
}

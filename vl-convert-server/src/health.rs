use axum::response::Json;
use serde_json::{json, Value};
use vl_convert_rs::module_loader::import_map;

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
        (status = 200, content_type = "application/json", description = "Readiness check"),
    ),
    tag = "Health"
)]
// TODO: Check actual worker health (e.g., try a lightweight operation
// on the worker pool) instead of always returning 200.
pub async fn readyz() -> Json<Value> {
    Json(json!({ "status": "ready" }))
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
        "vegalite_versions": super::vegalite_versions(),
    }))
}

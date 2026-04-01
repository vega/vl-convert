use axum::response::Json;
use serde_json::{json, Value};
use vl_convert_rs::module_loader::import_map;

pub async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn readyz() -> Json<Value> {
    Json(json!({ "status": "ready" }))
}

pub async fn infoz() -> Json<Value> {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "vega_version": import_map::VEGA_VERSION,
        "vega_themes_version": import_map::VEGA_THEMES_VERSION,
        "vega_embed_version": import_map::VEGA_EMBED_VERSION,
        "vegalite_versions": super::VEGALITE_VERSIONS,
    }))
}

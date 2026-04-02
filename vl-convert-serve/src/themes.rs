use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use itertools::Itertools;
use serde_json::Value;
use std::sync::Arc;

use super::{append_vlc_logs_header, AppState};

#[utoipa::path(
    get,
    path = "/themes",
    responses(
        (status = 200, content_type = "application/json", description = "List of theme names"),
        (status = 500, body = super::types::ErrorResponse, description = "Internal error"),
    ),
    tag = "Themes"
)]
pub async fn list_themes(State(state): State<Arc<AppState>>) -> Response {
    let result = state.converter.get_themes().await;
    let mut headers = HeaderMap::new();
    append_vlc_logs_header(&mut headers, &[]);

    match result {
        Ok(Value::Object(themes)) => {
            let names: Vec<String> = themes.keys().sorted().cloned().collect();
            (
                headers,
                Json(Value::Array(names.into_iter().map(Value::String).collect())),
            )
                .into_response()
        }
        Ok(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            headers,
            Json(serde_json::json!({"error": "unexpected themes format"})),
        )
            .into_response(),
        Err(e) => super::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to load themes: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    get,
    path = "/themes/{name}",
    params(
        ("name" = String, Path, description = "Theme name"),
    ),
    responses(
        (status = 200, content_type = "application/json", description = "Theme configuration object"),
        (status = 404, body = super::types::ErrorResponse, description = "Theme not found"),
        (status = 500, body = super::types::ErrorResponse, description = "Internal error"),
    ),
    tag = "Themes"
)]
pub async fn get_theme(State(state): State<Arc<AppState>>, Path(name): Path<String>) -> Response {
    let result = state.converter.get_themes().await;
    let mut headers = HeaderMap::new();
    append_vlc_logs_header(&mut headers, &[]);

    match result {
        Ok(Value::Object(themes)) => {
            if let Some(theme_config) = themes.get(&name) {
                (headers, Json(theme_config.clone())).into_response()
            } else {
                super::error_response(
                    StatusCode::NOT_FOUND,
                    &format!("unknown theme: {name}"),
                    state.opaque_errors,
                )
            }
        }
        Ok(_) => super::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected themes format",
            state.opaque_errors,
        ),
        Err(e) => super::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to load themes: {e}"),
            state.opaque_errors,
        ),
    }
}

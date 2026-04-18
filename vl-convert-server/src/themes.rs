use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use itertools::Itertools;
use serde_json::Value;
use std::sync::Arc;

use crate::config::AppState;
use crate::util::{append_vlc_logs_header, error_response};

#[utoipa::path(
    get,
    path = "/themes",
    responses(
        (status = 200, content_type = "application/json", description = "List of theme names"),
        (status = 500, body = crate::types::ErrorResponse, description = "Internal error"),
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
        Ok(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected themes format",
            state.opaque_errors,
        ),
        Err(e) => error_response(
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
        (status = 404, body = crate::types::ErrorResponse, description = "Theme not found"),
        (status = 500, body = crate::types::ErrorResponse, description = "Internal error"),
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
                error_response(
                    StatusCode::NOT_FOUND,
                    &format!("unknown theme: {name}"),
                    state.opaque_errors,
                )
            }
        }
        Ok(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected themes format",
            state.opaque_errors,
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to load themes: {e}"),
            state.opaque_errors,
        ),
    }
}

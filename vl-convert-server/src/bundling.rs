use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use std::str::FromStr;
use std::sync::Arc;

use vl_convert_rs::module_loader::import_map::VlVersion;

use crate::config::AppState;
use crate::types::{BundleQuery, BundleSnippetRequest, ErrorResponse};
use crate::util::error_response;

#[utoipa::path(
    get,
    path = "/bundling/bundle",
    params(BundleQuery),
    responses(
        (status = 200, content_type = "application/javascript", description = "Bundled JavaScript"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Bundling failed"),
    ),
    tag = "Bundling"
)]
pub async fn bundle(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BundleQuery>,
) -> Response {
    let snap = state.runtime.load_full();
    let vl_version_str = query.vl_version.as_deref().unwrap_or("6.4");
    let vl_version = match VlVersion::from_str(vl_version_str) {
        Ok(v) => v,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("invalid vl_version: {vl_version_str}"),
                state.opaque_errors,
            )
        }
    };

    match snap.converter.get_vegaembed_bundle(vl_version).await {
        Ok(js) => (
            [
                (
                    axum::http::header::CONTENT_TYPE,
                    "application/javascript; charset=utf-8",
                ),
                (
                    axum::http::header::CACHE_CONTROL,
                    "public, max-age=86400, immutable",
                ),
            ],
            js,
        )
            .into_response(),
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("Bundling failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/bundling/bundle-snippet",
    request_body = BundleSnippetRequest,
    responses(
        (status = 200, content_type = "application/javascript", description = "Bundled JavaScript snippet"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Bundling failed"),
    ),
    tag = "Bundling"
)]
pub async fn bundle_snippet(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BundleSnippetRequest>,
) -> Response {
    let snap = state.runtime.load_full();
    let vl_version = match VlVersion::from_str(&req.vl_version) {
        Ok(v) => v,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("invalid vl_version: {}", req.vl_version),
                state.opaque_errors,
            )
        }
    };

    match snap
        .converter
        .bundle_vega_snippet(req.snippet, vl_version)
        .await
    {
        Ok(js) => (
            [(
                axum::http::header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            )],
            js,
        )
            .into_response(),
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("Snippet bundling failed: {e}"),
            state.opaque_errors,
        ),
    }
}

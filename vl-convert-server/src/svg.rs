use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::sync::Arc;

use vl_convert_rs::converter::{JpegOpts, PdfOpts, PngOpts};

use crate::config::AppState;
use crate::types::{ErrorResponse, SvgJpegRequest, SvgPdfRequest, SvgPngRequest};
use crate::util::{append_vlc_logs_header, error_response, format_log_entries};

#[utoipa::path(
    post,
    path = "/svg/png",
    request_body = SvgPngRequest,
    responses(
        (status = 200, content_type = "image/png", description = "PNG image"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "SVG"
)]
pub async fn svg_to_png(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SvgPngRequest>,
) -> Response {
    let png_opts = PngOpts {
        scale: req.scale,
        ppi: req.ppi,
    };

    match state.converter.svg_to_png(&req.svg, png_opts).await {
        Ok(output) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &format_log_entries(&output.logs));
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "image/png")],
                output.data,
            )
                .into_response()
        }
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("SVG to PNG conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/svg/jpeg",
    request_body = SvgJpegRequest,
    responses(
        (status = 200, content_type = "image/jpeg", description = "JPEG image"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "SVG"
)]
pub async fn svg_to_jpeg(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SvgJpegRequest>,
) -> Response {
    let jpeg_opts = JpegOpts {
        scale: req.scale,
        quality: req.quality,
    };

    match state.converter.svg_to_jpeg(&req.svg, jpeg_opts).await {
        Ok(output) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &format_log_entries(&output.logs));
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "image/jpeg")],
                output.data,
            )
                .into_response()
        }
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("SVG to JPEG conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/svg/pdf",
    request_body = SvgPdfRequest,
    responses(
        (status = 200, content_type = "application/pdf", description = "PDF document"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "SVG"
)]
pub async fn svg_to_pdf(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SvgPdfRequest>,
) -> Response {
    match state
        .converter
        .svg_to_pdf(&req.svg, PdfOpts::default())
        .await
    {
        Ok(output) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &format_log_entries(&output.logs));
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "application/pdf")],
                output.data,
            )
                .into_response()
        }
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("SVG to PDF conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

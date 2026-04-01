use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::sync::Arc;

use vl_convert_rs::converter::{JpegOpts, PdfOpts, PngOpts};

use super::types::SvgRequest;
use super::{append_vlc_logs_header, error_response, AppState};

pub async fn svg_to_png(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SvgRequest>,
) -> Response {
    let png_opts = PngOpts {
        scale: req.scale,
        ppi: req.ppi,
    };

    match state.converter.svg_to_png(&req.svg, png_opts).await {
        Ok(data) => {
            let logs = state.converter.drain_logs().await;
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &logs);
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "image/png")],
                data,
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

pub async fn svg_to_jpeg(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SvgRequest>,
) -> Response {
    let jpeg_opts = JpegOpts {
        scale: req.scale,
        quality: req.quality,
    };

    match state.converter.svg_to_jpeg(&req.svg, jpeg_opts).await {
        Ok(data) => {
            let logs = state.converter.drain_logs().await;
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &logs);
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "image/jpeg")],
                data,
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

pub async fn svg_to_pdf(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SvgRequest>,
) -> Response {
    match state
        .converter
        .svg_to_pdf(&req.svg, PdfOpts::default())
        .await
    {
        Ok(data) => {
            let logs = state.converter.drain_logs().await;
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &logs);
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "application/pdf")],
                data,
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

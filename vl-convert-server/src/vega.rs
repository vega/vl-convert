use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use std::str::FromStr;
use std::sync::Arc;

use vl_convert_rs::converter::{
    vega_to_url as converter_vega_to_url, FormatLocale, HtmlOpts, JpegOpts, PdfOpts, PngOpts,
    Renderer, SvgOpts, TimeFormatLocale, UrlOpts, VgOpts,
};

use super::types::{
    ErrorResponse, VegaCommon, VegaHtmlRequest, VegaJpegRequest, VegaPdfRequest, VegaPngRequest,
    VegaSvgRequest, VegaUrlRequest,
};
use super::{
    append_vlc_logs_header, error_response, format_log_entries, parse_google_font_args, AppState,
};

fn build_vg_opts(req: &VegaCommon, state: &AppState) -> Result<VgOpts, String> {
    let format_locale = req
        .format_locale
        .as_ref()
        .map(|v| match v {
            serde_json::Value::String(s) => Ok(FormatLocale::Name(s.clone())),
            obj @ serde_json::Value::Object(_) => Ok(FormatLocale::Object(obj.clone())),
            _ => Err("format_locale must be a string or object".to_string()),
        })
        .transpose()?;

    let time_format_locale = req
        .time_format_locale
        .as_ref()
        .map(|v| match v {
            serde_json::Value::String(s) => Ok(TimeFormatLocale::Name(s.clone())),
            obj @ serde_json::Value::Object(_) => Ok(TimeFormatLocale::Object(obj.clone())),
            _ => Err("time_format_locale must be a string or object".to_string()),
        })
        .transpose()?;

    let google_fonts = req
        .google_fonts
        .as_ref()
        .map(|fonts| parse_google_font_args(fonts))
        .transpose()?;

    if google_fonts.is_some() && !state.config.allow_google_fonts {
        return Err("google_fonts requires allow_google_fonts: true in server config".to_string());
    }

    if req.vega_plugin.is_some() && !state.config.allow_per_request_plugins {
        return Err(
            "vega_plugin requires allow_per_request_plugins: true in server config".to_string(),
        );
    }

    Ok(VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin: req.vega_plugin.clone(),
        config: req.config.clone(),
        background: req.background.clone(),
        width: req.width,
        height: req.height,
    })
}

#[utoipa::path(
    post,
    path = "/vega/svg",
    request_body = VegaSvgRequest,
    responses(
        (status = 200, content_type = "image/svg+xml", description = "SVG markup"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega"
)]
pub async fn vega_to_svg(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaSvgRequest>,
) -> Response {
    let vg_opts = match build_vg_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let svg_opts = SvgOpts { bundle: req.bundle };
    let spec = req.common.spec;

    match state.converter.vega_to_svg(spec, vg_opts, svg_opts).await {
        Ok(output) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &format_log_entries(&output.logs));
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
                output.svg,
            )
                .into_response()
        }
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("Vega to SVG conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/vega/png",
    request_body = VegaPngRequest,
    responses(
        (status = 200, content_type = "image/png", description = "PNG image"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega"
)]
pub async fn vega_to_png(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaPngRequest>,
) -> Response {
    let vg_opts = match build_vg_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let png_opts = PngOpts {
        scale: req.scale,
        ppi: req.ppi,
    };
    let spec = req.common.spec;

    match state.converter.vega_to_png(spec, vg_opts, png_opts).await {
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
            &format!("Vega to PNG conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/vega/jpeg",
    request_body = VegaJpegRequest,
    responses(
        (status = 200, content_type = "image/jpeg", description = "JPEG image"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega"
)]
pub async fn vega_to_jpeg(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaJpegRequest>,
) -> Response {
    let vg_opts = match build_vg_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let jpeg_opts = JpegOpts {
        scale: req.scale,
        quality: req.quality,
    };
    let spec = req.common.spec;

    match state.converter.vega_to_jpeg(spec, vg_opts, jpeg_opts).await {
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
            &format!("Vega to JPEG conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/vega/pdf",
    request_body = VegaPdfRequest,
    responses(
        (status = 200, content_type = "application/pdf", description = "PDF document"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega"
)]
pub async fn vega_to_pdf(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaPdfRequest>,
) -> Response {
    let vg_opts = match build_vg_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let spec = req.common.spec;

    match state
        .converter
        .vega_to_pdf(spec, vg_opts, PdfOpts::default())
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
            &format!("Vega to PDF conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/vega/html",
    request_body = VegaHtmlRequest,
    responses(
        (status = 200, content_type = "text/html", description = "HTML page"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega"
)]
pub async fn vega_to_html(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaHtmlRequest>,
) -> Response {
    let vg_opts = match build_vg_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let renderer_str = req.renderer.as_deref().unwrap_or("svg");
    let renderer = match Renderer::from_str(renderer_str) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("invalid renderer: {e}"),
                state.opaque_errors,
            )
        }
    };
    let html_opts = HtmlOpts {
        bundle: req.bundle,
        renderer,
    };
    let spec = req.common.spec;

    match state.converter.vega_to_html(spec, vg_opts, html_opts).await {
        Ok(output) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &format_log_entries(&output.logs));
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                output.html,
            )
                .into_response()
        }
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("Vega to HTML conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/vega/url",
    request_body = VegaUrlRequest,
    responses(
        (status = 200, content_type = "text/plain", description = "Vega Editor URL"),
        (status = 422, body = ErrorResponse, description = "URL generation failed"),
    ),
    tag = "Vega"
)]
pub async fn vega_to_url(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaUrlRequest>,
) -> Response {
    let fullscreen = req.fullscreen;
    let spec = req.spec;

    match converter_vega_to_url(&spec, UrlOpts { fullscreen }) {
        Ok(url) => ([(axum::http::header::CONTENT_TYPE, "text/plain")], url).into_response(),
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("Vega URL generation failed: {e}"),
            state.opaque_errors,
        ),
    }
}

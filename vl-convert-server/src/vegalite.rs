use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use std::str::FromStr;
use std::sync::Arc;

use vl_convert_rs::converter::{
    vegalite_to_url as converter_vegalite_to_url, FormatLocale, HtmlOpts, JpegOpts, PdfOpts,
    PngOpts, Renderer, SvgOpts, TimeFormatLocale, UrlOpts, VlOpts,
};
use vl_convert_rs::module_loader::import_map::VlVersion;

use super::types::{
    ErrorResponse, VegaliteCommon, VegaliteHtmlRequest, VegaliteJpegRequest, VegalitePdfRequest,
    VegalitePngRequest, VegaliteSvgRequest, VegaliteUrlRequest, VegaliteVegaRequest,
};
use super::{
    append_vlc_logs_header, error_response, format_log_entries, parse_google_font_args, AppState,
};

fn build_vl_opts(req: &VegaliteCommon, state: &AppState) -> Result<VlOpts, String> {
    let vl_version = VlVersion::from_str(&req.vl_version)
        .map_err(|_| format!("invalid vl_version: {}", req.vl_version))?;

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

    Ok(VlOpts {
        config: req.config.clone(),
        theme: req.theme.clone(),
        vl_version,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin: req.vega_plugin.clone(),
        background: req.background.clone(),
        width: req.width,
        height: req.height,
    })
}

#[utoipa::path(
    post,
    path = "/vegalite/vega",
    request_body = VegaliteVegaRequest,
    responses(
        (status = 200, content_type = "application/json", description = "Vega specification"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega-Lite"
)]
pub async fn vegalite_to_vega(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteVegaRequest>,
) -> Response {
    let mut vl_opts = match build_vl_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    // Apply server-level defaults (theme, locale) that apply_vl_defaults normally handles
    if vl_opts.theme.is_none() {
        vl_opts.theme = state.config.default_theme.clone();
    }
    if vl_opts.format_locale.is_none() {
        vl_opts.format_locale = state.config.default_format_locale.clone();
    }
    if vl_opts.time_format_locale.is_none() {
        vl_opts.time_format_locale = state.config.default_time_format_locale.clone();
    }
    let spec = req.common.spec;

    match state.converter.vegalite_to_vega(spec, vl_opts).await {
        Ok(output) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &format_log_entries(&output.logs));
            let body = serde_json::to_string(&output.spec).unwrap_or_default();
            (
                headers,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                body,
            )
                .into_response()
        }
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("Vega-Lite compilation failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/vegalite/svg",
    request_body = VegaliteSvgRequest,
    responses(
        (status = 200, content_type = "image/svg+xml", description = "SVG markup"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega-Lite"
)]
pub async fn vegalite_to_svg(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteSvgRequest>,
) -> Response {
    let vl_opts = match build_vl_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let svg_opts = SvgOpts { bundle: req.bundle };
    let spec = req.common.spec;

    match state
        .converter
        .vegalite_to_svg(spec, vl_opts, svg_opts)
        .await
    {
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
            &format!("Vega-Lite to SVG conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/vegalite/png",
    request_body = VegalitePngRequest,
    responses(
        (status = 200, content_type = "image/png", description = "PNG image"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega-Lite"
)]
pub async fn vegalite_to_png(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegalitePngRequest>,
) -> Response {
    let vl_opts = match build_vl_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let png_opts = PngOpts {
        scale: req.scale,
        ppi: req.ppi,
    };
    let spec = req.common.spec;

    match state
        .converter
        .vegalite_to_png(spec, vl_opts, png_opts)
        .await
    {
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
            &format!("Vega-Lite to PNG conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/vegalite/jpeg",
    request_body = VegaliteJpegRequest,
    responses(
        (status = 200, content_type = "image/jpeg", description = "JPEG image"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega-Lite"
)]
pub async fn vegalite_to_jpeg(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteJpegRequest>,
) -> Response {
    let vl_opts = match build_vl_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let jpeg_opts = JpegOpts {
        scale: req.scale,
        quality: req.quality,
    };
    let spec = req.common.spec;

    match state
        .converter
        .vegalite_to_jpeg(spec, vl_opts, jpeg_opts)
        .await
    {
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
            &format!("Vega-Lite to JPEG conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/vegalite/pdf",
    request_body = VegalitePdfRequest,
    responses(
        (status = 200, content_type = "application/pdf", description = "PDF document"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega-Lite"
)]
pub async fn vegalite_to_pdf(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegalitePdfRequest>,
) -> Response {
    let vl_opts = match build_vl_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let spec = req.common.spec;

    match state
        .converter
        .vegalite_to_pdf(spec, vl_opts, PdfOpts::default())
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
            &format!("Vega-Lite to PDF conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/vegalite/html",
    request_body = VegaliteHtmlRequest,
    responses(
        (status = 200, content_type = "text/html", description = "HTML page"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega-Lite"
)]
pub async fn vegalite_to_html(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteHtmlRequest>,
) -> Response {
    let vl_opts = match build_vl_opts(&req.common, &state) {
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

    match state
        .converter
        .vegalite_to_html(spec, vl_opts, html_opts)
        .await
    {
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
            &format!("Vega-Lite to HTML conversion failed: {e}"),
            state.opaque_errors,
        ),
    }
}

#[utoipa::path(
    post,
    path = "/vegalite/url",
    request_body = VegaliteUrlRequest,
    responses(
        (status = 200, content_type = "text/plain", description = "Vega Editor URL"),
        (status = 422, body = ErrorResponse, description = "URL generation failed"),
    ),
    tag = "Vega-Lite"
)]
pub async fn vegalite_to_url(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteUrlRequest>,
) -> Response {
    let fullscreen = req.fullscreen;
    let spec = req.spec;

    match converter_vegalite_to_url(&spec, UrlOpts { fullscreen }) {
        Ok(url) => ([(axum::http::header::CONTENT_TYPE, "text/plain")], url).into_response(),
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("Vega-Lite URL generation failed: {e}"),
            state.opaque_errors,
        ),
    }
}

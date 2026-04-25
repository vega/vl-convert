use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use std::str::FromStr;
use std::sync::Arc;

use vl_convert_rs::converter::{
    vega_to_url as converter_vega_to_url, HtmlOpts, JpegOpts, PdfOpts, PngOpts, Renderer, SvgOpts,
    UrlOpts, VgOpts, VlcConfig,
};

use crate::accept::{preferred_scenegraph_format, ScenegraphFormat};
use crate::config::AppState;
use crate::types::{
    ErrorResponse, VegaCommon, VegaFontsRequest, VegaHtmlRequest, VegaJpegRequest, VegaPdfRequest,
    VegaPngRequest, VegaScenegraphRequest, VegaSvgRequest, VegaUrlRequest,
};
use crate::util::{
    append_vlc_logs_header, error_response, format_log_entries, validate_common_opts,
};

fn build_vg_opts(req: &VegaCommon, config: &VlcConfig) -> Result<VgOpts, String> {
    let common = validate_common_opts(req, config)?;

    Ok(VgOpts {
        format_locale: common.format_locale,
        time_format_locale: common.time_format_locale,
        google_fonts: common.google_fonts,
        vega_plugin: common.vega_plugin,
        config: common.config,
        background: common.background,
        width: common.width,
        height: common.height,
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
    let snap = state.runtime.load_full();
    let vg_opts = match build_vg_opts(&req.common, &snap.config) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let svg_opts = SvgOpts { bundle: req.bundle };
    let spec = req.common.spec;

    match snap.converter.vega_to_svg(spec, vg_opts, svg_opts).await {
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
    let snap = state.runtime.load_full();
    let vg_opts = match build_vg_opts(&req.common, &snap.config) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let png_opts = PngOpts {
        scale: req.scale,
        ppi: req.ppi,
    };
    let spec = req.common.spec;

    match snap.converter.vega_to_png(spec, vg_opts, png_opts).await {
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
    let snap = state.runtime.load_full();
    let vg_opts = match build_vg_opts(&req.common, &snap.config) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let jpeg_opts = JpegOpts {
        scale: req.scale,
        quality: req.quality,
    };
    let spec = req.common.spec;

    match snap.converter.vega_to_jpeg(spec, vg_opts, jpeg_opts).await {
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
    let snap = state.runtime.load_full();
    let vg_opts = match build_vg_opts(&req.common, &snap.config) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let spec = req.common.spec;

    match snap
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
    let snap = state.runtime.load_full();
    let vg_opts = match build_vg_opts(&req.common, &snap.config) {
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

    match snap.converter.vega_to_html(spec, vg_opts, html_opts).await {
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

#[utoipa::path(
    post,
    path = "/vega/scenegraph",
    request_body = VegaScenegraphRequest,
    responses(
        (status = 200, description = "Scenegraph (set Accept: application/msgpack for binary format)", content(
            (serde_json::Value = "application/json"),
            (Vec<u8> = "application/msgpack")
        )),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega"
)]
pub async fn vega_scenegraph(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<VegaScenegraphRequest>,
) -> Response {
    let snap = state.runtime.load_full();
    let vg_opts = match build_vg_opts(&req.common, &snap.config) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let spec = req.common.spec;
    let wants_msgpack = preferred_scenegraph_format(&headers) == ScenegraphFormat::Msgpack;

    if wants_msgpack {
        match snap
            .converter
            .vega_to_scenegraph_msgpack(spec, vg_opts)
            .await
        {
            Ok(output) => {
                let mut resp_headers = HeaderMap::new();
                append_vlc_logs_header(&mut resp_headers, &format_log_entries(&output.logs));
                (
                    resp_headers,
                    [(axum::http::header::CONTENT_TYPE, "application/msgpack")],
                    output.data,
                )
                    .into_response()
            }
            Err(e) => error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                &format!("Vega scenegraph extraction failed: {e}"),
                state.opaque_errors,
            ),
        }
    } else {
        match snap.converter.vega_to_scenegraph(spec, vg_opts).await {
            Ok(output) => {
                let mut resp_headers = HeaderMap::new();
                append_vlc_logs_header(&mut resp_headers, &format_log_entries(&output.logs));
                let body = match serde_json::to_string(&output.scenegraph) {
                    Ok(json) => json,
                    Err(e) => {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            &format!("Failed to serialize scenegraph: {e}"),
                            state.opaque_errors,
                        )
                    }
                };
                (
                    resp_headers,
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
                &format!("Vega scenegraph extraction failed: {e}"),
                state.opaque_errors,
            ),
        }
    }
}

#[utoipa::path(
    post,
    path = "/vega/fonts",
    request_body = VegaFontsRequest,
    responses(
        (status = 200, content_type = "application/json", description = "Font information"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Font analysis failed"),
    ),
    tag = "Vega"
)]
pub async fn vega_fonts(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaFontsRequest>,
) -> Response {
    let snap = state.runtime.load_full();
    let vg_opts = match build_vg_opts(&req.common, &snap.config) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let spec = req.common.spec;

    match snap
        .converter
        .vega_fonts(
            spec,
            vg_opts,
            snap.config.auto_google_fonts,
            snap.config.embed_local_fonts,
            req.include_font_face,
            snap.config.subset_fonts,
        )
        .await
    {
        Ok(fonts) => Json(fonts).into_response(),
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("Vega font analysis failed: {e}"),
            state.opaque_errors,
        ),
    }
}

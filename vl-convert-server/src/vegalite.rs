use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use std::str::FromStr;
use std::sync::Arc;

use vl_convert_rs::converter::{
    vegalite_to_url as converter_vegalite_to_url, HtmlOpts, JpegOpts, PdfOpts, PngOpts, Renderer,
    SvgOpts, UrlOpts, VlOpts,
};
use vl_convert_rs::module_loader::import_map::VlVersion;

use super::types::{
    ErrorResponse, VegaliteCommon, VegaliteFontsRequest, VegaliteHtmlRequest, VegaliteJpegRequest,
    VegalitePdfRequest, VegalitePngRequest, VegaliteScenegraphRequest, VegaliteSvgRequest,
    VegaliteUrlRequest, VegaliteVegaRequest,
};
use super::{
    append_vlc_logs_header, error_response, format_log_entries, preferred_scenegraph_format,
    validate_common_opts, AppState, ScenegraphFormat,
};

fn build_vl_opts(req: &VegaliteCommon, state: &AppState) -> Result<VlOpts, String> {
    let vl_version = VlVersion::from_str(&req.vl_version)
        .map_err(|_| format!("invalid vl_version: {}", req.vl_version))?;

    let common = validate_common_opts(
        &req.format_locale,
        &req.time_format_locale,
        &req.google_fonts,
        &req.vega_plugin,
        &req.config,
        &req.background,
        req.width,
        req.height,
        state,
    )?;

    Ok(VlOpts {
        config: common.config,
        theme: req.theme.clone(),
        vl_version,
        format_locale: common.format_locale,
        time_format_locale: common.time_format_locale,
        google_fonts: common.google_fonts,
        vega_plugin: common.vega_plugin,
        background: common.background,
        width: common.width,
        height: common.height,
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
    let vl_opts = match build_vl_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let spec = req.common.spec;

    match state.converter.vegalite_to_vega(spec, vl_opts).await {
        Ok(output) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &format_log_entries(&output.logs));
            let body = match serde_json::to_string(&output.spec) {
                Ok(json) => json,
                Err(e) => {
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("failed to serialize Vega spec: {e}"),
                        state.opaque_errors,
                    );
                }
            };
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

#[utoipa::path(
    post,
    path = "/vegalite/scenegraph",
    request_body = VegaliteScenegraphRequest,
    responses(
        (status = 200, description = "Scenegraph (set Accept: application/msgpack for binary format)", content(
            (serde_json::Value = "application/json"),
            (Vec<u8> = "application/msgpack")
        )),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Conversion failed"),
    ),
    tag = "Vega-Lite"
)]
pub async fn vegalite_scenegraph(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<VegaliteScenegraphRequest>,
) -> Response {
    let vl_opts = match build_vl_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let spec = req.common.spec;
    let wants_msgpack = preferred_scenegraph_format(&headers) == ScenegraphFormat::Msgpack;

    if wants_msgpack {
        match state
            .converter
            .vegalite_to_scenegraph_msgpack(spec, vl_opts)
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
                &format!("Vega-Lite scenegraph extraction failed: {e}"),
                state.opaque_errors,
            ),
        }
    } else {
        match state.converter.vegalite_to_scenegraph(spec, vl_opts).await {
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
                &format!("Vega-Lite scenegraph extraction failed: {e}"),
                state.opaque_errors,
            ),
        }
    }
}

#[utoipa::path(
    post,
    path = "/vegalite/fonts",
    request_body = VegaliteFontsRequest,
    responses(
        (status = 200, content_type = "application/json", description = "Font information"),
        (status = 400, body = ErrorResponse, description = "Invalid request"),
        (status = 422, body = ErrorResponse, description = "Font analysis failed"),
    ),
    tag = "Vega-Lite"
)]
pub async fn vegalite_fonts(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteFontsRequest>,
) -> Response {
    let vl_opts = match build_vl_opts(&req.common, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let spec = req.common.spec;

    match state
        .converter
        .vegalite_fonts(
            spec,
            vl_opts,
            state.config.auto_google_fonts,
            state.config.embed_local_fonts,
            req.include_font_face,
            state.config.subset_fonts,
        )
        .await
    {
        Ok(fonts) => Json(fonts).into_response(),
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("Vega-Lite font analysis failed: {e}"),
            state.opaque_errors,
        ),
    }
}

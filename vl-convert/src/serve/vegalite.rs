use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use std::str::FromStr;
use std::sync::Arc;

use vl_convert_rs::converter::{
    vegalite_to_url as converter_vegalite_to_url, FormatLocale, HtmlOpts, JpegOpts, PdfOpts,
    PngOpts, Renderer, SvgOpts, TimeFormatLocale, VlOpts,
};
use vl_convert_rs::module_loader::import_map::VlVersion;

use super::types::{UrlResponse, VegaliteRequest};
use super::{append_vlc_logs_header, error_response, parse_google_font_args, AppState};

fn build_vl_opts(req: &VegaliteRequest, state: &AppState) -> Result<VlOpts, String> {
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

pub async fn vegalite_to_vega(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteRequest>,
) -> Response {
    let mut vl_opts = match build_vl_opts(&req, &state) {
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
    let pretty = req.pretty;
    let spec = req.spec;

    let result = state
        .converter
        .run_on_worker(move |inner| {
            Box::pin(async move {
                let vega = inner.vegalite_to_vega(spec, vl_opts).await?;
                let logs = inner.drain_log_entries();
                Ok((vega, logs))
            })
        })
        .await;

    match result {
        Ok((vega, logs)) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &logs);
            let body = if pretty {
                serde_json::to_string_pretty(&vega).unwrap_or_default()
            } else {
                serde_json::to_string(&vega).unwrap_or_default()
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

pub async fn vegalite_to_svg(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteRequest>,
) -> Response {
    let vl_opts = match build_vl_opts(&req, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let svg_opts = SvgOpts { bundle: req.bundle };
    let spec = req.spec;

    match state
        .converter
        .vegalite_to_svg(spec, vl_opts, svg_opts)
        .await
    {
        Ok(svg) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &[]);
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
                svg,
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

pub async fn vegalite_to_png(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteRequest>,
) -> Response {
    let vl_opts = match build_vl_opts(&req, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let png_opts = PngOpts {
        scale: req.scale,
        ppi: req.ppi,
    };
    let spec = req.spec;

    match state
        .converter
        .vegalite_to_png(spec, vl_opts, png_opts)
        .await
    {
        Ok(data) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &[]);
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "image/png")],
                data,
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

pub async fn vegalite_to_jpeg(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteRequest>,
) -> Response {
    let vl_opts = match build_vl_opts(&req, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let jpeg_opts = JpegOpts {
        scale: req.scale,
        quality: req.quality,
    };
    let spec = req.spec;

    match state
        .converter
        .vegalite_to_jpeg(spec, vl_opts, jpeg_opts)
        .await
    {
        Ok(data) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &[]);
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "image/jpeg")],
                data,
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

pub async fn vegalite_to_pdf(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteRequest>,
) -> Response {
    let vl_opts = match build_vl_opts(&req, &state) {
        Ok(opts) => opts,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e, state.opaque_errors),
    };
    let spec = req.spec;

    match state
        .converter
        .vegalite_to_pdf(spec, vl_opts, PdfOpts::default())
        .await
    {
        Ok(data) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &[]);
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "application/pdf")],
                data,
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

pub async fn vegalite_to_html(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteRequest>,
) -> Response {
    let vl_opts = match build_vl_opts(&req, &state) {
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
    let spec = req.spec;

    match state
        .converter
        .vegalite_to_html(spec, vl_opts, html_opts)
        .await
    {
        Ok(html) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &[]);
            (
                headers,
                [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                html,
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

pub async fn vegalite_to_url(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VegaliteRequest>,
) -> Response {
    let fullscreen = req.fullscreen;
    let spec = req.spec;

    match converter_vegalite_to_url(&spec, fullscreen) {
        Ok(url) => {
            let mut headers = HeaderMap::new();
            append_vlc_logs_header(&mut headers, &[]);
            (headers, Json(UrlResponse { url })).into_response()
        }
        Err(e) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("Vega-Lite URL generation failed: {e}"),
            state.opaque_errors,
        ),
    }
}

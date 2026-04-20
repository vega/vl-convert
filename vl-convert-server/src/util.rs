use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use vl_convert_rs::converter::{FormatLocale, GoogleFontRequest, LogEntry, TimeFormatLocale};

use crate::types::ErrorResponse;
use crate::AppState;

pub(crate) fn format_log_entries(logs: &[LogEntry]) -> Vec<String> {
    logs.iter()
        .map(|e| format!("{}: {}", e.level, e.message))
        .collect()
}

pub(crate) fn vegalite_versions() -> Vec<&'static str> {
    vl_convert_rs::module_loader::import_map::VL_VERSIONS
        .iter()
        .map(|v| v.to_semver())
        .collect()
}

pub(crate) fn error_response(status: StatusCode, message: &str, opaque: bool) -> Response {
    if opaque {
        status.into_response()
    } else {
        (
            status,
            Json(ErrorResponse {
                error: message.to_string(),
            }),
        )
            .into_response()
    }
}

pub(crate) fn append_vlc_logs_header(headers: &mut HeaderMap, logs: &[String]) {
    let truncated: Vec<&str> = logs.iter().take(50).map(|s| s.as_str()).collect();
    let json = serde_json::to_string(&truncated).unwrap_or_else(|_| "[]".to_string());
    if let Ok(val) = HeaderValue::from_str(&json) {
        headers.insert("x-vlc-logs", val);
    } else {
        let safe: String = json
            .chars()
            .filter(|c| c.is_ascii_graphic() || *c == ' ')
            .collect();
        if let Ok(val) = HeaderValue::from_str(&safe) {
            headers.insert("x-vlc-logs", val);
        } else {
            headers.insert("x-vlc-logs", HeaderValue::from_static("[]"));
        }
    }
}

pub(crate) fn parse_google_font_args(fonts: &[String]) -> Result<Vec<GoogleFontRequest>, String> {
    fonts
        .iter()
        .map(|s| {
            let Some((family, variants_str)) = s.split_once(':') else {
                return Ok(GoogleFontRequest {
                    family: s.to_string(),
                    variants: None,
                });
            };
            let mut variants = Vec::new();
            for token in variants_str.split(',') {
                let token = token.trim();
                if token.is_empty() {
                    continue;
                }
                let (weight_str, style) = if let Some(w) = token.strip_suffix("italic") {
                    (w, vl_convert_google_fonts::FontStyle::Italic)
                } else {
                    (token, vl_convert_google_fonts::FontStyle::Normal)
                };
                let weight: u16 = weight_str
                    .parse()
                    .map_err(|_| format!("invalid font variant '{token}' in '{s}'"))?;
                variants.push(vl_convert_google_fonts::VariantRequest { weight, style });
            }
            Ok(GoogleFontRequest {
                family: family.to_string(),
                variants: if variants.is_empty() {
                    None
                } else {
                    Some(variants)
                },
            })
        })
        .collect()
}

pub(crate) struct CommonOpts {
    pub format_locale: Option<FormatLocale>,
    pub time_format_locale: Option<TimeFormatLocale>,
    pub google_fonts: Option<Vec<GoogleFontRequest>>,
    pub vega_plugin: Option<String>,
    pub config: Option<serde_json::Value>,
    pub background: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

/// Accessor trait implemented by [`crate::types::VegaCommon`] and
/// [`crate::types::VegaliteCommon`]. Lets [`validate_common_opts`] take a
/// single borrowed argument regardless of which request flavor it came from.
pub(crate) trait CommonOptsInput {
    fn format_locale(&self) -> &Option<serde_json::Value>;
    fn time_format_locale(&self) -> &Option<serde_json::Value>;
    fn google_fonts(&self) -> &Option<Vec<String>>;
    fn vega_plugin(&self) -> &Option<String>;
    fn config(&self) -> &Option<serde_json::Value>;
    fn background(&self) -> &Option<String>;
    fn width(&self) -> Option<f32>;
    fn height(&self) -> Option<f32>;
}

pub(crate) fn validate_common_opts(
    req: &impl CommonOptsInput,
    state: &AppState,
) -> Result<CommonOpts, String> {
    let format_locale = req
        .format_locale()
        .as_ref()
        .map(|v| match v {
            serde_json::Value::String(s) => Ok(FormatLocale::Name(s.clone())),
            obj @ serde_json::Value::Object(_) => Ok(FormatLocale::Object(obj.clone())),
            _ => Err("format_locale must be a string or object".to_string()),
        })
        .transpose()?;

    let time_format_locale = req
        .time_format_locale()
        .as_ref()
        .map(|v| match v {
            serde_json::Value::String(s) => Ok(TimeFormatLocale::Name(s.clone())),
            obj @ serde_json::Value::Object(_) => Ok(TimeFormatLocale::Object(obj.clone())),
            _ => Err("time_format_locale must be a string or object".to_string()),
        })
        .transpose()?;

    let google_fonts = req
        .google_fonts()
        .as_ref()
        .map(|fonts| parse_google_font_args(fonts))
        .transpose()?;

    if google_fonts.is_some() && !state.config.allow_google_fonts {
        return Err("google_fonts requires allow_google_fonts: true in server config".to_string());
    }

    if req.vega_plugin().is_some() && !state.config.allow_per_request_plugins {
        return Err(
            "vega_plugin requires allow_per_request_plugins: true in server config".to_string(),
        );
    }

    Ok(CommonOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin: req.vega_plugin().clone(),
        config: req.config().clone(),
        background: req.background().clone(),
        width: req.width(),
        height: req.height(),
    })
}

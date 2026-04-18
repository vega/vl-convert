use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use vl_convert_rs::converter::{FormatLocale, GoogleFontRequest, LogEntry, TimeFormatLocale};

use crate::types::ErrorResponse;
use crate::AppState;

pub fn format_log_entries(logs: &[LogEntry]) -> Vec<String> {
    logs.iter()
        .map(|e| format!("{}: {}", e.level, e.message))
        .collect()
}

pub fn vegalite_versions() -> Vec<&'static str> {
    vl_convert_rs::module_loader::import_map::VL_VERSIONS
        .iter()
        .map(|v| v.to_semver())
        .collect()
}

pub fn error_response(status: StatusCode, message: &str, opaque: bool) -> Response {
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

pub fn append_vlc_logs_header(headers: &mut HeaderMap, logs: &[String]) {
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

pub fn parse_google_font_args(fonts: &[String]) -> Result<Vec<GoogleFontRequest>, String> {
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_common_opts(
    format_locale: &Option<serde_json::Value>,
    time_format_locale: &Option<serde_json::Value>,
    google_fonts: &Option<Vec<String>>,
    vega_plugin: &Option<String>,
    config: &Option<serde_json::Value>,
    background: &Option<String>,
    width: Option<f32>,
    height: Option<f32>,
    state: &AppState,
) -> Result<CommonOpts, String> {
    let format_locale = format_locale
        .as_ref()
        .map(|v| match v {
            serde_json::Value::String(s) => Ok(FormatLocale::Name(s.clone())),
            obj @ serde_json::Value::Object(_) => Ok(FormatLocale::Object(obj.clone())),
            _ => Err("format_locale must be a string or object".to_string()),
        })
        .transpose()?;

    let time_format_locale = time_format_locale
        .as_ref()
        .map(|v| match v {
            serde_json::Value::String(s) => Ok(TimeFormatLocale::Name(s.clone())),
            obj @ serde_json::Value::Object(_) => Ok(TimeFormatLocale::Object(obj.clone())),
            _ => Err("time_format_locale must be a string or object".to_string()),
        })
        .transpose()?;

    let google_fonts = google_fonts
        .as_ref()
        .map(|fonts| parse_google_font_args(fonts))
        .transpose()?;

    if google_fonts.is_some() && !state.config.allow_google_fonts {
        return Err("google_fonts requires allow_google_fonts: true in server config".to_string());
    }

    if vega_plugin.is_some() && !state.config.allow_per_request_plugins {
        return Err(
            "vega_plugin requires allow_per_request_plugins: true in server config".to_string(),
        );
    }

    Ok(CommonOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin: vega_plugin.clone(),
        config: config.clone(),
        background: background.clone(),
        width,
        height,
    })
}

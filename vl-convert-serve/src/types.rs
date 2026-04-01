use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct VegaliteRequest {
    pub spec: serde_json::Value,
    #[serde(default = "default_vl_version")]
    pub vl_version: String,
    pub theme: Option<String>,
    pub config: Option<serde_json::Value>,
    pub background: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub format_locale: Option<serde_json::Value>,
    pub time_format_locale: Option<serde_json::Value>,
    pub google_fonts: Option<Vec<String>>,
    pub vega_plugin: Option<String>,
    #[serde(default)]
    pub scale: Option<f32>,
    #[serde(default)]
    pub ppi: Option<f32>,
    #[serde(default)]
    pub quality: Option<u8>,
    #[serde(default)]
    pub bundle: bool,
    pub renderer: Option<String>,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub pretty: bool,
}

#[derive(Debug, Deserialize)]
pub struct VegaRequest {
    pub spec: serde_json::Value,
    pub config: Option<serde_json::Value>,
    pub background: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub format_locale: Option<serde_json::Value>,
    pub time_format_locale: Option<serde_json::Value>,
    pub google_fonts: Option<Vec<String>>,
    pub vega_plugin: Option<String>,
    #[serde(default)]
    pub scale: Option<f32>,
    #[serde(default)]
    pub ppi: Option<f32>,
    #[serde(default)]
    pub quality: Option<u8>,
    #[serde(default)]
    pub bundle: bool,
    pub renderer: Option<String>,
    #[serde(default)]
    pub fullscreen: bool,
}

#[derive(Debug, Deserialize)]
pub struct SvgRequest {
    pub svg: String,
    #[serde(default)]
    pub scale: Option<f32>,
    #[serde(default)]
    pub ppi: Option<f32>,
    #[serde(default)]
    pub quality: Option<u8>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct UrlResponse {
    pub url: String,
}

fn default_vl_version() -> String {
    "6.4".to_string()
}

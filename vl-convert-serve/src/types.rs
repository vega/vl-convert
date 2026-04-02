use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

fn default_vl_version() -> String {
    "6.4".to_string()
}

// --- Shared sub-structs ---

/// Fields common to all Vega-Lite conversion requests.
#[derive(Debug, Deserialize, ToSchema)]
pub struct VegaliteCommon {
    /// Vega-Lite specification as a JSON object.
    pub spec: serde_json::Value,
    /// Vega-Lite version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4.
    #[serde(default = "default_vl_version")]
    pub vl_version: String,
    /// Named theme from vega-themes (e.g., "dark").
    pub theme: Option<String>,
    /// Vega-Lite config object.
    pub config: Option<serde_json::Value>,
    /// Background color (applied to spec.background).
    pub background: Option<String>,
    /// Override spec width.
    pub width: Option<f32>,
    /// Override spec height.
    pub height: Option<f32>,
    /// d3-format locale (name or inline object).
    pub format_locale: Option<serde_json::Value>,
    /// d3-time-format locale (name or inline object).
    pub time_format_locale: Option<serde_json::Value>,
    /// Google Fonts to register (e.g., ["Roboto", "Pacifico:400,700italic"]).
    pub google_fonts: Option<Vec<String>>,
    /// Per-request Vega plugin (inline ESM or URL).
    pub vega_plugin: Option<String>,
}

/// Fields common to all Vega conversion requests.
#[derive(Debug, Deserialize, ToSchema)]
pub struct VegaCommon {
    /// Vega specification as a JSON object.
    pub spec: serde_json::Value,
    /// Vega config object merged via vega.mergeConfig.
    pub config: Option<serde_json::Value>,
    /// Background color (applied to spec.background).
    pub background: Option<String>,
    /// Override spec width.
    pub width: Option<f32>,
    /// Override spec height.
    pub height: Option<f32>,
    /// d3-format locale (name or inline object).
    pub format_locale: Option<serde_json::Value>,
    /// d3-time-format locale (name or inline object).
    pub time_format_locale: Option<serde_json::Value>,
    /// Google Fonts to register.
    pub google_fonts: Option<Vec<String>>,
    /// Per-request Vega plugin (inline ESM or URL).
    pub vega_plugin: Option<String>,
}

// --- Vega-Lite per-endpoint types ---

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaliteVegaRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaliteSvgRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
    /// Bundle fonts and images into a self-contained SVG.
    #[serde(default)]
    pub bundle: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegalitePngRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
    /// Image scale factor.
    pub scale: Option<f32>,
    /// Pixels per inch.
    pub ppi: Option<f32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaliteJpegRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
    /// Image scale factor.
    pub scale: Option<f32>,
    /// JPEG quality (0-100).
    pub quality: Option<u8>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegalitePdfRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaliteHtmlRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
    /// Bundle Vega JS inline instead of loading from CDN.
    #[serde(default)]
    pub bundle: bool,
    /// Renderer: "svg", "canvas", or "hybrid".
    pub renderer: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaliteUrlRequest {
    /// Vega-Lite specification as a JSON object.
    pub spec: serde_json::Value,
    /// Open in fullscreen view in the Vega Editor.
    #[serde(default)]
    pub fullscreen: bool,
}

// --- Vega per-endpoint types ---

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaSvgRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
    #[serde(default)]
    pub bundle: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaPngRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
    pub scale: Option<f32>,
    pub ppi: Option<f32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaJpegRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
    pub scale: Option<f32>,
    pub quality: Option<u8>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaPdfRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaHtmlRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
    #[serde(default)]
    pub bundle: bool,
    pub renderer: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaUrlRequest {
    pub spec: serde_json::Value,
    #[serde(default)]
    pub fullscreen: bool,
}

// --- SVG input per-endpoint types ---

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SvgPngRequest {
    /// SVG markup string.
    pub svg: String,
    pub scale: Option<f32>,
    pub ppi: Option<f32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SvgJpegRequest {
    pub svg: String,
    pub scale: Option<f32>,
    pub quality: Option<u8>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SvgPdfRequest {
    pub svg: String,
}

// --- Response types ---

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}

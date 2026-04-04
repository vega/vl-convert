use crate::module_loader::import_map::VlVersion;
use crate::module_loader::{FORMATE_LOCALE_MAP, TIME_FORMATE_LOCALE_MAP};
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Clone, Default)]
pub struct VgOpts {
    pub format_locale: Option<FormatLocale>,
    pub time_format_locale: Option<TimeFormatLocale>,
    /// Per-request overlay plugin (inline ESM or URL). Requires `allow_per_request_plugins`.
    pub vega_plugin: Option<String>,
    pub google_fonts: Option<Vec<super::GoogleFontRequest>>,
    /// Vega config object merged via `vega.mergeConfig(spec.config, config)`.
    pub config: Option<serde_json::Value>,
    /// Sets `spec.background` (top-level Vega property).
    pub background: Option<String>,
    /// Override the spec's width.
    pub width: Option<f32>,
    /// Override the spec's height.
    pub height: Option<f32>,
}

impl VgOpts {
    pub fn to_embed_opts(&self, renderer: Renderer) -> Result<serde_json::Value, AnyError> {
        let mut opts_map = serde_json::Map::new();

        opts_map.insert(
            "renderer".to_string(),
            serde_json::Value::String(renderer.to_string()),
        );

        if let Some(config) = &self.config {
            opts_map.insert("config".to_string(), config.clone());
        }
        if let Some(w) = self.width {
            if let Some(n) = serde_json::Number::from_f64(w as f64) {
                opts_map.insert("width".to_string(), serde_json::Value::Number(n));
            }
        }
        if let Some(h) = self.height {
            if let Some(n) = serde_json::Number::from_f64(h as f64) {
                opts_map.insert("height".to_string(), serde_json::Value::Number(n));
            }
        }
        if let Some(format_locale) = &self.format_locale {
            opts_map.insert("formatLocale".to_string(), format_locale.as_object()?);
        }
        if let Some(time_format_locale) = &self.time_format_locale {
            opts_map.insert(
                "timeFormatLocale".to_string(),
                time_format_locale.as_object()?,
            );
        }

        Ok(serde_json::Value::Object(opts_map))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(untagged)]
pub enum FormatLocale {
    Name(String),
    Object(serde_json::Value),
}

impl FormatLocale {
    pub fn as_object(&self) -> Result<serde_json::Value, AnyError> {
        match self {
            FormatLocale::Name(name) => {
                let Some(locale_str) = FORMATE_LOCALE_MAP.get(name) else {
                    return Err(anyhow!("No built-in format locale named {}", name));
                };
                Ok(serde_json::from_str(locale_str)?)
            }
            FormatLocale::Object(object) => Ok(object.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(untagged)]
pub enum TimeFormatLocale {
    Name(String),
    Object(serde_json::Value),
}

impl TimeFormatLocale {
    pub fn as_object(&self) -> Result<serde_json::Value, AnyError> {
        match self {
            TimeFormatLocale::Name(name) => {
                let Some(locale_str) = TIME_FORMATE_LOCALE_MAP.get(name) else {
                    return Err(anyhow!("No built-in time format locale named {}", name));
                };
                Ok(serde_json::from_str(locale_str)?)
            }
            TimeFormatLocale::Object(object) => Ok(object.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Renderer {
    Svg,
    Canvas,
    Hybrid,
}

impl Display for Renderer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let r = match self {
            Renderer::Svg => "svg",
            Renderer::Canvas => "canvas",
            Renderer::Hybrid => "hybrid",
        };
        std::fmt::Display::fmt(r, f)
    }
}

impl FromStr for Renderer {
    type Err = AnyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "svg" => Self::Svg,
            "canvas" => Self::Canvas,
            "hybrid" => Self::Hybrid,
            _ => return Err(anyhow!("Unsupported renderer: {}", s)),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct VlOpts {
    pub config: Option<serde_json::Value>,
    pub theme: Option<String>,
    pub vl_version: VlVersion,
    pub format_locale: Option<FormatLocale>,
    pub time_format_locale: Option<TimeFormatLocale>,
    pub google_fonts: Option<Vec<super::GoogleFontRequest>>,
    /// Per-request overlay plugin (inline ESM or URL). Requires `allow_per_request_plugins`.
    pub vega_plugin: Option<String>,
    /// Sets `spec.background` before Vega-Lite compilation.
    pub background: Option<String>,
    /// Override the spec's width.
    pub width: Option<f32>,
    /// Override the spec's height.
    pub height: Option<f32>,
}

impl VlOpts {
    pub fn to_embed_opts(&self, renderer: Renderer) -> Result<serde_json::Value, AnyError> {
        let mut opts_map = serde_json::Map::new();

        opts_map.insert(
            "renderer".to_string(),
            serde_json::Value::String(renderer.to_string()),
        );

        if let Some(theme) = &self.theme {
            opts_map.insert(
                "theme".to_string(),
                serde_json::Value::String(theme.clone()),
            );
        }

        if let Some(config) = &self.config {
            opts_map.insert("config".to_string(), config.clone());
        }

        if let Some(w) = self.width {
            if let Some(n) = serde_json::Number::from_f64(w as f64) {
                opts_map.insert("width".to_string(), serde_json::Value::Number(n));
            }
        }
        if let Some(h) = self.height {
            if let Some(n) = serde_json::Number::from_f64(h as f64) {
                opts_map.insert("height".to_string(), serde_json::Value::Number(n));
            }
        }
        if let Some(format_locale) = &self.format_locale {
            opts_map.insert("formatLocale".to_string(), format_locale.as_object()?);
        }
        if let Some(time_format_locale) = &self.time_format_locale {
            opts_map.insert(
                "timeFormatLocale".to_string(),
                time_format_locale.as_object()?,
            );
        }

        Ok(serde_json::Value::Object(opts_map))
    }
}

/// Options specific to SVG output format.
#[derive(Debug, Clone, Default)]
pub struct SvgOpts {
    pub bundle: bool,
}

/// Options specific to HTML output format.
#[derive(Debug, Clone)]
pub struct HtmlOpts {
    pub bundle: bool,
    pub renderer: Renderer,
}

impl Default for HtmlOpts {
    fn default() -> Self {
        Self {
            bundle: false,
            renderer: Renderer::Svg,
        }
    }
}

/// Options specific to PNG output format.
#[derive(Debug, Clone, Default)]
pub struct PngOpts {
    pub scale: Option<f32>,
    pub ppi: Option<f32>,
}

/// Options specific to JPEG output format.
#[derive(Debug, Clone, Default)]
pub struct JpegOpts {
    pub scale: Option<f32>,
    pub quality: Option<u8>,
}

/// Options specific to PDF output format.
#[derive(Debug, Clone, Default)]
pub struct PdfOpts {}

/// Options specific to URL output format.
#[derive(Debug, Clone, Default)]
pub struct UrlOpts {
    pub fullscreen: bool,
}

/// Log level for entries captured during Vega/VL evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Debug => write!(f, "DEBUG"),
        }
    }
}

/// A log entry captured during Vega/VL evaluation.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

/// Output from a Vega-Lite -> Vega compilation.
#[derive(Debug)]
pub struct VegaOutput {
    pub spec: serde_json::Value,
    pub logs: Vec<LogEntry>,
}

/// Output from an SVG conversion.
#[derive(Debug)]
pub struct SvgOutput {
    pub svg: String,
    pub logs: Vec<LogEntry>,
}

/// Output from a PNG conversion.
#[derive(Debug)]
pub struct PngOutput {
    pub data: Vec<u8>,
    pub logs: Vec<LogEntry>,
}

/// Output from a JPEG conversion.
#[derive(Debug)]
pub struct JpegOutput {
    pub data: Vec<u8>,
    pub logs: Vec<LogEntry>,
}

/// Output from a PDF conversion.
#[derive(Debug)]
pub struct PdfOutput {
    pub data: Vec<u8>,
    pub logs: Vec<LogEntry>,
}

/// Output from an HTML conversion.
#[derive(Debug)]
pub struct HtmlOutput {
    pub html: String,
    pub logs: Vec<LogEntry>,
}

/// Output from a scenegraph extraction.
#[derive(Debug)]
pub struct ScenegraphOutput {
    pub scenegraph: serde_json::Value,
    pub logs: Vec<LogEntry>,
}

/// Output from a scenegraph msgpack extraction.
#[derive(Debug)]
pub struct ScenegraphMsgpackOutput {
    pub data: Vec<u8>,
    pub logs: Vec<LogEntry>,
}

/// V8 memory usage for a single worker.
#[derive(Debug, Clone)]
pub struct WorkerMemoryUsage {
    /// Index of the worker in the pool (0-based).
    pub worker_index: usize,
    /// Bytes of heap currently in use by V8.
    pub used_heap_size: usize,
    /// Total heap size allocated by V8.
    pub total_heap_size: usize,
    /// Maximum heap size allowed by V8 for this isolate.
    pub heap_size_limit: usize,
    /// External memory reported to V8.
    pub external_memory: usize,
}

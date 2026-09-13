use crate::module_loader::import_map::VlVersion;
use crate::module_loader::{FORMATE_LOCALE_MAP, TIME_FORMATE_LOCALE_MAP};
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use vl_convert_google_fonts::GoogleFontUsage;

/// Options applied to a Vega specification before conversion.
///
/// Unset locale options inherit the converter defaults. Size overrides change the
/// logical chart dimensions, which can differ from the final image dimensions
/// because of Vega autosizing, axes, legends, and padding.
#[derive(Debug, Clone, Default)]
pub struct VgOpts {
    /// Number-format locale. `None` inherits the converter default.
    pub format_locale: Option<FormatLocale>,
    /// Date/time-format locale. `None` inherits the converter default.
    pub time_format_locale: Option<TimeFormatLocale>,
    /// Per-request overlay plugin (inline ESM or URL). Requires `allow_per_request_plugins`.
    pub vega_plugin: Option<String>,
    /// Additional Google Fonts to load for this conversion.
    /// Configured font requests still apply. `None` adds no per-call requests.
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
    pub(crate) fn to_embed_opts(&self, renderer: Renderer) -> Result<serde_json::Value, AnyError> {
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

/// Number-format locale, supplied as a bundled locale name or a d3-format object.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum FormatLocale {
    /// Bundled locale name, such as `"en-US"`.
    Name(String),
    /// Custom d3-format locale object.
    Object(serde_json::Value),
}

impl FormatLocale {
    /// Resolve a bundled locale name to its JSON object, or clone a custom object.
    /// Returns an error if the bundled locale name is unknown.
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

/// Date/time-format locale, supplied as a bundled locale name or a d3-time-format object.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum TimeFormatLocale {
    /// Bundled locale name, such as `"en-US"`.
    Name(String),
    /// Custom d3-time-format locale object.
    Object(serde_json::Value),
}

impl TimeFormatLocale {
    /// Resolve a bundled locale name to its JSON object, or clone a custom object.
    /// Returns an error if the bundled locale name is unknown.
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

/// Renderer used by Vega Embed in exported HTML.
#[derive(Debug, Clone, Copy)]
pub enum Renderer {
    /// Vector SVG rendering.
    Svg,
    /// Raster Canvas rendering.
    Canvas,
    /// Combined SVG and Canvas rendering.
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

/// Options applied when compiling a Vega-Lite specification.
///
/// Unset theme and locale options inherit the converter defaults. Other `None`
/// fields leave the specification unchanged. Use [`Default`] to select the bundled
/// default Vega-Lite version.
#[derive(Debug, Clone, Default)]
pub struct VlOpts {
    /// Vega-Lite configuration passed to the compiler, overriding specification config.
    pub config: Option<serde_json::Value>,
    /// Built-in or configured theme name. `None` inherits the converter default.
    pub theme: Option<String>,
    /// Bundled Vega-Lite compiler version. Defaults to [`VlVersion::default()`].
    pub vl_version: VlVersion,
    /// Number-format locale. `None` inherits the converter default.
    pub format_locale: Option<FormatLocale>,
    /// Date/time-format locale. `None` inherits the converter default.
    pub time_format_locale: Option<TimeFormatLocale>,
    /// Additional Google Fonts to load for this conversion.
    /// Configured font requests still apply. `None` adds no per-call requests.
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
    pub(crate) fn to_embed_opts(&self, renderer: Renderer) -> Result<serde_json::Value, AnyError> {
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
    /// Inline external images and enabled Google Fonts in the SVG. Defaults to false.
    /// Local-font embedding is controlled separately by `VlcConfig::embed_local_fonts`.
    pub bundle: bool,
}

/// Options specific to HTML output format.
#[derive(Debug, Clone)]
pub struct HtmlOpts {
    /// Embed JavaScript dependencies and enabled Google Fonts in the HTML.
    /// Defaults to false, so the browser loads libraries from a CDN.
    /// Local-font embedding is controlled separately by `VlcConfig::embed_local_fonts`.
    pub bundle: bool,
    /// Browser renderer. Defaults to [`Renderer::Svg`].
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
    /// Pixel scale factor. `None` means 1.0. Multiplies the PPI scale as well.
    pub scale: Option<f32>,
    /// Pixels per inch. `None` means 72.0. Scales pixel dimensions by `ppi / 72`
    /// and records pixel density in PNG metadata for consumers that use physical size.
    pub ppi: Option<f32>,
}

/// Options specific to JPEG output format.
#[derive(Debug, Clone, Default)]
pub struct JpegOpts {
    /// Pixel scale factor. `None` means 1.0.
    pub scale: Option<f32>,
    /// JPEG quality from 1 to 100. `None` means 90.
    pub quality: Option<u8>,
}

/// Options specific to PDF output format.
#[derive(Debug, Clone, Default)]
pub struct PdfOpts {}

/// Options specific to URL output format.
#[derive(Debug, Clone, Default)]
pub struct UrlOpts {
    /// Open the Vega Editor in chart-only view. Defaults to false.
    pub fullscreen: bool,
}

/// Log level for entries captured during Vega/VL evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogLevel {
    /// An error reported by the JavaScript libraries.
    Error,
    /// A warning that did not prevent a result.
    Warn,
    /// An informational message.
    Info,
    /// A debug message.
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
    /// Severity reported by Vega or Vega-Lite.
    pub level: LogLevel,
    /// Diagnostic text captured during evaluation.
    pub message: String,
}

pub(crate) trait WithGoogleFonts {
    fn add_google_fonts(&mut self, usage: GoogleFontUsage);
}

/// Output from a Vega-Lite -> Vega compilation.
#[derive(Debug)]
pub struct VegaOutput {
    /// Compiled Vega specification.
    pub spec: serde_json::Value,
    /// Diagnostics captured while compiling the specification.
    pub logs: Vec<LogEntry>,
}

/// Output from an SVG conversion.
#[derive(Debug)]
pub struct SvgOutput {
    /// SVG document as UTF-8 text.
    pub svg: String,
    /// Diagnostics captured during conversion.
    pub logs: Vec<LogEntry>,
    /// Google Fonts downloads and resolved variants used by this conversion.
    pub google_fonts: GoogleFontUsage,
}

/// Output from a PNG conversion.
#[derive(Debug)]
pub struct PngOutput {
    /// PNG-encoded image bytes.
    pub data: Vec<u8>,
    /// Diagnostics captured during conversion.
    pub logs: Vec<LogEntry>,
    /// Google Fonts downloads and resolved variants used by this conversion.
    pub google_fonts: GoogleFontUsage,
}

/// Output from a JPEG conversion.
#[derive(Debug)]
pub struct JpegOutput {
    /// JPEG-encoded image bytes.
    pub data: Vec<u8>,
    /// Diagnostics captured during conversion.
    pub logs: Vec<LogEntry>,
    /// Google Fonts downloads and resolved variants used by this conversion.
    pub google_fonts: GoogleFontUsage,
}

/// Output from a PDF conversion.
#[derive(Debug)]
pub struct PdfOutput {
    /// PDF document bytes.
    pub data: Vec<u8>,
    /// Diagnostics captured during conversion.
    pub logs: Vec<LogEntry>,
    /// Google Fonts downloads and resolved variants used by this conversion.
    pub google_fonts: GoogleFontUsage,
}

/// Output from an HTML conversion.
#[derive(Debug)]
pub struct HtmlOutput {
    /// Complete HTML document as UTF-8 text.
    pub html: String,
    /// Diagnostics captured during export and font analysis.
    pub logs: Vec<LogEntry>,
    /// Google Fonts downloads and resolved variants used by this export.
    pub google_fonts: GoogleFontUsage,
}

/// Output from a scenegraph extraction.
#[derive(Debug)]
pub struct ScenegraphOutput {
    /// Evaluated scenegraph as a JSON value.
    pub scenegraph: serde_json::Value,
    /// Diagnostics captured during evaluation.
    pub logs: Vec<LogEntry>,
    /// Google Fonts downloads and resolved variants used by this conversion.
    pub google_fonts: GoogleFontUsage,
}

/// Output from a scenegraph msgpack extraction.
#[derive(Debug)]
pub struct ScenegraphMsgpackOutput {
    /// MessagePack-encoded evaluated scenegraph.
    pub data: Vec<u8>,
    /// Diagnostics captured during evaluation.
    pub logs: Vec<LogEntry>,
    /// Google Fonts downloads and resolved variants used by this conversion.
    pub google_fonts: GoogleFontUsage,
}

macro_rules! impl_with_google_fonts {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl WithGoogleFonts for $ty {
                fn add_google_fonts(&mut self, usage: GoogleFontUsage) {
                    self.google_fonts.add_assign(usage);
                }
            }
        )+
    };
}

impl_with_google_fonts!(
    SvgOutput,
    PngOutput,
    JpegOutput,
    PdfOutput,
    HtmlOutput,
    ScenegraphOutput,
    ScenegraphMsgpackOutput,
);

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

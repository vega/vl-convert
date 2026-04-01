#![allow(clippy::uninlined_format_args)]
#![doc = include_str!("../README.md")]

mod serve;

use clap::{Parser, Subcommand};
use itertools::Itertools;
use std::io::{self, IsTerminal, Read, Write};
use std::str::FromStr;
use vl_convert_google_fonts::{FontStyle, VariantRequest};
use vl_convert_rs::converter::{
    vega_to_url, vegalite_to_url, BaseUrlSetting, FormatLocale, GoogleFontRequest, HtmlOpts,
    JpegOpts, MissingFontsPolicy, PdfOpts, PngOpts, Renderer, SvgOpts, TimeFormatLocale, VgOpts,
    VlConverter, VlOpts, VlcConfig,
};
use vl_convert_rs::module_loader::import_map::VlVersion;
use vl_convert_rs::text::register_font_directory;
use vl_convert_rs::{anyhow, anyhow::bail};

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
enum MissingFontsArg {
    #[default]
    Fallback,
    Warn,
    Error,
}

impl MissingFontsArg {
    fn to_policy(self) -> MissingFontsPolicy {
        match self {
            MissingFontsArg::Fallback => MissingFontsPolicy::Fallback,
            MissingFontsArg::Warn => MissingFontsPolicy::Warn,
            MissingFontsArg::Error => MissingFontsPolicy::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
enum LogLevel {
    Error,
    #[default]
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    fn to_filter(self) -> log::LevelFilter {
        match self {
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
        }
    }

    fn to_tracing_filter(self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default, PartialEq, Eq)]
enum LogFormat {
    #[default]
    Text,
    Json,
    Datadog,
}

const DEFAULT_VL_VERSION: &str = "6.4";

#[derive(Debug, Parser)] // requires `derive` feature
#[command(version, name = "vl-convert")]
#[command(about = "vl-convert: A utility for converting Vega-Lite specifications", long_about = None)]
struct Cli {
    /// Path to JSONC converter config file.
    /// Defaults to the platform config directory if the file exists.
    #[arg(long, global = true)]
    vlc_config: Option<String>,

    /// Disable loading the default vlc-config file
    #[arg(long, global = true, conflicts_with = "vlc_config")]
    no_vlc_config: bool,

    /// Custom base URL for resolving relative data paths in Vega specs.
    /// Can be a URL (https://...) or a local filesystem path (/data/).
    #[arg(long, global = true)]
    base_url: Option<String>,

    /// Disable relative path resolution (relative data paths produce an error)
    #[arg(long, global = true, conflicts_with = "base_url")]
    no_base_url: bool,

    /// Allowed base URL pattern for external data access.
    /// Supports CSP-style patterns: "https:" (scheme), "https://example.com/" (prefix),
    /// "/data/" (filesystem). May be specified multiple times.
    #[arg(long = "allowed-base-url", global = true)]
    allowed_base_url: Vec<String>,

    /// Disable all external data access (empty allowlist)
    #[arg(long, global = true, conflicts_with = "allowed_base_url")]
    no_allowed_urls: bool,

    /// Register a font from Google Fonts. Use "Family" for all variants,
    /// or "Family:400,700italic" for specific weight/style combinations.
    /// May be specified multiple times.
    #[arg(long = "google-font", global = true)]
    google_font: Vec<String>,

    /// Automatically download missing fonts from Google Fonts.
    #[arg(long, global = true)]
    auto_google_fonts: bool,

    /// Embed locally installed fonts as base64 @font-face in HTML and SVG output.
    #[arg(long, global = true)]
    embed_local_fonts: bool,

    /// Disable font subsetting (embed full font files instead of only used characters)
    #[arg(long, global = true)]
    no_subset_fonts: bool,

    /// Missing-font behavior: fallback silently, warn, or error.
    #[arg(long, global = true, value_enum)]
    missing_fonts: Option<MissingFontsArg>,

    /// Maximum V8 heap size per worker in megabytes [default: 0 = no limit]
    #[arg(long, global = true)]
    max_v8_heap_size_mb: Option<usize>,

    /// Maximum V8 execution time in seconds [default: 0 = no limit]
    #[arg(long, global = true)]
    max_v8_execution_time_secs: Option<u64>,

    /// Run V8 garbage collection after each conversion to release memory
    #[arg(long, global = true)]
    gc_after_conversion: bool,

    /// Vega plugin: file path (.js/.mjs), URL (https://...), or inline ESM string.
    /// The plugin must be a single ESM module that exports a default function
    /// accepting a vega object. Multi-file plugins should be pre-bundled
    /// with esbuild or Rollup. URL plugins auto-allow their domain for imports.
    /// May be specified multiple times; plugins execute in order.
    #[arg(long = "vega-plugin", global = true)]
    vega_plugin: Vec<String>,

    /// Domains allowed for HTTP imports in plugins, comma-separated.
    /// Examples: '*' (any), 'esm.sh', '*.jsdelivr.net'.
    /// May be specified multiple times. Omit to disable HTTP imports.
    #[arg(long = "plugin-import-domains", global = true)]
    plugin_import_domains: Vec<String>,

    /// Log level for Vega/Vega-Lite messages
    #[arg(long, global = true, value_enum, default_value_t = LogLevel::Warn)]
    log_level: LogLevel,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Convert a Vega-Lite specification to a Vega specification
    Vl2vg {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output Vega file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(short, long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// Pretty-print JSON in output file
        #[arg(short, long)]
        pretty: bool,
    },

    /// Convert a Vega-Lite specification to an SVG image
    Vl2svg {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output SVG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Bundle fonts and images into a self-contained SVG
        #[arg(long)]
        bundle: bool,
    },

    /// Convert a Vega-Lite specification to an PNG image
    Vl2png {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PNG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// Pixels per inch
        #[arg(short, long, default_value = "72.0")]
        ppi: f32,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega-Lite specification to an JPEG image
    Vl2jpeg {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JPEG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// JPEG Quality between 0 (worst) and 100 (best)
        #[arg(short, long, default_value = "90")]
        quality: u8,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega-Lite specification to a PDF image
    Vl2pdf {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PDF file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega-Lite specification to a URL that opens the chart in the Vega editor
    Vl2url {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Open chart in fullscreen mode
        #[arg(long, default_value = "false")]
        fullscreen: bool,
    },

    /// Convert a Vega-Lite specification to an HTML file
    Vl2html {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output HTML file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// Whether to bundle JavaScript dependencies in the HTML file
        /// instead of loading them from a CDN
        #[arg(short, long)]
        bundle: bool,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Vega renderer. One of 'svg' (default), 'canvas', or 'hybrid'
        #[arg(long)]
        renderer: Option<String>,
    },

    /// Return font metadata for a rendered Vega-Lite specification
    Vl2fonts {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JSON file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// Include @font-face CSS blocks in the output
        #[arg(long = "include-font-face")]
        include_font_face: bool,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Pretty-print JSON output
        #[arg(short, long)]
        pretty: bool,
    },

    /// Convert a Vega specification to an SVG image
    Vg2svg {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output SVG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Bundle fonts and images into a self-contained SVG
        #[arg(long)]
        bundle: bool,
    },

    /// Convert a Vega specification to an PNG image
    Vg2png {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PNG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// Pixels per inch
        #[arg(short, long, default_value = "72.0")]
        ppi: f32,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega specification to an JPEG image
    Vg2jpeg {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JPEG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// JPEG Quality between 0 (worst) and 100 (best)
        #[arg(short, long, default_value = "90")]
        quality: u8,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega specification to an PDF image
    Vg2pdf {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PDF file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega specification to a URL that opens the chart in the Vega editor
    Vg2url {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Open chart in fullscreen mode
        #[arg(long, default_value = "false")]
        fullscreen: bool,
    },

    /// Convert a Vega specification to an HTML file
    Vg2html {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output HTML file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Whether to bundle JavaScript dependencies in the HTML file
        /// instead of loading them from a CDN
        #[arg(short, long)]
        bundle: bool,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Vega renderer. One of 'svg' (default), 'canvas', or 'hybrid'
        #[arg(long)]
        renderer: Option<String>,
    },

    /// Return font metadata for a rendered Vega specification
    Vg2fonts {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JSON file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Include @font-face CSS blocks in the output
        #[arg(long = "include-font-face")]
        include_font_face: bool,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Pretty-print JSON output
        #[arg(short, long)]
        pretty: bool,
    },

    /// Convert an SVG image to a PNG image
    Svg2png {
        /// Path to input SVG file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PNG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// Pixels per inch
        #[arg(short, long, default_value = "72.0")]
        ppi: f32,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,
    },

    /// Convert an SVG image to a JPEG image
    Svg2jpeg {
        /// Path to input SVG file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JPEG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// JPEG Quality between 0 (worst) and 100 (best)
        #[arg(short, long, default_value = "90")]
        quality: u8,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,
    },

    /// Convert an SVG image to a PDF image
    Svg2pdf {
        /// Path to input SVG file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PDF file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,
    },

    /// List available themes
    LsThemes,

    /// Print the config JSON for a theme
    #[command(arg_required_else_help = true)]
    CatTheme {
        /// Name of a theme
        theme: String,
    },

    /// Print the default vlc-config file path
    ConfigPath,

    /// Start an HTTP server for chart conversion
    Serve {
        /// Bind address [default: 127.0.0.1]
        #[arg(long, env = "VLC_HOST", default_value = "127.0.0.1")]
        host: String,

        /// Port [default: 3000]
        #[arg(long, env = "VLC_PORT", default_value_t = 3000)]
        port: u16,

        /// Number of converter worker threads [default: CPU count]
        #[arg(long, env = "VLC_WORKERS")]
        workers: Option<usize>,

        /// Additional directory to search for fonts
        #[arg(long, env = "VLC_FONT_DIR")]
        font_dir: Option<String>,

        /// API key for Bearer token authentication
        #[arg(long, env = "VLC_API_KEY")]
        api_key: Option<String>,

        /// Allowed CORS origin(s), comma-separated or "*".
        /// Default: localhost/127.0.0.1/[::1] on any port.
        /// Set to "" to disable CORS.
        #[arg(long, env = "VLC_CORS_ORIGIN")]
        cors_origin: Option<String>,

        /// Maximum simultaneous in-flight requests [default: unlimited]
        #[arg(long, env = "VLC_MAX_CONCURRENT_REQUESTS")]
        max_concurrent_requests: Option<usize>,

        /// HTTP request timeout in seconds [default: 30]
        #[arg(long, env = "VLC_REQUEST_TIMEOUT_SECS", default_value_t = 30)]
        request_timeout_secs: u64,

        /// Graceful shutdown drain timeout in seconds [default: 30]
        #[arg(long, env = "VLC_DRAIN_TIMEOUT_SECS", default_value_t = 30)]
        drain_timeout_secs: u64,

        /// Maximum request body size in megabytes [default: 50]
        #[arg(long, env = "VLC_MAX_BODY_SIZE_MB", default_value_t = 50)]
        max_body_size_mb: usize,

        /// Return only HTTP status codes on error (no error messages)
        #[arg(long, env = "VLC_OPAQUE_ERRORS")]
        opaque_errors: bool,

        /// Reject requests without a User-Agent header
        #[arg(long, env = "VLC_REQUIRE_USER_AGENT")]
        require_user_agent: bool,

        /// Log output format
        #[arg(long, env = "VLC_LOG_FORMAT", value_enum, default_value_t = LogFormat::Text)]
        log_format: LogFormat,

        /// Max requests per IP per second (disabled if not set)
        #[arg(long, env = "VLC_RATE_LIMIT_PER_SECOND")]
        rate_limit_per_second: Option<u64>,

        /// Burst allowance per IP [default: 5]
        #[arg(long, env = "VLC_RATE_LIMIT_BURST", default_value_t = 5)]
        rate_limit_burst: u32,
    },
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    // Serve uses tracing-subscriber (initialized in serve::run); other commands use env_logger
    if !matches!(cli.command, Commands::Serve { .. }) {
        env_logger::Builder::new()
            .filter_module("vl_convert", cli.log_level.to_filter())
            .init();
    }

    // Handle config-path before loading the config so it works even with a broken config file.
    if let Commands::ConfigPath = cli.command {
        println!("{}", vl_convert_rs::vlc_config_path().display());
        return Ok(());
    }

    let google_font_families = cli.google_font.clone();
    let plugin_import_domains = flatten_plugin_domains(&cli.plugin_import_domains);
    let vega_plugins = if cli.vega_plugin.is_empty() {
        None
    } else {
        Some(cli.vega_plugin.clone())
    };

    let mut base_config = resolve_vlc_config(cli.vlc_config.as_deref(), cli.no_vlc_config)?;

    if cli.base_url.is_some() || cli.no_base_url {
        let base_url_setting = if cli.no_base_url {
            BaseUrlSetting::Disabled
        } else if let Some(ref url) = cli.base_url {
            BaseUrlSetting::Custom(url.clone())
        } else {
            BaseUrlSetting::Default
        };
        base_config.base_url = base_url_setting;
    }
    if !cli.allowed_base_url.is_empty() || cli.no_allowed_urls {
        let allowed_base_urls = if cli.no_allowed_urls {
            Some(vec![])
        } else {
            Some(cli.allowed_base_url.clone())
        };
        base_config.allowed_base_urls = allowed_base_urls;
    }
    if cli.auto_google_fonts {
        base_config.auto_google_fonts = true;
    }
    if cli.embed_local_fonts {
        base_config.embed_local_fonts = true;
    }
    if cli.no_subset_fonts {
        base_config.subset_fonts = false;
    }
    if let Some(ref mf) = cli.missing_fonts {
        base_config.missing_fonts = mf.to_policy();
    }
    if let Some(heap) = cli.max_v8_heap_size_mb {
        base_config.max_v8_heap_size_mb = heap;
    }
    if let Some(timeout) = cli.max_v8_execution_time_secs {
        base_config.max_v8_execution_time_secs = timeout;
    }
    if cli.gc_after_conversion {
        base_config.gc_after_conversion = true;
    }
    if vega_plugins.is_some() {
        base_config.vega_plugins = vega_plugins;
    }
    if !plugin_import_domains.is_empty() {
        base_config.plugin_import_domains = plugin_import_domains;
    }
    let google_fonts = parse_google_font_requests(&google_font_families)?;
    if google_fonts.is_some() {
        base_config.google_fonts = google_fonts;
    }
    let command = cli.command;

    if !matches!(command, Commands::Serve { .. }) {
        base_config.num_workers = 1;
    }

    use crate::Commands::*;
    match command {
        Vl2vg {
            input: input_vegalite_file,
            output: output_vega_file,
            vl_version,
            theme,
            config,
            pretty,
        } => {
            vl_2_vg(
                input_vegalite_file.as_deref(),
                output_vega_file.as_deref(),
                &vl_version,
                theme,
                config,
                pretty,
                base_config,
            )
            .await?
        }
        Vl2svg {
            input,
            output,
            vl_version,
            theme,
            config,
            font_dir,
            format_locale,
            time_format_locale,
            bundle,
        } => {
            register_font_dir(font_dir)?;
            let svg_opts = SvgOpts { bundle };
            vl_2_svg(
                input.as_deref(),
                output.as_deref(),
                &vl_version,
                theme,
                config,
                format_locale,
                time_format_locale,
                svg_opts,
                base_config,
            )
            .await?
        }
        Vl2png {
            input,
            output,
            vl_version,
            theme,
            config,
            scale,
            ppi,
            font_dir,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vl_2_png(
                input.as_deref(),
                output.as_deref(),
                &vl_version,
                theme,
                config,
                scale,
                ppi,
                format_locale,
                time_format_locale,
                base_config,
            )
            .await?
        }
        Vl2jpeg {
            input,
            output,
            vl_version,
            theme,
            config,
            scale,
            quality,
            font_dir,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vl_2_jpeg(
                input.as_deref(),
                output.as_deref(),
                &vl_version,
                theme,
                config,
                scale,
                quality,
                format_locale,
                time_format_locale,
                base_config,
            )
            .await?
        }
        Vl2pdf {
            input,
            output,
            vl_version,
            theme,
            config,
            font_dir,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vl_2_pdf(
                input.as_deref(),
                output.as_deref(),
                &vl_version,
                theme,
                config,
                format_locale,
                time_format_locale,
                base_config,
            )
            .await?
        }
        Vl2url {
            input,
            output,
            fullscreen,
        } => {
            let vl_str = read_input_string(input.as_deref())?;
            let vl_spec = serde_json::from_str(&vl_str)?;
            let url = vegalite_to_url(&vl_spec, fullscreen)?;
            write_output_string(output.as_deref(), &url)?
        }
        Vl2html {
            input,
            output,
            vl_version,
            theme,
            config,
            bundle,
            format_locale,
            time_format_locale,
            renderer,
        } => {
            let google_fonts = parse_google_font_requests(&google_font_families)?;
            let vl_str = read_input_string(input.as_deref())?;
            let vl_spec: serde_json::Value = serde_json::from_str(&vl_str)?;
            let config = read_config_json(config)?;
            let vl_version = parse_vl_version(&vl_version)?;
            let format_locale = parse_format_locale_option(format_locale.as_deref())?;
            let time_format_locale =
                parse_time_format_locale_option(time_format_locale.as_deref())?;
            let renderer = renderer.unwrap_or_else(|| "svg".to_string());

            let converter = VlConverter::with_config(base_config)?;
            let html = converter
                .vegalite_to_html(
                    vl_spec,
                    VlOpts {
                        config,
                        theme,
                        vl_version,
                        format_locale,
                        time_format_locale,
                        google_fonts,
                        ..Default::default()
                    },
                    HtmlOpts {
                        bundle,
                        renderer: Renderer::from_str(&renderer)?,
                    },
                )
                .await?;
            write_output_string(output.as_deref(), &html)?;
        }
        Vl2fonts {
            input,
            output,
            vl_version,
            theme,
            config,
            include_font_face,
            format_locale,
            time_format_locale,
            pretty,
        } => {
            let google_fonts = parse_google_font_requests(&google_font_families)?;
            let vl_str = read_input_string(input.as_deref())?;
            let vl_spec: serde_json::Value = serde_json::from_str(&vl_str)?;
            let config = read_config_json(config)?;
            let vl_version = parse_vl_version(&vl_version)?;
            let format_locale = parse_format_locale_option(format_locale.as_deref())?;
            let time_format_locale =
                parse_time_format_locale_option(time_format_locale.as_deref())?;

            let auto_google_fonts = base_config.auto_google_fonts;
            let embed_local_fonts = base_config.embed_local_fonts;
            let subset_fonts = base_config.subset_fonts;
            let converter = VlConverter::with_config(base_config)?;
            let fonts = converter
                .vegalite_fonts(
                    vl_spec,
                    VlOpts {
                        config,
                        theme,
                        vl_version,
                        format_locale,
                        time_format_locale,
                        google_fonts,
                        ..Default::default()
                    },
                    auto_google_fonts,
                    embed_local_fonts,
                    include_font_face,
                    subset_fonts,
                )
                .await?;
            let json = if pretty {
                serde_json::to_string_pretty(&fonts)?
            } else {
                serde_json::to_string(&fonts)?
            };
            write_output_string(output.as_deref(), &json)?;
        }
        Vg2svg {
            input,
            output,
            font_dir,
            format_locale,
            bundle,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            let svg_opts = SvgOpts { bundle };
            vg_2_svg(
                input.as_deref(),
                output.as_deref(),
                format_locale,
                time_format_locale,
                svg_opts,
                base_config,
            )
            .await?
        }
        Vg2png {
            input,
            output,
            scale,
            ppi,
            font_dir,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vg_2_png(
                input.as_deref(),
                output.as_deref(),
                scale,
                ppi,
                format_locale,
                time_format_locale,
                base_config,
            )
            .await?
        }
        Vg2jpeg {
            input,
            output,
            scale,
            quality,
            font_dir,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vg_2_jpeg(
                input.as_deref(),
                output.as_deref(),
                scale,
                quality,
                format_locale,
                time_format_locale,
                base_config,
            )
            .await?
        }
        Vg2pdf {
            input,
            output,
            font_dir,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vg_2_pdf(
                input.as_deref(),
                output.as_deref(),
                format_locale,
                time_format_locale,
                base_config,
            )
            .await?
        }
        Vg2url {
            input,
            output,
            fullscreen,
        } => {
            let vg_str = read_input_string(input.as_deref())?;
            let vg_spec = serde_json::from_str(&vg_str)?;
            let url = vega_to_url(&vg_spec, fullscreen)?;
            write_output_string(output.as_deref(), &url)?
        }
        Vg2html {
            input,
            output,
            bundle,
            format_locale,
            time_format_locale,
            renderer,
        } => {
            let google_fonts = parse_google_font_requests(&google_font_families)?;
            let vg_str = read_input_string(input.as_deref())?;
            let vg_spec: serde_json::Value = serde_json::from_str(&vg_str)?;

            let format_locale = parse_format_locale_option(format_locale.as_deref())?;
            let time_format_locale =
                parse_time_format_locale_option(time_format_locale.as_deref())?;

            let renderer = renderer.unwrap_or_else(|| "svg".to_string());

            let converter = VlConverter::with_config(base_config)?;
            let html = converter
                .vega_to_html(
                    vg_spec,
                    VgOpts {
                        format_locale,
                        time_format_locale,
                        google_fonts,
                        ..Default::default()
                    },
                    HtmlOpts {
                        bundle,
                        renderer: Renderer::from_str(&renderer)?,
                    },
                )
                .await?;
            write_output_string(output.as_deref(), &html)?;
        }
        Vg2fonts {
            input,
            output,
            include_font_face,
            format_locale,
            time_format_locale,
            pretty,
        } => {
            let google_fonts = parse_google_font_requests(&google_font_families)?;
            let vg_str = read_input_string(input.as_deref())?;
            let vg_spec: serde_json::Value = serde_json::from_str(&vg_str)?;
            let format_locale = parse_format_locale_option(format_locale.as_deref())?;
            let time_format_locale =
                parse_time_format_locale_option(time_format_locale.as_deref())?;

            let auto_google_fonts = base_config.auto_google_fonts;
            let embed_local_fonts = base_config.embed_local_fonts;
            let subset_fonts = base_config.subset_fonts;
            let converter = VlConverter::with_config(base_config)?;
            let fonts = converter
                .vega_fonts(
                    vg_spec,
                    VgOpts {
                        google_fonts,
                        format_locale,
                        time_format_locale,
                        ..Default::default()
                    },
                    auto_google_fonts,
                    embed_local_fonts,
                    include_font_face,
                    subset_fonts,
                )
                .await?;
            let json = if pretty {
                serde_json::to_string_pretty(&fonts)?
            } else {
                serde_json::to_string(&fonts)?
            };
            write_output_string(output.as_deref(), &json)?;
        }
        Svg2png {
            input,
            output,
            scale,
            ppi,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            let svg = read_input_string(input.as_deref())?;
            let converter = VlConverter::with_config(base_config)?;
            let png_data = converter
                .svg_to_png(
                    &svg,
                    PngOpts {
                        scale: Some(scale),
                        ppi: Some(ppi),
                    },
                )
                .await?;
            write_output_binary(output.as_deref(), &png_data, "PNG")?;
        }
        Svg2jpeg {
            input,
            output,
            scale,
            quality,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            let svg = read_input_string(input.as_deref())?;
            let converter = VlConverter::with_config(base_config)?;
            let jpeg_data = converter
                .svg_to_jpeg(
                    &svg,
                    JpegOpts {
                        scale: Some(scale),
                        quality: Some(quality),
                    },
                )
                .await?;
            write_output_binary(output.as_deref(), &jpeg_data, "JPEG")?;
        }
        Svg2pdf {
            input,
            output,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            let svg = read_input_string(input.as_deref())?;
            let converter = VlConverter::with_config(base_config)?;
            let pdf_data = converter.svg_to_pdf(&svg, PdfOpts::default()).await?;
            write_output_binary(output.as_deref(), &pdf_data, "PDF")?;
        }
        LsThemes => list_themes(base_config).await?,
        CatTheme { theme } => cat_theme(&theme, base_config).await?,
        ConfigPath => unreachable!("handled before config loading"),
        Serve {
            host,
            port,
            workers,
            font_dir,
            api_key,
            cors_origin,
            max_concurrent_requests,
            request_timeout_secs,
            drain_timeout_secs,
            max_body_size_mb,
            opaque_errors,
            require_user_agent,
            log_format,
            rate_limit_per_second,
            rate_limit_burst,
        } => {
            register_font_dir(font_dir)?;

            serve::init_tracing(cli.log_level.to_tracing_filter(), log_format);

            // Resolve worker count: CLI flag > vlc-config > CPU count
            let num_workers = workers
                .or(if base_config.num_workers > 0 {
                    Some(base_config.num_workers)
                } else {
                    None
                })
                .unwrap_or_else(|| {
                    std::thread::available_parallelism()
                        .map(|n| n.get())
                        .unwrap_or(1)
                });
            base_config.num_workers = num_workers;

            // Clamp V8 timeout to HTTP timeout
            if request_timeout_secs > 0 {
                let current = base_config.max_v8_execution_time_secs;
                if current == 0 || current > request_timeout_secs {
                    base_config.max_v8_execution_time_secs = request_timeout_secs;
                }
            }

            let serve_config = serve::ServeConfig {
                host,
                port,
                api_key,
                cors_origin,
                max_concurrent_requests,
                request_timeout_secs,
                drain_timeout_secs,
                max_body_size_mb,
                opaque_errors,
                require_user_agent,
                log_format,
                rate_limit_per_second,
                rate_limit_burst,
            };

            serve::run(base_config, serve_config).await?
        }
    }

    Ok(())
}

fn register_font_dir(dir: Option<String>) -> Result<(), anyhow::Error> {
    if let Some(dir) = dir {
        register_font_directory(&dir)?
    }
    Ok(())
}

/// Parse a `--google-font` value like `"Roboto"` or `"Roboto:400,700italic"`
/// into a family name and optional variant list.
fn parse_google_font_arg(s: &str) -> Result<(String, Option<Vec<VariantRequest>>), anyhow::Error> {
    let Some((family, variants_str)) = s.split_once(':') else {
        return Ok((s.to_string(), None));
    };
    let mut variants = Vec::new();
    for token in variants_str.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let (weight_str, style) = if let Some(w) = token.strip_suffix("italic") {
            (w, FontStyle::Italic)
        } else {
            (token, FontStyle::Normal)
        };
        let weight: u16 = weight_str.parse().map_err(|_| {
            anyhow::anyhow!(
                "Invalid font variant '{token}' in --google-font '{s}'. \
                 Expected format: 400, 700italic, etc."
            )
        })?;
        variants.push(VariantRequest { weight, style });
    }
    if variants.is_empty() {
        Ok((family.to_string(), None))
    } else {
        Ok((family.to_string(), Some(variants)))
    }
}

/// Parse `--google-font` args into `GoogleFontRequest`s for per-call opts.
fn parse_google_font_requests(
    fonts: &[String],
) -> Result<Option<Vec<GoogleFontRequest>>, anyhow::Error> {
    if fonts.is_empty() {
        return Ok(None);
    }
    let mut requests = Vec::new();
    for spec in fonts {
        let (family, variants) = parse_google_font_arg(spec)?;
        requests.push(GoogleFontRequest { family, variants });
    }
    Ok(Some(requests))
}

fn flatten_plugin_domains(raw: &[String]) -> Vec<String> {
    raw.iter()
        .flat_map(|s| s.split(','))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_vl_version(vl_version: &str) -> Result<VlVersion, anyhow::Error> {
    VlVersion::from_str(vl_version)
        .map_err(|_| anyhow::anyhow!("Invalid or unsupported Vega-Lite version: {vl_version}"))
}

fn read_input_string(input: Option<&str>) -> Result<String, anyhow::Error> {
    match input {
        Some(path) if path != "-" => std::fs::read_to_string(path)
            .map_err(|err| anyhow::anyhow!("Failed to read input file: {}\n{}", path, err)),
        _ => {
            // Check if stdin is an interactive terminal
            if io::stdin().is_terminal() {
                eprintln!("Reading from stdin... (Press Ctrl-D when done, or use -i <file>)");
            }

            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .map_err(|err| anyhow::anyhow!("Failed to read from stdin: {}", err))?;

            // Check for empty or whitespace-only input
            if buffer.trim().is_empty() {
                bail!("No input provided. Provide a specification via stdin or use -i <file>");
            }

            Ok(buffer)
        }
    }
}

/// Read a file that is always a filesystem path (never stdin).
///
/// This function is used for reading configuration files (locale, time format)
/// that should not come from stdin. For reading input specifications that may
/// come from stdin or a file, use `read_input_string()` instead.
fn read_file_string(path: &str) -> Result<String, anyhow::Error> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(err) => {
            bail!("Failed to read file: {}\n{}", path, err);
        }
    }
}

fn parse_as_json(input_str: &str) -> Result<serde_json::Value, anyhow::Error> {
    match serde_json::from_str::<serde_json::Value>(input_str) {
        Ok(input_json) => Ok(input_json),
        Err(err) => {
            bail!("Failed to parse input file as JSON: {}", err);
        }
    }
}

fn format_locale_from_str(s: &str) -> Result<FormatLocale, anyhow::Error> {
    if s.ends_with(".json") {
        let s = read_file_string(s)?;
        Ok(FormatLocale::Object(parse_as_json(&s)?))
    } else {
        Ok(FormatLocale::Name(s.to_string()))
    }
}

fn parse_format_locale_option(
    format_locale: Option<&str>,
) -> Result<Option<FormatLocale>, anyhow::Error> {
    format_locale.map(format_locale_from_str).transpose()
}

fn time_format_locale_from_str(s: &str) -> Result<TimeFormatLocale, anyhow::Error> {
    if s.ends_with(".json") {
        let s = read_file_string(s)?;
        Ok(TimeFormatLocale::Object(parse_as_json(&s)?))
    } else {
        Ok(TimeFormatLocale::Name(s.to_string()))
    }
}

fn parse_time_format_locale_option(
    time_format_locale: Option<&str>,
) -> Result<Option<TimeFormatLocale>, anyhow::Error> {
    time_format_locale
        .map(time_format_locale_from_str)
        .transpose()
}

fn write_output_string(output: Option<&str>, output_str: &str) -> Result<(), anyhow::Error> {
    match output {
        Some(path) if path != "-" => {
            // File output: write as-is without modification
            std::fs::write(path, output_str)
                .map_err(|err| anyhow::anyhow!("Failed to write output to {}\n{}", path, err))
        }
        _ => {
            // Stdout output: ensure trailing newline and handle BrokenPipe
            let stdout = io::stdout();
            let mut handle = stdout.lock();

            // Write the string
            if let Err(err) = handle.write_all(output_str.as_bytes()) {
                if err.kind() == io::ErrorKind::BrokenPipe {
                    std::process::exit(0);
                }
                return Err(anyhow::anyhow!("Failed to write to stdout: {}", err));
            }

            // Add trailing newline if not already present
            if !output_str.ends_with('\n') {
                if let Err(err) = handle.write_all(b"\n") {
                    if err.kind() == io::ErrorKind::BrokenPipe {
                        std::process::exit(0);
                    }
                    return Err(anyhow::anyhow!(
                        "Failed to write newline to stdout: {}",
                        err
                    ));
                }
            }

            // Flush
            if let Err(err) = handle.flush() {
                if err.kind() == io::ErrorKind::BrokenPipe {
                    std::process::exit(0);
                }
                return Err(anyhow::anyhow!("Failed to flush stdout: {}", err));
            }

            Ok(())
        }
    }
}

/// Write binary output data to a file or stdout with TTY safety guard.
///
/// # Behavior
/// - `output = Some(path)` where `path != "-"`: Write to file
/// - `output = Some("-")`: Force write to stdout (user override)
/// - `output = None`: Write to stdout only if not a TTY (safety guard)
///
/// # TTY Safety Guard
/// When `output = None` and stdout is a terminal, this function refuses to write
/// binary data to prevent terminal corruption. Users must either:
/// - Redirect to a file: `vl-convert vl2png -o output.png`
/// - Pipe to another command: `vl-convert vl2png | display`
/// - Force stdout: `vl-convert vl2png -o -`
///
/// # Testing Note
/// The TTY safety guard is tested manually because automated tests run with
/// piped stdout (not a TTY). To verify:
/// ```bash
/// # Should refuse (interactive terminal)
/// $ echo '{"$schema": "..."}' | vl-convert vl2png
///
/// # Should succeed (explicit override)
/// $ echo '{"$schema": "..."}' | vl-convert vl2png -o -
///
/// # Should succeed (piped)
/// $ echo '{"$schema": "..."}' | vl-convert vl2png | cat > test.png
/// ```
fn write_output_binary(
    output: Option<&str>,
    output_data: &[u8],
    format_name: &str,
) -> Result<(), anyhow::Error> {
    match output {
        Some(path) if path != "-" => std::fs::write(path, output_data)
            .map_err(|err| anyhow::anyhow!("Failed to write output to {}\n{}", path, err)),
        Some(_) => {
            // Explicit "-": write to stdout unconditionally (user override)
            write_stdout_bytes(output_data)
        }
        None => {
            // Implicit stdout: TTY safety guard
            if io::stdout().is_terminal() {
                bail!(
                    "Refusing to write binary {} data to terminal.\n\
                     Use -o <file> to write to a file, or pipe to another command.\n\
                     Use -o - to force output to stdout.",
                    format_name
                );
            }
            write_stdout_bytes(output_data)
        }
    }
}

/// Set stdout to binary mode on Windows to prevent newline translation.
///
/// On Windows, stdout defaults to "text mode" which translates `\n` (0x0A) to `\r\n` (0x0D 0x0A)
/// and treats `\x1A` (Ctrl-Z) as EOF. This corrupts binary data like PNG, JPEG, and PDF files.
///
/// This function uses the Windows C runtime `_setmode` function to switch stdout to binary mode.
/// On Unix systems (Linux, macOS), this is a no-op because stdout is always binary.
///
/// # References
/// - [Microsoft _setmode Documentation](https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/setmode)
///
/// # Safety
/// Uses unsafe FFI to call the Windows CRT function `_setmode`.
#[cfg(target_family = "windows")]
fn set_stdout_binary_mode() -> Result<(), anyhow::Error> {
    extern "C" {
        fn _setmode(fd: i32, mode: i32) -> i32;
    }
    const STDOUT_FILENO: i32 = 1;
    const O_BINARY: i32 = 0x8000;
    unsafe {
        let result = _setmode(STDOUT_FILENO, O_BINARY);
        if result == -1 {
            Err(anyhow::anyhow!("Failed to set binary mode on stdout"))
        } else {
            Ok(())
        }
    }
}

/// No-op on Unix systems where stdout is always binary.
#[cfg(not(target_family = "windows"))]
fn set_stdout_binary_mode() -> Result<(), anyhow::Error> {
    Ok(())
}

fn write_stdout_bytes(data: &[u8]) -> Result<(), anyhow::Error> {
    // Set stdout to binary mode on Windows before writing
    set_stdout_binary_mode()?;

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Write data, handling BrokenPipe as clean exit
    if let Err(err) = handle.write_all(data) {
        if err.kind() == io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(anyhow::anyhow!("Failed to write to stdout: {}", err));
    }

    // Flush, handling BrokenPipe as clean exit
    if let Err(err) = handle.flush() {
        if err.kind() == io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(anyhow::anyhow!("Failed to flush stdout: {}", err));
    }

    Ok(())
}

fn resolve_vlc_config(
    vlc_config: Option<&str>,
    no_vlc_config: bool,
) -> Result<VlcConfig, anyhow::Error> {
    if no_vlc_config {
        return Ok(VlcConfig::default());
    }
    let path = match vlc_config {
        Some(p) => {
            let expanded = shellexpand::tilde(p.trim()).to_string();
            std::path::PathBuf::from(expanded)
        }
        None => {
            let default = vl_convert_rs::vlc_config_path();
            if !default.exists() {
                return Ok(VlcConfig::default());
            }
            default
        }
    };
    VlcConfig::from_file(&path)
}

fn normalize_config_path(config: Option<String>) -> Option<String> {
    config.map(|c| shellexpand::tilde(c.trim()).to_string())
}

fn read_config_json(config: Option<String>) -> Result<Option<serde_json::Value>, anyhow::Error> {
    let config = normalize_config_path(config);
    match config {
        None => Ok(None),
        Some(config) => {
            let config_str = match std::fs::read_to_string(&config) {
                Ok(config_str) => config_str,
                Err(err) => {
                    bail!("Failed to read config file: {}\n{}", config, err);
                }
            };
            let config_json: serde_json::Value = serde_json::from_str(&config_str)?;
            Ok(Some(config_json))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_vg(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    pretty: bool,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vl_version = parse_vl_version(vl_version)?;
    let vegalite_str = read_input_string(input)?;
    let vegalite_json = parse_as_json(&vegalite_str)?;
    let config = read_config_json(config)?;

    let converter = VlConverter::with_config(converter_config)?;

    let vega_json = match converter
        .vegalite_to_vega(
            vegalite_json,
            VlOpts {
                vl_version,
                theme,
                config,
                ..Default::default()
            },
        )
        .await
    {
        Ok(vega_str) => vega_str,
        Err(err) => {
            bail!("Vega-Lite to Vega conversion failed: {}", err);
        }
    };
    let vega_str_res = if pretty {
        serde_json::to_string_pretty(&vega_json)
    } else {
        serde_json::to_string(&vega_json)
    };
    match vega_str_res {
        Ok(vega_str) => {
            write_output_string(output, &vega_str)?;
        }
        Err(err) => {
            bail!("Failed to serialize Vega spec to JSON string: {err}")
        }
    }

    Ok(())
}

async fn vg_2_svg(
    input: Option<&str>,
    output: Option<&str>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    svg_opts: SvgOpts,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vega_str = read_input_string(input)?;
    let vg_spec = parse_as_json(&vega_str)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let svg = match converter
        .vega_to_svg(
            vg_spec,
            VgOpts {
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            svg_opts,
        )
        .await
    {
        Ok(svg) => svg,
        Err(err) => {
            bail!("Vega to SVG conversion failed: {}", err);
        }
    };

    write_output_string(output, &svg)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vg_2_png(
    input: Option<&str>,
    output: Option<&str>,
    scale: f32,
    ppi: f32,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vega_str = read_input_string(input)?;
    let vg_spec = parse_as_json(&vega_str)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let png_data = match converter
        .vega_to_png(
            vg_spec,
            VgOpts {
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            PngOpts {
                scale: Some(scale),
                ppi: Some(ppi),
            },
        )
        .await
    {
        Ok(png_data) => png_data,
        Err(err) => {
            bail!("Vega to PNG conversion failed: {}", err);
        }
    };

    write_output_binary(output, &png_data, "PNG")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vg_2_jpeg(
    input: Option<&str>,
    output: Option<&str>,
    scale: f32,
    quality: u8,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vega_str = read_input_string(input)?;
    let vg_spec = parse_as_json(&vega_str)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let jpeg_data = match converter
        .vega_to_jpeg(
            vg_spec,
            VgOpts {
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            JpegOpts {
                scale: Some(scale),
                quality: Some(quality),
            },
        )
        .await
    {
        Ok(jpeg_data) => jpeg_data,
        Err(err) => {
            bail!("Vega to JPEG conversion failed: {}", err);
        }
    };

    write_output_binary(output, &jpeg_data, "JPEG")?;

    Ok(())
}

async fn vg_2_pdf(
    input: Option<&str>,
    output: Option<&str>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vega_str = read_input_string(input)?;
    let vg_spec = parse_as_json(&vega_str)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let pdf_data = match converter
        .vega_to_pdf(
            vg_spec,
            VgOpts {
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            PdfOpts::default(),
        )
        .await
    {
        Ok(pdf_data) => pdf_data,
        Err(err) => {
            bail!("Vega to PDF conversion failed: {}", err);
        }
    };

    write_output_binary(output, &pdf_data, "PDF")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_svg(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    svg_opts: SvgOpts,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vl_version = parse_vl_version(vl_version)?;
    let vegalite_str = read_input_string(input)?;
    let vl_spec = parse_as_json(&vegalite_str)?;
    let config = read_config_json(config)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let svg = match converter
        .vegalite_to_svg(
            vl_spec,
            VlOpts {
                vl_version,
                config,
                theme,
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            svg_opts,
        )
        .await
    {
        Ok(svg) => svg,
        Err(err) => {
            bail!("Vega-Lite to SVG conversion failed: {}", err);
        }
    };

    write_output_string(output, &svg)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_png(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    scale: f32,
    ppi: f32,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vl_version = parse_vl_version(vl_version)?;
    let vegalite_str = read_input_string(input)?;
    let vl_spec = parse_as_json(&vegalite_str)?;
    let config = read_config_json(config)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let png_data = match converter
        .vegalite_to_png(
            vl_spec,
            VlOpts {
                vl_version,
                config,
                theme,
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            PngOpts {
                scale: Some(scale),
                ppi: Some(ppi),
            },
        )
        .await
    {
        Ok(png_data) => png_data,
        Err(err) => {
            bail!("Vega-Lite to PNG conversion failed: {}", err);
        }
    };

    write_output_binary(output, &png_data, "PNG")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_jpeg(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    scale: f32,
    quality: u8,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vl_version = parse_vl_version(vl_version)?;
    let vegalite_str = read_input_string(input)?;
    let vl_spec = parse_as_json(&vegalite_str)?;
    let config = read_config_json(config)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let jpeg_data = match converter
        .vegalite_to_jpeg(
            vl_spec,
            VlOpts {
                vl_version,
                config,
                theme,
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            JpegOpts {
                scale: Some(scale),
                quality: Some(quality),
            },
        )
        .await
    {
        Ok(jpeg_data) => jpeg_data,
        Err(err) => {
            bail!("Vega-Lite to JPEG conversion failed: {}", err);
        }
    };

    write_output_binary(output, &jpeg_data, "JPEG")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_pdf(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vl_version = parse_vl_version(vl_version)?;
    let vegalite_str = read_input_string(input)?;
    let vl_spec = parse_as_json(&vegalite_str)?;
    let config = read_config_json(config)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let pdf_data = match converter
        .vegalite_to_pdf(
            vl_spec,
            VlOpts {
                vl_version,
                config,
                theme,
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            PdfOpts::default(),
        )
        .await
    {
        Ok(pdf_data) => pdf_data,
        Err(err) => {
            bail!("Vega-Lite to PDF conversion failed: {}", err);
        }
    };

    write_output_binary(output, &pdf_data, "PDF")?;

    Ok(())
}

async fn list_themes(config: VlcConfig) -> Result<(), anyhow::Error> {
    let converter = VlConverter::with_config(config)?;

    if let serde_json::Value::Object(themes) = converter.get_themes().await? {
        for theme in themes.keys().sorted() {
            println!("{}", theme)
        }
    } else {
        bail!("Failed to load themes")
    }

    Ok(())
}

async fn cat_theme(theme: &str, config: VlcConfig) -> Result<(), anyhow::Error> {
    let converter = VlConverter::with_config(config)?;

    if let serde_json::Value::Object(themes) = converter.get_themes().await? {
        if let Some(theme_config) = themes.get(theme) {
            let theme_config_str = serde_json::to_string_pretty(theme_config).unwrap();
            println!("{}", theme_config_str);
        } else {
            bail!("No theme named '{}'", theme)
        }
    } else {
        bail!("Failed to load themes")
    }
    Ok(())
}

use clap::Parser;
use vl_convert_rs::converter::MissingFontsPolicy;
pub(crate) use vl_convert_rs::DEFAULT_VL_VERSION;

use crate::commands::Commands;
use crate::io_utils::parse_boolish_arg;

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
pub(crate) enum MissingFontsArg {
    #[default]
    Fallback,
    Warn,
    Error,
}

impl MissingFontsArg {
    pub(crate) fn to_policy(self) -> MissingFontsPolicy {
        match self {
            MissingFontsArg::Fallback => MissingFontsPolicy::Fallback,
            MissingFontsArg::Warn => MissingFontsPolicy::Warn,
            MissingFontsArg::Error => MissingFontsPolicy::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
pub(crate) enum LogLevel {
    Error,
    #[default]
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    pub(crate) fn to_filter(self) -> log::LevelFilter {
        match self {
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
        }
    }
}

#[derive(Debug, Parser)]
#[command(version, name = "vl-convert")]
#[command(about = "vl-convert: A utility for converting Vega-Lite specifications", long_about = None)]
pub(crate) struct Cli {
    /// Converter config: an absolute path to a JSONC config file, or the
    /// reserved value `disabled` to skip config-file loading. When
    /// omitted, the platform default config path is loaded if it exists.
    #[arg(long, global = true, value_name = "disabled|PATH")]
    pub(crate) vlc_config: Option<String>,

    /// Base URL for resolving relative data paths. Reserved values:
    /// `default` (use vega-datasets CDN), `disabled` (relative paths
    /// error). Any other string is taken as a custom URL or filesystem
    /// path.
    #[arg(long, global = true)]
    pub(crate) base_url: Option<String>,

    /// Allowed base URLs. Reserved values: `default` (HTTP/HTTPS,
    /// library default), `none` (block all), `all` (allow everything
    /// incl. filesystem). Otherwise a JSON array literal of CSP-style
    /// patterns — `"https:"` (scheme), `"https://example.com/"` (prefix),
    /// `"/data/"` (absolute filesystem path) — or `@<path>` to read the
    /// JSON from a file.
    #[arg(long, global = true, value_name = "default|none|all|JSON|@FILE")]
    pub(crate) allowed_base_urls: Option<String>,

    /// Register a font from Google Fonts. Use "Family" for all variants,
    /// or "Family:400,700italic" for specific weight/style combinations.
    /// May be specified multiple times.
    #[arg(long = "google-font", global = true)]
    pub(crate) google_font: Vec<String>,

    /// Automatically download missing fonts from Google Fonts (default: false).
    #[arg(
        long,
        global = true,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
    )]
    pub(crate) auto_google_fonts: Option<bool>,

    /// Embed locally installed fonts as base64 @font-face in HTML and SVG output (default: false).
    #[arg(
        long,
        global = true,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
    )]
    pub(crate) embed_local_fonts: Option<bool>,

    /// Subset embedded fonts to only the characters used (default: true).
    /// Pass `=false` to embed full font files.
    #[arg(
        long,
        global = true,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
    )]
    pub(crate) subset_fonts: Option<bool>,

    /// Missing-font behavior: fallback silently, warn, or error.
    #[arg(long, global = true, value_enum)]
    pub(crate) missing_fonts: Option<MissingFontsArg>,

    /// Maximum V8 heap size per worker in megabytes [default: 0 = no limit]
    #[arg(long, global = true)]
    pub(crate) max_v8_heap_size_mb: Option<u64>,

    /// Maximum V8 execution time in seconds [default: 0 = no limit]
    #[arg(long, global = true)]
    pub(crate) max_v8_execution_time_secs: Option<u64>,

    /// Run V8 garbage collection after each conversion to release memory (default: false).
    #[arg(
        long,
        global = true,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
    )]
    pub(crate) gc_after_conversion: Option<bool>,

    /// Vega plugin: file path (.js/.mjs), URL (https://...), or inline ESM string.
    /// The plugin must be a single ESM module that exports a default function
    /// accepting a vega object. Multi-file plugins should be pre-bundled
    /// with esbuild or Rollup. URL plugins auto-allow their domain for imports.
    /// May be specified multiple times; plugins execute in order.
    #[arg(long = "vega-plugin", global = true)]
    pub(crate) vega_plugin: Vec<String>,

    /// Domains allowed for HTTP imports in plugins, comma-separated.
    /// Examples: '*' (any), 'esm.sh', '*.jsdelivr.net'.
    /// May be specified multiple times. Omit to disable HTTP imports.
    #[arg(long = "plugin-import-domains", global = true)]
    pub(crate) plugin_import_domains: Vec<String>,

    /// Log level for Vega/Vega-Lite messages
    #[arg(long, global = true, value_enum, default_value_t = LogLevel::Warn)]
    pub(crate) log_level: LogLevel,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

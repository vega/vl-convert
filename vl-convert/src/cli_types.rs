use std::path::PathBuf;

use clap::Parser;
use vl_convert_rs::converter::MissingFontsPolicy;
pub(crate) use vl_convert_rs::DEFAULT_VL_VERSION;
pub(crate) use vl_convert_server::LogFormat;

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
    pub(crate) fn as_directive_str(self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
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
    /// error). Otherwise either a URL with scheme (`https://...`,
    /// `file://...`) or an absolute filesystem path. Relative paths
    /// are rejected.
    #[arg(long, global = true, value_name = "default|disabled|URL|PATH")]
    pub(crate) base_url: Option<String>,

    /// Allowed base URLs. Reserved values: `none` (block all),
    /// `net` (HTTP/HTTPS only, no filesystem), `all` (allow everything
    /// incl. filesystem). Otherwise a JSON array literal of CSP-style
    /// patterns: `"https:"` (scheme), `"https://example.com/"` (prefix),
    /// `"/data/"` (absolute filesystem path); or `@<path>` to read the
    /// JSON from a file.
    #[arg(long, global = true, value_name = "none|net|all|JSON|@FILE")]
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

    /// Additional directory to search for fonts. Repeatable: pass the
    /// flag multiple times (`--font-dir /a --font-dir /b`) to register
    /// multiple directories. Calls
    /// `vl_convert_rs::set_font_directories` once at startup with the
    /// combined list (replace semantics).
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) font_dir: Vec<PathBuf>,

    /// Capacity (MB) of the on-disk Google Fonts LRU cache. `0` resolves
    /// to the library default (`Option<NonZeroU64>::None`).
    #[arg(long, global = true, value_name = "MB")]
    pub(crate) google_fonts_cache_size_mb: Option<u64>,

    /// Default Vega-Lite theme applied when a request omits `theme`.
    /// Pass the literal string `null` to clear a value loaded from the
    /// `--vlc-config` file.
    #[arg(long, global = true, value_name = "THEME|null")]
    pub(crate) default_theme: Option<String>,

    /// Default d3-format locale: a locale name string, JSON object
    /// literal, `@<path>` to a JSON file, or the literal string `null`
    /// to clear a value loaded from the `--vlc-config` file.
    #[arg(long, global = true, value_name = "LOCALE|JSON|@FILE|null")]
    pub(crate) default_format_locale: Option<String>,

    /// Default d3-time-format locale: a locale name string, JSON object
    /// literal, `@<path>` to a JSON file, or the literal string `null`
    /// to clear a value loaded from the `--vlc-config` file.
    #[arg(long, global = true, value_name = "LOCALE|JSON|@FILE|null")]
    pub(crate) default_time_format_locale: Option<String>,

    /// Custom named themes as a JSON object literal, `@<path>` to a JSON
    /// file, or the literal string `null` to clear a map loaded from the
    /// `--vlc-config` file.
    #[arg(long, global = true, value_name = "JSON|@FILE|null")]
    pub(crate) themes: Option<String>,

    /// Log level for Vega/Vega-Lite messages
    #[arg(long, global = true, value_enum, default_value_t = LogLevel::Warn)]
    pub(crate) log_level: LogLevel,

    /// Tracing-subscriber output format. `text` is human-readable;
    /// `json` emits one structured line per event for log aggregators.
    #[arg(long, global = true, value_enum, default_value_t = LogFormat::Text)]
    pub(crate) log_format: LogFormat,

    /// Raw `tracing-subscriber::EnvFilter` directive (e.g.
    /// `"vl_convert=debug,tower_http=info"`). When set, this wins over
    /// `--log-level`. When unset, a directive is synthesized from
    /// `--log-level`.
    #[arg(long, global = true, value_name = "DIRECTIVE")]
    pub(crate) log_filter: Option<String>,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

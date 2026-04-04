use clap::Parser;
use vl_convert_rs::converter::MissingFontsPolicy;

use crate::commands::Commands;

pub(crate) const DEFAULT_VL_VERSION: &str = "6.4";

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
    /// Path to JSONC converter config file.
    /// Defaults to the platform config directory if the file exists.
    #[arg(long, global = true)]
    pub(crate) vlc_config: Option<String>,

    /// Disable loading the default vlc-config file
    #[arg(long, global = true, conflicts_with = "vlc_config")]
    pub(crate) no_vlc_config: bool,

    /// Custom base URL for resolving relative data paths in Vega specs.
    /// Can be a URL (https://...) or a local filesystem path (/data/).
    #[arg(long, global = true)]
    pub(crate) base_url: Option<String>,

    /// Disable relative path resolution (relative data paths produce an error)
    #[arg(long, global = true, conflicts_with = "base_url")]
    pub(crate) no_base_url: bool,

    /// Allowed base URL pattern for external data access.
    /// Supports CSP-style patterns: "https:" (scheme), "https://example.com/" (prefix),
    /// "/data/" (filesystem). May be specified multiple times.
    #[arg(long = "allowed-base-url", global = true)]
    pub(crate) allowed_base_url: Vec<String>,

    /// Disable all external data access (empty allowlist)
    #[arg(long, global = true, conflicts_with = "allowed_base_url")]
    pub(crate) no_allowed_urls: bool,

    /// Register a font from Google Fonts. Use "Family" for all variants,
    /// or "Family:400,700italic" for specific weight/style combinations.
    /// May be specified multiple times.
    #[arg(long = "google-font", global = true)]
    pub(crate) google_font: Vec<String>,

    /// Automatically download missing fonts from Google Fonts.
    #[arg(long, global = true)]
    pub(crate) auto_google_fonts: bool,

    /// Embed locally installed fonts as base64 @font-face in HTML and SVG output.
    #[arg(long, global = true)]
    pub(crate) embed_local_fonts: bool,

    /// Disable font subsetting (embed full font files instead of only used characters)
    #[arg(long, global = true)]
    pub(crate) no_subset_fonts: bool,

    /// Missing-font behavior: fallback silently, warn, or error.
    #[arg(long, global = true, value_enum)]
    pub(crate) missing_fonts: Option<MissingFontsArg>,

    /// Maximum V8 heap size per worker in megabytes [default: 0 = no limit]
    #[arg(long, global = true)]
    pub(crate) max_v8_heap_size_mb: Option<usize>,

    /// Maximum V8 execution time in seconds [default: 0 = no limit]
    #[arg(long, global = true)]
    pub(crate) max_v8_execution_time_secs: Option<u64>,

    /// Run V8 garbage collection after each conversion to release memory
    #[arg(long, global = true)]
    pub(crate) gc_after_conversion: bool,

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

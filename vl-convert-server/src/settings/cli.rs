use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use vl_convert_rs::anyhow::{self, bail};
use vl_convert_rs::converter::MissingFontsPolicy;
use vl_convert_server::LogFormat;

use super::parsers::{parse_boolish_arg, parse_path_arg, parse_positive_i64_arg};

#[derive(Debug, Parser, Clone)]
#[command(version, name = "vl-convert-server")]
#[command(about = "HTTP server for converting Vega-Lite and Vega specifications to static images")]
pub(crate) struct Cli {
    /// Path to JSONC converter config file
    #[arg(long, value_parser = parse_path_arg)]
    pub(crate) config: Option<PathBuf>,

    /// Whether to load the default or explicit config file
    #[arg(long, value_name = "BOOL", num_args = 0..=1, require_equals = true, default_missing_value = "true", value_parser = parse_boolish_arg)]
    pub(crate) load_config: Option<bool>,

    /// Base URL setting: default, disabled, or a custom URL/filesystem path
    #[arg(long)]
    pub(crate) base_url: Option<String>,

    /// Data access mode: default, none, all, or allowlist
    #[arg(long, value_enum, value_name = "MODE", ignore_case = true)]
    pub(crate) data_access: Option<DataAccessMode>,

    /// Allowed base URLs as a JSON array or @file
    #[arg(long, value_name = "JSON|@FILE")]
    pub(crate) allowed_base_urls: Option<String>,

    /// Google Fonts as a JSON array of strings/objects or @file
    #[arg(long, value_name = "JSON|@FILE")]
    pub(crate) google_fonts: Option<String>,

    /// Automatically download missing fonts from Google Fonts
    #[arg(long, value_name = "BOOL", num_args = 0..=1, require_equals = true, default_missing_value = "true", value_parser = parse_boolish_arg)]
    pub(crate) auto_google_fonts: Option<bool>,

    /// Allow per-request google_fonts overrides
    #[arg(long, value_name = "BOOL", num_args = 0..=1, require_equals = true, default_missing_value = "true", value_parser = parse_boolish_arg)]
    pub(crate) allow_google_fonts: Option<bool>,

    /// Embed locally installed fonts as base64 @font-face in HTML and SVG output
    #[arg(long, value_name = "BOOL", num_args = 0..=1, require_equals = true, default_missing_value = "true", value_parser = parse_boolish_arg)]
    pub(crate) embed_local_fonts: Option<bool>,

    /// Whether to subset embedded fonts
    #[arg(long, value_name = "BOOL", num_args = 0..=1, require_equals = true, default_missing_value = "true", value_parser = parse_boolish_arg)]
    pub(crate) subset_fonts: Option<bool>,

    /// Missing-font behavior
    #[arg(long, value_enum, value_name = "POLICY")]
    pub(crate) missing_fonts: Option<MissingFontsArg>,

    /// Maximum V8 heap size per worker in megabytes
    #[arg(long)]
    pub(crate) max_v8_heap_size_mb: Option<usize>,

    /// Maximum V8 execution time in seconds
    #[arg(long)]
    pub(crate) max_v8_execution_time_secs: Option<u64>,

    /// Run V8 garbage collection after each conversion
    #[arg(long, value_name = "BOOL", num_args = 0..=1, require_equals = true, default_missing_value = "true", value_parser = parse_boolish_arg)]
    pub(crate) gc_after_conversion: Option<bool>,

    /// Vega plugins as a JSON array or @file
    #[arg(long, value_name = "JSON|@FILE")]
    pub(crate) vega_plugins: Option<String>,

    /// Domains allowed for HTTP imports in plugins as a JSON array or @file
    #[arg(long, value_name = "JSON|@FILE")]
    pub(crate) plugin_import_domains: Option<String>,

    /// Allow per-request Vega plugins
    #[arg(long, value_name = "BOOL", num_args = 0..=1, require_equals = true, default_missing_value = "true", value_parser = parse_boolish_arg)]
    pub(crate) allow_per_request_plugins: Option<bool>,

    /// Maximum concurrent ephemeral workers for per-request plugins
    #[arg(long)]
    pub(crate) max_ephemeral_workers: Option<usize>,

    /// Domains allowed for per-request plugin imports as a JSON array or @file
    #[arg(long, value_name = "JSON|@FILE")]
    pub(crate) per_request_plugin_import_domains: Option<String>,

    /// Default Vega-Lite theme for all requests
    #[arg(long)]
    pub(crate) default_theme: Option<String>,

    /// Default d3-format locale name or JSON object / @file
    #[arg(long, value_name = "LOCALE|JSON|@FILE")]
    pub(crate) default_format_locale: Option<String>,

    /// Default d3-time-format locale name or JSON object / @file
    #[arg(long, value_name = "LOCALE|JSON|@FILE")]
    pub(crate) default_time_format_locale: Option<String>,

    /// Custom named themes as a JSON object or @file
    #[arg(long, value_name = "JSON|@FILE")]
    pub(crate) themes: Option<String>,

    /// Additional directory to search for fonts
    #[arg(long)]
    pub(crate) font_dir: Option<String>,

    /// Log level for vl_convert and tower_http output
    #[arg(long, value_enum, value_name = "LEVEL", ignore_case = true)]
    pub(crate) log_level: Option<LogLevel>,

    /// Full tracing filter string
    #[arg(long)]
    pub(crate) log_filter: Option<String>,

    /// Log output format: text or json
    #[arg(long, value_enum, value_name = "FORMAT", ignore_case = true)]
    pub(crate) log_format: Option<LogFormatArg>,

    /// Bind address
    #[arg(long)]
    pub(crate) host: Option<String>,

    /// Port (defaults to $PORT env var if set, else 3000)
    #[arg(long)]
    pub(crate) port: Option<u16>,

    /// Number of converter worker threads
    #[arg(long)]
    pub(crate) workers: Option<usize>,

    /// API key for Bearer token authentication
    #[arg(long)]
    pub(crate) api_key: Option<String>,

    /// Allowed CORS origin(s), comma-separated or "*"
    #[arg(long)]
    pub(crate) cors_origin: Option<String>,

    /// Maximum simultaneous in-flight requests
    #[arg(long)]
    pub(crate) max_concurrent_requests: Option<String>,

    /// HTTP request timeout in seconds
    #[arg(long)]
    pub(crate) request_timeout_secs: Option<u64>,

    /// Graceful shutdown drain timeout in seconds
    #[arg(long)]
    pub(crate) drain_timeout_secs: Option<u64>,

    /// Maximum request body size in megabytes
    #[arg(long)]
    pub(crate) max_body_size_mb: Option<usize>,

    /// Return only HTTP status codes on error
    #[arg(long, value_name = "BOOL", num_args = 0..=1, require_equals = true, default_missing_value = "true", value_parser = parse_boolish_arg)]
    pub(crate) opaque_errors: Option<bool>,

    /// Reject requests without a User-Agent header
    #[arg(long, value_name = "BOOL", num_args = 0..=1, require_equals = true, default_missing_value = "true", value_parser = parse_boolish_arg)]
    pub(crate) require_user_agent: Option<bool>,

    /// Conversion time budget per IP in ms/min
    #[arg(long)]
    pub(crate) per_ip_budget_ms: Option<String>,

    /// Total conversion time budget for the server in ms/min
    #[arg(long)]
    pub(crate) global_budget_ms: Option<String>,

    /// Per-request budget hold in ms
    #[arg(long, value_parser = parse_positive_i64_arg)]
    pub(crate) budget_hold_ms: Option<i64>,

    /// Enable admin API on 127.0.0.1:<port> for dynamic budget updates
    #[arg(long)]
    pub(crate) admin_port: Option<String>,

    /// Trust X-Forwarded-For and X-Real-IP headers for client IP extraction
    #[arg(long, value_name = "BOOL", num_args = 0..=1, require_equals = true, default_missing_value = "true", value_parser = parse_boolish_arg)]
    pub(crate) trust_proxy: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum MissingFontsArg {
    Fallback,
    Warn,
    Error,
}

impl MissingFontsArg {
    pub(super) fn into_policy(self) -> MissingFontsPolicy {
        match self {
            Self::Fallback => MissingFontsPolicy::Fallback,
            Self::Warn => MissingFontsPolicy::Warn,
            Self::Error => MissingFontsPolicy::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum LogLevel {
    Error,
    #[default]
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    pub(super) fn to_tracing_filter(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }

    pub(super) fn parse(raw: &str, what: &str) -> Result<Self, anyhow::Error> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "error" => Ok(Self::Error),
            "warn" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            _ => bail!("Invalid {what} '{raw}'. Expected one of: error, warn, info, debug."),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum DataAccessMode {
    Default,
    None,
    All,
    Allowlist,
}

impl DataAccessMode {
    pub(super) fn parse(raw: &str, what: &str) -> Result<Self, anyhow::Error> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "default" => Ok(Self::Default),
            "none" => Ok(Self::None),
            "all" => Ok(Self::All),
            "allowlist" => Ok(Self::Allowlist),
            _ => bail!("Invalid {what} '{raw}'. Expected one of: default, none, all, allowlist."),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum LogFormatArg {
    Text,
    Json,
}

impl LogFormatArg {
    pub(super) fn into_log_format(self) -> LogFormat {
        match self {
            Self::Text => LogFormat::Text,
            Self::Json => LogFormat::Json,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::env::SETTING_PAIRS;
    use super::*;
    use clap::CommandFactory;
    use std::collections::HashSet;

    fn parse_cli(args: &[&str]) -> Cli {
        let mut argv = vec!["vl-convert-server"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv).unwrap()
    }

    #[test]
    fn test_cli_flag_and_env_surface_stay_in_sync() {
        let expected_flags: HashSet<&str> = SETTING_PAIRS.iter().map(|(flag, _)| *flag).collect();
        let command = Cli::command();
        let actual_flags: HashSet<&str> = command
            .get_arguments()
            .filter_map(|arg| arg.get_long())
            .collect();

        assert_eq!(actual_flags, expected_flags);

        let expected_envs: HashSet<&str> = SETTING_PAIRS.iter().map(|(_, env)| *env).collect();
        let actual_envs: HashSet<&str> = SETTING_PAIRS.iter().map(|(_, env)| *env).collect();
        assert_eq!(actual_envs, expected_envs);
    }

    #[test]
    fn test_cli_bool_flags_accept_bare_and_false() {
        let cli = parse_cli(&["--load-config", "--subset-fonts=false", "--trust-proxy"]);
        assert_eq!(cli.load_config, Some(true));
        assert_eq!(cli.subset_fonts, Some(false));
        assert_eq!(cli.trust_proxy, Some(true));
    }

    #[test]
    fn test_cli_missing_fonts_uses_typed_enum() {
        let cli = parse_cli(&["--missing-fonts", "warn"]);
        assert_eq!(cli.missing_fonts, Some(MissingFontsArg::Warn));

        assert!(Cli::try_parse_from(["vl-convert-server", "--missing-fonts", "nope"]).is_err());
    }

    #[test]
    fn test_cli_old_flags_removed() {
        assert!(Cli::try_parse_from(["vl-convert-server", "--no-vlc-config"]).is_err());
        assert!(Cli::try_parse_from(["vl-convert-server", "--no-subset-fonts"]).is_err());
        assert!(Cli::try_parse_from(["vl-convert-server", "--google-font", "Roboto"]).is_err());
    }
}

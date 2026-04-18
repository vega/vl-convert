use clap::{Parser, ValueEnum};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;
use vl_convert_google_fonts::{FontStyle, VariantRequest};
use vl_convert_rs::anyhow::{self, anyhow, bail};
use vl_convert_rs::converter::{
    BaseUrlSetting, FormatLocale, GoogleFontRequest, MissingFontsPolicy, TimeFormatLocale,
    VlcConfig,
};
use vl_convert_server::{LogFormat, ServeConfig};

const ENV_CONFIG: &str = "VLC_CONFIG";
const ENV_LOAD_CONFIG: &str = "VLC_LOAD_CONFIG";
const ENV_BASE_URL: &str = "VLC_BASE_URL";
const ENV_DATA_ACCESS: &str = "VLC_DATA_ACCESS";
const ENV_ALLOWED_BASE_URLS: &str = "VLC_ALLOWED_BASE_URLS";
const ENV_GOOGLE_FONTS: &str = "VLC_GOOGLE_FONTS";
const ENV_AUTO_GOOGLE_FONTS: &str = "VLC_AUTO_GOOGLE_FONTS";
const ENV_ALLOW_GOOGLE_FONTS: &str = "VLC_ALLOW_GOOGLE_FONTS";
const ENV_EMBED_LOCAL_FONTS: &str = "VLC_EMBED_LOCAL_FONTS";
const ENV_SUBSET_FONTS: &str = "VLC_SUBSET_FONTS";
const ENV_MISSING_FONTS: &str = "VLC_MISSING_FONTS";
const ENV_MAX_V8_HEAP_SIZE_MB: &str = "VLC_MAX_V8_HEAP_SIZE_MB";
const ENV_MAX_V8_EXECUTION_TIME_SECS: &str = "VLC_MAX_V8_EXECUTION_TIME_SECS";
const ENV_GC_AFTER_CONVERSION: &str = "VLC_GC_AFTER_CONVERSION";
const ENV_VEGA_PLUGINS: &str = "VLC_VEGA_PLUGINS";
const ENV_PLUGIN_IMPORT_DOMAINS: &str = "VLC_PLUGIN_IMPORT_DOMAINS";
const ENV_ALLOW_PER_REQUEST_PLUGINS: &str = "VLC_ALLOW_PER_REQUEST_PLUGINS";
const ENV_MAX_EPHEMERAL_WORKERS: &str = "VLC_MAX_EPHEMERAL_WORKERS";
const ENV_PER_REQUEST_PLUGIN_IMPORT_DOMAINS: &str = "VLC_PER_REQUEST_PLUGIN_IMPORT_DOMAINS";
const ENV_DEFAULT_THEME: &str = "VLC_DEFAULT_THEME";
const ENV_DEFAULT_FORMAT_LOCALE: &str = "VLC_DEFAULT_FORMAT_LOCALE";
const ENV_DEFAULT_TIME_FORMAT_LOCALE: &str = "VLC_DEFAULT_TIME_FORMAT_LOCALE";
const ENV_THEMES: &str = "VLC_THEMES";
const ENV_FONT_DIR: &str = "VLC_FONT_DIR";
const ENV_LOG_LEVEL: &str = "VLC_LOG_LEVEL";
const ENV_LOG_FILTER: &str = "VLC_LOG_FILTER";
const ENV_LOG_FORMAT: &str = "VLC_LOG_FORMAT";
const ENV_HOST: &str = "VLC_HOST";
const ENV_PORT: &str = "VLC_PORT";
const ENV_WORKERS: &str = "VLC_WORKERS";
const ENV_API_KEY: &str = "VLC_API_KEY";
const ENV_CORS_ORIGIN: &str = "VLC_CORS_ORIGIN";
const ENV_MAX_CONCURRENT_REQUESTS: &str = "VLC_MAX_CONCURRENT_REQUESTS";
const ENV_REQUEST_TIMEOUT_SECS: &str = "VLC_REQUEST_TIMEOUT_SECS";
const ENV_DRAIN_TIMEOUT_SECS: &str = "VLC_DRAIN_TIMEOUT_SECS";
const ENV_MAX_BODY_SIZE_MB: &str = "VLC_MAX_BODY_SIZE_MB";
const ENV_OPAQUE_ERRORS: &str = "VLC_OPAQUE_ERRORS";
const ENV_REQUIRE_USER_AGENT: &str = "VLC_REQUIRE_USER_AGENT";
const ENV_PER_IP_BUDGET_MS: &str = "VLC_PER_IP_BUDGET_MS";
const ENV_GLOBAL_BUDGET_MS: &str = "VLC_GLOBAL_BUDGET_MS";
const ENV_BUDGET_HOLD_MS: &str = "VLC_BUDGET_HOLD_MS";
const ENV_ADMIN_PORT: &str = "VLC_ADMIN_PORT";
const ENV_TRUST_PROXY: &str = "VLC_TRUST_PROXY";

#[cfg(test)]
pub(crate) const SETTING_PAIRS: &[(&str, &str)] = &[
    ("config", ENV_CONFIG),
    ("load-config", ENV_LOAD_CONFIG),
    ("base-url", ENV_BASE_URL),
    ("data-access", ENV_DATA_ACCESS),
    ("allowed-base-urls", ENV_ALLOWED_BASE_URLS),
    ("google-fonts", ENV_GOOGLE_FONTS),
    ("auto-google-fonts", ENV_AUTO_GOOGLE_FONTS),
    ("allow-google-fonts", ENV_ALLOW_GOOGLE_FONTS),
    ("embed-local-fonts", ENV_EMBED_LOCAL_FONTS),
    ("subset-fonts", ENV_SUBSET_FONTS),
    ("missing-fonts", ENV_MISSING_FONTS),
    ("max-v8-heap-size-mb", ENV_MAX_V8_HEAP_SIZE_MB),
    ("max-v8-execution-time-secs", ENV_MAX_V8_EXECUTION_TIME_SECS),
    ("gc-after-conversion", ENV_GC_AFTER_CONVERSION),
    ("vega-plugins", ENV_VEGA_PLUGINS),
    ("plugin-import-domains", ENV_PLUGIN_IMPORT_DOMAINS),
    ("allow-per-request-plugins", ENV_ALLOW_PER_REQUEST_PLUGINS),
    ("max-ephemeral-workers", ENV_MAX_EPHEMERAL_WORKERS),
    (
        "per-request-plugin-import-domains",
        ENV_PER_REQUEST_PLUGIN_IMPORT_DOMAINS,
    ),
    ("default-theme", ENV_DEFAULT_THEME),
    ("default-format-locale", ENV_DEFAULT_FORMAT_LOCALE),
    ("default-time-format-locale", ENV_DEFAULT_TIME_FORMAT_LOCALE),
    ("themes", ENV_THEMES),
    ("font-dir", ENV_FONT_DIR),
    ("log-level", ENV_LOG_LEVEL),
    ("log-filter", ENV_LOG_FILTER),
    ("log-format", ENV_LOG_FORMAT),
    ("host", ENV_HOST),
    ("port", ENV_PORT),
    ("workers", ENV_WORKERS),
    ("api-key", ENV_API_KEY),
    ("cors-origin", ENV_CORS_ORIGIN),
    ("max-concurrent-requests", ENV_MAX_CONCURRENT_REQUESTS),
    ("request-timeout-secs", ENV_REQUEST_TIMEOUT_SECS),
    ("drain-timeout-secs", ENV_DRAIN_TIMEOUT_SECS),
    ("max-body-size-mb", ENV_MAX_BODY_SIZE_MB),
    ("opaque-errors", ENV_OPAQUE_ERRORS),
    ("require-user-agent", ENV_REQUIRE_USER_AGENT),
    ("per-ip-budget-ms", ENV_PER_IP_BUDGET_MS),
    ("global-budget-ms", ENV_GLOBAL_BUDGET_MS),
    ("budget-hold-ms", ENV_BUDGET_HOLD_MS),
    ("admin-port", ENV_ADMIN_PORT),
    ("trust-proxy", ENV_TRUST_PROXY),
];

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

#[derive(Debug)]
pub(crate) struct ResolvedSettings {
    pub(crate) converter_config: VlcConfig,
    pub(crate) serve_config: ServeConfig,
    pub(crate) font_dir: Option<String>,
    pub(crate) log_filter: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum MissingFontsArg {
    Fallback,
    Warn,
    Error,
}

impl MissingFontsArg {
    fn into_policy(self) -> MissingFontsPolicy {
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
    fn to_tracing_filter(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }

    fn parse(raw: &str, what: &str) -> Result<Self, anyhow::Error> {
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
    fn parse(raw: &str, what: &str) -> Result<Self, anyhow::Error> {
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
    fn into_log_format(self) -> LogFormat {
        match self {
            Self::Text => LogFormat::Text,
            Self::Json => LogFormat::Json,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum InputKind {
    Cli,
    Env,
}

impl InputKind {
    fn label(self) -> &'static str {
        match self {
            Self::Cli => "CLI",
            Self::Env => "environment",
        }
    }
}

#[derive(Debug, Default, Clone)]
struct EnvValues {
    config: Option<String>,
    load_config: Option<String>,
    base_url: Option<String>,
    data_access: Option<String>,
    allowed_base_urls: Option<String>,
    google_fonts: Option<String>,
    auto_google_fonts: Option<String>,
    allow_google_fonts: Option<String>,
    embed_local_fonts: Option<String>,
    subset_fonts: Option<String>,
    missing_fonts: Option<String>,
    max_v8_heap_size_mb: Option<String>,
    max_v8_execution_time_secs: Option<String>,
    gc_after_conversion: Option<String>,
    vega_plugins: Option<String>,
    plugin_import_domains: Option<String>,
    allow_per_request_plugins: Option<String>,
    max_ephemeral_workers: Option<String>,
    per_request_plugin_import_domains: Option<String>,
    default_theme: Option<String>,
    default_format_locale: Option<String>,
    default_time_format_locale: Option<String>,
    themes: Option<String>,
    font_dir: Option<String>,
    log_level: Option<String>,
    log_filter: Option<String>,
    log_format: Option<String>,
    host: Option<String>,
    port: Option<String>,
    workers: Option<String>,
    api_key: Option<String>,
    cors_origin: Option<String>,
    max_concurrent_requests: Option<String>,
    request_timeout_secs: Option<String>,
    drain_timeout_secs: Option<String>,
    max_body_size_mb: Option<String>,
    opaque_errors: Option<String>,
    require_user_agent: Option<String>,
    per_ip_budget_ms: Option<String>,
    global_budget_ms: Option<String>,
    budget_hold_ms: Option<String>,
    admin_port: Option<String>,
    trust_proxy: Option<String>,
}

impl EnvValues {
    fn from_env() -> Self {
        Self {
            config: env_var(ENV_CONFIG),
            load_config: env_var(ENV_LOAD_CONFIG),
            base_url: env_var(ENV_BASE_URL),
            data_access: env_var(ENV_DATA_ACCESS),
            allowed_base_urls: env_var(ENV_ALLOWED_BASE_URLS),
            google_fonts: env_var(ENV_GOOGLE_FONTS),
            auto_google_fonts: env_var(ENV_AUTO_GOOGLE_FONTS),
            allow_google_fonts: env_var(ENV_ALLOW_GOOGLE_FONTS),
            embed_local_fonts: env_var(ENV_EMBED_LOCAL_FONTS),
            subset_fonts: env_var(ENV_SUBSET_FONTS),
            missing_fonts: env_var(ENV_MISSING_FONTS),
            max_v8_heap_size_mb: env_var(ENV_MAX_V8_HEAP_SIZE_MB),
            max_v8_execution_time_secs: env_var(ENV_MAX_V8_EXECUTION_TIME_SECS),
            gc_after_conversion: env_var(ENV_GC_AFTER_CONVERSION),
            vega_plugins: env_var(ENV_VEGA_PLUGINS),
            plugin_import_domains: env_var(ENV_PLUGIN_IMPORT_DOMAINS),
            allow_per_request_plugins: env_var(ENV_ALLOW_PER_REQUEST_PLUGINS),
            max_ephemeral_workers: env_var(ENV_MAX_EPHEMERAL_WORKERS),
            per_request_plugin_import_domains: env_var(ENV_PER_REQUEST_PLUGIN_IMPORT_DOMAINS),
            default_theme: env_var(ENV_DEFAULT_THEME),
            default_format_locale: env_var(ENV_DEFAULT_FORMAT_LOCALE),
            default_time_format_locale: env_var(ENV_DEFAULT_TIME_FORMAT_LOCALE),
            themes: env_var(ENV_THEMES),
            font_dir: env_var(ENV_FONT_DIR),
            log_level: env_var(ENV_LOG_LEVEL),
            log_filter: env_var(ENV_LOG_FILTER),
            log_format: env_var(ENV_LOG_FORMAT),
            host: env_var(ENV_HOST),
            port: env_var(ENV_PORT).or_else(|| {
                // PaaS convention: Railway/Heroku/Fly/Render/Cloud Run
                // all inject PORT. Silently ignore a non-numeric value
                // rather than failing startup on an unrelated collision.
                env_var("PORT").filter(|v| v.parse::<u16>().is_ok())
            }),
            workers: env_var(ENV_WORKERS),
            api_key: env_var(ENV_API_KEY),
            cors_origin: env_var(ENV_CORS_ORIGIN),
            max_concurrent_requests: env_var(ENV_MAX_CONCURRENT_REQUESTS),
            request_timeout_secs: env_var(ENV_REQUEST_TIMEOUT_SECS),
            drain_timeout_secs: env_var(ENV_DRAIN_TIMEOUT_SECS),
            max_body_size_mb: env_var(ENV_MAX_BODY_SIZE_MB),
            opaque_errors: env_var(ENV_OPAQUE_ERRORS),
            require_user_agent: env_var(ENV_REQUIRE_USER_AGENT),
            per_ip_budget_ms: env_var(ENV_PER_IP_BUDGET_MS),
            global_budget_ms: env_var(ENV_GLOBAL_BUDGET_MS),
            budget_hold_ms: env_var(ENV_BUDGET_HOLD_MS),
            admin_port: env_var(ENV_ADMIN_PORT),
            trust_proxy: env_var(ENV_TRUST_PROXY),
        }
    }
}

#[derive(Debug, Default)]
struct BootstrapOverrides {
    config: Option<PathBuf>,
    load_config: Option<bool>,
}

#[derive(Debug, Default)]
struct Overrides {
    bootstrap: BootstrapOverrides,
    font_dir: Option<Option<String>>,
    log_level: Option<LogLevel>,
    log_filter: Option<Option<String>>,
    log_format: Option<LogFormat>,
    host: Option<String>,
    port: Option<u16>,
    workers: Option<usize>,
    api_key: Option<Option<String>>,
    cors_origin: Option<Option<String>>,
    max_concurrent_requests: Option<Option<usize>>,
    request_timeout_secs: Option<u64>,
    drain_timeout_secs: Option<u64>,
    max_body_size_mb: Option<usize>,
    opaque_errors: Option<bool>,
    require_user_agent: Option<bool>,
    per_ip_budget_ms: Option<Option<i64>>,
    global_budget_ms: Option<Option<i64>>,
    budget_hold_ms: Option<i64>,
    admin_port: Option<Option<u16>>,
    trust_proxy: Option<bool>,
    base_url: Option<BaseUrlSetting>,
    data_access: Option<DataAccessMode>,
    allowed_base_urls: Option<Option<Vec<String>>>,
    google_fonts: Option<Option<Vec<GoogleFontRequest>>>,
    auto_google_fonts: Option<bool>,
    allow_google_fonts: Option<bool>,
    embed_local_fonts: Option<bool>,
    subset_fonts: Option<bool>,
    missing_fonts: Option<MissingFontsPolicy>,
    max_v8_heap_size_mb: Option<usize>,
    max_v8_execution_time_secs: Option<u64>,
    gc_after_conversion: Option<bool>,
    vega_plugins: Option<Option<Vec<String>>>,
    plugin_import_domains: Option<Option<Vec<String>>>,
    allow_per_request_plugins: Option<bool>,
    max_ephemeral_workers: Option<usize>,
    per_request_plugin_import_domains: Option<Option<Vec<String>>>,
    default_theme: Option<Option<String>>,
    default_format_locale: Option<Option<FormatLocale>>,
    default_time_format_locale: Option<Option<TimeFormatLocale>>,
    themes: Option<Option<HashMap<String, serde_json::Value>>>,
}

#[derive(Debug)]
struct WorkingSettings {
    converter_config: VlcConfig,
    serve_config: ServeConfig,
    font_dir: Option<String>,
    log_level: LogLevel,
    log_filter: Option<String>,
    data_access: DataAccessMode,
    allowed_base_urls: Option<Vec<String>>,
}

pub(crate) fn resolve_settings(cli: Cli) -> Result<ResolvedSettings, anyhow::Error> {
    let env_overrides = parse_env_overrides(EnvValues::from_env())?;
    let cli_overrides = parse_cli_overrides(&cli)?;
    let bootstrap = resolve_bootstrap(&env_overrides.bootstrap, &cli_overrides.bootstrap)?;
    let config = load_converter_config(&bootstrap)?;
    let mut working = WorkingSettings::new(config);
    working.apply(env_overrides);
    working.apply(cli_overrides);
    working.finalize()
}

impl WorkingSettings {
    fn new(mut converter_config: VlcConfig) -> Self {
        let (data_access, allowed_base_urls) =
            derive_data_access_state(converter_config.allowed_base_urls.take());
        Self {
            converter_config,
            serve_config: default_serve_config(),
            font_dir: None,
            log_level: LogLevel::Warn,
            log_filter: None,
            data_access,
            allowed_base_urls,
        }
    }

    fn apply(&mut self, overrides: Overrides) {
        let data_access_explicit = overrides.data_access.is_some();

        if let Some(value) = overrides.font_dir {
            self.font_dir = value;
        }
        if let Some(value) = overrides.log_level {
            self.log_level = value;
        }
        if let Some(value) = overrides.log_filter {
            self.log_filter = value;
        }
        if let Some(value) = overrides.log_format {
            self.serve_config.log_format = value;
        }
        if let Some(value) = overrides.host {
            self.serve_config.host = value;
        }
        if let Some(value) = overrides.port {
            self.serve_config.port = value;
        }
        if let Some(value) = overrides.workers {
            self.converter_config.num_workers = value;
        }
        if let Some(value) = overrides.api_key {
            self.serve_config.api_key = value;
        }
        if let Some(value) = overrides.cors_origin {
            self.serve_config.cors_origin = value;
        }
        if let Some(value) = overrides.max_concurrent_requests {
            self.serve_config.max_concurrent_requests = value;
        }
        if let Some(value) = overrides.request_timeout_secs {
            self.serve_config.request_timeout_secs = value;
        }
        if let Some(value) = overrides.drain_timeout_secs {
            self.serve_config.drain_timeout_secs = value;
        }
        if let Some(value) = overrides.max_body_size_mb {
            self.serve_config.max_body_size_mb = value;
        }
        if let Some(value) = overrides.opaque_errors {
            self.serve_config.opaque_errors = value;
        }
        if let Some(value) = overrides.require_user_agent {
            self.serve_config.require_user_agent = value;
        }
        if let Some(value) = overrides.per_ip_budget_ms {
            self.serve_config.per_ip_budget_ms = value;
        }
        if let Some(value) = overrides.global_budget_ms {
            self.serve_config.global_budget_ms = value;
        }
        if let Some(value) = overrides.budget_hold_ms {
            self.serve_config.budget_hold_ms = value;
        }
        if let Some(value) = overrides.admin_port {
            self.serve_config.admin_port = value;
        }
        if let Some(value) = overrides.trust_proxy {
            self.serve_config.trust_proxy = value;
        }
        if let Some(value) = overrides.base_url {
            self.converter_config.base_url = value;
        }
        if let Some(value) = overrides.data_access {
            self.data_access = value;
        }
        if let Some(value) = overrides.allowed_base_urls {
            let cleared = value.is_none();
            self.allowed_base_urls = value;
            if cleared
                && !data_access_explicit
                && matches!(self.data_access, DataAccessMode::Allowlist)
            {
                self.data_access = DataAccessMode::Default;
            }
        }
        if let Some(value) = overrides.google_fonts {
            self.converter_config.google_fonts = value;
        }
        if let Some(value) = overrides.auto_google_fonts {
            self.converter_config.auto_google_fonts = value;
        }
        if let Some(value) = overrides.allow_google_fonts {
            self.converter_config.allow_google_fonts = value;
        }
        if let Some(value) = overrides.embed_local_fonts {
            self.converter_config.embed_local_fonts = value;
        }
        if let Some(value) = overrides.subset_fonts {
            self.converter_config.subset_fonts = value;
        }
        if let Some(value) = overrides.missing_fonts {
            self.converter_config.missing_fonts = value;
        }
        if let Some(value) = overrides.max_v8_heap_size_mb {
            self.converter_config.max_v8_heap_size_mb = value;
        }
        if let Some(value) = overrides.max_v8_execution_time_secs {
            self.converter_config.max_v8_execution_time_secs = value;
        }
        if let Some(value) = overrides.gc_after_conversion {
            self.converter_config.gc_after_conversion = value;
        }
        if let Some(value) = overrides.vega_plugins {
            self.converter_config.vega_plugins = value;
        }
        if let Some(value) = overrides.plugin_import_domains {
            self.converter_config.plugin_import_domains = value.unwrap_or_default();
        }
        if let Some(value) = overrides.allow_per_request_plugins {
            self.converter_config.allow_per_request_plugins = value;
        }
        if let Some(value) = overrides.max_ephemeral_workers {
            self.converter_config.max_ephemeral_workers = value;
        }
        if let Some(value) = overrides.per_request_plugin_import_domains {
            self.converter_config.per_request_plugin_import_domains = value.unwrap_or_default();
        }
        if let Some(value) = overrides.default_theme {
            self.converter_config.default_theme = value;
        }
        if let Some(value) = overrides.default_format_locale {
            self.converter_config.default_format_locale = value;
        }
        if let Some(value) = overrides.default_time_format_locale {
            self.converter_config.default_time_format_locale = value;
        }
        if let Some(value) = overrides.themes {
            self.converter_config.themes = value;
        }
    }

    fn finalize(mut self) -> Result<ResolvedSettings, anyhow::Error> {
        self.converter_config.allowed_base_urls =
            finalize_allowed_base_urls(self.data_access, self.allowed_base_urls)?;

        if let Some(locale) = &self.converter_config.default_format_locale {
            locale.as_object()?;
        }
        if let Some(locale) = &self.converter_config.default_time_format_locale {
            locale.as_object()?;
        }

        if self.serve_config.request_timeout_secs > 0 {
            let current = self.converter_config.max_v8_execution_time_secs;
            if current == 0 || current > self.serve_config.request_timeout_secs {
                self.converter_config.max_v8_execution_time_secs =
                    self.serve_config.request_timeout_secs;
            }
        }

        let log_filter = self.log_filter.unwrap_or_else(|| {
            let level = self.log_level.to_tracing_filter();
            format!("vl_convert={level},tower_http={level}")
        });

        Ok(ResolvedSettings {
            converter_config: self.converter_config,
            serve_config: self.serve_config,
            font_dir: self.font_dir,
            log_filter,
        })
    }
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn default_serve_config() -> ServeConfig {
    ServeConfig {
        host: "127.0.0.1".to_string(),
        port: 3000,
        api_key: None,
        cors_origin: None,
        max_concurrent_requests: None,
        request_timeout_secs: 30,
        drain_timeout_secs: 30,
        max_body_size_mb: 50,
        opaque_errors: false,
        require_user_agent: false,
        log_format: LogFormat::Text,
        per_ip_budget_ms: None,
        global_budget_ms: None,
        budget_hold_ms: 1000,
        admin_port: None,
        trust_proxy: false,
    }
}

fn parse_env_overrides(raw: EnvValues) -> Result<Overrides, anyhow::Error> {
    let input = InputKind::Env;
    let mut overrides = Overrides::default();

    overrides.bootstrap.load_config = raw
        .load_config
        .as_deref()
        .map(|raw| parse_bool(raw, field_name(input, "load_config")))
        .transpose()?;
    overrides.bootstrap.config = raw.config.as_deref().map(expand_path);
    if overrides.bootstrap.config.is_some() && overrides.bootstrap.load_config.is_none() {
        overrides.bootstrap.load_config = Some(true);
    }
    if overrides.bootstrap.config.is_some() && overrides.bootstrap.load_config == Some(false) {
        bail!(
            "{} cannot set both config path and load_config=false",
            input.label()
        );
    }

    overrides.base_url = raw
        .base_url
        .as_deref()
        .map(|raw| parse_base_url(raw, field_name(input, "base_url")))
        .transpose()?;
    overrides.data_access = raw
        .data_access
        .as_deref()
        .map(|raw| DataAccessMode::parse(raw, &field_name(input, "data_access")))
        .transpose()?;
    overrides.allowed_base_urls = raw
        .allowed_base_urls
        .as_deref()
        .map(|raw| parse_string_vec(raw, input, field_name(input, "allowed_base_urls")))
        .transpose()?;
    if matches!(overrides.allowed_base_urls, Some(Some(_))) && overrides.data_access.is_none() {
        overrides.data_access = Some(DataAccessMode::Allowlist);
    }
    if matches!(
        (overrides.data_access, &overrides.allowed_base_urls),
        (
            Some(DataAccessMode::Default | DataAccessMode::None | DataAccessMode::All),
            Some(Some(_))
        )
    ) {
        bail!(
            "{} cannot combine data_access={} with allowed_base_urls",
            input.label(),
            match overrides.data_access.unwrap() {
                DataAccessMode::Default => "default",
                DataAccessMode::None => "none",
                DataAccessMode::All => "all",
                DataAccessMode::Allowlist => "allowlist",
            }
        );
    }

    overrides.google_fonts = raw
        .google_fonts
        .as_deref()
        .map(|raw| parse_google_fonts(raw, input, field_name(input, "google_fonts")))
        .transpose()?;
    overrides.auto_google_fonts = raw
        .auto_google_fonts
        .as_deref()
        .map(|raw| parse_bool(raw, field_name(input, "auto_google_fonts")))
        .transpose()?;
    overrides.allow_google_fonts = raw
        .allow_google_fonts
        .as_deref()
        .map(|raw| parse_bool(raw, field_name(input, "allow_google_fonts")))
        .transpose()?;
    overrides.embed_local_fonts = raw
        .embed_local_fonts
        .as_deref()
        .map(|raw| parse_bool(raw, field_name(input, "embed_local_fonts")))
        .transpose()?;
    overrides.subset_fonts = raw
        .subset_fonts
        .as_deref()
        .map(|raw| parse_bool(raw, field_name(input, "subset_fonts")))
        .transpose()?;
    overrides.missing_fonts = raw
        .missing_fonts
        .as_deref()
        .map(|raw| parse_missing_fonts(raw, field_name(input, "missing_fonts")))
        .transpose()?;
    overrides.max_v8_heap_size_mb = raw
        .max_v8_heap_size_mb
        .as_deref()
        .map(|raw| parse_usize(raw, field_name(input, "max_v8_heap_size_mb")))
        .transpose()?;
    overrides.max_v8_execution_time_secs = raw
        .max_v8_execution_time_secs
        .as_deref()
        .map(|raw| parse_u64(raw, field_name(input, "max_v8_execution_time_secs")))
        .transpose()?;
    overrides.gc_after_conversion = raw
        .gc_after_conversion
        .as_deref()
        .map(|raw| parse_bool(raw, field_name(input, "gc_after_conversion")))
        .transpose()?;
    overrides.vega_plugins = raw
        .vega_plugins
        .as_deref()
        .map(|raw| parse_vega_plugins(raw, input, field_name(input, "vega_plugins")))
        .transpose()?;
    overrides.plugin_import_domains = raw
        .plugin_import_domains
        .as_deref()
        .map(|raw| parse_string_vec(raw, input, field_name(input, "plugin_import_domains")))
        .transpose()?;
    overrides.allow_per_request_plugins = raw
        .allow_per_request_plugins
        .as_deref()
        .map(|raw| parse_bool(raw, field_name(input, "allow_per_request_plugins")))
        .transpose()?;
    overrides.max_ephemeral_workers = raw
        .max_ephemeral_workers
        .as_deref()
        .map(|raw| parse_usize(raw, field_name(input, "max_ephemeral_workers")))
        .transpose()?;
    overrides.per_request_plugin_import_domains = raw
        .per_request_plugin_import_domains
        .as_deref()
        .map(|raw| {
            parse_string_vec(
                raw,
                input,
                field_name(input, "per_request_plugin_import_domains"),
            )
        })
        .transpose()?;
    overrides.default_theme = raw
        .default_theme
        .as_deref()
        .map(parse_nullable_string)
        .transpose()?;
    overrides.default_format_locale = raw
        .default_format_locale
        .as_deref()
        .map(|raw| parse_format_locale(raw, input, field_name(input, "default_format_locale")))
        .transpose()?;
    overrides.default_time_format_locale = raw
        .default_time_format_locale
        .as_deref()
        .map(|raw| {
            parse_time_format_locale(raw, input, field_name(input, "default_time_format_locale"))
        })
        .transpose()?;
    overrides.themes = raw
        .themes
        .as_deref()
        .map(|raw| parse_json_map(raw, input, field_name(input, "themes")))
        .transpose()?;

    overrides.font_dir = raw
        .font_dir
        .as_deref()
        .map(parse_nullable_string)
        .transpose()?;
    overrides.log_level = raw
        .log_level
        .as_deref()
        .map(|raw| LogLevel::parse(raw, &field_name(input, "log_level")))
        .transpose()?;
    overrides.log_filter = raw
        .log_filter
        .as_deref()
        .map(|raw| parse_log_filter_value(raw, field_name(input, "log_filter")))
        .transpose()?;
    overrides.log_format = raw
        .log_format
        .as_deref()
        .map(|raw| parse_log_format(raw, field_name(input, "log_format")))
        .transpose()?;
    overrides.host = raw.host;
    overrides.port = raw
        .port
        .as_deref()
        .map(|raw| parse_u16(raw, field_name(input, "port")))
        .transpose()?;
    overrides.workers = raw
        .workers
        .as_deref()
        .map(|raw| parse_usize(raw, field_name(input, "workers")))
        .transpose()?;
    overrides.api_key = raw
        .api_key
        .as_deref()
        .map(parse_nullable_string)
        .transpose()?;
    overrides.cors_origin = raw
        .cors_origin
        .as_deref()
        .map(parse_nullable_string)
        .transpose()?;
    overrides.max_concurrent_requests = raw
        .max_concurrent_requests
        .as_deref()
        .map(|raw| parse_nullable_usize(raw, field_name(input, "max_concurrent_requests")))
        .transpose()?;
    overrides.request_timeout_secs = raw
        .request_timeout_secs
        .as_deref()
        .map(|raw| parse_u64(raw, field_name(input, "request_timeout_secs")))
        .transpose()?;
    overrides.drain_timeout_secs = raw
        .drain_timeout_secs
        .as_deref()
        .map(|raw| parse_u64(raw, field_name(input, "drain_timeout_secs")))
        .transpose()?;
    overrides.max_body_size_mb = raw
        .max_body_size_mb
        .as_deref()
        .map(|raw| parse_usize(raw, field_name(input, "max_body_size_mb")))
        .transpose()?;
    overrides.opaque_errors = raw
        .opaque_errors
        .as_deref()
        .map(|raw| parse_bool(raw, field_name(input, "opaque_errors")))
        .transpose()?;
    overrides.require_user_agent = raw
        .require_user_agent
        .as_deref()
        .map(|raw| parse_bool(raw, field_name(input, "require_user_agent")))
        .transpose()?;
    overrides.per_ip_budget_ms = raw
        .per_ip_budget_ms
        .as_deref()
        .map(|raw| parse_nullable_i64(raw, field_name(input, "per_ip_budget_ms")))
        .transpose()?;
    overrides.global_budget_ms = raw
        .global_budget_ms
        .as_deref()
        .map(|raw| parse_nullable_i64(raw, field_name(input, "global_budget_ms")))
        .transpose()?;
    overrides.budget_hold_ms = raw
        .budget_hold_ms
        .as_deref()
        .map(|raw| parse_positive_i64(raw, field_name(input, "budget_hold_ms")))
        .transpose()?;
    overrides.admin_port = raw
        .admin_port
        .as_deref()
        .map(|raw| parse_nullable_u16(raw, field_name(input, "admin_port")))
        .transpose()?;
    overrides.trust_proxy = raw
        .trust_proxy
        .as_deref()
        .map(|raw| parse_bool(raw, field_name(input, "trust_proxy")))
        .transpose()?;

    Ok(overrides)
}

fn parse_cli_overrides(cli: &Cli) -> Result<Overrides, anyhow::Error> {
    let input = InputKind::Cli;
    let mut overrides = Overrides::default();

    overrides.bootstrap.load_config = cli.load_config;
    overrides.bootstrap.config = cli.config.clone();
    if overrides.bootstrap.config.is_some() && overrides.bootstrap.load_config.is_none() {
        overrides.bootstrap.load_config = Some(true);
    }
    if overrides.bootstrap.config.is_some() && overrides.bootstrap.load_config == Some(false) {
        bail!(
            "{} cannot set both config path and load_config=false",
            input.label()
        );
    }

    overrides.base_url = cli
        .base_url
        .as_deref()
        .map(|raw| parse_base_url(raw, field_name(input, "base_url")))
        .transpose()?;
    overrides.data_access = cli.data_access;
    overrides.allowed_base_urls = cli
        .allowed_base_urls
        .as_deref()
        .map(|raw| parse_string_vec(raw, input, field_name(input, "allowed_base_urls")))
        .transpose()?;
    if matches!(overrides.allowed_base_urls, Some(Some(_))) && overrides.data_access.is_none() {
        overrides.data_access = Some(DataAccessMode::Allowlist);
    }
    if matches!(
        (overrides.data_access, &overrides.allowed_base_urls),
        (
            Some(DataAccessMode::Default | DataAccessMode::None | DataAccessMode::All),
            Some(Some(_))
        )
    ) {
        bail!(
            "{} cannot combine data_access={} with allowed_base_urls",
            input.label(),
            match overrides.data_access.unwrap() {
                DataAccessMode::Default => "default",
                DataAccessMode::None => "none",
                DataAccessMode::All => "all",
                DataAccessMode::Allowlist => "allowlist",
            }
        );
    }

    overrides.google_fonts = cli
        .google_fonts
        .as_deref()
        .map(|raw| parse_google_fonts(raw, input, field_name(input, "google_fonts")))
        .transpose()?;
    overrides.auto_google_fonts = cli.auto_google_fonts;
    overrides.allow_google_fonts = cli.allow_google_fonts;
    overrides.embed_local_fonts = cli.embed_local_fonts;
    overrides.subset_fonts = cli.subset_fonts;
    overrides.missing_fonts = cli.missing_fonts.map(MissingFontsArg::into_policy);
    overrides.max_v8_heap_size_mb = cli.max_v8_heap_size_mb;
    overrides.max_v8_execution_time_secs = cli.max_v8_execution_time_secs;
    overrides.gc_after_conversion = cli.gc_after_conversion;
    overrides.vega_plugins = cli
        .vega_plugins
        .as_deref()
        .map(|raw| parse_vega_plugins(raw, input, field_name(input, "vega_plugins")))
        .transpose()?;
    overrides.plugin_import_domains = cli
        .plugin_import_domains
        .as_deref()
        .map(|raw| parse_string_vec(raw, input, field_name(input, "plugin_import_domains")))
        .transpose()?;
    overrides.allow_per_request_plugins = cli.allow_per_request_plugins;
    overrides.max_ephemeral_workers = cli.max_ephemeral_workers;
    overrides.per_request_plugin_import_domains = cli
        .per_request_plugin_import_domains
        .as_deref()
        .map(|raw| {
            parse_string_vec(
                raw,
                input,
                field_name(input, "per_request_plugin_import_domains"),
            )
        })
        .transpose()?;
    overrides.default_theme = cli
        .default_theme
        .as_deref()
        .map(parse_nullable_string)
        .transpose()?;
    overrides.default_format_locale = cli
        .default_format_locale
        .as_deref()
        .map(|raw| parse_format_locale(raw, input, field_name(input, "default_format_locale")))
        .transpose()?;
    overrides.default_time_format_locale = cli
        .default_time_format_locale
        .as_deref()
        .map(|raw| {
            parse_time_format_locale(raw, input, field_name(input, "default_time_format_locale"))
        })
        .transpose()?;
    overrides.themes = cli
        .themes
        .as_deref()
        .map(|raw| parse_json_map(raw, input, field_name(input, "themes")))
        .transpose()?;

    overrides.font_dir = cli
        .font_dir
        .as_deref()
        .map(parse_nullable_string)
        .transpose()?;
    overrides.log_level = cli.log_level;
    overrides.log_filter = cli
        .log_filter
        .as_deref()
        .map(|raw| parse_log_filter_value(raw, field_name(input, "log_filter")))
        .transpose()?;
    overrides.log_format = cli.log_format.map(LogFormatArg::into_log_format);
    overrides.host = cli.host.clone();
    overrides.port = cli.port;
    overrides.workers = cli.workers;
    overrides.api_key = cli
        .api_key
        .as_deref()
        .map(parse_nullable_string)
        .transpose()?;
    overrides.cors_origin = cli
        .cors_origin
        .as_deref()
        .map(parse_nullable_string)
        .transpose()?;
    overrides.max_concurrent_requests = cli
        .max_concurrent_requests
        .as_deref()
        .map(|raw| parse_nullable_usize(raw, field_name(input, "max_concurrent_requests")))
        .transpose()?;
    overrides.request_timeout_secs = cli.request_timeout_secs;
    overrides.drain_timeout_secs = cli.drain_timeout_secs;
    overrides.max_body_size_mb = cli.max_body_size_mb;
    overrides.opaque_errors = cli.opaque_errors;
    overrides.require_user_agent = cli.require_user_agent;
    overrides.per_ip_budget_ms = cli
        .per_ip_budget_ms
        .as_deref()
        .map(|raw| parse_nullable_i64(raw, field_name(input, "per_ip_budget_ms")))
        .transpose()?;
    overrides.global_budget_ms = cli
        .global_budget_ms
        .as_deref()
        .map(|raw| parse_nullable_i64(raw, field_name(input, "global_budget_ms")))
        .transpose()?;
    overrides.budget_hold_ms = cli.budget_hold_ms;
    overrides.admin_port = cli
        .admin_port
        .as_deref()
        .map(|raw| parse_nullable_u16(raw, field_name(input, "admin_port")))
        .transpose()?;
    overrides.trust_proxy = cli.trust_proxy;

    Ok(overrides)
}

fn resolve_bootstrap(
    env: &BootstrapOverrides,
    cli: &BootstrapOverrides,
) -> Result<BootstrapOverrides, anyhow::Error> {
    let mut merged = BootstrapOverrides {
        config: None,
        load_config: Some(true),
    };

    if let Some(value) = &env.config {
        merged.config = Some(value.clone());
    }
    if let Some(value) = env.load_config {
        merged.load_config = Some(value);
    }
    if let Some(value) = &cli.config {
        merged.config = Some(value.clone());
    }
    if let Some(value) = cli.load_config {
        merged.load_config = Some(value);
    }

    if merged.load_config == Some(false) {
        merged.config = None;
    }

    Ok(merged)
}

fn load_converter_config(bootstrap: &BootstrapOverrides) -> Result<VlcConfig, anyhow::Error> {
    if bootstrap.load_config == Some(false) {
        return Ok(VlcConfig::default());
    }

    let path = match &bootstrap.config {
        Some(path) => Some(path.clone()),
        None => {
            let default = vl_convert_rs::vlc_config_path();
            default.exists().then_some(default)
        }
    };

    match path {
        Some(path) => VlcConfig::from_file(&path),
        None => Ok(VlcConfig::default()),
    }
}

fn field_name(source: InputKind, field: &'static str) -> String {
    match source {
        InputKind::Cli => format!("CLI {field}"),
        InputKind::Env => format!("env {field}"),
    }
}

fn parse_path_arg(raw: &str) -> Result<PathBuf, String> {
    Ok(expand_path(raw))
}

fn parse_boolish_arg(raw: &str) -> Result<bool, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err("expected one of: true, false, 1, 0, yes, no, on, off".to_string()),
    }
}

fn parse_positive_i64_arg(raw: &str) -> Result<i64, String> {
    let parsed: i64 = raw
        .trim()
        .parse()
        .map_err(|err| format!("invalid integer '{raw}': {err}"))?;
    if parsed <= 0 {
        return Err("must be positive".to_string());
    }
    Ok(parsed)
}

fn parse_base_url(raw: &str, what: String) -> Result<BaseUrlSetting, anyhow::Error> {
    match raw.trim() {
        "default" => Ok(BaseUrlSetting::Default),
        "disabled" => Ok(BaseUrlSetting::Disabled),
        "" => bail!("{what} must not be empty"),
        other => Ok(BaseUrlSetting::Custom(other.to_string())),
    }
}

fn parse_log_format(raw: &str, what: String) -> Result<LogFormat, anyhow::Error> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "text" => Ok(LogFormat::Text),
        "json" => Ok(LogFormat::Json),
        _ => bail!("Invalid {what} '{raw}'. Expected one of: text, json."),
    }
}

fn parse_missing_fonts(raw: &str, what: String) -> Result<MissingFontsPolicy, anyhow::Error> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "fallback" => Ok(MissingFontsPolicy::Fallback),
        "warn" => Ok(MissingFontsPolicy::Warn),
        "error" => Ok(MissingFontsPolicy::Error),
        _ => bail!("Invalid {what} '{raw}'. Expected one of: fallback, warn, error."),
    }
}

fn parse_bool(raw: &str, what: String) -> Result<bool, anyhow::Error> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("Invalid {what} '{raw}'. Expected a boolean value."),
    }
}

fn parse_u16(raw: &str, what: String) -> Result<u16, anyhow::Error> {
    raw.trim()
        .parse()
        .map_err(|err| anyhow!("Invalid {what} '{raw}': {err}"))
}

fn parse_usize(raw: &str, what: String) -> Result<usize, anyhow::Error> {
    raw.trim()
        .parse()
        .map_err(|err| anyhow!("Invalid {what} '{raw}': {err}"))
}

fn parse_u64(raw: &str, what: String) -> Result<u64, anyhow::Error> {
    raw.trim()
        .parse()
        .map_err(|err| anyhow!("Invalid {what} '{raw}': {err}"))
}

fn parse_i64(raw: &str, what: String) -> Result<i64, anyhow::Error> {
    raw.trim()
        .parse()
        .map_err(|err| anyhow!("Invalid {what} '{raw}': {err}"))
}

fn parse_positive_i64(raw: &str, what: String) -> Result<i64, anyhow::Error> {
    let parsed = parse_i64(raw, what.clone())?;
    if parsed <= 0 {
        bail!("{what} must be positive");
    }
    Ok(parsed)
}

fn parse_nullable_string(raw: &str) -> Result<Option<String>, anyhow::Error> {
    if is_null_literal(raw) {
        Ok(None)
    } else {
        Ok(Some(raw.to_string()))
    }
}

fn parse_log_filter_value(raw: &str, what: String) -> Result<Option<String>, anyhow::Error> {
    let value = parse_nullable_string(raw)?;
    if let Some(ref filter) = value {
        filter
            .parse::<EnvFilter>()
            .map_err(|err| anyhow!("Invalid {what} '{filter}': {err}"))?;
    }
    Ok(value)
}

fn parse_nullable_usize(raw: &str, what: String) -> Result<Option<usize>, anyhow::Error> {
    if is_null_literal(raw) {
        Ok(None)
    } else {
        parse_usize(raw, what).map(Some)
    }
}

fn parse_nullable_u16(raw: &str, what: String) -> Result<Option<u16>, anyhow::Error> {
    if is_null_literal(raw) {
        Ok(None)
    } else {
        parse_u16(raw, what).map(Some)
    }
}

fn parse_nullable_i64(raw: &str, what: String) -> Result<Option<i64>, anyhow::Error> {
    if is_null_literal(raw) {
        Ok(None)
    } else {
        parse_i64(raw, what).map(Some)
    }
}

fn is_null_literal(raw: &str) -> bool {
    raw.trim().eq_ignore_ascii_case("null")
}

#[derive(Debug)]
struct LoadedText {
    text: String,
    source_path: Option<PathBuf>,
}

fn load_text(raw: &str, input: InputKind, what: String) -> Result<LoadedText, anyhow::Error> {
    load_text_with_stdin(raw, input, what, || {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|err| anyhow!("Failed to read stdin: {err}"))?;
        Ok(text)
    })
}

fn load_text_with_stdin<F>(
    raw: &str,
    input: InputKind,
    what: String,
    read_stdin: F,
) -> Result<LoadedText, anyhow::Error>
where
    F: FnOnce() -> Result<String, anyhow::Error>,
{
    if let Some(path) = raw.strip_prefix('@') {
        if path.is_empty() {
            bail!("{what} must specify a path after '@'");
        }
        if path == "-" {
            if matches!(input, InputKind::Env) {
                bail!("{what} does not support @- from the environment");
            }
            return Ok(LoadedText {
                text: read_stdin()?,
                source_path: None,
            });
        }
        let resolved = resolve_input_path(path)?;
        let text = std::fs::read_to_string(&resolved)
            .map_err(|err| anyhow!("Failed to read {what} from {}: {err}", resolved.display()))?;
        Ok(LoadedText {
            text,
            source_path: Some(resolved),
        })
    } else {
        Ok(LoadedText {
            text: raw.to_string(),
            source_path: None,
        })
    }
}

fn resolve_input_path(raw: &str) -> Result<PathBuf, anyhow::Error> {
    let expanded = expand_path(raw);
    if expanded.is_absolute() {
        Ok(expanded)
    } else {
        Ok(std::env::current_dir()?.join(expanded))
    }
}

fn expand_path(raw: &str) -> PathBuf {
    PathBuf::from(shellexpand::tilde(raw.trim()).to_string())
}

fn parse_json_value(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<(serde_json::Value, Option<PathBuf>), anyhow::Error> {
    let loaded = load_text(raw, input, what.clone())?;
    let value = serde_json::from_str::<serde_json::Value>(&loaded.text).map_err(|err| {
        anyhow!(
            "Invalid JSON for {what}{}: {err}",
            loaded
                .source_path
                .as_ref()
                .map(|path| format!(" in {}", path.display()))
                .unwrap_or_default()
        )
    })?;
    Ok((value, loaded.source_path))
}

fn parse_string_vec(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<Option<Vec<String>>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }
    let (value, _) = parse_json_value(raw, input, what.clone())?;
    if value.is_null() {
        return Ok(None);
    }
    match value {
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(|value| match value {
                serde_json::Value::String(text) => Ok(text),
                _ => bail!("{what} must be a JSON array of strings"),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        _ => bail!("{what} must be a JSON array of strings"),
    }
}

fn parse_json_map(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<Option<HashMap<String, serde_json::Value>>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }
    let (value, _) = parse_json_value(raw, input, what.clone())?;
    if value.is_null() {
        return Ok(None);
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(|err| anyhow!("{what} must be a JSON object: {err}"))
}

fn parse_vega_plugins(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<Option<Vec<String>>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }
    let (value, source_path) = parse_json_value(raw, input, what.clone())?;
    if value.is_null() {
        return Ok(None);
    }
    let mut plugins: Vec<String> = match value {
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(|value| match value {
                serde_json::Value::String(text) => Ok(text),
                _ => bail!("{what} must be a JSON array of strings"),
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => bail!("{what} must be a JSON array of strings"),
    };

    if let Some(path) = source_path {
        if let Some(dir) = path.parent() {
            resolve_plugin_paths_relative_to(dir, &mut plugins);
        }
    }

    Ok(Some(plugins))
}

fn resolve_plugin_paths_relative_to(dir: &Path, plugins: &mut [String]) {
    for plugin in plugins.iter_mut() {
        if plugin.contains("://")
            || plugin.contains('\n')
            || plugin.starts_with("export")
            || plugin.starts_with("import")
        {
            continue;
        }

        let path = Path::new(plugin.as_str());
        if path.is_relative() {
            let normalized: PathBuf = dir.join(path).components().collect();
            *plugin = normalized.to_string_lossy().to_string();
        }
    }
}

fn parse_format_locale(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<Option<FormatLocale>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }

    if raw.starts_with('@') {
        let (value, _) = parse_json_value(raw, input, what.clone())?;
        return parse_locale_value(value, what).map(Some);
    }

    let trimmed = raw.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('"') {
        let value = serde_json::from_str::<serde_json::Value>(trimmed)
            .map_err(|err| anyhow!("Invalid JSON for {what}: {err}"))?;
        if value.is_null() {
            return Ok(None);
        }
        return parse_locale_value(value, what).map(Some);
    }

    Ok(Some(FormatLocale::Name(raw.to_string())))
}

fn parse_time_format_locale(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<Option<TimeFormatLocale>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }

    if raw.starts_with('@') {
        let (value, _) = parse_json_value(raw, input, what.clone())?;
        return parse_time_locale_value(value, what).map(Some);
    }

    let trimmed = raw.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('"') {
        let value = serde_json::from_str::<serde_json::Value>(trimmed)
            .map_err(|err| anyhow!("Invalid JSON for {what}: {err}"))?;
        if value.is_null() {
            return Ok(None);
        }
        return parse_time_locale_value(value, what).map(Some);
    }

    Ok(Some(TimeFormatLocale::Name(raw.to_string())))
}

fn parse_locale_value(
    value: serde_json::Value,
    what: String,
) -> Result<FormatLocale, anyhow::Error> {
    match value {
        serde_json::Value::String(name) => Ok(FormatLocale::Name(name)),
        serde_json::Value::Object(_) => Ok(FormatLocale::Object(value)),
        _ => bail!("{what} must be a locale name string or JSON object"),
    }
}

fn parse_time_locale_value(
    value: serde_json::Value,
    what: String,
) -> Result<TimeFormatLocale, anyhow::Error> {
    match value {
        serde_json::Value::String(name) => Ok(TimeFormatLocale::Name(name)),
        serde_json::Value::Object(_) => Ok(TimeFormatLocale::Object(value)),
        _ => bail!("{what} must be a locale name string or JSON object"),
    }
}

fn parse_google_fonts(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<Option<Vec<GoogleFontRequest>>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }

    let (value, _) = parse_json_value(raw, input, what.clone())?;
    if value.is_null() {
        return Ok(None);
    }

    match value {
        serde_json::Value::Array(items) => items
            .into_iter()
            .map(|item| match item {
                serde_json::Value::String(spec) => parse_google_font_arg(&spec),
                serde_json::Value::Object(_) => {
                    serde_json::from_value(item).map_err(|err| anyhow!("{what}: {err}"))
                }
                _ => bail!("{what} must be a JSON array of strings or objects"),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        _ => bail!("{what} must be a JSON array of strings or objects"),
    }
}

fn parse_google_font_arg(s: &str) -> Result<GoogleFontRequest, anyhow::Error> {
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
        let (weight_str, style) = if let Some(weight) = token.strip_suffix("italic") {
            (weight, FontStyle::Italic)
        } else {
            (token, FontStyle::Normal)
        };
        let weight: u16 = weight_str.parse().map_err(|_| {
            anyhow!(
                "Invalid font variant '{token}' in google font '{s}'. Expected format: 400, 700italic, etc."
            )
        })?;
        variants.push(VariantRequest { weight, style });
    }

    Ok(GoogleFontRequest {
        family: family.to_string(),
        variants: if variants.is_empty() {
            None
        } else {
            Some(variants)
        },
    })
}

fn derive_data_access_state(
    allowed_base_urls: Option<Vec<String>>,
) -> (DataAccessMode, Option<Vec<String>>) {
    match allowed_base_urls {
        None => (DataAccessMode::Default, None),
        Some(urls) if urls.is_empty() => (DataAccessMode::None, None),
        Some(urls) if urls.len() == 1 && urls[0] == "*" => (DataAccessMode::All, None),
        Some(urls) => (DataAccessMode::Allowlist, Some(urls)),
    }
}

fn finalize_allowed_base_urls(
    data_access: DataAccessMode,
    allowed_base_urls: Option<Vec<String>>,
) -> Result<Option<Vec<String>>, anyhow::Error> {
    match data_access {
        DataAccessMode::Default => {
            if allowed_base_urls.is_some() {
                bail!("allowed_base_urls may only be set when data_access=allowlist");
            }
            Ok(None)
        }
        DataAccessMode::None => {
            if allowed_base_urls.is_some() {
                bail!("allowed_base_urls may only be set when data_access=allowlist");
            }
            Ok(Some(vec![]))
        }
        DataAccessMode::All => {
            if allowed_base_urls.is_some() {
                bail!("allowed_base_urls may only be set when data_access=allowlist");
            }
            Ok(Some(vec!["*".to_string()]))
        }
        DataAccessMode::Allowlist => {
            let urls = allowed_base_urls
                .ok_or_else(|| anyhow!("data_access=allowlist requires allowed_base_urls"))?;
            if urls.is_empty() {
                bail!("allowed_base_urls must not be empty when data_access=allowlist");
            }
            if urls.len() == 1 && urls[0] == "*" {
                bail!("Use data_access=all instead of allowed_base_urls=[\"*\"]");
            }
            Ok(Some(urls))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::collections::HashSet;
    use std::io::Write;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Env vars that mirror a CLI flag; tracked by SETTING_PAIRS.
    fn all_env_vars() -> Vec<&'static str> {
        SETTING_PAIRS.iter().map(|(_, env)| *env).collect()
    }

    /// Extra env vars the guard must save/restore/clear but that are
    /// intentionally absent from SETTING_PAIRS (they have no matching
    /// CLI flag). `PORT` is read as a PaaS fallback for `VLC_PORT`.
    const GUARD_EXTRA_ENV_VARS: &[&str] = &["PORT"];

    struct EnvGuard {
        saved: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn new() -> Self {
            let saved = all_env_vars()
                .into_iter()
                .chain(GUARD_EXTRA_ENV_VARS.iter().copied())
                .map(|name| (name, std::env::var(name).ok()))
                .collect();
            Self { saved }
        }

        fn clear_all(&self) {
            for (name, _) in &self.saved {
                std::env::remove_var(name);
            }
        }

        fn set(&self, key: &str, value: &str) {
            std::env::set_var(key, value);
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, original) in &self.saved {
                match original {
                    Some(val) => std::env::set_var(name, val),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

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
        let actual_envs: HashSet<&str> = all_env_vars().into_iter().collect();
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

    #[test]
    fn test_parse_string_vec_supports_inline_file_and_null() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "[\"https://example.com/\",\"/data/\"]").unwrap();

        assert_eq!(
            parse_string_vec(
                "[\"https://example.com/\"]",
                InputKind::Cli,
                "test".to_string()
            )
            .unwrap(),
            Some(vec!["https://example.com/".to_string()])
        );
        assert_eq!(
            parse_string_vec(
                &format!("@{}", file.path().display()),
                InputKind::Cli,
                "test".to_string()
            )
            .unwrap(),
            Some(vec![
                "https://example.com/".to_string(),
                "/data/".to_string()
            ])
        );
        assert_eq!(
            parse_string_vec("null", InputKind::Cli, "test".to_string()).unwrap(),
            None
        );
    }

    #[test]
    fn test_load_text_supports_cli_stdin_but_not_env() {
        let loaded = load_text_with_stdin("@-", InputKind::Cli, "test".to_string(), || {
            Ok("[1,2,3]".to_string())
        })
        .unwrap();
        assert_eq!(loaded.text, "[1,2,3]");

        let err = load_text_with_stdin("@-", InputKind::Env, "test".to_string(), || {
            Ok(String::new())
        })
        .unwrap_err();
        assert!(err.to_string().contains("does not support @-"));
    }

    #[test]
    fn test_parse_google_fonts_accepts_shorthand_and_objects() {
        let parsed = parse_google_fonts(
            r#"["Roboto:400,700italic",{"family":"Inter","variants":[{"weight":400,"style":"Normal"}]}]"#,
            InputKind::Cli,
            "google_fonts".to_string(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].family, "Roboto");
        assert_eq!(parsed[0].variants.as_ref().unwrap().len(), 2);
        assert_eq!(parsed[1].family, "Inter");
    }

    #[test]
    fn test_parse_google_fonts_rejects_invalid_json_and_missing_files() {
        assert!(parse_google_fonts("{", InputKind::Cli, "google_fonts".to_string()).is_err());
        assert!(parse_google_fonts(
            "@/definitely/missing.json",
            InputKind::Cli,
            "google_fonts".to_string()
        )
        .is_err());
    }

    #[test]
    fn test_parse_locale_supports_inline_name_object_and_file() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"decimal":",","thousands":"."}}"#).unwrap();

        let name = parse_format_locale("de-DE", InputKind::Cli, "format".to_string())
            .unwrap()
            .unwrap();
        assert!(matches!(name, FormatLocale::Name(ref n) if n == "de-DE"));

        let object = parse_format_locale(
            r#"{"decimal":",","thousands":"."}"#,
            InputKind::Cli,
            "format".to_string(),
        )
        .unwrap()
        .unwrap();
        assert!(matches!(object, FormatLocale::Object(_)));

        let from_file = parse_format_locale(
            &format!("@{}", file.path().display()),
            InputKind::Cli,
            "format".to_string(),
        )
        .unwrap()
        .unwrap();
        assert!(matches!(from_file, FormatLocale::Object(_)));
    }

    #[test]
    fn test_parse_vega_plugins_resolves_relative_paths_from_fragment_file() {
        let dir = tempfile::tempdir().unwrap();
        let plugin_path = dir.path().join("plugin.js");
        std::fs::write(&plugin_path, "export default function(vega) {}").unwrap();
        let fragment_path = dir.path().join("plugins.json");
        std::fs::write(&fragment_path, "[\"./plugin.js\"]").unwrap();

        let parsed = parse_vega_plugins(
            &format!("@{}", fragment_path.display()),
            InputKind::Cli,
            "vega_plugins".to_string(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(parsed, vec![plugin_path.to_string_lossy().to_string()]);
    }

    #[test]
    fn test_resolve_settings_env_and_cli_precedence_and_null_clearing() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");
        guard.set(ENV_API_KEY, "env-secret");
        guard.set(ENV_LOG_FILTER, "vl_convert=debug");
        guard.set(ENV_DEFAULT_THEME, "dark");

        let cli = parse_cli(&[
            "--api-key",
            "null",
            "--log-filter",
            "null",
            "--log-level=error",
            "--default-theme",
            "null",
        ]);

        let resolved = resolve_settings(cli).unwrap();
        assert_eq!(resolved.serve_config.api_key, None);
        assert_eq!(resolved.log_filter, "vl_convert=error,tower_http=error");
        assert_eq!(resolved.converter_config.default_theme, None);
    }

    #[test]
    fn test_resolve_settings_rejects_invalid_log_filter() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();

        let cli = parse_cli(&["--load-config=false", "--log-filter", "["]);
        let err = resolve_settings(cli).unwrap_err();
        assert!(err.to_string().contains("CLI log_filter"));
    }

    #[test]
    fn test_resolve_settings_loads_config_then_env_then_cli() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();

        let mut config_file = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"{{
                "default_theme": "dark",
                "auto_google_fonts": false,
                "allowed_base_urls": ["https://config.example/"]
            }}"#
        )
        .unwrap();

        guard.set(ENV_DEFAULT_THEME, "env-theme");
        guard.set(ENV_AUTO_GOOGLE_FONTS, "true");

        let cli = parse_cli(&[
            "--config",
            &config_file.path().display().to_string(),
            "--default-theme",
            "cli-theme",
        ]);
        let resolved = resolve_settings(cli).unwrap();

        assert_eq!(
            resolved.converter_config.default_theme.as_deref(),
            Some("cli-theme")
        );
        assert!(resolved.converter_config.auto_google_fonts);
        assert_eq!(
            resolved.converter_config.allowed_base_urls,
            Some(vec!["https://config.example/".to_string()])
        );
    }

    #[test]
    fn test_allowed_base_urls_null_clears_inherited_allowlist() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();

        let mut config_file = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"{{
                "allowed_base_urls": ["https://config.example/"]
            }}"#
        )
        .unwrap();

        let cli = parse_cli(&[
            "--config",
            &config_file.path().display().to_string(),
            "--allowed-base-urls",
            "null",
        ]);
        let resolved = resolve_settings(cli).unwrap();

        assert_eq!(resolved.converter_config.allowed_base_urls, None);
    }

    #[test]
    fn test_resolve_settings_data_access_validation_matrix() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();

        let err = resolve_settings(parse_cli(&[
            "--load-config=false",
            "--data-access",
            "allowlist",
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("allowlist"));

        let err = resolve_settings(parse_cli(&[
            "--load-config=false",
            "--data-access",
            "none",
            "--allowed-base-urls",
            r#"["https://example.com/"]"#,
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("cannot combine"));

        let err = resolve_settings(parse_cli(&[
            "--load-config=false",
            "--allowed-base-urls",
            "[]",
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("must not be empty"));

        let err = resolve_settings(parse_cli(&[
            "--load-config=false",
            "--allowed-base-urls",
            r#"["*"]"#,
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("data_access=all"));

        let resolved =
            resolve_settings(parse_cli(&["--load-config=false", "--data-access", "all"])).unwrap();
        assert_eq!(
            resolved.converter_config.allowed_base_urls,
            Some(vec!["*".to_string()])
        );
    }

    #[test]
    fn test_resolve_settings_validates_bootstrap_conflicts_by_source() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_CONFIG, "/tmp/config.jsonc");
        guard.set(ENV_LOAD_CONFIG, "false");
        let err = resolve_settings(parse_cli(&[])).unwrap_err();
        assert!(err.to_string().contains("environment cannot set both"));
    }

    #[test]
    fn test_resolve_settings_log_filter_wins_over_log_level() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();

        let resolved = resolve_settings(parse_cli(&[
            "--load-config=false",
            "--log-level=debug",
            "--log-filter",
            "vl_convert=info",
        ]))
        .unwrap();
        assert_eq!(resolved.log_filter, "vl_convert=info");
    }

    #[test]
    fn test_resolve_settings_validates_locale_names_at_startup() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();

        let err = resolve_settings(parse_cli(&[
            "--load-config=false",
            "--default-format-locale",
            "not-a-real-locale",
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("No built-in format locale named"));
    }

    #[test]
    fn test_resolve_settings_port_default_3000() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");

        let resolved = resolve_settings(parse_cli(&[])).unwrap();
        assert_eq!(resolved.serve_config.port, 3000);
    }

    #[test]
    fn test_resolve_settings_port_fallback_to_paas_port() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");
        guard.set("PORT", "7777");

        let resolved = resolve_settings(parse_cli(&[])).unwrap();
        assert_eq!(resolved.serve_config.port, 7777);
    }

    #[test]
    fn test_resolve_settings_port_vlc_env_beats_paas_port() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");
        guard.set(ENV_PORT, "8888");
        guard.set("PORT", "7777");

        let resolved = resolve_settings(parse_cli(&[])).unwrap();
        assert_eq!(resolved.serve_config.port, 8888);
    }

    #[test]
    fn test_resolve_settings_port_flag_beats_both_env_vars() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");
        guard.set(ENV_PORT, "8888");
        guard.set("PORT", "7777");

        let resolved = resolve_settings(parse_cli(&["--port", "9999"])).unwrap();
        assert_eq!(resolved.serve_config.port, 9999);
    }

    #[test]
    fn test_resolve_settings_port_invalid_paas_port_falls_through() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");
        guard.set("PORT", "not-a-number");

        let resolved = resolve_settings(parse_cli(&[])).unwrap();
        assert_eq!(
            resolved.serve_config.port, 3000,
            "invalid PORT should be silently ignored"
        );
    }
}

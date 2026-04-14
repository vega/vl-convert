use clap::{Parser, ValueEnum};
use vl_convert_google_fonts::{FontStyle, VariantRequest};
use vl_convert_rs::anyhow;
use vl_convert_rs::converter::{BaseUrlSetting, GoogleFontRequest, MissingFontsPolicy, VlcConfig};

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
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

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
enum LogLevel {
    Error,
    #[default]
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    fn to_tracing_filter(self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
        }
    }
}

#[derive(Debug, Parser)]
#[command(version, name = "vl-convert-server")]
#[command(about = "HTTP server for converting Vega-Lite and Vega specifications to static images")]
struct Cli {
    /// Path to JSONC converter config file
    #[arg(long)]
    vlc_config: Option<String>,

    /// Disable loading the default vlc-config file
    #[arg(long, conflicts_with = "vlc_config")]
    no_vlc_config: bool,

    /// Custom base URL for resolving relative data paths in Vega specs
    #[arg(long)]
    base_url: Option<String>,

    /// Disable relative path resolution
    #[arg(long, conflicts_with = "base_url")]
    no_base_url: bool,

    /// Allowed base URL pattern for external data access. May be specified multiple times.
    #[arg(long = "allowed-base-url")]
    allowed_base_url: Vec<String>,

    /// Disable all external data access
    #[arg(long, conflicts_with = "allowed_base_url")]
    no_allowed_urls: bool,

    /// Register a font from Google Fonts. May be specified multiple times.
    #[arg(long = "google-font")]
    google_font: Vec<String>,

    /// Automatically download missing fonts from Google Fonts
    #[arg(long)]
    auto_google_fonts: bool,

    /// Embed locally installed fonts as base64 @font-face in HTML and SVG output
    #[arg(long)]
    embed_local_fonts: bool,

    /// Disable font subsetting
    #[arg(long)]
    no_subset_fonts: bool,

    /// Missing-font behavior: fallback silently, warn, or error
    #[arg(long, value_enum)]
    missing_fonts: Option<MissingFontsArg>,

    /// Maximum V8 heap size per worker in megabytes [default: 0 = no limit]
    #[arg(long)]
    max_v8_heap_size_mb: Option<usize>,

    /// Maximum V8 execution time in seconds [default: 0 = no limit]
    #[arg(long)]
    max_v8_execution_time_secs: Option<u64>,

    /// Run V8 garbage collection after each conversion
    #[arg(long)]
    gc_after_conversion: bool,

    /// Vega plugin. May be specified multiple times.
    #[arg(long = "vega-plugin")]
    vega_plugin: Vec<String>,

    /// Domains allowed for HTTP imports in plugins. May be specified multiple times.
    #[arg(long = "plugin-import-domains")]
    plugin_import_domains: Vec<String>,

    /// Additional directory to search for fonts
    #[arg(long, env = "VLC_FONT_DIR")]
    font_dir: Option<String>,

    /// Log level for Vega/Vega-Lite messages
    #[arg(long, value_enum, default_value_t = LogLevel::Warn)]
    log_level: LogLevel,

    /// Bind address
    #[arg(long, env = "VLC_HOST", default_value = "127.0.0.1")]
    host: String,

    /// Port
    #[arg(long, env = "VLC_PORT", default_value_t = 3000)]
    port: u16,

    /// Number of converter worker threads [default: min(CPU count, 4)]
    #[arg(long, env = "VLC_WORKERS")]
    workers: Option<usize>,

    /// Maximum concurrent ephemeral workers for per-request plugins
    /// [default: same as --workers when plugins are enabled]
    #[arg(long, env = "VLC_MAX_EPHEMERAL_WORKERS")]
    max_ephemeral_workers: Option<usize>,

    /// API key for Bearer token authentication
    #[arg(long, env = "VLC_API_KEY")]
    api_key: Option<String>,

    /// Allowed CORS origin(s), comma-separated or "*"
    #[arg(long, env = "VLC_CORS_ORIGIN")]
    cors_origin: Option<String>,

    /// Maximum simultaneous in-flight requests [default: unlimited]
    #[arg(long, env = "VLC_MAX_CONCURRENT_REQUESTS")]
    max_concurrent_requests: Option<usize>,

    /// HTTP request timeout in seconds
    #[arg(long, env = "VLC_REQUEST_TIMEOUT_SECS", default_value_t = 30)]
    request_timeout_secs: u64,

    /// Graceful shutdown drain timeout in seconds
    #[arg(long, env = "VLC_DRAIN_TIMEOUT_SECS", default_value_t = 30)]
    drain_timeout_secs: u64,

    /// Maximum request body size in megabytes
    #[arg(long, env = "VLC_MAX_BODY_SIZE_MB", default_value_t = 50)]
    max_body_size_mb: usize,

    /// Return only HTTP status codes on error (no error messages)
    #[arg(long, env = "VLC_OPAQUE_ERRORS")]
    opaque_errors: bool,

    /// Reject requests without a User-Agent header
    #[arg(long, env = "VLC_REQUIRE_USER_AGENT")]
    require_user_agent: bool,

    /// Log output format
    #[arg(long, env = "VLC_LOG_FORMAT", value_enum, default_value_t = vl_convert_server::LogFormat::Text)]
    log_format: vl_convert_server::LogFormat,

    /// Conversion time budget per IP in ms/min (disabled if not set)
    #[arg(long, env = "VLC_PER_IP_BUDGET_MS")]
    per_ip_budget_ms: Option<i64>,

    /// Total conversion time budget for the server in ms/min (disabled if not set)
    #[arg(long, env = "VLC_GLOBAL_BUDGET_MS")]
    global_budget_ms: Option<i64>,

    /// Per-request budget hold in ms (reserved upfront, adjusted to actual
    /// conversion time after completion)
    #[arg(long, env = "VLC_BUDGET_HOLD_MS", default_value_t = 1000)]
    budget_hold_ms: i64,

    /// Enable admin API on 127.0.0.1:<port> for dynamic budget updates
    #[arg(long, env = "VLC_ADMIN_PORT")]
    admin_port: Option<u16>,

    /// Trust X-Forwarded-For and X-Real-IP headers for client IP extraction
    #[arg(long, env = "VLC_TRUST_PROXY")]
    trust_proxy: bool,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    vl_convert_server::init_tracing(cli.log_level.to_tracing_filter(), cli.log_format);

    // Build converter config
    let mut config = resolve_vlc_config(cli.vlc_config.as_deref(), cli.no_vlc_config)?;

    if cli.base_url.is_some() || cli.no_base_url {
        config.base_url = if cli.no_base_url {
            BaseUrlSetting::Disabled
        } else if let Some(ref url) = cli.base_url {
            BaseUrlSetting::Custom(url.clone())
        } else {
            BaseUrlSetting::Default
        };
    }
    if !cli.allowed_base_url.is_empty() || cli.no_allowed_urls {
        config.allowed_base_urls = if cli.no_allowed_urls {
            Some(vec![])
        } else {
            Some(cli.allowed_base_url.clone())
        };
    }
    if cli.auto_google_fonts {
        config.auto_google_fonts = true;
    }
    if cli.embed_local_fonts {
        config.embed_local_fonts = true;
    }
    if cli.no_subset_fonts {
        config.subset_fonts = false;
    }
    if let Some(ref mf) = cli.missing_fonts {
        config.missing_fonts = mf.to_policy();
    }
    if let Some(heap) = cli.max_v8_heap_size_mb {
        config.max_v8_heap_size_mb = heap;
    }
    if let Some(timeout) = cli.max_v8_execution_time_secs {
        config.max_v8_execution_time_secs = timeout;
    }
    if cli.gc_after_conversion {
        config.gc_after_conversion = true;
    }
    if let Some(max_eph) = cli.max_ephemeral_workers {
        config.max_ephemeral_workers = max_eph;
    }
    if !cli.vega_plugin.is_empty() {
        config.vega_plugins = Some(cli.vega_plugin.clone());
    }
    let plugin_import_domains = flatten_plugin_domains(&cli.plugin_import_domains);
    if !plugin_import_domains.is_empty() {
        config.plugin_import_domains = plugin_import_domains;
    }
    let google_fonts = parse_google_font_requests(&cli.google_font)?;
    if google_fonts.is_some() {
        config.google_fonts = google_fonts;
    }

    // Register extra font directory
    if let Some(ref dir) = cli.font_dir {
        vl_convert_rs::text::register_font_directory(dir)?;
    }

    // Resolve worker count: CLI flag > vlc-config > CPU count
    let num_workers = cli
        .workers
        .or(if config.num_workers > 0 {
            Some(config.num_workers)
        } else {
            None
        })
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get().min(4))
                .unwrap_or(1)
        });
    config.num_workers = num_workers;

    // Clamp V8 timeout to HTTP timeout
    if cli.request_timeout_secs > 0 {
        let current = config.max_v8_execution_time_secs;
        if current == 0 || current > cli.request_timeout_secs {
            config.max_v8_execution_time_secs = cli.request_timeout_secs;
        }
    }

    let serve_config = vl_convert_server::ServeConfig {
        host: cli.host,
        port: cli.port,
        api_key: cli.api_key,
        cors_origin: cli.cors_origin,
        max_concurrent_requests: cli.max_concurrent_requests,
        request_timeout_secs: cli.request_timeout_secs,
        drain_timeout_secs: cli.drain_timeout_secs,
        max_body_size_mb: cli.max_body_size_mb,
        opaque_errors: cli.opaque_errors,
        require_user_agent: cli.require_user_agent,
        log_format: cli.log_format,
        per_ip_budget_ms: cli.per_ip_budget_ms,
        global_budget_ms: cli.global_budget_ms,
        budget_hold_ms: cli.budget_hold_ms,
        admin_port: cli.admin_port,
        trust_proxy: cli.trust_proxy,
    };

    vl_convert_server::run(config, serve_config).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    const ALL_VLC_VARS: &[&str] = &[
        "VLC_HOST",
        "VLC_PORT",
        "VLC_API_KEY",
        "VLC_CORS_ORIGIN",
        "VLC_WORKERS",
        "VLC_MAX_EPHEMERAL_WORKERS",
        "VLC_MAX_CONCURRENT_REQUESTS",
        "VLC_REQUEST_TIMEOUT_SECS",
        "VLC_DRAIN_TIMEOUT_SECS",
        "VLC_MAX_BODY_SIZE_MB",
        "VLC_OPAQUE_ERRORS",
        "VLC_REQUIRE_USER_AGENT",
        "VLC_LOG_FORMAT",
        "VLC_PER_IP_BUDGET_MS",
        "VLC_GLOBAL_BUDGET_MS",
        "VLC_BUDGET_HOLD_MS",
        "VLC_ADMIN_PORT",
        "VLC_TRUST_PROXY",
        "VLC_FONT_DIR",
    ];

    struct EnvGuard {
        saved: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn new(vars: &[&'static str]) -> Self {
            let saved = vars
                .iter()
                .map(|&name| (name, std::env::var(name).ok()))
                .collect();
            Self { saved }
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

    #[test]
    fn test_cli_defaults() {
        let guard = EnvGuard::new(ALL_VLC_VARS);
        for &var in ALL_VLC_VARS {
            std::env::remove_var(var);
        }

        let cli = Cli::try_parse_from(["vl-convert-server"]).unwrap();

        assert_eq!(cli.host, "127.0.0.1");
        assert_eq!(cli.port, 3000);
        assert_eq!(cli.request_timeout_secs, 30);
        assert_eq!(cli.max_body_size_mb, 50);
        assert_eq!(cli.budget_hold_ms, 1000);
        assert!(!cli.trust_proxy);
        assert!(!cli.opaque_errors);
        assert!(cli.api_key.is_none());

        drop(guard);
    }

    #[test]
    fn test_cli_env_var_parsing() {
        let guard = EnvGuard::new(ALL_VLC_VARS);
        for &var in ALL_VLC_VARS {
            std::env::remove_var(var);
        }

        guard.set("VLC_PORT", "8080");
        guard.set("VLC_API_KEY", "my-secret");
        guard.set("VLC_TRUST_PROXY", "true");

        let cli = Cli::try_parse_from(["vl-convert-server"]).unwrap();

        assert_eq!(cli.port, 8080);
        assert_eq!(cli.api_key, Some("my-secret".to_string()));
        assert!(cli.trust_proxy);

        drop(guard);
    }

    #[test]
    fn test_cli_flag_overrides_env() {
        let guard = EnvGuard::new(ALL_VLC_VARS);
        for &var in ALL_VLC_VARS {
            std::env::remove_var(var);
        }

        guard.set("VLC_PORT", "8080");

        let cli = Cli::try_parse_from(["vl-convert-server", "--port", "9090"]).unwrap();

        assert_eq!(cli.port, 9090);

        drop(guard);
    }

    #[test]
    fn test_cli_conflicts() {
        let guard = EnvGuard::new(ALL_VLC_VARS);
        for &var in ALL_VLC_VARS {
            std::env::remove_var(var);
        }

        let result = Cli::try_parse_from([
            "vl-convert-server",
            "--vlc-config",
            "foo.json",
            "--no-vlc-config",
        ]);

        assert!(result.is_err());

        drop(guard);
    }
}

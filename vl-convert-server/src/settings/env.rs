pub(super) const ENV_CONFIG: &str = "VLC_CONFIG";
pub(super) const ENV_LOAD_CONFIG: &str = "VLC_LOAD_CONFIG";
pub(super) const ENV_BASE_URL: &str = "VLC_BASE_URL";
pub(super) const ENV_DATA_ACCESS: &str = "VLC_DATA_ACCESS";
pub(super) const ENV_ALLOWED_BASE_URLS: &str = "VLC_ALLOWED_BASE_URLS";
pub(super) const ENV_GOOGLE_FONTS: &str = "VLC_GOOGLE_FONTS";
pub(super) const ENV_AUTO_GOOGLE_FONTS: &str = "VLC_AUTO_GOOGLE_FONTS";
pub(super) const ENV_ALLOW_GOOGLE_FONTS: &str = "VLC_ALLOW_GOOGLE_FONTS";
pub(super) const ENV_EMBED_LOCAL_FONTS: &str = "VLC_EMBED_LOCAL_FONTS";
pub(super) const ENV_SUBSET_FONTS: &str = "VLC_SUBSET_FONTS";
pub(super) const ENV_MISSING_FONTS: &str = "VLC_MISSING_FONTS";
pub(super) const ENV_MAX_V8_HEAP_SIZE_MB: &str = "VLC_MAX_V8_HEAP_SIZE_MB";
pub(super) const ENV_MAX_V8_EXECUTION_TIME_SECS: &str = "VLC_MAX_V8_EXECUTION_TIME_SECS";
pub(super) const ENV_GC_AFTER_CONVERSION: &str = "VLC_GC_AFTER_CONVERSION";
pub(super) const ENV_VEGA_PLUGINS: &str = "VLC_VEGA_PLUGINS";
pub(super) const ENV_PLUGIN_IMPORT_DOMAINS: &str = "VLC_PLUGIN_IMPORT_DOMAINS";
pub(super) const ENV_ALLOW_PER_REQUEST_PLUGINS: &str = "VLC_ALLOW_PER_REQUEST_PLUGINS";
pub(super) const ENV_MAX_EPHEMERAL_WORKERS: &str = "VLC_MAX_EPHEMERAL_WORKERS";
pub(super) const ENV_PER_REQUEST_PLUGIN_IMPORT_DOMAINS: &str =
    "VLC_PER_REQUEST_PLUGIN_IMPORT_DOMAINS";
pub(super) const ENV_DEFAULT_THEME: &str = "VLC_DEFAULT_THEME";
pub(super) const ENV_DEFAULT_FORMAT_LOCALE: &str = "VLC_DEFAULT_FORMAT_LOCALE";
pub(super) const ENV_DEFAULT_TIME_FORMAT_LOCALE: &str = "VLC_DEFAULT_TIME_FORMAT_LOCALE";
pub(super) const ENV_THEMES: &str = "VLC_THEMES";
pub(super) const ENV_GOOGLE_FONTS_CACHE_SIZE_MB: &str = "VLC_GOOGLE_FONTS_CACHE_SIZE_MB";
pub(super) const ENV_FONT_DIR: &str = "VLC_FONT_DIR";
pub(super) const ENV_LOG_LEVEL: &str = "VLC_LOG_LEVEL";
pub(super) const ENV_LOG_FILTER: &str = "VLC_LOG_FILTER";
pub(super) const ENV_LOG_FORMAT: &str = "VLC_LOG_FORMAT";
pub(super) const ENV_HOST: &str = "VLC_HOST";
pub(super) const ENV_PORT: &str = "VLC_PORT";
pub(super) const ENV_WORKERS: &str = "VLC_WORKERS";
pub(super) const ENV_API_KEY: &str = "VLC_API_KEY";
pub(super) const ENV_ADMIN_API_KEY: &str = "VLC_ADMIN_API_KEY";
pub(super) const ENV_CORS_ORIGIN: &str = "VLC_CORS_ORIGIN";
pub(super) const ENV_MAX_CONCURRENT_REQUESTS: &str = "VLC_MAX_CONCURRENT_REQUESTS";
pub(super) const ENV_REQUEST_TIMEOUT_SECS: &str = "VLC_REQUEST_TIMEOUT_SECS";
pub(super) const ENV_DRAIN_TIMEOUT_SECS: &str = "VLC_DRAIN_TIMEOUT_SECS";
pub(super) const ENV_RECONFIG_DRAIN_TIMEOUT_SECS: &str = "VLC_RECONFIG_DRAIN_TIMEOUT_SECS";
pub(super) const ENV_MAX_BODY_SIZE_MB: &str = "VLC_MAX_BODY_SIZE_MB";
pub(super) const ENV_OPAQUE_ERRORS: &str = "VLC_OPAQUE_ERRORS";
pub(super) const ENV_REQUIRE_USER_AGENT: &str = "VLC_REQUIRE_USER_AGENT";
pub(super) const ENV_PER_IP_BUDGET_MS: &str = "VLC_PER_IP_BUDGET_MS";
pub(super) const ENV_GLOBAL_BUDGET_MS: &str = "VLC_GLOBAL_BUDGET_MS";
pub(super) const ENV_BUDGET_HOLD_MS: &str = "VLC_BUDGET_HOLD_MS";
pub(super) const ENV_ADMIN_PORT: &str = "VLC_ADMIN_PORT";
pub(super) const ENV_TRUST_PROXY: &str = "VLC_TRUST_PROXY";
pub(super) const ENV_UNIX_SOCKET: &str = "VLC_UNIX_SOCKET";
pub(super) const ENV_ADMIN_UNIX_SOCKET: &str = "VLC_ADMIN_UNIX_SOCKET";
pub(super) const ENV_SOCKET_MODE: &str = "VLC_SOCKET_MODE";
pub(super) const ENV_READY_JSON: &str = "VLC_READY_JSON";
pub(super) const ENV_EXIT_ON_PARENT_CLOSE: &str = "VLC_EXIT_ON_PARENT_CLOSE";

#[cfg(test)]
pub(super) const SETTING_PAIRS: &[(&str, &str)] = &[
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
    ("google-fonts-cache-size-mb", ENV_GOOGLE_FONTS_CACHE_SIZE_MB),
    ("font-dir", ENV_FONT_DIR),
    ("log-level", ENV_LOG_LEVEL),
    ("log-filter", ENV_LOG_FILTER),
    ("log-format", ENV_LOG_FORMAT),
    ("host", ENV_HOST),
    ("port", ENV_PORT),
    ("workers", ENV_WORKERS),
    ("api-key", ENV_API_KEY),
    ("admin-api-key", ENV_ADMIN_API_KEY),
    ("cors-origin", ENV_CORS_ORIGIN),
    ("max-concurrent-requests", ENV_MAX_CONCURRENT_REQUESTS),
    ("request-timeout-secs", ENV_REQUEST_TIMEOUT_SECS),
    ("drain-timeout-secs", ENV_DRAIN_TIMEOUT_SECS),
    (
        "reconfig-drain-timeout-secs",
        ENV_RECONFIG_DRAIN_TIMEOUT_SECS,
    ),
    ("max-body-size-mb", ENV_MAX_BODY_SIZE_MB),
    ("opaque-errors", ENV_OPAQUE_ERRORS),
    ("require-user-agent", ENV_REQUIRE_USER_AGENT),
    ("per-ip-budget-ms", ENV_PER_IP_BUDGET_MS),
    ("global-budget-ms", ENV_GLOBAL_BUDGET_MS),
    ("budget-hold-ms", ENV_BUDGET_HOLD_MS),
    ("admin-port", ENV_ADMIN_PORT),
    ("trust-proxy", ENV_TRUST_PROXY),
    ("unix-socket", ENV_UNIX_SOCKET),
    ("admin-unix-socket", ENV_ADMIN_UNIX_SOCKET),
    ("socket-mode", ENV_SOCKET_MODE),
    ("ready-json", ENV_READY_JSON),
    ("exit-on-parent-close", ENV_EXIT_ON_PARENT_CLOSE),
];

#[derive(Debug, Default, Clone)]
pub(super) struct EnvValues {
    pub(super) config: Option<String>,
    pub(super) load_config: Option<String>,
    pub(super) base_url: Option<String>,
    pub(super) data_access: Option<String>,
    pub(super) allowed_base_urls: Option<String>,
    pub(super) google_fonts: Option<String>,
    pub(super) auto_google_fonts: Option<String>,
    pub(super) allow_google_fonts: Option<String>,
    pub(super) embed_local_fonts: Option<String>,
    pub(super) subset_fonts: Option<String>,
    pub(super) missing_fonts: Option<String>,
    pub(super) max_v8_heap_size_mb: Option<String>,
    pub(super) max_v8_execution_time_secs: Option<String>,
    pub(super) gc_after_conversion: Option<String>,
    pub(super) vega_plugins: Option<String>,
    pub(super) plugin_import_domains: Option<String>,
    pub(super) allow_per_request_plugins: Option<String>,
    pub(super) max_ephemeral_workers: Option<String>,
    pub(super) per_request_plugin_import_domains: Option<String>,
    pub(super) default_theme: Option<String>,
    pub(super) default_format_locale: Option<String>,
    pub(super) default_time_format_locale: Option<String>,
    pub(super) themes: Option<String>,
    pub(super) google_fonts_cache_size_mb: Option<String>,
    pub(super) font_dir: Option<String>,
    pub(super) log_level: Option<String>,
    pub(super) log_filter: Option<String>,
    pub(super) log_format: Option<String>,
    pub(super) host: Option<String>,
    pub(super) port: Option<String>,
    pub(super) workers: Option<String>,
    pub(super) api_key: Option<String>,
    pub(super) admin_api_key: Option<String>,
    pub(super) cors_origin: Option<String>,
    pub(super) max_concurrent_requests: Option<String>,
    pub(super) request_timeout_secs: Option<String>,
    pub(super) drain_timeout_secs: Option<String>,
    pub(super) reconfig_drain_timeout_secs: Option<String>,
    pub(super) max_body_size_mb: Option<String>,
    pub(super) opaque_errors: Option<String>,
    pub(super) require_user_agent: Option<String>,
    pub(super) per_ip_budget_ms: Option<String>,
    pub(super) global_budget_ms: Option<String>,
    pub(super) budget_hold_ms: Option<String>,
    pub(super) admin_port: Option<String>,
    pub(super) trust_proxy: Option<String>,
    pub(super) unix_socket: Option<String>,
    pub(super) admin_unix_socket: Option<String>,
    pub(super) socket_mode: Option<String>,
    pub(super) ready_json: Option<String>,
    pub(super) exit_on_parent_close: Option<String>,
}

impl EnvValues {
    pub(super) fn from_env() -> Self {
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
            google_fonts_cache_size_mb: env_var(ENV_GOOGLE_FONTS_CACHE_SIZE_MB),
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
            admin_api_key: env_var(ENV_ADMIN_API_KEY),
            cors_origin: env_var(ENV_CORS_ORIGIN),
            max_concurrent_requests: env_var(ENV_MAX_CONCURRENT_REQUESTS),
            request_timeout_secs: env_var(ENV_REQUEST_TIMEOUT_SECS),
            drain_timeout_secs: env_var(ENV_DRAIN_TIMEOUT_SECS),
            reconfig_drain_timeout_secs: env_var(ENV_RECONFIG_DRAIN_TIMEOUT_SECS),
            max_body_size_mb: env_var(ENV_MAX_BODY_SIZE_MB),
            opaque_errors: env_var(ENV_OPAQUE_ERRORS),
            require_user_agent: env_var(ENV_REQUIRE_USER_AGENT),
            per_ip_budget_ms: env_var(ENV_PER_IP_BUDGET_MS),
            global_budget_ms: env_var(ENV_GLOBAL_BUDGET_MS),
            budget_hold_ms: env_var(ENV_BUDGET_HOLD_MS),
            admin_port: env_var(ENV_ADMIN_PORT),
            trust_proxy: env_var(ENV_TRUST_PROXY),
            unix_socket: env_var(ENV_UNIX_SOCKET),
            admin_unix_socket: env_var(ENV_ADMIN_UNIX_SOCKET),
            socket_mode: env_var(ENV_SOCKET_MODE),
            ready_json: env_var(ENV_READY_JSON),
            exit_on_parent_close: env_var(ENV_EXIT_ON_PARENT_CLOSE),
        }
    }
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

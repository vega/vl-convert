use std::collections::HashMap;
use std::path::PathBuf;
use vl_convert_rs::anyhow::{self, anyhow, bail};
use vl_convert_rs::converter::{
    BaseUrlSetting, FormatLocale, GoogleFontRequest, MissingFontsPolicy, TimeFormatLocale,
    VlcConfig,
};
use vl_convert_server::{LogFormat, ServeConfig};

use super::cli::{Cli, DataAccessMode, LogFormatArg, LogLevel, MissingFontsArg};
use super::env::EnvValues;
use super::parsers::{
    expand_path, field_name, parse_base_url, parse_bool, parse_format_locale, parse_google_fonts,
    parse_json_map, parse_log_filter_value, parse_log_format, parse_missing_fonts,
    parse_nullable_i64, parse_nullable_string, parse_nullable_u16, parse_nullable_usize,
    parse_positive_i64, parse_string_vec, parse_time_format_locale, parse_u16, parse_u64,
    parse_usize, parse_vega_plugins, InputKind,
};

#[derive(Debug)]
pub(crate) struct ResolvedSettings {
    pub(crate) converter_config: VlcConfig,
    pub(crate) serve_config: ServeConfig,
    /// CLI-only: post-signal drain deadline before the binary forces
    /// exit. The library doesn't consume this — it's consumed by the
    /// drain watchdog in `main.rs`.
    pub(crate) drain_timeout_secs: u64,
    pub(crate) font_dir: Option<String>,
    pub(crate) log_filter: String,
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
    drain_timeout_secs: u64,
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
            serve_config: ServeConfig::default(),
            drain_timeout_secs: 30,
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
            self.drain_timeout_secs = value;
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
            drain_timeout_secs: self.drain_timeout_secs,
            font_dir: self.font_dir,
            log_filter,
        })
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
    use super::super::env::{
        ENV_API_KEY, ENV_AUTO_GOOGLE_FONTS, ENV_CONFIG, ENV_DEFAULT_THEME, ENV_LOAD_CONFIG,
        ENV_LOG_FILTER, ENV_PORT, SETTING_PAIRS,
    };
    use super::*;
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
        use clap::Parser;
        let mut argv = vec!["vl-convert-server"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv).unwrap()
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
        guard.set("PORT", "not-a-number");

        let resolved = resolve_settings(parse_cli(&[])).unwrap();
        assert_eq!(
            resolved.serve_config.port, 3000,
            "invalid PORT should be silently ignored"
        );
    }
}

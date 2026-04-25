use std::collections::HashMap;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::PathBuf;
use vl_convert_rs::anyhow::{self, anyhow, bail};
use vl_convert_rs::converter::{
    BaseUrlSetting, FormatLocale, GoogleFontRequest, MissingFontsPolicy, TimeFormatLocale,
    VlcConfig,
};
use vl_convert_server::{ListenAddr, LogFormat, ServeConfig};

use super::cli::{Cli, DataAccessMode, LogFormatArg, LogLevel, MissingFontsArg};
use super::env::EnvValues;
use super::parsers::{
    expand_path, field_name, parse_base_url, parse_bool, parse_cache_size_mb, parse_font_dir_list,
    parse_format_locale, parse_google_fonts, parse_json_map, parse_log_filter_value,
    parse_log_format, parse_missing_fonts, parse_nullable_i64, parse_nullable_string,
    parse_nullable_u16, parse_nullable_usize, parse_optional_non_zero_u64,
    parse_optional_non_zero_usize, parse_positive_i64, parse_string_vec, parse_time_format_locale,
    parse_u16, parse_u64, parse_usize, parse_vega_plugins, InputKind,
};

/// Fully-resolved configuration handed from the CLI/env resolution
/// layer to `main()`. Everything in `main.rs` reads from here.
///
/// Two groups of fields:
/// - `converter_config` and `serve_config` are consumed by the
///   library (`VlConverter::with_config`, `build_app`, `serve`).
/// - The rest are CLI-only — they configure behavior that lives in
///   the binary (logging, drain watchdog, readiness signaling,
///   parent-death watcher).
#[derive(Debug)]
pub(crate) struct ResolvedSettings {
    /// Converter configuration passed to `VlConverter::with_config`.
    /// Governs conversion-time behavior: font handling, allowed base
    /// URLs, worker count, etc.
    pub(crate) converter_config: VlcConfig,
    /// HTTP-server configuration passed to `build_app` / `serve`.
    /// Governs listener binding (TCP or UDS), auth, rate limiting,
    /// CORS, logging format, and related wire-level concerns.
    pub(crate) serve_config: ServeConfig,
    /// Seconds the binary waits for in-flight requests to drain after
    /// a shutdown signal (SIGINT / SIGTERM / stdin-EOF) before force-
    /// exiting via `std::process::exit(1)`. The library never calls
    /// exit itself — the drain watchdog in `main.rs` owns that path.
    pub(crate) drain_timeout_secs: u64,
    /// Tracing filter string handed to `init_tracing` (same shape as
    /// `RUST_LOG`, e.g. `"vl_convert=info,tower_http=info"`).
    /// Synthesized from either an explicit filter directive or a
    /// log-level shorthand; resolved once at startup and never
    /// re-read.
    pub(crate) log_filter: String,
    /// When true, the binary emits one `--ready-json` line on stdout
    /// after all listeners have bound. Opt-in; intended for
    /// subprocess parents that block on `read_line()` for a
    /// deterministic readiness signal.
    pub(crate) ready_json: bool,
    /// Parent-death watcher configuration. Tri-state:
    /// - `Some(true)` — watcher always spawned.
    /// - `Some(false)` — watcher never spawned, even on UDS.
    /// - `None` — auto: watcher spawns iff either listener is UDS.
    ///
    /// The watcher reads stdin and triggers graceful shutdown on EOF.
    /// UDS defaults it on because the subprocess-IPC use case
    /// typically has the parent holding the child's stdin; the
    /// parent dying closes stdin, which the child observes as EOF.
    pub(crate) exit_on_parent_close: Option<bool>,
}

#[derive(Debug, Default)]
struct BootstrapOverrides {
    config: Option<PathBuf>,
    load_config: Option<bool>,
}

#[derive(Debug, Default)]
struct Overrides {
    bootstrap: BootstrapOverrides,
    /// Multi-value: each CLI `--font-dir` or env `VLC_FONT_DIR` entry
    /// appends to this list. Empty list = unset at this layer (do not
    /// clear). Seeds `VlcConfig.font_directories` at finalize time.
    font_dir: Vec<PathBuf>,
    log_level: Option<LogLevel>,
    log_filter: Option<Option<String>>,
    log_format: Option<LogFormat>,
    host: Option<String>,
    port: Option<u16>,
    /// `#[cfg(unix)]`-only in practice; on Windows the flag/env parsers
    /// reject `--unix-socket`/`VLC_UNIX_SOCKET` before reaching here, so
    /// this field stays `None`.
    unix_socket: Option<PathBuf>,
    /// Outer `Option` = "was this override set?"; inner `Option` = the
    /// value it was set to (`None` sentinel = explicit disable). Mirrors
    /// the `admin_port` / `admin_listen` `null` convention.
    admin_unix_socket: Option<Option<PathBuf>>,
    socket_mode: Option<u32>,
    ready_json: Option<bool>,
    exit_on_parent_close: Option<Option<bool>>,
    /// `num_workers` is `NonZeroUsize` on the library side; a CLI `0`
    /// is rejected at parse time, so this is a plain Option (no
    /// `0` → `None` shorthand).
    workers: Option<NonZeroUsize>,
    api_key: Option<Option<String>>,
    admin_api_key: Option<Option<String>>,
    cors_origin: Option<Option<String>>,
    max_concurrent_requests: Option<Option<usize>>,
    request_timeout_secs: Option<u64>,
    drain_timeout_secs: Option<u64>,
    reconfig_drain_timeout_secs: Option<u64>,
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
    /// Outer Option = was this override set?; inner Option = the
    /// resolved library value (`None` = unbounded, `Some(NZ)` = cap).
    /// The CLI keeps `0` as a shorthand for "unbounded" which collapses
    /// to inner `None` in the parser.
    max_v8_heap_size_mb: Option<Option<NonZeroUsize>>,
    max_v8_execution_time_secs: Option<Option<NonZeroU64>>,
    gc_after_conversion: Option<bool>,
    vega_plugins: Option<Option<Vec<String>>>,
    plugin_import_domains: Option<Option<Vec<String>>>,
    allow_per_request_plugins: Option<bool>,
    max_ephemeral_workers: Option<Option<NonZeroUsize>>,
    per_request_plugin_import_domains: Option<Option<Vec<String>>>,
    default_theme: Option<Option<String>>,
    default_format_locale: Option<Option<FormatLocale>>,
    default_time_format_locale: Option<Option<TimeFormatLocale>>,
    themes: Option<Option<HashMap<String, serde_json::Value>>>,
    /// Outer Option = override set?; inner Option = the library value.
    /// CLI `0` and env `0` collapse to inner `None` (library default).
    google_fonts_cache_size_mb: Option<Option<NonZeroU64>>,
}

#[derive(Debug)]
struct WorkingSettings {
    converter_config: VlcConfig,
    serve_config: ServeConfig,
    drain_timeout_secs: u64,
    ready_json: bool,
    exit_on_parent_close: Option<bool>,
    log_level: LogLevel,
    log_filter: Option<String>,
    data_access: DataAccessMode,
    /// Working cache for allowed-base-urls during apply; `None` = unset,
    /// `Some(v)` = explicit list. The library field is a plain
    /// `Vec<String>` (no Option), but the resolve layer needs the
    /// tri-state to distinguish "CLI null cleared it" vs "no override."
    /// Seeded from the initial `VlcConfig` and finalized via
    /// `finalize_allowed_base_urls`.
    allowed_base_urls: Option<Vec<String>>,
    /// Whether the user explicitly set `reconfig_drain_timeout_secs` via
    /// CLI or env. If not, `finalize()` mirrors `drain_timeout_secs` so
    /// operators who care only about the shutdown-drain knob inherit a
    /// sensible default for reconfig-drain without a second knob to tune.
    reconfig_drain_explicit: bool,
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
        // `allowed_base_urls` is `Vec<String>` on the library side; the
        // resolve layer lifts it into `Option<Vec<String>>` to retain
        // the "unset vs explicit empty" distinction that the data-access
        // machinery needs. The initial state is `None` (no explicit
        // list) iff the loaded config's list is empty — an empty list
        // is the library's default ("block all"), which we map to the
        // `DataAccessMode::Default` / `None` working state so that a
        // startup CLI can promote it back to an explicit allowlist.
        let initial_list = std::mem::take(&mut converter_config.allowed_base_urls);
        let initial_opt = if initial_list.is_empty() {
            None
        } else {
            Some(initial_list)
        };
        let (data_access, allowed_base_urls) = derive_data_access_state(initial_opt);
        Self {
            converter_config,
            serve_config: ServeConfig::default(),
            drain_timeout_secs: 30,
            ready_json: false,
            exit_on_parent_close: None,
            log_level: LogLevel::Warn,
            log_filter: None,
            data_access,
            allowed_base_urls,
            reconfig_drain_explicit: false,
        }
    }

    /// Apply overrides to `self`. Last writer wins: within a pass,
    /// later fields override earlier ones; across passes, the caller
    /// controls precedence by ordering the calls (env first, CLI last).
    fn apply(&mut self, overrides: Overrides) {
        let data_access_explicit = overrides.data_access.is_some();

        // `font_dir` is multi-value (both CLI and env). Each pass
        // *appends* its values to the working converter config's
        // `font_directories` list rather than replacing — env entries
        // are followed by CLI entries so both are preserved. An empty
        // `overrides.font_dir` means this pass did not touch the list.
        if !overrides.font_dir.is_empty() {
            self.converter_config
                .font_directories
                .extend(overrides.font_dir);
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
        // A host or port override produces a Tcp variant, replacing any
        // Uds set by a previous pass. The fallback 3000 below only fires
        // on that cross-pass promotion (e.g. env set VLC_UNIX_SOCKET,
        // CLI passed --host but not --port).
        if let Some(value) = overrides.host {
            let port = match &self.serve_config.main {
                ListenAddr::Tcp { port, .. } => *port,
                #[cfg(unix)]
                ListenAddr::Uds { .. } => 3000,
            };
            self.serve_config.main = ListenAddr::Tcp { host: value, port };
        }
        if let Some(value) = overrides.port {
            let host = match &self.serve_config.main {
                ListenAddr::Tcp { host, .. } => host.clone(),
                #[cfg(unix)]
                ListenAddr::Uds { .. } => "127.0.0.1".to_string(),
            };
            self.serve_config.main = ListenAddr::Tcp { host, port: value };
        }
        #[cfg(unix)]
        if let Some(path) = overrides.unix_socket {
            self.serve_config.main = ListenAddr::Uds { path };
        }
        if let Some(value) = overrides.socket_mode {
            self.serve_config.socket_mode = value;
        }
        if let Some(value) = overrides.ready_json {
            self.ready_json = value;
        }
        if let Some(value) = overrides.exit_on_parent_close {
            self.exit_on_parent_close = value;
        }
        if let Some(value) = overrides.workers {
            // `num_workers: NonZeroUsize` post-Task-0; CLI parser
            // rejects 0 with a clap-level error, so this is infallible.
            self.converter_config.num_workers = value;
        }
        if let Some(value) = overrides.api_key {
            self.serve_config.api_key = value;
        }
        if let Some(value) = overrides.admin_api_key {
            self.serve_config.admin_api_key = value;
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
        if let Some(value) = overrides.reconfig_drain_timeout_secs {
            self.serve_config.reconfig_drain_timeout_secs = value;
            self.reconfig_drain_explicit = true;
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
            self.serve_config.admin = value.map(ListenAddr::loopback_tcp);
        }
        #[cfg(unix)]
        if let Some(value) = overrides.admin_unix_socket {
            self.serve_config.admin = value.map(|path| ListenAddr::Uds { path });
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
            // Post-Task-0 the library field is `Vec<GoogleFontRequest>`
            // (not Option). An inner `None` (CLI `null`) now resets to
            // library default, i.e. empty list.
            self.converter_config.google_fonts = value.unwrap_or_default();
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
            // Library field is `Option<NonZeroUsize>`; the override
            // already carries the resolved inner value (including `0` →
            // None from the CLI shorthand).
            self.converter_config.max_v8_heap_size_mb = value;
        }
        if let Some(value) = overrides.max_v8_execution_time_secs {
            self.converter_config.max_v8_execution_time_secs = value;
        }
        if let Some(value) = overrides.gc_after_conversion {
            self.converter_config.gc_after_conversion = value;
        }
        if let Some(value) = overrides.vega_plugins {
            // Post-Task-0 `vega_plugins: Vec<String>` (not Option).
            self.converter_config.vega_plugins = value.unwrap_or_default();
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
        if let Some(value) = overrides.google_fonts_cache_size_mb {
            self.converter_config.google_fonts_cache_size_mb = value;
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
            // Post-Task-0 `themes: HashMap<_,_>` (not Option).
            self.converter_config.themes = value.unwrap_or_default();
        }
    }

    fn finalize(mut self) -> Result<ResolvedSettings, anyhow::Error> {
        // `allowed_base_urls` is now `Vec<String>` on the library
        // (empty = block all); preserve that semantic here. The
        // data-access helper continues to gate between "default",
        // "none", "all", and explicit "allowlist" states.
        self.converter_config.allowed_base_urls =
            finalize_allowed_base_urls(self.data_access, self.allowed_base_urls)?;

        if let Some(locale) = &self.converter_config.default_format_locale {
            locale.as_object()?;
        }
        if let Some(locale) = &self.converter_config.default_time_format_locale {
            locale.as_object()?;
        }

        // Clamp `max_v8_execution_time_secs` to the HTTP
        // `request_timeout_secs` when the latter is tighter. With the
        // Task-0 type change (`Option<NonZeroU64>`), `None` means
        // "unbounded" — under a positive HTTP timeout we clamp to that
        // timeout. A pre-existing positive cap is only lowered, never
        // raised.
        if self.serve_config.request_timeout_secs > 0 {
            let http_cap = self.serve_config.request_timeout_secs;
            let should_clamp = match self.converter_config.max_v8_execution_time_secs {
                None => true,
                Some(current) => current.get() > http_cap,
            };
            if should_clamp {
                self.converter_config.max_v8_execution_time_secs = NonZeroU64::new(http_cap);
            }
        }

        // Dedupe font directories in order (Task 2.5: seed is additive
        // across config-file + env + CLI; two identical entries would
        // otherwise hit `set_font_directories` with a duplicate).
        let mut seen = std::collections::HashSet::new();
        self.converter_config
            .font_directories
            .retain(|path| seen.insert(path.clone()));

        // If the user didn't explicitly set `reconfig_drain_timeout_secs`,
        // mirror the shutdown-drain value. Operators who care only about the
        // binary-level drain shouldn't need a second knob to get a sensible
        // reconfig-drain window.
        if !self.reconfig_drain_explicit {
            self.serve_config.reconfig_drain_timeout_secs = self.drain_timeout_secs;
        }

        let log_filter = self.log_filter.unwrap_or_else(|| {
            let level = self.log_level.to_tracing_filter();
            format!("vl_convert={level},tower_http={level}")
        });

        Ok(ResolvedSettings {
            converter_config: self.converter_config,
            serve_config: self.serve_config,
            drain_timeout_secs: self.drain_timeout_secs,
            log_filter,
            ready_json: self.ready_json,
            exit_on_parent_close: self.exit_on_parent_close,
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
        .map(|raw| parse_optional_non_zero_usize(raw, field_name(input, "max_v8_heap_size_mb")))
        .transpose()?;
    overrides.max_v8_execution_time_secs = raw
        .max_v8_execution_time_secs
        .as_deref()
        .map(|raw| {
            parse_optional_non_zero_u64(raw, field_name(input, "max_v8_execution_time_secs"))
        })
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
        .map(|raw| parse_optional_non_zero_usize(raw, field_name(input, "max_ephemeral_workers")))
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

    // VLC_FONT_DIR is a colon-separated (Linux/macOS) or
    // semicolon-separated (Windows) list of directories. Empty or all-
    // separator values produce an empty list (== "no override" at this
    // layer). Entries are tilde-expanded.
    overrides.font_dir = raw
        .font_dir
        .as_deref()
        .map(parse_font_dir_list)
        .unwrap_or_default();
    overrides.google_fonts_cache_size_mb = raw
        .google_fonts_cache_size_mb
        .as_deref()
        .map(|raw| parse_cache_size_mb(raw, field_name(input, "google_fonts_cache_size_mb")))
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
    #[cfg(unix)]
    {
        overrides.unix_socket = raw
            .unix_socket
            .as_deref()
            .map(|raw| super::parsers::parse_socket_path_arg(raw).map_err(anyhow::Error::msg))
            .transpose()?;
        overrides.admin_unix_socket = raw
            .admin_unix_socket
            .as_deref()
            .map(|raw| -> Result<Option<PathBuf>, anyhow::Error> {
                if raw.eq_ignore_ascii_case("null") {
                    Ok(None)
                } else {
                    super::parsers::parse_socket_path_arg(raw)
                        .map(Some)
                        .map_err(anyhow::Error::msg)
                }
            })
            .transpose()?;
        // Mutual-exclusion: VLC_UNIX_SOCKET alongside VLC_HOST or VLC_PORT is a
        // user misconfiguration (clap's ArgGroup catches this for CLI; env
        // needs an explicit check).
        if overrides.unix_socket.is_some() && (overrides.host.is_some() || overrides.port.is_some())
        {
            bail!(
                "VLC_UNIX_SOCKET conflicts with VLC_HOST/VLC_PORT; set one transport or the other"
            );
        }
        // Admin-port conflict check lives below — `overrides.admin_port`
        // isn't populated until several blocks further down, so the
        // check must run after that assignment.
    }
    overrides.socket_mode = raw
        .socket_mode
        .as_deref()
        .map(|raw| super::parsers::parse_socket_mode_arg(raw).map_err(anyhow::Error::msg))
        .transpose()?;
    overrides.ready_json = raw
        .ready_json
        .as_deref()
        .map(|raw| parse_bool(raw, field_name(input, "ready_json")))
        .transpose()?;
    overrides.exit_on_parent_close = raw
        .exit_on_parent_close
        .as_deref()
        .map(|raw| {
            if raw.eq_ignore_ascii_case("null") || raw.eq_ignore_ascii_case("auto") {
                Ok(None)
            } else {
                parse_bool(raw, field_name(input, "exit_on_parent_close")).map(Some)
            }
        })
        .transpose()?;
    // VLC_WORKERS is a positive integer; `0` is rejected (library
    // field is `NonZeroUsize`).
    overrides.workers = raw
        .workers
        .as_deref()
        .map(|raw| -> Result<NonZeroUsize, anyhow::Error> {
            let value = parse_u64(raw, field_name(input, "workers"))?;
            let as_usize = usize::try_from(value).map_err(|_| {
                anyhow!(
                    "{} '{raw}': value exceeds platform usize",
                    field_name(input, "workers")
                )
            })?;
            NonZeroUsize::new(as_usize).ok_or_else(|| {
                anyhow!(
                    "{} must be a positive integer (>= 1); got '{raw}'",
                    field_name(input, "workers")
                )
            })
        })
        .transpose()?;
    overrides.api_key = raw
        .api_key
        .as_deref()
        .map(parse_nullable_string)
        .transpose()?;
    overrides.admin_api_key = raw
        .admin_api_key
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
    overrides.reconfig_drain_timeout_secs = raw
        .reconfig_drain_timeout_secs
        .as_deref()
        .map(|raw| parse_u64(raw, field_name(input, "reconfig_drain_timeout_secs")))
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
    // Env-layer mutual-exclusion for admin. Clap ArgGroup handles this
    // at CLI parse time; env needs an explicit check. Must run AFTER
    // admin_port is populated above.
    #[cfg(unix)]
    if matches!(overrides.admin_unix_socket, Some(Some(_)))
        && matches!(overrides.admin_port, Some(Some(_)))
    {
        bail!("VLC_ADMIN_UNIX_SOCKET conflicts with VLC_ADMIN_PORT; set one or the other");
    }
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
    // CLI ergonomics: `0` → unbounded (inner `None`); positive → `Some(NZ)`.
    // The library field rejects a literal `NonZeroUsize::new(0)`, so
    // the shorthand lives only at the resolve layer.
    overrides.max_v8_heap_size_mb = cli.max_v8_heap_size_mb.map(NonZeroUsize::new);
    overrides.max_v8_execution_time_secs = cli.max_v8_execution_time_secs.map(NonZeroU64::new);
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
    overrides.max_ephemeral_workers = cli.max_ephemeral_workers.map(NonZeroUsize::new);
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

    // `--font-dir` is a repeated flag → `Vec<PathBuf>`. Empty =
    // no CLI override at this layer; non-empty values are appended
    // to the working converter config's `font_directories` during
    // `apply()`.
    overrides.font_dir = cli.font_dir.clone();
    // `--google-fonts-cache-size-mb` carries the CLI `0`-shorthand:
    // `0` → `None` (library default), positive → `Some(NZ)`.
    overrides.google_fonts_cache_size_mb = cli.google_fonts_cache_size_mb.map(NonZeroU64::new);
    overrides.log_level = cli.log_level;
    overrides.log_filter = cli
        .log_filter
        .as_deref()
        .map(|raw| parse_log_filter_value(raw, field_name(input, "log_filter")))
        .transpose()?;
    overrides.log_format = cli.log_format.map(LogFormatArg::into_log_format);
    overrides.host = cli.host.clone();
    overrides.port = cli.port;
    #[cfg(unix)]
    {
        overrides.unix_socket = cli.unix_socket.clone();
        overrides.admin_unix_socket = cli.admin_unix_socket.clone().map(Some);
        // Clap ArgGroup/conflicts_with catches main-listener conflicts at parse time;
        // this check is a belt-and-suspenders guard in case future refactors loosen clap.
        if overrides.unix_socket.is_some() && (overrides.host.is_some() || overrides.port.is_some())
        {
            bail!(
                "--unix-socket cannot be combined with --host/--port (clap ArgGroup should have caught this)"
            );
        }
    }
    overrides.socket_mode = cli.socket_mode;
    overrides.ready_json = cli.ready_json;
    overrides.exit_on_parent_close = cli.exit_on_parent_close.map(Some);
    // CLI `--workers` is already `Option<NonZeroUsize>` — clap's
    // value-parser rejected `0` with an inline error.
    overrides.workers = cli.workers;
    overrides.api_key = cli
        .api_key
        .as_deref()
        .map(parse_nullable_string)
        .transpose()?;
    overrides.admin_api_key = cli
        .admin_api_key
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
    overrides.reconfig_drain_timeout_secs = cli.reconfig_drain_timeout_secs;
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
) -> Result<Vec<String>, anyhow::Error> {
    // Library field is `Vec<String>` (post-Task-0); empty list = "block
    // all network data" (secure-by-default). The data-access machinery
    // stays as a resolver-layer DSL that writes into that single field.
    match data_access {
        DataAccessMode::Default => {
            if allowed_base_urls.is_some() {
                bail!("allowed_base_urls may only be set when data_access=allowlist");
            }
            // Library default: empty list = block all.
            Ok(Vec::new())
        }
        DataAccessMode::None => {
            if allowed_base_urls.is_some() {
                bail!("allowed_base_urls may only be set when data_access=allowlist");
            }
            Ok(Vec::new())
        }
        DataAccessMode::All => {
            if allowed_base_urls.is_some() {
                bail!("allowed_base_urls may only be set when data_access=allowlist");
            }
            Ok(vec!["*".to_string()])
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
            Ok(urls)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::env::{
        ENV_ADMIN_PORT, ENV_ADMIN_UNIX_SOCKET, ENV_API_KEY, ENV_AUTO_GOOGLE_FONTS, ENV_CONFIG,
        ENV_DEFAULT_THEME, ENV_LOAD_CONFIG, ENV_LOG_FILTER, ENV_PORT, ENV_UNIX_SOCKET,
        SETTING_PAIRS,
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
            vec!["https://config.example/".to_string()]
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

        // Post-Task-0 `allowed_base_urls: Vec<String>` (no Option);
        // `--allowed-base-urls null` clears the inherited allowlist to
        // the library default (empty list = block all network data).
        assert!(resolved.converter_config.allowed_base_urls.is_empty());
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
            vec!["*".to_string()]
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

    /// Setting both `VLC_ADMIN_UNIX_SOCKET` and `VLC_ADMIN_PORT` must
    /// be rejected at resolve time. Clap's `ArgGroup` covers the CLI
    /// path; the env path needs an explicit check, which must run
    /// after `overrides.admin_port` is populated.
    #[cfg(unix)]
    #[test]
    fn test_resolve_settings_env_rejects_admin_uds_with_admin_port() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");
        guard.set(ENV_ADMIN_UNIX_SOCKET, "/tmp/admin.sock");
        guard.set(ENV_ADMIN_PORT, "9000");

        let err = resolve_settings(parse_cli(&[])).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("VLC_ADMIN_UNIX_SOCKET") && msg.contains("VLC_ADMIN_PORT"),
            "error message should name both conflicting vars; got: {msg}"
        );
    }

    /// Mirrors the admin check for the main listener: setting
    /// `VLC_UNIX_SOCKET` alongside `VLC_PORT` (or `VLC_HOST`) must be
    /// rejected at resolve time.
    #[cfg(unix)]
    #[test]
    fn test_resolve_settings_env_rejects_unix_socket_with_port() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");
        guard.set(ENV_UNIX_SOCKET, "/tmp/main.sock");
        guard.set(ENV_PORT, "3000");

        let err = resolve_settings(parse_cli(&[])).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("VLC_UNIX_SOCKET"),
            "error message should name VLC_UNIX_SOCKET; got: {msg}"
        );
    }

    #[test]
    fn test_resolve_settings_port_default_3000() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");

        let resolved = resolve_settings(parse_cli(&[])).unwrap();
        assert!(
            matches!(
                resolved.serve_config.main,
                ListenAddr::Tcp { port: 3000, .. }
            ),
            "default main should be TCP port 3000; got {:?}",
            resolved.serve_config.main
        );
    }

    #[test]
    fn test_resolve_settings_port_fallback_to_paas_port() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");
        guard.set("PORT", "7777");

        let resolved = resolve_settings(parse_cli(&[])).unwrap();
        assert!(matches!(
            resolved.serve_config.main,
            ListenAddr::Tcp { port: 7777, .. }
        ));
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
        assert!(matches!(
            resolved.serve_config.main,
            ListenAddr::Tcp { port: 8888, .. }
        ));
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
        assert!(matches!(
            resolved.serve_config.main,
            ListenAddr::Tcp { port: 9999, .. }
        ));
    }

    #[test]
    fn test_resolve_settings_port_invalid_paas_port_falls_through() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set("PORT", "not-a-number");

        let resolved = resolve_settings(parse_cli(&[])).unwrap();
        assert!(
            matches!(
                resolved.serve_config.main,
                ListenAddr::Tcp { port: 3000, .. }
            ),
            "invalid PORT should be silently ignored; got {:?}",
            resolved.serve_config.main
        );
    }

    #[test]
    fn test_font_dir_cli_seeds_vlc_config_font_directories() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");

        let resolved = resolve_settings(parse_cli(&[
            "--font-dir",
            "/tmp/fonts-a",
            "--font-dir",
            "/tmp/fonts-b",
        ]))
        .unwrap();
        assert_eq!(
            resolved.converter_config.font_directories,
            vec![
                PathBuf::from("/tmp/fonts-a"),
                PathBuf::from("/tmp/fonts-b"),
            ],
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn test_font_dir_env_parses_colon_separated_list() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");
        guard.set(super::super::env::ENV_FONT_DIR, "/tmp/fonts-a:/tmp/fonts-b");

        let resolved = resolve_settings(parse_cli(&[])).unwrap();
        assert_eq!(
            resolved.converter_config.font_directories,
            vec![
                PathBuf::from("/tmp/fonts-a"),
                PathBuf::from("/tmp/fonts-b"),
            ],
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn test_font_dir_env_and_cli_compose_in_order() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");
        guard.set(super::super::env::ENV_FONT_DIR, "/env-a:/env-b");

        // Env entries are applied first, then CLI entries. Resolver
        // dedupes in finalize.
        let resolved =
            resolve_settings(parse_cli(&["--font-dir", "/cli-a", "--font-dir", "/env-a"]))
                .unwrap();
        assert_eq!(
            resolved.converter_config.font_directories,
            vec![
                PathBuf::from("/env-a"),
                PathBuf::from("/env-b"),
                PathBuf::from("/cli-a"),
                // `/env-a` appears twice across passes; dedupe removes
                // the second hit so `set_font_directories` sees a
                // unique list.
            ],
        );
    }

    #[test]
    fn test_google_fonts_cache_size_mb_cli_plumbing() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");

        let resolved =
            resolve_settings(parse_cli(&["--google-fonts-cache-size-mb", "256"])).unwrap();
        assert_eq!(
            resolved.converter_config.google_fonts_cache_size_mb,
            NonZeroU64::new(256),
        );

        let resolved_zero =
            resolve_settings(parse_cli(&["--google-fonts-cache-size-mb", "0"])).unwrap();
        assert_eq!(
            resolved_zero.converter_config.google_fonts_cache_size_mb,
            None,
            "CLI `0` should collapse to `None` (library default)",
        );

        let resolved_absent = resolve_settings(parse_cli(&[])).unwrap();
        assert_eq!(
            resolved_absent.converter_config.google_fonts_cache_size_mb,
            None,
        );
    }

    #[test]
    fn test_google_fonts_cache_size_mb_env_plumbing() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");
        guard.set(
            super::super::env::ENV_GOOGLE_FONTS_CACHE_SIZE_MB,
            "128",
        );

        let resolved = resolve_settings(parse_cli(&[])).unwrap();
        assert_eq!(
            resolved.converter_config.google_fonts_cache_size_mb,
            NonZeroU64::new(128),
        );
    }

    #[test]
    fn test_num_workers_zero_cli_rejected() {
        // `--workers 0` is rejected at clap parse time (value_parser);
        // `resolve_settings` therefore never observes it. Mirrors the
        // library-side `NonZeroUsize` invariant.
        use clap::Parser;
        let err = Cli::try_parse_from(["vl-convert-server", "--workers", "0"]).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("positive") || msg.contains(">= 1"),
            "expected positive-integer message; got: {msg}",
        );
    }

    #[test]
    fn test_numeric_caps_zero_shorthand_is_none() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");

        let resolved = resolve_settings(parse_cli(&[
            "--max-v8-heap-size-mb",
            "0",
            "--max-v8-execution-time-secs",
            "0",
            "--max-ephemeral-workers",
            "0",
        ]))
        .unwrap();
        assert_eq!(resolved.converter_config.max_v8_heap_size_mb, None);
        assert_eq!(resolved.converter_config.max_ephemeral_workers, None);
        // `max_v8_execution_time_secs` gets clamped by
        // `request_timeout_secs` in finalize (default 30s). Before the
        // clamp the value is `None`; after the clamp it's
        // `Some(NZ(30))` because `None` < any positive HTTP timeout.
        assert_eq!(
            resolved.converter_config.max_v8_execution_time_secs,
            NonZeroU64::new(30),
            "finalize clamps None to the HTTP request_timeout_secs",
        );
    }

    #[test]
    fn test_numeric_caps_positive_round_trip() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = EnvGuard::new();
        guard.clear_all();
        guard.set(ENV_LOAD_CONFIG, "false");

        let resolved = resolve_settings(parse_cli(&[
            "--max-v8-heap-size-mb",
            "1024",
            "--max-ephemeral-workers",
            "4",
            "--request-timeout-secs",
            "120",
            "--max-v8-execution-time-secs",
            "60",
        ]))
        .unwrap();
        assert_eq!(
            resolved.converter_config.max_v8_heap_size_mb,
            NonZeroUsize::new(1024),
        );
        assert_eq!(
            resolved.converter_config.max_ephemeral_workers,
            NonZeroUsize::new(4),
        );
        // 60s < 120s HTTP cap; finalize leaves it alone.
        assert_eq!(
            resolved.converter_config.max_v8_execution_time_secs,
            NonZeroU64::new(60),
        );
    }
}

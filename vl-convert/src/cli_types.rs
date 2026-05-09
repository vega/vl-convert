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
    #[arg(long, global = true, value_name = "disabled|PATH", env = "VLC_CONFIG")]
    pub(crate) vlc_config: Option<String>,

    /// Base URL for resolving relative data paths. Reserved values:
    /// `default` (use vega-datasets CDN), `disabled` (relative paths
    /// error). Otherwise either a URL with scheme (`https://...`,
    /// `file://...`) or an absolute filesystem path. Relative paths
    /// are rejected.
    #[arg(
        long,
        global = true,
        value_name = "default|disabled|URL|PATH",
        env = "VLC_BASE_URL"
    )]
    pub(crate) base_url: Option<String>,

    /// Allowed base URLs. Reserved single-value shortcuts: `none`
    /// (block all), `net` (HTTP/HTTPS only, no filesystem), `all`
    /// (allow everything incl. filesystem). Otherwise a `;`-separated
    /// list of CSP-style patterns: `"https:"` (scheme),
    /// `"https://example.com/"` (prefix), `"/data/"` (absolute
    /// filesystem path). Long allowlists belong in `--vlc-config`
    /// JSONC.
    #[arg(
        long,
        global = true,
        value_name = "none|net|all|URL_PREFIX",
        env = "VLC_ALLOWED_BASE_URLS",
        value_delimiter = ';'
    )]
    pub(crate) allowed_base_urls: Vec<String>,

    /// Register a font from Google Fonts. Use "Family" for all variants,
    /// or "Family:400,700italic" for specific weight/style combinations.
    /// May be specified multiple times; `;` separates entries when
    /// passed via `VLC_GOOGLE_FONT`.
    #[arg(
        long = "google-font",
        global = true,
        env = "VLC_GOOGLE_FONT",
        value_delimiter = ';'
    )]
    pub(crate) google_font: Vec<String>,

    /// Maximum unique Google Font family/weight/style variants per conversion.
    /// `0` disables the cap.
    #[arg(
        long,
        global = true,
        value_name = "N",
        env = "VLC_MAX_GOOGLE_FONT_VARIANTS_PER_REQUEST"
    )]
    pub(crate) max_google_font_variants_per_request: Option<u64>,

    /// Automatically download missing fonts from Google Fonts (default: false).
    #[arg(
        long,
        global = true,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = parse_boolish_arg,
        env = "VLC_AUTO_GOOGLE_FONTS",
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
        env = "VLC_EMBED_LOCAL_FONTS",
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
        env = "VLC_SUBSET_FONTS",
    )]
    pub(crate) subset_fonts: Option<bool>,

    /// Missing-font behavior: fallback silently, warn, or error.
    #[arg(long, global = true, value_enum, env = "VLC_MISSING_FONTS")]
    pub(crate) missing_fonts: Option<MissingFontsArg>,

    /// Maximum V8 heap size per worker in megabytes [default: 0 = no limit]
    #[arg(long, global = true, env = "VLC_MAX_V8_HEAP_SIZE_MB")]
    pub(crate) max_v8_heap_size_mb: Option<u64>,

    /// Maximum V8 execution time in seconds [default: 0 = no limit]
    #[arg(long, global = true, env = "VLC_MAX_V8_EXECUTION_TIME_SECS")]
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
        env = "VLC_GC_AFTER_CONVERSION",
    )]
    pub(crate) gc_after_conversion: Option<bool>,

    /// Vega plugin: file path (`.js`/`.mjs`) or URL (`https://...`).
    /// The plugin must be a single ESM module that exports a default function
    /// accepting a vega object. Multi-file plugins should be pre-bundled
    /// with esbuild or Rollup. URL plugins auto-allow their domain for imports.
    /// May be specified multiple times; plugins execute in order. Put inline
    /// ESM plugins in `--vlc-config`.
    #[arg(
        long = "vega-plugin",
        global = true,
        value_name = "path|URL",
        value_parser = crate::io_utils::parse_vega_plugin_arg,
        env = "VLC_VEGA_PLUGIN",
        value_delimiter = ';',
    )]
    pub(crate) vega_plugin: Vec<String>,

    /// Domains allowed for HTTP imports in plugins, `;`-separated.
    /// Examples: `'*'` (any), `'esm.sh'`, `'*.jsdelivr.net'`.
    /// May be specified multiple times.
    #[arg(
        long = "plugin-import-domains",
        global = true,
        env = "VLC_PLUGIN_IMPORT_DOMAINS",
        value_delimiter = ';'
    )]
    pub(crate) plugin_import_domains: Vec<String>,

    /// Additional directory to search for fonts. Repeatable: pass the
    /// flag multiple times (`--font-dir /a --font-dir /b`) to register
    /// multiple directories. Calls
    /// `vl_convert_rs::set_font_directories` once at startup with the
    /// combined list (replace semantics).
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        env = "VLC_FONT_DIR",
        value_delimiter = if cfg!(windows) { ';' } else { ':' },
    )]
    pub(crate) font_dir: Vec<PathBuf>,

    /// Capacity (MB) of the on-disk Google Fonts LRU cache. `0` resolves
    /// to the library default (`Option<NonZeroU64>::None`).
    #[arg(
        long,
        global = true,
        value_name = "MB",
        env = "VLC_GOOGLE_FONTS_CACHE_SIZE_MB"
    )]
    pub(crate) google_fonts_cache_size_mb: Option<u64>,

    /// Default Vega-Lite theme applied when a request omits `theme`.
    /// Pass the literal string `null` to clear a value loaded from the
    /// `--vlc-config` file.
    #[arg(
        long,
        global = true,
        value_name = "THEME|null",
        env = "VLC_DEFAULT_THEME"
    )]
    pub(crate) default_theme: Option<String>,

    /// Default d3-format locale: a locale name string, an inline JSON
    /// object literal, a path to a `.json` / `.jsonc` file, or the
    /// literal string `null` to clear a value loaded from the
    /// `--vlc-config` file.
    #[arg(
        long,
        global = true,
        value_name = "LOCALE|JSON|FILE.json|null",
        env = "VLC_DEFAULT_FORMAT_LOCALE"
    )]
    pub(crate) default_format_locale: Option<String>,

    /// Default d3-time-format locale: a locale name string, an inline
    /// JSON object literal, a path to a `.json` / `.jsonc` file, or the
    /// literal string `null` to clear a value loaded from the
    /// `--vlc-config` file.
    #[arg(
        long,
        global = true,
        value_name = "LOCALE|JSON|FILE.json|null",
        env = "VLC_DEFAULT_TIME_FORMAT_LOCALE"
    )]
    pub(crate) default_time_format_locale: Option<String>,

    /// Custom named themes as an inline JSON object literal, a path to
    /// a `.json` / `.jsonc` file, or the literal string `null` to clear
    /// a map loaded from the `--vlc-config` file.
    #[arg(
        long,
        global = true,
        value_name = "JSON|FILE.json|null",
        env = "VLC_THEMES"
    )]
    pub(crate) themes: Option<String>,

    /// Log level for Vega/Vega-Lite messages
    #[arg(long, global = true, value_enum, default_value_t = LogLevel::Warn, env = "VLC_LOG_LEVEL")]
    pub(crate) log_level: LogLevel,

    /// Tracing-subscriber output format. `text` is human-readable;
    /// `json` emits one structured line per event for log aggregators.
    #[arg(long, global = true, value_enum, default_value_t = LogFormat::Text, env = "VLC_LOG_FORMAT")]
    pub(crate) log_format: LogFormat,

    /// Raw `tracing-subscriber::EnvFilter` directive (e.g.
    /// `"vl_convert=debug,tower_http=info"`). When set, this wins over
    /// `--log-level`. When unset, a directive is synthesized from
    /// `--log-level`.
    #[arg(long, global = true, value_name = "DIRECTIVE", env = "VLC_LOG_FILTER")]
    pub(crate) log_filter: Option<String>,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[cfg(test)]
mod tests {
    //! In-process parse-time tests for the `VLC_*` env-var fallback layer.
    //!
    //! These tests mutate the parent process environment so clap's `env =`
    //! attribute resolves against a known value at parse time. `ENV_LOCK`
    //! serializes that process-global state.
    //!
    //! The complementary subprocess-level coverage lives in
    //! `tests/test_env_vars.rs` (port precedence ladder, log-level
    //! plumbing, etc.).
    use super::*;
    use crate::commands::Commands;
    use clap::Parser;
    use std::sync::Mutex;

    /// Process-wide mutex serializing the parse-time env-var tests.
    /// `std::env::set_var` is process-global; without serialization
    /// two parallel tests could read each other's values mid-parse.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// RAII guard that unsets env vars on drop, restoring a clean baseline
    /// for later tests in the same process.
    struct EnvScrub<'a>(&'a [&'a str]);
    impl Drop for EnvScrub<'_> {
        fn drop(&mut self) {
            for k in self.0 {
                std::env::remove_var(k);
            }
        }
    }

    fn parse_with_env(env: &[(&str, &str)]) -> Cli {
        // SAFETY: serialized through `ENV_LOCK`; this prevents concurrent
        // environment mutation while exercising clap's env path.
        for (k, v) in env {
            std::env::set_var(k, v);
        }
        Cli::try_parse_from(["vl-convert", "vl2svg"]).expect("Cli parse must succeed")
    }

    #[test]
    fn vlc_font_dir_splits_on_path_separator() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_FONT_DIR"]);
        // PATH-style splitter: `:` on Unix, `;` on Windows. Two paths
        // joined by the platform separator must surface as two entries
        // on `Cli.font_dir` after env-resolution.
        #[cfg(unix)]
        let value = "/tmp/vlc_test_a:/tmp/vlc_test_b";
        #[cfg(windows)]
        let value = "C:\\tmp\\vlc_test_a;C:\\tmp\\vlc_test_b";
        let cli = parse_with_env(&[("VLC_FONT_DIR", value)]);
        assert_eq!(
            cli.font_dir.len(),
            2,
            "VLC_FONT_DIR must split on the platform PATH separator: {:?}",
            cli.font_dir
        );
        #[cfg(unix)]
        {
            assert_eq!(cli.font_dir[0].to_str(), Some("/tmp/vlc_test_a"));
            assert_eq!(cli.font_dir[1].to_str(), Some("/tmp/vlc_test_b"));
        }
    }

    #[test]
    fn vlc_plugin_import_domains_splits_on_semicolon() {
        // Vec-shaped env values split on `;`, except `--font-dir`, which uses
        // the OS PATH separator.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_PLUGIN_IMPORT_DOMAINS"]);
        let cli = parse_with_env(&[("VLC_PLUGIN_IMPORT_DOMAINS", "esm.sh;*.jsdelivr.net")]);
        assert_eq!(
            cli.plugin_import_domains,
            vec!["esm.sh".to_string(), "*.jsdelivr.net".to_string()],
            "VLC_PLUGIN_IMPORT_DOMAINS must split on `;` into 2 entries"
        );
    }

    #[test]
    fn vlc_vega_plugin_splits_on_semicolon() {
        // `--vega-plugin` env values split into path-or-URL entries.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_VEGA_PLUGIN"]);
        let cli = parse_with_env(&[("VLC_VEGA_PLUGIN", "https://example.com/p1.js;/tmp/p2.js")]);
        assert_eq!(
            cli.vega_plugin,
            vec![
                "https://example.com/p1.js".to_string(),
                "/tmp/p2.js".to_string()
            ],
            "VLC_VEGA_PLUGIN must split on `;` into 2 entries"
        );
    }

    #[test]
    fn vlc_google_font_splits_on_semicolon_preserves_variant_commas() {
        // `--google-font` env values split on `;`, while variant weights keep
        // their comma-separated grammar inside each entry.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_GOOGLE_FONT"]);
        let cli = parse_with_env(&[("VLC_GOOGLE_FONT", "Roboto:400,700italic;Inter:400")]);
        assert_eq!(
            cli.google_font,
            vec!["Roboto:400,700italic".to_string(), "Inter:400".to_string()],
            "VLC_GOOGLE_FONT must split on `;` into 2 entries; commas inside variants survive"
        );
    }

    #[test]
    fn vlc_allowed_base_urls_reserved_literal_none() {
        // Reserved single-value shortcut `none` expands to an empty
        // allowlist (block all data access).
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_ALLOWED_BASE_URLS"]);
        let cli = parse_with_env(&[("VLC_ALLOWED_BASE_URLS", "none")]);
        assert_eq!(cli.allowed_base_urls, vec!["none".to_string()]);
        let expanded = crate::io_utils::expand_allowed_base_urls(&cli.allowed_base_urls);
        assert!(
            expanded.is_empty(),
            "`none` must expand to an empty allowlist, got {expanded:?}"
        );
    }

    #[test]
    fn vlc_allowed_base_urls_reserved_literal_net() {
        // Reserved single-value shortcut `net` expands to the HTTP/HTTPS-only
        // library default.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_ALLOWED_BASE_URLS"]);
        let cli = parse_with_env(&[("VLC_ALLOWED_BASE_URLS", "net")]);
        assert_eq!(cli.allowed_base_urls, vec!["net".to_string()]);
        let expanded = crate::io_utils::expand_allowed_base_urls(&cli.allowed_base_urls);
        assert_eq!(
            expanded,
            vec!["http:".to_string(), "https:".to_string()],
            "`net` must expand to the library default [http:, https:]"
        );
    }

    #[test]
    fn vlc_allowed_base_urls_reserved_literal_all() {
        // Reserved single-value shortcut `all` expands to `["*"]`
        // (allow everything, including filesystem reads).
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_ALLOWED_BASE_URLS"]);
        let cli = parse_with_env(&[("VLC_ALLOWED_BASE_URLS", "all")]);
        assert_eq!(cli.allowed_base_urls, vec!["all".to_string()]);
        let expanded = crate::io_utils::expand_allowed_base_urls(&cli.allowed_base_urls);
        assert_eq!(
            expanded,
            vec!["*".to_string()],
            "`all` must expand to [\"*\"]"
        );
    }

    #[test]
    fn vlc_allowed_base_urls_multi_prefix_splits_on_semicolon() {
        // Multi-prefix env values split on `;` and pass through verbatim
        // (no reserved-literal expansion when the Vec has > 1 entry).
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_ALLOWED_BASE_URLS"]);
        let cli = parse_with_env(&[("VLC_ALLOWED_BASE_URLS", "https://x.com/;https://y.com/")]);
        assert_eq!(
            cli.allowed_base_urls,
            vec!["https://x.com/".to_string(), "https://y.com/".to_string()],
            "VLC_ALLOWED_BASE_URLS must split on `;` into 2 entries"
        );
        let expanded = crate::io_utils::expand_allowed_base_urls(&cli.allowed_base_urls);
        assert_eq!(
            expanded,
            vec!["https://x.com/".to_string(), "https://y.com/".to_string()],
            "multi-prefix Vec must pass through verbatim — reserved literals \
             only fire on a single-value invocation"
        );
    }

    #[test]
    fn vlc_log_level_resolves_value_enum_from_env() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_LOG_LEVEL"]);
        let cli = parse_with_env(&[("VLC_LOG_LEVEL", "error")]);
        assert!(matches!(cli.log_level, LogLevel::Error));
    }

    #[test]
    fn vlc_auto_google_fonts_runs_through_boolish_parser() {
        // `value_parser = parse_boolish_arg` must run on env-resolved values.
        // `=true`, `=1`, `=yes`, `=on` are all accepted by the boolish parser.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_AUTO_GOOGLE_FONTS"]);
        let cli = parse_with_env(&[("VLC_AUTO_GOOGLE_FONTS", "true")]);
        assert_eq!(cli.auto_google_fonts, Some(true));

        let cli = parse_with_env(&[("VLC_AUTO_GOOGLE_FONTS", "no")]);
        assert_eq!(cli.auto_google_fonts, Some(false));
    }

    #[test]
    fn vlc_drain_timeout_secs_overrides_default() {
        // `default_value_t` applies only when neither CLI nor env supplies a
        // value, even for non-Option fields.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_DRAIN_TIMEOUT_SECS"]);
        std::env::set_var("VLC_DRAIN_TIMEOUT_SECS", "60");
        let cli = Cli::try_parse_from(["vl-convert", "serve"])
            .expect("Cli::try_parse_from(['vl-convert', 'serve']) must succeed");
        let Commands::Serve(args) = cli.command else {
            panic!("expected Commands::Serve");
        };
        assert_eq!(
            args.drain_timeout_secs, 60,
            "VLC_DRAIN_TIMEOUT_SECS=60 must override default_value_t=30"
        );
    }

    #[test]
    fn vlc_admin_host_env_var_round_trips() {
        // VLC_ADMIN_HOST resolves through the env path; --admin-port is
        // required by clap (the admin listener is opt-in), so we pass
        // it on the CLI.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_ADMIN_HOST"]);
        std::env::set_var("VLC_ADMIN_HOST", "0.0.0.0");
        let cli = Cli::try_parse_from(["vl-convert", "serve", "--admin-port", "9000"])
            .expect("Cli::try_parse_from must succeed");
        let Commands::Serve(args) = cli.command else {
            panic!("expected Commands::Serve");
        };
        assert_eq!(
            args.admin_host.as_deref(),
            Some("0.0.0.0"),
            "VLC_ADMIN_HOST=0.0.0.0 must populate args.admin_host"
        );
    }

    #[test]
    fn admin_host_without_admin_port_is_clap_error() {
        // `requires = "admin_port"` on --admin-host means passing the
        // host alone is rejected by clap before parsing finishes.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_ADMIN_HOST", "VLC_ADMIN_PORT"]);
        let err = Cli::try_parse_from(["vl-convert", "serve", "--admin-host", "127.0.0.1"])
            .expect_err("--admin-host without --admin-port must be a clap error");
        let msg = err.to_string();
        assert!(
            msg.contains("--admin-port") || msg.contains("admin_port"),
            "error should call out the missing --admin-port; got: {msg}"
        );
    }

    #[test]
    fn cli_flag_overrides_vlc_env_var() {
        // CLI flags override their `VLC_*` env fallback.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _scrub = EnvScrub(&["VLC_LOG_LEVEL"]);
        std::env::set_var("VLC_LOG_LEVEL", "error");
        let cli = Cli::try_parse_from(["vl-convert", "--log-level", "debug", "vl2svg"])
            .expect("Cli parse must succeed");
        assert!(
            matches!(cli.log_level, LogLevel::Debug),
            "CLI --log-level=debug must override VLC_LOG_LEVEL=error"
        );
        // Confirm the resolved subcommand.
        assert!(matches!(cli.command, Commands::Vl2svg { .. }));
    }
}

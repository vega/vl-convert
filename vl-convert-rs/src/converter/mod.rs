mod config;
mod fonts;
mod inner;
mod permissions;
mod plugin;
mod rendering;
mod transfer;
mod types;
mod value_or_string;
mod worker_pool;

pub use config::*;
pub use fonts::GoogleFontRequest;
pub(crate) use fonts::*;
pub(crate) use inner::InnerVlConverter;
pub(crate) use inner::VlConverterInner;
pub(crate) use permissions::*;
pub use permissions::{domain_matches_patterns, vlc_config_path};
pub(crate) use plugin::resolve_plugin;
pub use rendering::*;
pub use types::*;
pub use value_or_string::*;
pub(crate) use worker_pool::{CallerGoneGuard, OutstandingTicket, QueuedWork, WorkFn};

// Re-export the #[op2] functions so the deno_core::extension! macro can find them
pub(crate) use transfer::op_get_json_arg;
pub(crate) use transfer::op_set_msgpack_result;
// JsonArgGuard, MsgpackResultGuard, WorkerTransferState, WorkerTransferStateHandle
// are imported directly by inner.rs from the transfer module.
pub(crate) use worker_pool::spawn_worker_pool;

use crate::image_loading::ImageAccessPolicy;
use crate::module_loader::import_map::VlVersion;
use crate::text::get_font_baseline_snapshot;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::url::Url;
use std::collections::hash_map::Entry;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use deno_core::anyhow::anyhow;
use futures::channel::oneshot;

use crate::with_font_overlay;
use std::sync::atomic::AtomicUsize;

// Extension with our custom ops - MainWorker provides all Web APIs (URL, fetch, etc.)
// Canvas 2D ops are now in the separate vl_convert_canvas2d extension from vl-convert-canvas2d-deno
deno_core::extension!(
    vl_convert_runtime,
    ops = [
        op_get_json_arg,
        op_set_msgpack_result,
        crate::data_ops::op_vega_data_fetch,
        crate::data_ops::op_vega_data_fetch_bytes,
        crate::data_ops::op_vega_file_read,
        crate::data_ops::op_vega_file_read_bytes,
    ],
    esm_entry_point = "ext:vl_convert_runtime/bootstrap.js",
    esm = [
        dir "src/js",
        "bootstrap.js",
    ],
);

const VEGAEMBED_GLOBAL_SNIPPET: &str =
    "window.vegaEmbed=vegaEmbed; window.vega=vega; window.vegaLite=vegaLite; window.lodashDebounce=lodashDebounce;";
pub const ACCESS_DENIED_MARKER: &str = "VLC_ACCESS_DENIED";

/// Canonicalize a path, stripping the Windows extended-length prefix (`\\?\`)
/// that `std::fs::canonicalize` adds on Windows.
pub(crate) fn portable_canonicalize(
    path: &std::path::Path,
) -> Result<std::path::PathBuf, AnyError> {
    let canonical = std::fs::canonicalize(path)?;
    #[cfg(target_os = "windows")]
    {
        let s = canonical.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return Ok(std::path::PathBuf::from(stripped));
        }
    }
    Ok(canonical)
}

/// Struct for performing Vega-Lite to Vega conversions using the Deno v8 runtime.
///
/// # Examples
///
/// ```
/// use vl_convert_rs::{VlConverter, VlOpts, VlVersion};
/// let converter = VlConverter::new();
///
/// let vl_spec: serde_json::Value = serde_json::from_str(r#"
/// {
///   "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
///   "data": {"url": "data/movies.json"},
///   "mark": "circle",
///   "encoding": {
///     "x": {
///       "bin": {"maxbins": 10},
///       "field": "IMDB Rating"
///     },
///     "y": {
///       "bin": {"maxbins": 10},
///       "field": "Rotten Tomatoes Rating"
///     },
///     "size": {"aggregate": "count"}
///   }
/// }"#).unwrap();
///
/// let vega_output = futures::executor::block_on(
///     converter.vegalite_to_vega(
///         vl_spec,
///         VlOpts {
///             vl_version: VlVersion::default(),
///             ..Default::default()
///         }
///     )
/// ).expect("Failed to perform Vega-Lite to Vega conversion");
///
/// println!("{}", vega_output.spec);
/// ```
#[derive(Clone)]
pub struct VlConverter {
    pub(crate) inner: Arc<VlConverterInner>,
}

impl VlConverter {
    pub fn new() -> Self {
        Self::with_config(VlcConfig::default()).expect("default converter config is valid")
    }

    pub fn with_config(config: VlcConfig) -> Result<Self, AnyError> {
        let config = Arc::new(normalize_converter_config(config)?);

        // Initialize environment logger with filter to suppress noisy SWC tree-shaker spans
        // The swc_ecma_transforms_optimization module logs tracing spans at ERROR level
        // which are not actual errors - just instrumentation.
        env_logger::Builder::from_env(env_logger::Env::default())
            .filter_module("swc_ecma_transforms_optimization", log::LevelFilter::Off)
            .try_init()
            .ok();

        // Apply process-global Google Fonts cache cap from the config.
        crate::text::apply_hot_font_cache(config.google_fonts_cache_size_mb)?;

        let ephemeral_semaphore = if config.allow_per_request_plugins {
            config.max_ephemeral_workers.map(|n| {
                let permits: usize = n
                    .get()
                    .try_into()
                    .expect("max_ephemeral_workers fits in usize");
                Arc::new(tokio::sync::Semaphore::new(permits))
            })
        } else {
            None
        };

        Ok(Self {
            inner: Arc::new(VlConverterInner {
                vegaembed_bundles: Default::default(),
                pool: Default::default(),
                config,
                resolved_plugins: Mutex::new(Vec::new()),
                ephemeral_semaphore,
            }),
        })
    }

    pub fn config(&self) -> VlcConfig {
        (*self.inner.config).clone()
    }

    fn image_access_policy(&self) -> ImageAccessPolicy {
        let parsed = parse_allowed_base_urls_from_config(&self.inner.config)
            .expect("allowed_base_urls were already validated");
        // Use base_url as usvg's resources_dir when it points to a local path
        let filesystem_root = if self.inner.config.base_url.is_filesystem() {
            self.inner
                .config
                .base_url
                .resolved_url()
                .ok()
                .flatten()
                .and_then(|url_str| Url::parse(&url_str).ok())
                .and_then(|url| url.to_file_path().ok())
        } else {
            None
        };
        ImageAccessPolicy {
            // Always engage the allowlist enforcer — secure-by-default means
            // an empty list blocks everything rather than falling back to
            // "allow any http/https".
            allowed_base_urls: Some(parsed),
            filesystem_root,
        }
    }

    /// Eagerly start the worker pool for this converter instance.
    ///
    /// This is optional; if not called, the pool starts lazily on first request.
    pub fn warm_up(&self) -> Result<(), AnyError> {
        let _ = self.get_or_spawn_sender()?;
        Ok(())
    }

    pub async fn health_check(&self) -> Result<(), AnyError> {
        self.run_on_worker(|inner| {
            Box::pin(async move {
                let result = inner.execute_script_to_json("1+1").await?;
                if result != serde_json::json!(2) {
                    bail!("worker health check failed");
                }
                Ok(())
            })
        })
        .await
    }

    fn get_or_spawn_sender(
        &self,
    ) -> Result<(tokio::sync::mpsc::Sender<QueuedWork>, OutstandingTicket), AnyError> {
        let mut guard = self
            .inner
            .pool
            .lock()
            .map_err(|e| anyhow!("Failed to lock worker pool: {e}"))?;

        if let Some(pool) = guard.as_ref() {
            if !pool.is_closed() {
                if let Some(sender) = pool.next_sender() {
                    return Ok(sender);
                }
            }
            *guard = None;
        }

        let (pool, ctx) = spawn_worker_pool(self.inner.config.clone())?;
        if !ctx.resolved_plugins.is_empty() {
            *self.inner.resolved_plugins.lock().unwrap() = ctx.resolved_plugins.clone();
        }
        let sender = pool
            .next_sender()
            .ok_or_else(|| anyhow!("Worker pool has no senders"))?;
        *guard = Some(pool);
        Ok(sender)
    }

    async fn send_work_with_retry(
        &self,
        work: WorkFn,
        caller_gone: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), AnyError> {
        let (sender, ticket) = self.get_or_spawn_sender()?;
        let queued = QueuedWork::new(work, ticket, caller_gone.clone());
        match sender.send(queued).await {
            Ok(()) => Ok(()),
            Err(tokio::sync::mpsc::error::SendError(queued)) => {
                let (work, _old_ticket, _) = queued.into_parts();
                let (sender, ticket) = self.get_or_spawn_sender()?;
                sender
                    .send(QueuedWork::new(work, ticket, caller_gone))
                    .await
                    .map_err(|err| anyhow!("Failed to send request after retry: {err}"))
            }
        }
    }

    pub(crate) async fn run_on_worker<R: Send + 'static>(
        &self,
        work: impl FnOnce(
                &mut InnerVlConverter,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<R, AnyError>> + '_>>
            + Send
            + 'static,
    ) -> Result<R, AnyError> {
        let caller_gone = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (resp_tx, resp_rx) = oneshot::channel::<Result<R, AnyError>>();
        let boxed_work: WorkFn = Box::new(move |inner| {
            Box::pin(async move {
                if let Err(e) = inner.refresh_font_config_if_needed() {
                    let _ = resp_tx.send(Err(e));
                    return;
                }
                let result = work(inner).await;
                // Decorate heap-limit and timeout errors
                let result = inner.annotate_heap_limit_error(result);
                let result = inner.annotate_timeout_error(result);
                let _ = resp_tx.send(result);
            })
        });

        self.send_work_with_retry(boxed_work, caller_gone.clone())
            .await?;

        // Guard signals caller_gone on drop (HTTP timeout / client disconnect).
        // Disarmed on normal completion so the timer thread doesn't spuriously
        // terminate V8.
        let mut guard = CallerGoneGuard::new(caller_gone);
        let result = resp_rx.await.map_err(|e| anyhow!("Worker dropped: {e}"));
        guard.disarm();
        result?
    }

    /// Run a conversion on an ephemeral worker with a per-request plugin.
    ///
    /// Uses a oneshot channel (like `run_on_worker`) so the caller awaits a
    /// future. If that future is dropped (e.g. HTTP disconnect), the
    /// `CallerGoneGuard` signals the timer thread to terminate V8.
    async fn run_on_ephemeral_worker<R: Send + 'static>(
        &self,
        plugin_source: String,
        work: impl FnOnce(
                &mut InnerVlConverter,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<R, AnyError>> + '_>>
            + Send
            + 'static,
    ) -> Result<R, AnyError> {
        if !self.inner.config.allow_per_request_plugins {
            bail!(
                "Per-request plugins are disabled. Set allow_per_request_plugins=true \
                 in the converter config to enable."
            );
        }

        // Acquire ephemeral worker permit if a semaphore is configured.
        // The permit is moved into the spawned thread and released when it completes.
        let _ephemeral_permit = match &self.inner.ephemeral_semaphore {
            Some(sem) => Some(
                sem.clone()
                    .acquire_owned()
                    .await
                    .map_err(|_| anyhow!("Ephemeral worker semaphore closed unexpectedly"))?,
            ),
            None => None,
        };

        // Resolve config-level plugins if needed
        if !self.inner.config.vega_plugins.is_empty() {
            self.warm_up()?;
        }
        let resolved_plugins = self
            .inner
            .resolved_plugins
            .lock()
            .map_err(|e| anyhow!("Failed to lock resolved_plugins: {e}"))?
            .clone();
        let parsed_allowed_base_urls = parse_allowed_base_urls_from_config(&self.inner.config)?;
        let ctx = Arc::new(ConverterContext {
            config: (*self.inner.config).clone(),
            parsed_allowed_base_urls,
            resolved_plugins,
        });

        let font_baseline = get_font_baseline_snapshot()?;
        let caller_gone = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let caller_gone_inner = caller_gone.clone();
        let (resp_tx, resp_rx) = oneshot::channel::<Result<R, AnyError>>();

        std::thread::spawn(move || {
            let _permit = _ephemeral_permit;
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| anyhow!("Failed to build ephemeral worker runtime: {e}"));
            let rt = match rt {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = resp_tx.send(Err(e));
                    return;
                }
            };
            let local = tokio::task::LocalSet::new();
            let result = local.block_on(&rt, async move {
                let timeout_secs = ctx.config.max_v8_execution_time_secs;
                let deadline = timeout_secs
                    .map(|n| std::time::Instant::now() + std::time::Duration::from_secs(n.get()));

                // Wrap plugin resolution in tokio::time::timeout if a deadline is set,
                // since terminate_execution() can't help before V8 exists.
                let resolved = if let Some(deadline) = deadline {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    let configured_secs = timeout_secs.map(|n| n.get()).unwrap_or(0);
                    tokio::time::timeout(
                        remaining,
                        resolve_plugin(
                            &plugin_source,
                            &ctx.config.per_request_plugin_import_domains,
                        ),
                    )
                    .await
                    .map_err(|_| {
                        anyhow!(
                            "Conversion timed out during plugin resolution \
                             (configured: {configured_secs} seconds). \
                             Increase max_v8_execution_time_secs or omit for no limit."
                        )
                    })?
                    .map_err(|e| anyhow!("Per-request plugin bundling failed: {e}"))?
                } else {
                    resolve_plugin(
                        &plugin_source,
                        &ctx.config.per_request_plugin_import_domains,
                    )
                    .await
                    .map_err(|e| anyhow!("Per-request plugin bundling failed: {e}"))?
                };

                let mut inner = InnerVlConverter::try_new(ctx, font_baseline).await?;

                // Arm the V8 watchdog timer with remaining budget
                let timer = if let Some(deadline) = deadline {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    inner.start_conversion_timer_with_duration(remaining, caller_gone_inner)
                } else {
                    None
                };

                // Run init, plugin load, and conversion under the timer.
                let result = async {
                    inner.init_vega().await?;
                    let plugin_index = inner.ctx.resolved_plugins.len();
                    inner
                        .load_plugin(plugin_index, &resolved.bundled_source, false)
                        .await?;
                    work(&mut inner).await
                }
                .await;

                inner.cancel_conversion_timer(timer);
                inner
                    .worker
                    .js_runtime
                    .v8_isolate()
                    .cancel_terminate_execution();
                let result = inner.annotate_timeout_error(result);
                inner.reset_timeout_if_needed();
                result
            });
            let _ = resp_tx.send(result);
        });

        let mut guard = CallerGoneGuard::new(caller_gone);
        let result = resp_rx
            .await
            .map_err(|_| anyhow!("Ephemeral worker thread panicked"));
        guard.disarm();
        result?
    }

    fn should_preprocess_fonts(&self) -> bool {
        self.inner.config.auto_google_fonts
            || self.inner.config.missing_fonts != MissingFontsPolicy::Fallback
    }

    /// If font preprocessing is enabled, compile VL→Vega and process referenced fonts.
    ///
    /// Returns `Some((vega_spec, vg_opts))` with the compiled Vega spec and options
    /// for the caller to render directly, or `None` when both font options are disabled.
    async fn maybe_compile_vl_with_preprocessed_fonts(
        &self,
        vl_spec: &ValueOrString,
        vl_opts: &VlOpts,
    ) -> Result<Option<(serde_json::Value, VgOpts, Vec<LogEntry>)>, AnyError> {
        if !self.should_preprocess_fonts() {
            return Ok(None);
        }
        let mut vg_opts = VgOpts {
            format_locale: vl_opts.format_locale.clone(),
            time_format_locale: vl_opts.time_format_locale.clone(),
            google_fonts: vl_opts.google_fonts.clone(),
            vega_plugin: vl_opts.vega_plugin.clone(),
            config: vl_opts.config.clone(),
            background: vl_opts.background.clone(),
            width: vl_opts.width,
            height: vl_opts.height,
        };
        let vega_output = self
            .vegalite_to_vega(vl_spec.clone(), vl_opts.clone())
            .await?;
        let vega_spec = vega_output.spec;
        let compile_logs = vega_output.logs;
        let auto_requests = preprocess_fonts(
            &vega_spec,
            self.inner.config.auto_google_fonts,
            self.inner.config.missing_fonts,
        )
        .await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }
        Ok(Some((vega_spec, vg_opts, compile_logs)))
    }

    /// If font preprocessing is enabled, parse the Vega spec and process missing fonts.
    ///
    /// Note: font downloads are governed solely by `auto_google_fonts`, independently
    /// of `allowed_base_urls`. The two settings control different concerns:
    /// `allowed_base_urls` governs data-fetching URLs in specs, while `auto_google_fonts`
    /// governs on-demand font installation from Google Fonts.
    async fn maybe_preprocess_vega_fonts(
        &self,
        spec: &ValueOrString,
    ) -> Result<Vec<GoogleFontRequest>, AnyError> {
        if self.should_preprocess_fonts() {
            let spec_value: serde_json::Value = match spec {
                ValueOrString::JsonString(s) => serde_json::from_str(s)?,
                ValueOrString::Value(v) => v.clone(),
            };
            preprocess_fonts(
                &spec_value,
                self.inner.config.auto_google_fonts,
                self.inner.config.missing_fonts,
            )
            .await
        } else {
            Ok(Vec::new())
        }
    }

    /// If font preprocessing is enabled, extract fonts from the SVG and resolve
    /// them via Google Fonts. Returns loaded font batches ready for overlay.
    /// Extract font requests from an SVG for worker-side resolution.
    async fn preprocess_svg_font_requests(
        &self,
        svg: &str,
    ) -> Result<Option<Vec<GoogleFontRequest>>, AnyError> {
        if !self.should_preprocess_fonts() {
            return Ok(None);
        }

        let font_strings = crate::extract::extract_fonts_from_svg(svg);
        let auto_requests = classify_and_request_fonts(
            font_strings,
            self.inner.config.auto_google_fonts,
            self.inner.config.missing_fonts,
            false,
        )
        .await?;

        if auto_requests.is_empty() {
            Ok(None)
        } else {
            Ok(Some(auto_requests))
        }
    }

    /// Apply config-level defaults to VlOpts where the per-request value is None.
    pub(crate) fn apply_vl_defaults(&self, opts: &mut VlOpts) {
        let config = &self.inner.config;
        if opts.theme.is_none() {
            opts.theme = config.default_theme.clone();
        }
        if opts.format_locale.is_none() {
            opts.format_locale = config.default_format_locale.clone();
        }
        if opts.time_format_locale.is_none() {
            opts.time_format_locale = config.default_time_format_locale.clone();
        }
    }

    /// Apply config-level defaults to VgOpts where the per-request value is None.
    pub(crate) fn apply_vg_defaults(&self, opts: &mut VgOpts) {
        let config = &self.inner.config;
        if opts.format_locale.is_none() {
            opts.format_locale = config.default_format_locale.clone();
        }
        if opts.time_format_locale.is_none() {
            opts.time_format_locale = config.default_time_format_locale.clone();
        }
    }

    pub async fn vegalite_to_vega(
        &self,
        vl_spec: impl Into<ValueOrString>,
        mut vl_opts: VlOpts,
    ) -> Result<VegaOutput, AnyError> {
        self.apply_vl_defaults(&mut vl_opts);
        let vl_spec = vl_spec.into();
        self.run_on_worker(move |inner| Box::pin(inner.vegalite_to_vega(vl_spec, vl_opts)))
            .await
    }

    pub async fn vega_to_svg(
        &self,
        vg_spec: impl Into<ValueOrString>,
        mut vg_opts: VgOpts,
        svg_opts: SvgOpts,
    ) -> Result<SvgOutput, AnyError> {
        self.apply_vg_defaults(&mut vg_opts);
        let vg_spec = vg_spec.into();
        let plugin = vg_opts.vega_plugin.take();

        let explicit_google_families: HashSet<String> = vg_opts
            .google_fonts
            .as_ref()
            .map(|reqs| reqs.iter().map(|r| r.family.clone()).collect())
            .unwrap_or_default();

        let auto_requests = self.maybe_preprocess_vega_fonts(&vg_spec).await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }

        let mut output = if let Some(plugin_source) = plugin {
            self.run_on_ephemeral_worker(plugin_source, move |inner| {
                let gf = vg_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(inner, gf, inner.vega_to_svg(vg_spec, vg_opts).await)
                })
            })
            .await?
        } else {
            self.run_on_worker(move |inner| {
                let gf = vg_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(inner, gf, inner.vega_to_svg(vg_spec, vg_opts).await)
                })
            })
            .await?
        };

        output.svg = self
            .postprocess_svg(output.svg, &svg_opts, explicit_google_families)
            .await?;
        Ok(output)
    }

    /// Post-process a rendered SVG to embed fonts and/or inline images
    /// according to the given `SvgOpts`.
    ///
    /// Parses the SVG once, classifies fonts, resolves Google Font data, then
    /// delegates to [`crate::svg_font::process_svg`] with all pre-computed data.
    async fn postprocess_svg(
        &self,
        svg: String,
        svg_opts: &SvgOpts,
        explicit_google_families: HashSet<String>,
    ) -> Result<String, AnyError> {
        let config = self.config();
        let image_policy = self.image_access_policy();
        let resources_dir = if config.base_url.is_filesystem() {
            config
                .base_url
                .resolved_url()
                .ok()
                .flatten()
                .and_then(|url_str| Url::parse(&url_str).ok())
                .and_then(|url| url.to_file_path().ok())
        } else {
            None
        };

        // Get fontdb snapshot
        let fontdb = crate::text::USVG_OPTIONS
            .lock()
            .map_err(|e| anyhow!("failed to lock USVG_OPTIONS: {e}"))?
            .fontdb
            .clone();

        // Analyze SVG once — extract fonts, image refs, insertion point
        let analysis = crate::extract::analyze_svg(&svg)?;
        let families: std::collections::BTreeSet<String> =
            analysis.families.iter().cloned().collect();

        // Classify fonts once with proper explicit families
        let classified_fonts = classify_scenegraph_fonts(
            &families,
            config.auto_google_fonts,
            config.embed_local_fonts,
            config.missing_fonts,
            &explicit_google_families,
        )
        .await?;

        // Compute variants once before building Google font requests
        let family_variants = crate::font_embed::variants_by_family(&analysis.chars_by_key);

        // Resolve Google Font batches (needed for @font-face subsetting)
        let google_font_requests: Vec<GoogleFontRequest> = classified_fonts
            .iter()
            .filter_map(|f| match &f.source {
                crate::extract::FontSource::Google { .. } => {
                    let variants = family_variants.get(&f.family).map(|vs| {
                        vs.iter()
                            .map(|(w, s)| vl_convert_google_fonts::VariantRequest {
                                weight: w.parse().unwrap_or(400),
                                style: s
                                    .parse()
                                    .unwrap_or(vl_convert_google_fonts::FontStyle::Normal),
                            })
                            .collect::<Vec<_>>()
                    });
                    Some(GoogleFontRequest {
                        family: f.family.clone(),
                        variants,
                    })
                }
                _ => None,
            })
            .collect();

        let loaded_batches = if google_font_requests.is_empty() {
            Vec::new()
        } else {
            self.run_on_worker(move |inner: &mut InnerVlConverter| {
                Box::pin(inner.resolve_google_fonts(Some(google_font_requests)))
            })
            .await?
        };

        crate::svg_font::process_svg(
            svg,
            svg_opts,
            &analysis,
            &classified_fonts,
            &family_variants,
            &config,
            &fontdb,
            &loaded_batches,
            &image_policy,
            resources_dir.as_deref(),
        )
        .await
    }

    pub async fn vega_to_scenegraph(
        &self,
        vg_spec: impl Into<ValueOrString>,
        mut vg_opts: VgOpts,
    ) -> Result<ScenegraphOutput, AnyError> {
        self.apply_vg_defaults(&mut vg_opts);
        let vg_spec = vg_spec.into();
        let auto_requests = self.maybe_preprocess_vega_fonts(&vg_spec).await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }
        self.run_on_worker(move |inner| {
            let gf = vg_opts.google_fonts.take();
            let inner = &mut *inner;
            Box::pin(async move {
                with_font_overlay!(inner, gf, inner.vega_to_scenegraph(vg_spec, vg_opts).await)
            })
        })
        .await
    }

    pub async fn vega_to_scenegraph_msgpack(
        &self,
        vg_spec: impl Into<ValueOrString>,
        mut vg_opts: VgOpts,
    ) -> Result<ScenegraphMsgpackOutput, AnyError> {
        self.apply_vg_defaults(&mut vg_opts);
        let vg_spec = vg_spec.into();
        let auto_requests = self.maybe_preprocess_vega_fonts(&vg_spec).await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }
        self.run_on_worker(move |inner| {
            let gf = vg_opts.google_fonts.take();
            let inner = &mut *inner;
            Box::pin(async move {
                with_font_overlay!(
                    inner,
                    gf,
                    inner.vega_to_scenegraph_msgpack(vg_spec, vg_opts).await
                )
            })
        })
        .await
    }

    pub async fn vegalite_to_svg(
        &self,
        vl_spec: impl Into<ValueOrString>,
        mut vl_opts: VlOpts,
        svg_opts: SvgOpts,
    ) -> Result<SvgOutput, AnyError> {
        self.apply_vl_defaults(&mut vl_opts);
        let vl_spec = vl_spec.into();
        let plugin = vl_opts.vega_plugin.take();

        let explicit_google_families: HashSet<String> = vl_opts
            .google_fonts
            .as_ref()
            .map(|reqs| reqs.iter().map(|r| r.family.clone()).collect())
            .unwrap_or_default();

        let mut output = if let Some((vega_spec, mut vg_opts, compile_logs)) = self
            .maybe_compile_vl_with_preprocessed_fonts(&vl_spec, &vl_opts)
            .await?
        {
            let vg_spec: ValueOrString = vega_spec.into();
            let mut output = if let Some(plugin_source) = plugin {
                self.run_on_ephemeral_worker(plugin_source, move |inner| {
                    let gf = vg_opts.google_fonts.take();
                    let inner = &mut *inner;
                    Box::pin(async move {
                        with_font_overlay!(inner, gf, inner.vega_to_svg(vg_spec, vg_opts).await)
                    })
                })
                .await?
            } else {
                self.run_on_worker(move |inner| {
                    let gf = vg_opts.google_fonts.take();
                    let inner = &mut *inner;
                    Box::pin(async move {
                        with_font_overlay!(inner, gf, inner.vega_to_svg(vg_spec, vg_opts).await)
                    })
                })
                .await?
            };
            let mut all_logs = compile_logs;
            all_logs.extend(output.logs);
            output.logs = all_logs;
            output
        } else if let Some(plugin_source) = plugin {
            self.run_on_ephemeral_worker(plugin_source, move |inner| {
                let gf = vl_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(inner, gf, inner.vegalite_to_svg(vl_spec, vl_opts).await)
                })
            })
            .await?
        } else {
            self.run_on_worker(move |inner| {
                let gf = vl_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(inner, gf, inner.vegalite_to_svg(vl_spec, vl_opts).await)
                })
            })
            .await?
        };

        output.svg = self
            .postprocess_svg(output.svg, &svg_opts, explicit_google_families)
            .await?;
        Ok(output)
    }

    pub async fn vegalite_to_scenegraph(
        &self,
        vl_spec: impl Into<ValueOrString>,
        mut vl_opts: VlOpts,
    ) -> Result<ScenegraphOutput, AnyError> {
        self.apply_vl_defaults(&mut vl_opts);
        let vl_spec = vl_spec.into();

        if let Some((vega_spec, mut vg_opts, compile_logs)) = self
            .maybe_compile_vl_with_preprocessed_fonts(&vl_spec, &vl_opts)
            .await?
        {
            let vg_spec: ValueOrString = vega_spec.into();
            let mut output = self
                .run_on_worker(move |inner| {
                    let gf = vg_opts.google_fonts.take();
                    let inner = &mut *inner;
                    Box::pin(async move {
                        with_font_overlay!(
                            inner,
                            gf,
                            inner.vega_to_scenegraph(vg_spec, vg_opts).await
                        )
                    })
                })
                .await?;
            let mut all_logs = compile_logs;
            all_logs.extend(output.logs);
            output.logs = all_logs;
            Ok(output)
        } else {
            self.run_on_worker(move |inner| {
                let gf = vl_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        gf,
                        inner.vegalite_to_scenegraph(vl_spec, vl_opts).await
                    )
                })
            })
            .await
        }
    }

    pub async fn vegalite_to_scenegraph_msgpack(
        &self,
        vl_spec: impl Into<ValueOrString>,
        mut vl_opts: VlOpts,
    ) -> Result<ScenegraphMsgpackOutput, AnyError> {
        self.apply_vl_defaults(&mut vl_opts);
        let vl_spec = vl_spec.into();

        if let Some((vega_spec, mut vg_opts, compile_logs)) = self
            .maybe_compile_vl_with_preprocessed_fonts(&vl_spec, &vl_opts)
            .await?
        {
            let vg_spec: ValueOrString = vega_spec.into();
            let mut output = self
                .run_on_worker(move |inner| {
                    let gf = vg_opts.google_fonts.take();
                    let inner = &mut *inner;
                    Box::pin(async move {
                        with_font_overlay!(
                            inner,
                            gf,
                            inner.vega_to_scenegraph_msgpack(vg_spec, vg_opts).await
                        )
                    })
                })
                .await?;
            let mut all_logs = compile_logs;
            all_logs.extend(output.logs);
            output.logs = all_logs;
            Ok(output)
        } else {
            self.run_on_worker(move |inner| {
                let gf = vl_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        gf,
                        inner.vegalite_to_scenegraph_msgpack(vl_spec, vl_opts).await
                    )
                })
            })
            .await
        }
    }

    pub async fn vega_to_png(
        &self,
        vg_spec: impl Into<ValueOrString>,
        mut vg_opts: VgOpts,
        png_opts: PngOpts,
    ) -> Result<PngOutput, AnyError> {
        self.apply_vg_defaults(&mut vg_opts);
        let vg_spec = vg_spec.into();
        let scale = png_opts.scale.unwrap_or(1.0);
        let ppi = png_opts.ppi.unwrap_or(72.0);
        let effective_scale = scale * ppi / 72.0;
        let plugin = vg_opts.vega_plugin.take();

        let auto_requests = self.maybe_preprocess_vega_fonts(&vg_spec).await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }

        if let Some(plugin_source) = plugin {
            self.run_on_ephemeral_worker(plugin_source, move |inner| {
                let gf = vg_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(inner, gf, {
                        let spec_value = vg_spec.to_value()?;
                        inner
                            .vega_to_png(&spec_value, vg_opts, effective_scale, ppi)
                            .await
                    })
                })
            })
            .await
        } else {
            self.run_on_worker(move |inner| {
                let gf = vg_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(inner, gf, {
                        let spec_value = vg_spec.to_value()?;
                        inner
                            .vega_to_png(&spec_value, vg_opts, effective_scale, ppi)
                            .await
                    })
                })
            })
            .await
        }
    }

    pub async fn vegalite_to_png(
        &self,
        vl_spec: impl Into<ValueOrString>,
        mut vl_opts: VlOpts,
        png_opts: PngOpts,
    ) -> Result<PngOutput, AnyError> {
        self.apply_vl_defaults(&mut vl_opts);
        let vl_spec = vl_spec.into();
        let scale = png_opts.scale.unwrap_or(1.0);
        let ppi = png_opts.ppi.unwrap_or(72.0);
        let effective_scale = scale * ppi / 72.0;
        let plugin = vl_opts.vega_plugin.take();

        if let Some((vega_spec, mut vg_opts, compile_logs)) = self
            .maybe_compile_vl_with_preprocessed_fonts(&vl_spec, &vl_opts)
            .await?
        {
            let vg_spec: ValueOrString = vega_spec.into();
            let mut output = if let Some(plugin_source) = plugin {
                self.run_on_ephemeral_worker(plugin_source, move |inner| {
                    let gf = vg_opts.google_fonts.take();
                    let inner = &mut *inner;
                    Box::pin(async move {
                        with_font_overlay!(inner, gf, {
                            let spec_value = vg_spec.to_value()?;
                            inner
                                .vega_to_png(&spec_value, vg_opts, effective_scale, ppi)
                                .await
                        })
                    })
                })
                .await?
            } else {
                self.run_on_worker(move |inner| {
                    let gf = vg_opts.google_fonts.take();
                    let inner = &mut *inner;
                    Box::pin(async move {
                        with_font_overlay!(inner, gf, {
                            let spec_value = vg_spec.to_value()?;
                            inner
                                .vega_to_png(&spec_value, vg_opts, effective_scale, ppi)
                                .await
                        })
                    })
                })
                .await?
            };
            let mut all_logs = compile_logs;
            all_logs.extend(output.logs);
            output.logs = all_logs;
            Ok(output)
        } else if let Some(plugin_source) = plugin {
            self.run_on_ephemeral_worker(plugin_source, move |inner| {
                let gf = vl_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(inner, gf, {
                        let spec_value = vl_spec.to_value()?;
                        inner
                            .vegalite_to_png(&spec_value, vl_opts, effective_scale, ppi)
                            .await
                    })
                })
            })
            .await
        } else {
            self.run_on_worker(move |inner| {
                let gf = vl_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(inner, gf, {
                        let spec_value = vl_spec.to_value()?;
                        inner
                            .vegalite_to_png(&spec_value, vl_opts, effective_scale, ppi)
                            .await
                    })
                })
            })
            .await
        }
    }

    pub async fn vega_to_jpeg(
        &self,
        vg_spec: impl Into<ValueOrString>,
        mut vg_opts: VgOpts,
        jpeg_opts: JpegOpts,
    ) -> Result<JpegOutput, AnyError> {
        self.apply_vg_defaults(&mut vg_opts);
        let scale = jpeg_opts.scale.unwrap_or(1.0);
        let quality = jpeg_opts.quality;
        let vg_spec = vg_spec.into();
        let plugin = vg_opts.vega_plugin.take();

        let auto_requests = self.maybe_preprocess_vega_fonts(&vg_spec).await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }
        let image_policy = self.image_access_policy();

        if let Some(plugin_source) = plugin {
            self.run_on_ephemeral_worker(plugin_source, move |inner| {
                let gf = vg_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        gf,
                        inner
                            .vega_to_jpeg(vg_spec, vg_opts, scale, quality, image_policy)
                            .await
                    )
                })
            })
            .await
        } else {
            self.run_on_worker(move |inner| {
                let gf = vg_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        gf,
                        inner
                            .vega_to_jpeg(vg_spec, vg_opts, scale, quality, image_policy)
                            .await
                    )
                })
            })
            .await
        }
    }

    pub async fn vegalite_to_jpeg(
        &self,
        vl_spec: impl Into<ValueOrString>,
        mut vl_opts: VlOpts,
        jpeg_opts: JpegOpts,
    ) -> Result<JpegOutput, AnyError> {
        self.apply_vl_defaults(&mut vl_opts);
        let scale = jpeg_opts.scale.unwrap_or(1.0);
        let quality = jpeg_opts.quality;
        let vl_spec = vl_spec.into();
        let plugin = vl_opts.vega_plugin.take();
        let image_policy = self.image_access_policy();

        if let Some((vega_spec, mut vg_opts, compile_logs)) = self
            .maybe_compile_vl_with_preprocessed_fonts(&vl_spec, &vl_opts)
            .await?
        {
            let vg_spec: ValueOrString = vega_spec.into();
            let mut output = if let Some(plugin_source) = plugin {
                self.run_on_ephemeral_worker(plugin_source, move |inner| {
                    let gf = vg_opts.google_fonts.take();
                    let inner = &mut *inner;
                    Box::pin(async move {
                        with_font_overlay!(
                            inner,
                            gf,
                            inner
                                .vega_to_jpeg(vg_spec, vg_opts, scale, quality, image_policy)
                                .await
                        )
                    })
                })
                .await?
            } else {
                self.run_on_worker(move |inner| {
                    let gf = vg_opts.google_fonts.take();
                    let inner = &mut *inner;
                    Box::pin(async move {
                        with_font_overlay!(
                            inner,
                            gf,
                            inner
                                .vega_to_jpeg(vg_spec, vg_opts, scale, quality, image_policy)
                                .await
                        )
                    })
                })
                .await?
            };
            let mut all_logs = compile_logs;
            all_logs.extend(output.logs);
            output.logs = all_logs;
            Ok(output)
        } else if let Some(plugin_source) = plugin {
            self.run_on_ephemeral_worker(plugin_source, move |inner| {
                let gf = vl_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        gf,
                        inner
                            .vegalite_to_jpeg(vl_spec, vl_opts, scale, quality, image_policy)
                            .await
                    )
                })
            })
            .await
        } else {
            self.run_on_worker(move |inner| {
                let gf = vl_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        gf,
                        inner
                            .vegalite_to_jpeg(vl_spec, vl_opts, scale, quality, image_policy)
                            .await
                    )
                })
            })
            .await
        }
    }

    pub async fn vega_to_pdf(
        &self,
        vg_spec: impl Into<ValueOrString>,
        mut vg_opts: VgOpts,
        _pdf_opts: PdfOpts,
    ) -> Result<PdfOutput, AnyError> {
        self.apply_vg_defaults(&mut vg_opts);
        let vg_spec = vg_spec.into();
        let plugin = vg_opts.vega_plugin.take();

        let auto_requests = self.maybe_preprocess_vega_fonts(&vg_spec).await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }
        let image_policy = self.image_access_policy();

        if let Some(plugin_source) = plugin {
            self.run_on_ephemeral_worker(plugin_source, move |inner| {
                let gf = vg_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        gf,
                        inner.vega_to_pdf(vg_spec, vg_opts, image_policy).await
                    )
                })
            })
            .await
        } else {
            self.run_on_worker(move |inner| {
                let gf = vg_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        gf,
                        inner.vega_to_pdf(vg_spec, vg_opts, image_policy).await
                    )
                })
            })
            .await
        }
    }

    pub async fn vegalite_to_pdf(
        &self,
        vl_spec: impl Into<ValueOrString>,
        mut vl_opts: VlOpts,
        _pdf_opts: PdfOpts,
    ) -> Result<PdfOutput, AnyError> {
        self.apply_vl_defaults(&mut vl_opts);
        let vl_spec = vl_spec.into();
        let plugin = vl_opts.vega_plugin.take();
        let image_policy = self.image_access_policy();

        if let Some((vega_spec, mut vg_opts, compile_logs)) = self
            .maybe_compile_vl_with_preprocessed_fonts(&vl_spec, &vl_opts)
            .await?
        {
            let vg_spec: ValueOrString = vega_spec.into();
            let mut output = if let Some(plugin_source) = plugin {
                self.run_on_ephemeral_worker(plugin_source, move |inner| {
                    let gf = vg_opts.google_fonts.take();
                    let inner = &mut *inner;
                    Box::pin(async move {
                        with_font_overlay!(
                            inner,
                            gf,
                            inner.vega_to_pdf(vg_spec, vg_opts, image_policy).await
                        )
                    })
                })
                .await?
            } else {
                self.run_on_worker(move |inner| {
                    let gf = vg_opts.google_fonts.take();
                    let inner = &mut *inner;
                    Box::pin(async move {
                        with_font_overlay!(
                            inner,
                            gf,
                            inner.vega_to_pdf(vg_spec, vg_opts, image_policy).await
                        )
                    })
                })
                .await?
            };
            let mut all_logs = compile_logs;
            all_logs.extend(output.logs);
            output.logs = all_logs;
            Ok(output)
        } else if let Some(plugin_source) = plugin {
            self.run_on_ephemeral_worker(plugin_source, move |inner| {
                let gf = vl_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        gf,
                        inner.vegalite_to_pdf(vl_spec, vl_opts, image_policy).await
                    )
                })
            })
            .await
        } else {
            self.run_on_worker(move |inner| {
                let gf = vl_opts.google_fonts.take();
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        gf,
                        inner.vegalite_to_pdf(vl_spec, vl_opts, image_policy).await
                    )
                })
            })
            .await
        }
    }

    pub async fn svg_to_png(&self, svg: &str, png_opts: PngOpts) -> Result<PngOutput, AnyError> {
        let scale = png_opts.scale.unwrap_or(1.0);
        let ppi = png_opts.ppi;
        let image_policy = self.image_access_policy();
        let google_fonts = self.preprocess_svg_font_requests(svg).await?;
        let svg = svg.to_string();
        let data = self
            .run_on_worker(move |inner| {
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        google_fonts,
                        inner.svg_to_png_with_worker_options(&svg, scale, ppi, &image_policy)
                    )
                })
            })
            .await?;
        Ok(PngOutput {
            data,
            logs: Vec::new(),
        })
    }

    pub async fn svg_to_jpeg(
        &self,
        svg: &str,
        jpeg_opts: JpegOpts,
    ) -> Result<JpegOutput, AnyError> {
        let scale = jpeg_opts.scale.unwrap_or(1.0);
        let quality = jpeg_opts.quality;
        let image_policy = self.image_access_policy();
        let google_fonts = self.preprocess_svg_font_requests(svg).await?;
        let svg = svg.to_string();
        let data = self
            .run_on_worker(move |inner| {
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        google_fonts,
                        inner.svg_to_jpeg_with_worker_options(&svg, scale, quality, &image_policy)
                    )
                })
            })
            .await?;
        Ok(JpegOutput {
            data,
            logs: Vec::new(),
        })
    }

    pub async fn svg_to_pdf(&self, svg: &str, _pdf_opts: PdfOpts) -> Result<PdfOutput, AnyError> {
        let image_policy = self.image_access_policy();
        let google_fonts = self.preprocess_svg_font_requests(svg).await?;
        let svg = svg.to_string();
        let data = self
            .run_on_worker(move |inner| {
                let inner = &mut *inner;
                Box::pin(async move {
                    with_font_overlay!(
                        inner,
                        google_fonts,
                        inner.svg_to_pdf_with_worker_options(&svg, &image_policy)
                    )
                })
            })
            .await?;
        Ok(PdfOutput {
            data,
            logs: Vec::new(),
        })
    }

    pub async fn get_vegaembed_bundle(&self, vl_version: VlVersion) -> Result<String, AnyError> {
        if let Some(bundle) = self
            .inner
            .vegaembed_bundles
            .lock()
            .map_err(|e| anyhow!("Failed to lock bundle cache: {e}"))?
            .get(&vl_version)
            .cloned()
        {
            return Ok(bundle);
        }

        let computed_bundle = self
            .run_on_worker(move |_inner: &mut InnerVlConverter| {
                Box::pin(async move {
                    crate::html::bundle_vega_snippet(VEGAEMBED_GLOBAL_SNIPPET, vl_version).await
                })
            })
            .await?;

        let mut guard = self
            .inner
            .vegaembed_bundles
            .lock()
            .map_err(|e| anyhow!("Failed to lock bundle cache: {e}"))?;
        let bundle = match guard.entry(vl_version) {
            Entry::Occupied(occupied) => occupied.get().clone(),
            Entry::Vacant(vacant) => {
                vacant.insert(computed_bundle.clone());
                computed_bundle
            }
        };
        Ok(bundle)
    }

    pub async fn bundle_vega_snippet(
        &self,
        snippet: impl Into<String>,
        vl_version: VlVersion,
    ) -> Result<String, AnyError> {
        let snippet = snippet.into();
        self.run_on_worker(move |_inner: &mut InnerVlConverter| {
            Box::pin(async move { crate::html::bundle_vega_snippet(&snippet, vl_version).await })
        })
        .await
    }

    /// Return V8 memory usage for every worker in the pool.
    ///
    /// Spawns the worker pool if it hasn't been created yet, so callers
    /// always get stats for all configured workers.
    pub async fn get_worker_memory_usage(&self) -> Result<Vec<WorkerMemoryUsage>, AnyError> {
        // Ensure the pool is spawned (same as warm_up).
        self.get_or_spawn_sender()?;

        // Collect senders and outstanding counters while holding the lock,
        // then drop the lock before awaiting sends.
        let worker_senders: Vec<(
            usize,
            tokio::sync::mpsc::Sender<QueuedWork>,
            Arc<AtomicUsize>,
        )> = {
            let guard = self
                .inner
                .pool
                .lock()
                .map_err(|e| anyhow!("Failed to lock worker pool: {e}"))?;

            let pool = guard
                .as_ref()
                .ok_or_else(|| anyhow!("Worker pool not available"))?;

            pool.senders
                .iter()
                .enumerate()
                .filter(|(_, s)| !s.is_closed())
                .map(|(idx, s)| (idx, s.clone(), pool.outstanding[idx].clone()))
                .collect()
        };

        let mut receivers = Vec::with_capacity(worker_senders.len());
        for (idx, sender, outstanding) in worker_senders {
            let (resp_tx, resp_rx) = oneshot::channel::<WorkerMemoryUsage>();
            let ticket = OutstandingTicket::new(outstanding);
            let work: WorkFn = Box::new(move |inner: &mut InnerVlConverter| {
                Box::pin(async move {
                    let stats = inner.worker.js_runtime.v8_isolate().get_heap_statistics();
                    resp_tx
                        .send(WorkerMemoryUsage {
                            worker_index: idx,
                            used_heap_size: stats.used_heap_size(),
                            total_heap_size: stats.total_heap_size(),
                            heap_size_limit: stats.heap_size_limit(),
                            external_memory: stats.external_memory(),
                        })
                        .ok();
                })
            });
            sender
                .send(QueuedWork::new(
                    work,
                    ticket,
                    Arc::new(std::sync::atomic::AtomicBool::new(false)),
                ))
                .await
                .map_err(|e| anyhow!("Failed to send GetMemoryUsage to worker {idx}: {e}"))?;
            receivers.push(resp_rx);
        }

        let mut results = Vec::with_capacity(receivers.len());
        for rx in receivers {
            match rx.await {
                Ok(stats) => results.push(stats),
                Err(e) => return Err(anyhow!("Failed to receive memory usage: {e}")),
            }
        }
        Ok(results)
    }

    pub async fn get_local_tz(&self) -> Result<Option<String>, AnyError> {
        self.run_on_worker(|inner| Box::pin(inner.get_local_tz()))
            .await
    }

    pub async fn get_themes(&self) -> Result<serde_json::Value, AnyError> {
        self.run_on_worker(|inner| Box::pin(inner.get_themes()))
            .await
    }
}

impl Default for VlConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::get_font_baseline_snapshot;
    use inner::tests::{TestHttpResponse, TestHttpServer, PNG_1X1_BYTES};
    use serde_json::json;
    use std::future::Future;
    use worker_pool::WorkerPool;

    fn assert_send_future<F: Future + Send>(_: F) {}

    fn write_test_png(path: &std::path::Path) {
        std::fs::write(path, PNG_1X1_BYTES).unwrap();
    }

    fn svg_with_href(href: &str) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image href="{href}" width="1" height="1"/></svg>"#
        )
    }

    fn vega_spec_with_data_url(url: &str) -> serde_json::Value {
        json!({
            "$schema": "https://vega.github.io/schema/vega/v5.json",
            "width": 20,
            "height": 20,
            "data": [{"name": "table", "url": url, "format": {"type": "csv"}}],
            "scales": [
                {"name": "x", "type": "linear", "range": "width", "domain": {"data": "table", "field": "a"}},
                {"name": "y", "type": "linear", "range": "height", "domain": {"data": "table", "field": "b"}}
            ],
            "marks": [{
                "type": "symbol",
                "from": {"data": "table"},
                "encode": {
                    "enter": {
                        "x": {"scale": "x", "field": "a"},
                        "y": {"scale": "y", "field": "b"}
                    }
                }
            }]
        })
    }

    fn vegalite_spec_with_data_url(url: &str) -> serde_json::Value {
        json!({
            "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
            "data": {"url": url},
            "mark": "point",
            "encoding": {
                "x": {"field": "a", "type": "quantitative"},
                "y": {"field": "b", "type": "quantitative"}
            }
        })
    }

    fn vegalite_spec_with_image_url(url: &str) -> serde_json::Value {
        json!({
            "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
            "data": {
                "values": [
                    {"img": url}
                ]
            },
            "mark": {"type": "image", "width": 20, "height": 20},
            "encoding": {
                "x": {"value": 10},
                "y": {"value": 10},
                "url": {"field": "img", "type": "nominal"}
            }
        })
    }

    #[test]
    fn test_config_reports_configured_num_workers() {
        use std::num::NonZeroU64;
        let converter = VlConverter::with_config(VlcConfig {
            num_workers: NonZeroU64::new(4).unwrap(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(converter.config().num_workers.get(), 4);
    }

    #[test]
    fn test_get_or_spawn_sender_respawns_closed_pool_without_explicit_reset() {
        use std::num::NonZeroU64;
        let num_workers: usize = 2;
        let converter = VlConverter::with_config(VlcConfig {
            num_workers: NonZeroU64::new(num_workers as u64).unwrap(),
            ..Default::default()
        })
        .unwrap();

        let mut closed_senders = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            let (sender, receiver) = tokio::sync::mpsc::channel::<QueuedWork>(1);
            drop(receiver);
            closed_senders.push(sender);
        }

        let closed_pool = WorkerPool {
            senders: closed_senders,
            outstanding: (0..num_workers)
                .map(|_| std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)))
                .collect(),
            dispatch_cursor: std::sync::atomic::AtomicUsize::new(0),
            _handles: Vec::new(),
        };

        {
            let mut guard = converter.inner.pool.lock().unwrap();
            *guard = Some(closed_pool);
        }

        let _ = converter.get_or_spawn_sender().unwrap();

        let guard = converter.inner.pool.lock().unwrap();
        let pool = guard
            .as_ref()
            .expect("get_or_spawn_sender should replace closed pool with a live pool");
        assert_eq!(pool.senders.len(), num_workers);
        assert!(!pool.is_closed(), "respawned pool should be open");
    }

    #[test]
    fn test_get_or_spawn_sender_spawns_pool_without_request() {
        use std::num::NonZeroU64;
        let converter = VlConverter::with_config(VlcConfig {
            num_workers: NonZeroU64::new(2).unwrap(),
            ..Default::default()
        })
        .unwrap();

        {
            let guard = converter.inner.pool.lock().unwrap();
            assert!(guard.is_none(), "pool should start uninitialized");
        }

        let _ = converter.get_or_spawn_sender().unwrap();

        {
            let guard = converter.inner.pool.lock().unwrap();
            let pool = guard
                .as_ref()
                .expect("pool should be initialized by get_or_spawn_sender");
            assert_eq!(pool.senders.len(), 2);
            assert!(!pool.is_closed(), "warmed pool should have open senders");
            assert_eq!(
                pool.outstanding
                    .iter()
                    .map(|outstanding| outstanding.load(std::sync::atomic::Ordering::Relaxed))
                    .sum::<usize>(),
                0,
                "get_or_spawn_sender should not leave outstanding reservations"
            );
        }
    }

    #[tokio::test]
    async fn test_get_or_spawn_sender_is_idempotent() {
        use std::num::NonZeroU64;
        let converter = VlConverter::with_config(VlcConfig {
            num_workers: NonZeroU64::new(2).unwrap(),
            ..Default::default()
        })
        .unwrap();
        let _ = converter.get_or_spawn_sender().unwrap();
        let _ = converter.get_or_spawn_sender().unwrap();

        let vl_spec = serde_json::json!({
            "data": {"values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]},
            "mark": "bar",
            "encoding": {
                "x": {"field": "a", "type": "nominal"},
                "y": {"field": "b", "type": "quantitative"}
            }
        });

        let output = converter
            .vegalite_to_svg(
                vl_spec,
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
                SvgOpts::default(),
            )
            .await
            .unwrap();
        assert!(output.svg.trim_start().starts_with("<svg"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_parallel_conversions_with_shared_converter() {
        use std::num::NonZeroU64;
        let converter = VlConverter::with_config(VlcConfig {
            num_workers: NonZeroU64::new(4).unwrap(),
            ..Default::default()
        })
        .unwrap();
        let vl_spec = serde_json::json!({
            "data": {"values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]},
            "mark": "bar",
            "encoding": {
                "x": {"field": "a", "type": "nominal"},
                "y": {"field": "b", "type": "quantitative"}
            }
        });

        let mut tasks = Vec::new();
        for _ in 0..16 {
            let converter = converter.clone();
            let vl_spec = vl_spec.clone();
            tasks.push(tokio::spawn(async move {
                converter
                    .vegalite_to_svg(
                        vl_spec,
                        VlOpts {
                            vl_version: VlVersion::v5_16,
                            ..Default::default()
                        },
                        SvgOpts::default(),
                    )
                    .await
            }));
        }

        for task in tasks {
            let output = task.await.unwrap().unwrap();
            assert!(output.svg.trim_start().starts_with("<svg"));
        }
    }

    #[tokio::test]
    async fn test_font_version_propagation() {
        use crate::text::{current_font_directories, set_font_directories, FONT_CONFIG_VERSION};
        use std::path::PathBuf;
        use std::sync::atomic::Ordering;

        // Do an initial conversion to ensure the worker is running.
        // `VlConverter::new()` calls `set_font_directories` on construction,
        // which also bumps FONT_CONFIG_VERSION; snapshot the version *after*
        // construction so we're measuring only the explicit
        // `set_font_directories` bump below.
        let ctx = VlConverter::new();
        let vl_spec: serde_json::Value = serde_json::from_str(
            r#"{
                "data": {"values": [{"a": 1}]},
                "mark": "point",
                "encoding": {"x": {"field": "a", "type": "quantitative"}}
            }"#,
        )
        .unwrap();
        ctx.vegalite_to_vega(
            vl_spec.clone(),
            VlOpts {
                vl_version: VlVersion::v5_16,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let version_before = FONT_CONFIG_VERSION.load(Ordering::Acquire);

        // Append a font directory via `set_font_directories` (re-registers
        // the built-in fonts plus liberation-sans, which is harmless).
        let mut paths = current_font_directories();
        paths.push(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fonts/liberation-sans"
        )));
        set_font_directories(&paths).unwrap();

        let version_after = FONT_CONFIG_VERSION.load(Ordering::Acquire);
        assert_eq!(
            version_after,
            version_before + 1,
            "FONT_CONFIG_VERSION should increment after set_font_directories"
        );

        // A subsequent conversion should still succeed, confirming the worker
        // picked up the font config change without dying
        let ctx2 = VlConverter::new();
        ctx2.vegalite_to_vega(
            vl_spec,
            VlOpts {
                vl_version: VlVersion::v5_16,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }

    #[test]
    fn test_html_and_bundle_futures_are_send() {
        let converter = VlConverter::new();
        let vl_spec = serde_json::json!({
            "data": {"values": [{"a": "A", "b": 1}]},
            "mark": "bar",
            "encoding": {"x": {"field": "a", "type": "nominal"}}
        });
        let vg_spec = serde_json::json!({
            "$schema": "https://vega.github.io/schema/vega/v6.json",
            "data": [{"name": "table", "values": [{"x": "A", "y": 1}]}],
            "marks": [{"type": "rect", "from": {"data": "table"}}]
        });

        assert_send_future(converter.get_vegaembed_bundle(VlVersion::v5_16));
        assert_send_future(
            converter.bundle_vega_snippet("window.__vlcBundleMarker = 'ok';", VlVersion::v5_16),
        );
        assert_send_future(converter.vegalite_to_html(
            vl_spec,
            VlOpts {
                vl_version: VlVersion::v5_16,
                ..Default::default()
            },
            HtmlOpts {
                bundle: true,
                renderer: Renderer::Svg,
            },
        ));
        assert_send_future(converter.vega_to_html(
            vg_spec,
            VgOpts::default(),
            HtmlOpts {
                bundle: true,
                renderer: Renderer::Svg,
            },
        ));
    }

    #[tokio::test]
    async fn test_get_vegaembed_bundle_caches_result() {
        use std::num::NonZeroU64;
        let converter = VlConverter::with_config(VlcConfig {
            num_workers: NonZeroU64::new(1).unwrap(),
            ..Default::default()
        })
        .unwrap();

        let first = converter
            .get_vegaembed_bundle(VlVersion::v5_16)
            .await
            .unwrap();
        let len_after_first = converter.inner.vegaembed_bundles.lock().unwrap().len();

        let second = converter
            .get_vegaembed_bundle(VlVersion::v5_16)
            .await
            .unwrap();
        let len_after_second = converter.inner.vegaembed_bundles.lock().unwrap().len();

        assert_eq!(first, second);
        assert_eq!(len_after_first, 1);
        assert_eq!(len_after_second, 1);
    }

    #[tokio::test]
    async fn test_bundle_vega_snippet_custom_snippet() {
        use std::num::NonZeroU64;
        let converter = VlConverter::with_config(VlcConfig {
            num_workers: NonZeroU64::new(1).unwrap(),
            ..Default::default()
        })
        .unwrap();
        let snippet = "window.__vlcBundleMarker = 'ok';";

        let bundle = converter
            .bundle_vega_snippet(snippet, VlVersion::v5_16)
            .await
            .unwrap();

        assert!(bundle.contains("__vlcBundleMarker"));
    }

    #[tokio::test]
    async fn test_svg_helper_denies_subdomain_and_userinfo_url_confusion() {
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec!["https://example.com".to_string()],
            ..Default::default()
        })
        .unwrap();

        let subdomain_err = converter
            .svg_to_png(
                &svg_with_href("https://example.com.evil.test/image.png"),
                PngOpts {
                    scale: Some(1.0),
                    ppi: None,
                },
            )
            .await
            .unwrap_err();
        assert!(subdomain_err
            .to_string()
            .contains("External data url not allowed"));

        let userinfo_err = converter
            .svg_to_png(
                &svg_with_href("https://example.com@evil.test/image.png"),
                PngOpts {
                    scale: Some(1.0),
                    ppi: None,
                },
            )
            .await
            .unwrap_err();
        assert!(userinfo_err
            .to_string()
            .contains("External data url not allowed"));
    }

    #[tokio::test]
    async fn test_svg_helper_denies_local_paths_without_filesystem_root() {
        let temp_dir = tempfile::tempdir().unwrap();
        let local_image_path = temp_dir.path().join("image.png");
        write_test_png(&local_image_path);
        let href = Url::from_file_path(&local_image_path).unwrap().to_string();

        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec![],
            ..Default::default()
        })
        .unwrap();

        let err = converter
            .svg_to_png(
                &svg_with_href(&href),
                PngOpts {
                    scale: Some(1.0),
                    ppi: None,
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Filesystem access denied"));
    }

    #[tokio::test]
    async fn test_svg_helper_enforces_filesystem_root() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path().join("root");
        std::fs::create_dir_all(&root).unwrap();

        let inside_path = root.join("inside.png");
        write_test_png(&inside_path);
        let outside_path = temp_dir.path().join("outside.png");
        write_test_png(&outside_path);

        let converter = VlConverter::with_config(VlcConfig {
            base_url: BaseUrlSetting::Custom(root.to_string_lossy().to_string()),
            allowed_base_urls: vec![root.to_string_lossy().to_string()],
            ..Default::default()
        })
        .unwrap();

        let allowed = converter
            .svg_to_png(
                &svg_with_href("inside.png"),
                PngOpts {
                    scale: Some(1.0),
                    ppi: None,
                },
            )
            .await;
        assert!(allowed.is_ok());

        let outside_href = Url::from_file_path(&outside_path).unwrap().to_string();
        let err = converter
            .svg_to_png(
                &svg_with_href(&outside_href),
                PngOpts {
                    scale: Some(1.0),
                    ppi: None,
                },
            )
            .await
            .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("filesystem_root") || message.contains("access denied"));

        let err = converter
            .svg_to_png(
                &svg_with_href("../outside.png"),
                PngOpts {
                    scale: Some(1.0),
                    ppi: None,
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("filesystem_root"));
    }

    #[tokio::test]
    async fn test_svg_helper_enforces_http_access_and_allowed_base_urls() {
        let remote_svg = svg_with_href("https://example.com/image.png");

        let no_http_converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec![],
            ..Default::default()
        })
        .unwrap();
        let err = no_http_converter
            .svg_to_png(
                &remote_svg,
                PngOpts {
                    scale: Some(1.0),
                    ppi: None,
                },
            )
            .await
            .unwrap_err();
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("vlc_access_denied")
                || msg.contains("http access denied")
                || msg.contains("not allowed"),
            "expected access denied, got: {msg}"
        );

        let allowlisted_converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec!["https://allowed.example/".to_string()],
            ..Default::default()
        })
        .unwrap();
        let err = allowlisted_converter
            .svg_to_png(
                &remote_svg,
                PngOpts {
                    scale: Some(1.0),
                    ppi: None,
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("External data url not allowed"));
    }

    #[tokio::test]
    async fn test_svg_helper_allows_data_uri_when_http_disabled() {
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec![],
            ..Default::default()
        })
        .unwrap();
        let svg = svg_with_href(
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/w8AAgMBgLP4r9kAAAAASUVORK5CYII=",
        );
        let output = converter
            .svg_to_png(
                &svg,
                PngOpts {
                    scale: Some(1.0),
                    ppi: None,
                },
            )
            .await
            .unwrap();
        assert!(output.data.starts_with(&[137, 80, 78, 71]));
    }

    #[tokio::test]
    async fn test_vega_to_pdf_denies_http_access() {
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec![],
            ..Default::default()
        })
        .unwrap();
        let spec = vega_spec_with_data_url("https://example.com/data.csv");

        let err = converter
            .vega_to_pdf(spec, VgOpts::default(), PdfOpts::default())
            .await
            .unwrap_err();
        let message = err.to_string().to_ascii_lowercase();
        assert!(
            message.contains("vlc_access_denied")
                || message.contains("http access denied")
                || message.contains("requires net access")
                || message.contains("permission"),
            "expected access denied error, got: {message}"
        );
    }

    #[tokio::test]
    async fn test_vega_loader_allows_data_uri_when_http_disabled() {
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec![],
            ..Default::default()
        })
        .unwrap();
        let spec = vega_spec_with_data_url("data:text/csv,a,b%0A1,2");

        let output = converter
            .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
            .await
            .unwrap();
        assert!(output.svg.contains("<svg"));
    }

    #[tokio::test]
    async fn test_vega_loader_rejects_unsupported_sanitized_scheme() {
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec!["*".to_string()],
            ..Default::default()
        })
        .unwrap();
        let spec = vega_spec_with_data_url("ftp://example.com/data.csv");

        let err = converter
            .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
            .await
            .unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("Unsupported data URL target after Vega loader sanitize")
                && message.contains("ftp://example.com/data.csv"),
            "expected unsupported sanitized target error, got: {message}"
        );
    }

    #[tokio::test]
    async fn test_vegalite_to_png_canvas_image_denies_http_access() {
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec![],
            ..Default::default()
        })
        .unwrap();
        let spec = vegalite_spec_with_image_url("https://example.com/image.png");

        // With HTTP denied, the canvas Image class catches the op error and
        // fires onerror. The conversion succeeds but the image is not rendered.
        let result = converter
            .vegalite_to_png(
                spec,
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
                PngOpts {
                    scale: Some(1.0),
                    ppi: Some(72.0),
                },
            )
            .await;
        // The conversion should succeed (image just not loaded)
        assert!(
            result.is_ok(),
            "conversion should succeed even with denied image"
        );
    }

    #[tokio::test]
    async fn test_vegalite_to_png_canvas_image_enforces_allowed_base_urls() {
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec!["https://allowed.example/".to_string()],
            ..Default::default()
        })
        .unwrap();
        let spec = vegalite_spec_with_image_url("https://example.com/image.png");

        // With allowlist not including example.com, the op denies the fetch.
        // Canvas Image catches the error; the conversion succeeds without the image.
        let result = converter
            .vegalite_to_png(
                spec,
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
                PngOpts {
                    scale: Some(1.0),
                    ppi: Some(72.0),
                },
            )
            .await;
        assert!(
            result.is_ok(),
            "conversion should succeed even with denied image"
        );
    }

    #[tokio::test]
    async fn test_vega_to_pdf_denies_disallowed_base_url() {
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec!["https://allowed.example/".to_string()],
            ..Default::default()
        })
        .unwrap();
        let spec = vega_spec_with_data_url("https://example.com/data.csv");

        let err = converter
            .vega_to_pdf(spec, VgOpts::default(), PdfOpts::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("External data url not allowed"));
    }

    #[tokio::test]
    async fn test_vegalite_to_pdf_denies_filesystem_access_outside_root() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        let outside_csv = temp_dir.path().join("outside.csv");
        std::fs::write(&outside_csv, "a,b\n1,2\n").unwrap();
        let outside_file_url = Url::from_file_path(&outside_csv).unwrap().to_string();

        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec![root.to_string_lossy().to_string()],
            ..Default::default()
        })
        .unwrap();
        let vl_spec = vegalite_spec_with_data_url(&outside_file_url);

        let err = converter
            .vegalite_to_pdf(
                vl_spec,
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
                PdfOpts::default(),
            )
            .await
            .unwrap_err();
        let message = err.to_string().to_ascii_lowercase();
        assert!(
            message.contains("filesystem access denied")
                || message.contains("requires read access")
                || message.contains("permission")
        );
    }

    #[tokio::test]
    async fn test_loader_blocks_percent_encoded_filesystem_traversal() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path().join("root");
        std::fs::create_dir_all(root.join("subdir")).unwrap();
        let outside_csv = temp_dir.path().join("outside.csv");
        std::fs::write(&outside_csv, "a,b\n1,2\n").unwrap();

        let converter = VlConverter::with_config(VlcConfig {
            base_url: BaseUrlSetting::Custom(root.to_string_lossy().to_string()),
            allowed_base_urls: vec![root.to_string_lossy().to_string()],
            ..Default::default()
        })
        .unwrap();

        let spec = vegalite_spec_with_data_url("subdir/..%2F..%2Foutside.csv");
        let err = converter
            .vegalite_to_svg(
                spec,
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
                SvgOpts::default(),
            )
            .await
            .unwrap_err();
        let err_msg = err.to_string();
        // The percent-encoded traversal must fail: either the Rust op rejects it
        // with an access-denied error, or the literal %2F path doesn't exist.
        assert!(
            err_msg.contains(&format!("{ACCESS_DENIED_MARKER}: Filesystem access denied"))
                || err_msg.contains("No such file or directory"),
            "Expected access denied or file-not-found, got: {err_msg}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_vegalite_to_pdf_config_allowlist_for_svg_rasterization() {
        let server = TestHttpServer::new(vec![(
            "/image.svg",
            TestHttpResponse::ok_svg(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#,
            ),
        )]);
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec![server.origin()],
            ..Default::default()
        })
        .unwrap();

        let output = converter
            .vegalite_to_pdf(
                vegalite_spec_with_image_url(&server.url("/image.svg")),
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
                PdfOpts::default(),
            )
            .await
            .unwrap();
        assert!(output.data.starts_with(b"%PDF"));
    }

    #[tokio::test]
    async fn test_base_url_disabled_blocks_relative_paths() {
        let converter = VlConverter::with_config(VlcConfig {
            base_url: BaseUrlSetting::Disabled,
            ..Default::default()
        })
        .unwrap();

        // Spec with a relative data URL that is used by marks, forcing the
        // loader to resolve it against the disabled base URL.
        let spec = vega_spec_with_data_url("data/cars.json");

        let err = converter
            .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
            .await
            .unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("Unsupported data URL target after Vega loader sanitize")
                && message.contains("about:invalid"),
            "expected disabled base URL to produce unsupported target error, got: {message}"
        );
    }

    #[tokio::test]
    async fn test_sandbox_lockdown_blocks_fetch() {
        // Verify that JS code calling fetch() directly gets a permission error
        let mut ctx = InnerVlConverter::try_new(
            std::sync::Arc::new(ConverterContext {
                config: VlcConfig::default(),
                parsed_allowed_base_urls: Vec::new(),
                resolved_plugins: Vec::new(),
            }),
            get_font_baseline_snapshot().unwrap(),
        )
        .await
        .unwrap();

        let code = r#"
    var __fetchResult = null;
    (async () => {
    try {
        await fetch("https://example.com");
        __fetchResult = "success";
    } catch (e) {
        __fetchResult = e.message || String(e);
    }
    })();
    "#;
        ctx.worker
            .js_runtime
            .execute_script("ext:<anon>", code.to_string())
            .unwrap();
        ctx.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await
            .unwrap();

        let result = ctx.execute_script_to_json("__fetchResult").await.unwrap();
        let msg = result.as_str().unwrap_or_default().to_lowercase();
        assert!(
            msg.contains("permission") || msg.contains("denied") || msg.contains("requires net"),
            "fetch() should be blocked by Deno permissions, got: {msg}"
        );
    }

    /// `allowed_base_urls = vec![]` must block all network data.
    #[tokio::test]
    async fn allowed_base_urls_empty_blocks_all() {
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec![],
            ..Default::default()
        })
        .unwrap();
        let spec = vega_spec_with_data_url("https://example.com/data.csv");
        let err = converter
            .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
            .await
            .unwrap_err();
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("denied") || msg.contains("not allowed") || msg.contains("requires net"),
            "empty allowlist should block network data, got: {msg}"
        );
    }

    /// Callers restore the pre-Task-0 "any http/https" default by passing a
    /// scheme allowlist.
    #[tokio::test(flavor = "multi_thread")]
    async fn allowed_base_urls_scheme_allowlist_works() {
        let server =
            TestHttpServer::new(vec![("/data.csv", TestHttpResponse::ok_text("a,b\n1,2\n"))]);
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: vec!["http:".to_string(), "https:".to_string()],
            ..Default::default()
        })
        .unwrap();
        let spec = vega_spec_with_data_url(&server.url("/data.csv"));
        let output = converter
            .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
            .await
            .unwrap();
        assert!(output.svg.contains("<svg"));
    }
}

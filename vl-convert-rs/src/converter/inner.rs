use crate::deno_stubs::{NoOpInNpmPackageChecker, NoOpNpmPackageFolderResolver, VlConvertNodeSys};
use crate::image_loading::ImageAccessPolicy;
use crate::module_loader::import_map::{msgpack_url, vega_themes_url, vega_url, VlVersion};
use crate::module_loader::VlConvertModuleLoader;
use crate::text::{
    build_usvg_options_with_fontdb, get_font_baseline_snapshot, FONT_CONFIG_VERSION,
    GOOGLE_FONTS_CLIENT,
};
use deno_core::anyhow::{anyhow, bail};
use deno_core::error::AnyError;
use deno_core::{serde_v8, v8, ModuleSpecifier};
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_permissions::{PermissionsContainer, RuntimePermissionDescriptorParser};
use deno_runtime::deno_web::{BlobStore, InMemoryBroadcastChannel};
use deno_runtime::worker::{MainWorker, WorkerOptions, WorkerServiceOptions};
use deno_runtime::FeatureChecker;
use image::codecs::jpeg::JpegEncoder;
use image::ImageReader;
use resvg::render;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::io::Cursor;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use svg2pdf::{ConversionOptions, PageOptions};
use vl_convert_google_fonts::{GoogleFontsDatabaseExt, LoadedFontBatch};

use super::config::{
    apply_spec_overrides, build_permissions, ConverterContext, GoogleFontRequest, ValueOrString,
};
use super::fonts::{google_font_request_key, WorkerFontState};
use super::rendering::{encode_png, parse_svg_with_options};
use super::transfer::{
    JsonArgGuard, MsgpackResultGuard, WorkerTransferState, WorkerTransferStateHandle,
};
use super::types::*;

/// Per-conversion timeout timer state. Created before each conversion
/// and cancelled/joined after. The timer thread sleeps using `recv_timeout`
/// on a cancellation channel; if the timeout elapses it sets the flag and
/// calls `terminate_execution()` on the V8 isolate handle.
pub(crate) struct ConversionTimer {
    cancel_tx: std::sync::mpsc::Sender<()>,
    thread: std::thread::JoinHandle<()>,
}

/// State shared between the near-heap-limit callback and the worker loop.
/// The callback sets `heap_limit_hit` to `true` when it fires, allowing
/// the worker loop to produce a specific error message.
struct HeapLimitCallbackData {
    handle: v8::IsolateHandle,
    heap_limit_hit: std::sync::atomic::AtomicBool,
}

/// V8 near-heap-limit callback that terminates JS execution instead of
/// letting V8 call `FatalProcessOutOfMemory()` (which aborts the process).
/// The `data` pointer is a leaked `Box<HeapLimitCallbackData>` registered
/// during worker creation. The Box is intentionally leaked (bounded: one
/// per worker) so it lives for the isolate's lifetime.
unsafe extern "C" fn near_heap_limit_callback(
    data: *mut std::ffi::c_void,
    current_heap_limit: usize,
    _initial_heap_limit: usize,
) -> usize {
    // SAFETY: `data` is a leaked `Box<HeapLimitCallbackData>` created during
    // worker init. It lives for the isolate's lifetime (bounded: one per worker).
    let cb_data = unsafe { &*(data as *const HeapLimitCallbackData) };

    // If the flag was already set (repeated callback invocation during the
    // same OOM episode), return the limit unchanged — do not keep doubling.
    if cb_data
        .heap_limit_hit
        .swap(true, std::sync::atomic::Ordering::AcqRel)
    {
        return current_heap_limit;
    }

    cb_data.handle.terminate_execution();
    // Grant temporary headroom so V8 can process the termination signal
    // rather than going directly to FatalProcessOutOfMemory.
    current_heap_limit.saturating_mul(2)
}

/// Struct that interacts directly with the Deno JavaScript runtime. Not Sendable
pub(crate) struct InnerVlConverter {
    pub(crate) worker: MainWorker,
    transfer_state: WorkerTransferStateHandle,
    initialized_vl_versions: HashSet<VlVersion>,
    vega_initialized: bool,
    font_state: WorkerFontState,
    usvg_options: usvg::Options<'static>,
    pub(crate) ctx: Arc<ConverterContext>,
    /// Pointer to the heap-limit callback data (leaked Box). `None` when
    /// `max_v8_heap_size_mb` is 0 (no limit).
    heap_limit_data: Option<*const HeapLimitCallbackData>,
    /// Shared flag set by the timeout timer thread when a conversion exceeds
    /// `max_v8_execution_time_secs`. Checked by `annotate_timeout_error()`.
    timeout_hit: Arc<std::sync::atomic::AtomicBool>,
    /// Set when a plugin fails during init_vega(). Subsequent commands
    /// return this error immediately (no retry on the tainted isolate).
    plugin_init_error: Option<String>,
    /// Log entries captured during the most recent conversion.
    /// Populated by `emit_js_log_messages()`, taken by `std::mem::take` before
    /// returning typed output structs.
    pub(crate) last_log_entries: Vec<LogEntry>,
}

impl InnerVlConverter {
    /// Returns `true` if the near-heap-limit callback has fired.
    fn heap_limit_was_hit(&self) -> bool {
        if let Some(ptr) = self.heap_limit_data {
            // SAFETY: ptr is a leaked Box<HeapLimitCallbackData> created via
            // Box::into_raw during worker init. It lives for the isolate's lifetime.
            let data = unsafe { &*ptr };
            data.heap_limit_hit
                .load(std::sync::atomic::Ordering::Acquire)
        } else {
            false
        }
    }

    /// If the near-heap-limit callback fired, reset the flag, restore the
    /// original heap limit, and re-register the callback so it fires again
    /// on the next OOM. This must be called after
    /// `cancel_terminate_execution()`.
    pub(crate) fn restore_heap_limit_if_needed(&mut self) {
        if let Some(ptr) = self.heap_limit_data {
            // SAFETY: ptr is a leaked Box<HeapLimitCallbackData> created via
            // Box::into_raw during worker init. It lives for the isolate's lifetime.
            let data = unsafe { &*ptr };
            if !data
                .heap_limit_hit
                .swap(false, std::sync::atomic::Ordering::AcqRel)
            {
                // Callback hasn't fired — nothing to restore.
                return;
            }

            let max_bytes = self
                .ctx
                .config
                .max_v8_heap_size_mb
                .saturating_mul(1024 * 1024);
            let isolate = self.worker.js_runtime.v8_isolate();

            // Remove the (already consumed) callback and restore the
            // original heap limit.
            isolate.remove_near_heap_limit_callback(near_heap_limit_callback, max_bytes);

            // GC to free garbage from the failed conversion before
            // re-registering the callback at the original limit.
            isolate.low_memory_notification();

            // Re-register so the next OOM is caught too.
            isolate.add_near_heap_limit_callback(
                near_heap_limit_callback,
                ptr as *const _ as *mut std::ffi::c_void,
            );

            // Clear plugin poisoning so the next request retries init.
            // Don't reset vega_initialized — the modules are still in V8's
            // cache and the GC above reclaimed enough headroom to continue.
            self.plugin_init_error = None;
        }
    }

    /// If the near-heap-limit callback fired, annotate the error with V8
    /// memory stats. Otherwise return the result unchanged.
    pub(crate) fn annotate_heap_limit_error<T>(
        &mut self,
        result: Result<T, AnyError>,
    ) -> Result<T, AnyError> {
        match result {
            Err(original) if self.heap_limit_was_hit() => {
                let stats = self.worker.js_runtime.v8_isolate().get_heap_statistics();
                let used_mb = stats.used_heap_size() as f64 / (1024.0 * 1024.0);
                let total_mb = stats.total_heap_size() as f64 / (1024.0 * 1024.0);
                let external_mb = stats.external_memory() as f64 / (1024.0 * 1024.0);
                Err(original.context(format!(
                    "V8 heap limit exceeded (configured: {} MB). \
                     Worker memory: {used_mb:.1} MB used, {total_mb:.1} MB total, \
                     {external_mb:.1} MB external. \
                     Increase max_v8_heap_size_mb or set to 0 for no limit.",
                    self.ctx.config.max_v8_heap_size_mb,
                )))
            }
            other => other,
        }
    }

    /// Start a conversion timeout timer that also watches for caller disconnect.
    /// Returns `None` if the timeout is disabled (0). The returned
    /// `ConversionTimer` must be cancelled after the conversion completes by
    /// calling `cancel_conversion_timer()`.
    pub(crate) fn start_conversion_timer(
        &mut self,
        caller_gone: Arc<std::sync::atomic::AtomicBool>,
    ) -> Option<ConversionTimer> {
        self.start_conversion_timer_with_duration(
            std::time::Duration::from_secs(self.ctx.config.max_v8_execution_time_secs),
            caller_gone,
        )
    }

    /// Start a conversion timeout timer with an explicit duration. Also monitors
    /// `caller_gone` so V8 can be terminated early if the caller disconnects.
    pub(crate) fn start_conversion_timer_with_duration(
        &mut self,
        duration: std::time::Duration,
        caller_gone: Arc<std::sync::atomic::AtomicBool>,
    ) -> Option<ConversionTimer> {
        if duration.is_zero() {
            return None;
        }
        let timeout_hit = self.timeout_hit.clone();
        let handle = self.worker.js_runtime.v8_isolate().thread_safe_handle();
        let (cancel_tx, cancel_rx) = std::sync::mpsc::channel::<()>();
        let poll_interval = std::time::Duration::from_millis(50);
        let thread = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + duration;
            loop {
                // Check if the conversion finished normally (Ok = explicit cancel,
                // Disconnected = cancel_tx dropped in cancel_conversion_timer)
                match cancel_rx.try_recv() {
                    Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
                // Check if the caller disconnected
                if caller_gone.load(std::sync::atomic::Ordering::Acquire) {
                    timeout_hit.store(true, std::sync::atomic::Ordering::Release);
                    handle.terminate_execution();
                    return;
                }
                // Check if the deadline expired
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    timeout_hit.store(true, std::sync::atomic::Ordering::Release);
                    handle.terminate_execution();
                    return;
                }
                std::thread::sleep(poll_interval.min(remaining));
            }
        });
        Some(ConversionTimer { cancel_tx, thread })
    }

    /// Cancel a running conversion timer and join its thread. Safe to call
    /// even if the timer already fired.
    pub(crate) fn cancel_conversion_timer(&self, timer: Option<ConversionTimer>) {
        if let Some(timer) = timer {
            drop(timer.cancel_tx);
            let _ = timer.thread.join();
        }
    }

    /// Returns `true` if the conversion timeout timer has fired.
    fn timeout_was_hit(&self) -> bool {
        self.timeout_hit.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Reset the timeout flag after a timed-out conversion so the worker
    /// can process subsequent requests. Also clears plugin poisoning and
    /// vega init state so the next request retries initialization.
    pub(crate) fn reset_timeout_if_needed(&mut self) {
        if self
            .timeout_hit
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            self.plugin_init_error = None;
            self.vega_initialized = false;
        }
    }

    /// If the timeout timer fired, annotate the error with timeout details.
    /// Otherwise return the result unchanged.
    pub(crate) fn annotate_timeout_error<T>(
        &self,
        result: Result<T, AnyError>,
    ) -> Result<T, AnyError> {
        match result {
            Err(original) if self.timeout_was_hit() => Err(original.context(format!(
                "Conversion timed out (configured: {} seconds). \
                 Increase max_v8_execution_time_secs or set to 0 for no limit.",
                self.ctx.config.max_v8_execution_time_secs,
            ))),
            other => other,
        }
    }

    fn publish_worker_font_state_to_opstate(&mut self) {
        self.font_state.shared_config_epoch = self.font_state.shared_config_epoch.wrapping_add(1);
        let resolved = vl_convert_canvas2d::ResolvedFontConfig::from_parts(
            self.font_state.db.clone(),
            self.font_state.hinting_enabled,
        );
        let shared_config = vl_convert_canvas2d_deno::SharedFontConfig::new(
            resolved,
            self.font_state.shared_config_epoch,
        );
        self.usvg_options.fontdb = Arc::new(self.font_state.db.clone());
        self.worker
            .js_runtime
            .op_state()
            .borrow_mut()
            .put(shared_config);
    }

    pub(crate) fn refresh_font_config_if_needed(&mut self) -> Result<(), AnyError> {
        let current = FONT_CONFIG_VERSION.load(Ordering::Acquire);
        if current != self.font_state.baseline_version {
            let snapshot = get_font_baseline_snapshot()?;
            self.font_state = WorkerFontState::from_baseline(&snapshot);
            self.publish_worker_font_state_to_opstate();
        }
        Ok(())
    }

    fn apply_google_fonts_overlay(&mut self, batches: Vec<LoadedFontBatch>) {
        if batches.is_empty() {
            return;
        }
        debug_assert!(
            self.font_state.overlay_registrations.is_empty(),
            "overlay registrations should be empty before applying a new request overlay"
        );
        for batch in batches {
            let registration = self.font_state.db.register_google_fonts_batch(batch);
            self.font_state.overlay_registrations.push(registration);
        }
        self.publish_worker_font_state_to_opstate();
    }

    pub(crate) fn clear_google_fonts_overlay(&mut self) {
        if self.font_state.overlay_registrations.is_empty() {
            return;
        }
        for registration in self.font_state.overlay_registrations.drain(..) {
            self.font_state
                .db
                .unregister_google_fonts_batch(registration);
        }
        self.publish_worker_font_state_to_opstate();
    }

    /// Resolve Google Fonts and apply them as an overlay on the worker's fontdb.
    /// Returns `Ok(true)` if fonts were applied, `Ok(false)` if none were needed.
    /// Caller must call `clear_google_fonts_overlay()` after the work is done.
    pub(crate) async fn apply_font_overlay_if_needed(
        &mut self,
        google_fonts: Option<Vec<GoogleFontRequest>>,
    ) -> Result<bool, AnyError> {
        let batches = self.resolve_google_fonts(google_fonts).await?;
        if !batches.is_empty() {
            self.apply_google_fonts_overlay(batches);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Wrap a worker operation with Google Fonts overlay (resolve, apply, work, clear).
/// Use inside `async move` blocks within closures passed to `run_on_worker` /
/// `run_on_ephemeral_worker`. The `$inner` expression must be a reborrowed
/// `&mut InnerVlConverter` (i.e., `let inner = &mut *inner;` before the async move block).
///
/// Usage: `with_font_overlay!(inner, gf_option, inner.async_method(args).await)`
/// or:    `with_font_overlay!(inner, gf_option, inner.sync_method(args))`
#[macro_export]
macro_rules! with_font_overlay {
    ($inner:expr, $google_fonts:expr, $work:expr) => {{
        $inner.apply_font_overlay_if_needed($google_fonts).await?;
        let result = $work;
        $inner.clear_google_fonts_overlay();
        result
    }};
}

impl InnerVlConverter {
    pub(crate) async fn init_vega(&mut self) -> Result<(), AnyError> {
        if let Some(ref err) = self.plugin_init_error {
            bail!("Worker poisoned by plugin failure: {err}. Reconfigure to reset.");
        }

        if !self.vega_initialized {
            // ops are now exposed on globalThis by the extension ESM bootstrap
            let import_code = format!(
                r#"
var vega;
import('{vega_url}').then((imported) => {{
    vega = imported;
}})

var vegaThemes;
import('{vega_themes_url}').then((imported) => {{
    vegaThemes = imported;
}})

var msgpack;
import('{msgpack_url}').then((imported) => {{
    msgpack = imported;
}})
"#,
                vega_url = vega_url(),
                vega_themes_url = vega_themes_url(),
                msgpack_url = msgpack_url(),
            );

            self.worker
                .js_runtime
                .execute_script("ext:<anon>", import_code)?;

            let logger_code = r#"""
var _logEntries = [];

function _clearLogMessages() {
  _logEntries.length = 0; // truncates array; old entries become GC-eligible
}

function _collapsedLogMessages() {
  if (_logEntries.length === 0) return "";
  let result = [];
  let i = 0;
  while (i < _logEntries.length) {
    let entry = _logEntries[i];
    let count = 1;
    while (i + count < _logEntries.length
        && _logEntries[i + count].level === entry.level
        && _logEntries[i + count].msg === entry.msg) {
      count++;
    }
    result.push({
      level: entry.level,
      msg: count > 1 ? `(${count}x) ${entry.msg}` : entry.msg
    });
    i += count;
  }
  return JSON.stringify(result);
}

class LogCollector {
  constructor() { this._level = 4; }
  level(lvl) {
    if (arguments.length === 0) return this._level;
    this._level = lvl;
    return this;
  }
  error(msg) { _logEntries.push({level: "error", msg}); return this; }
  warn(msg)  { _logEntries.push({level: "warn", msg});  return this; }
  info(msg)  { _logEntries.push({level: "info", msg});   return this; }
  debug(msg) { _logEntries.push({level: "debug", msg});  return this; }
}

var logCollector = new LogCollector();
            """#
            .to_string();

            self.worker
                .js_runtime
                .execute_script("ext:<anon>", logger_code.to_string())?;
            self.worker
                .js_runtime
                .run_event_loop(Default::default())
                .await?;

            // Create and initialize svg function string
            let resolved_base_url =
                serde_json::to_string(&self.ctx.config.base_url.resolved_url()?)?;
            let mut function_str = r#"
const CONVERTER_BASE_URL = __BASE_URL__;

function buildLoader(errors) {
    let baseURL = CONVERTER_BASE_URL;
    if (baseURL == null) {
        baseURL = 'about:invalid';
    }

    const loader = vega.loader({ baseURL });
    const originalSanitize = loader.sanitize.bind(loader);

    loader.load = async (uri, options) => {
        const sanitized = await originalSanitize(uri, options);
        const href = sanitized.href;
        const responseType = options?.http?.response;
        const wantBinary = responseType === 'arraybuffer';

        try {
            // data: URIs are handled inline (no network, no op needed)
            if (href.startsWith('data:')) {
                const resp = await fetch(href);
                return wantBinary ? await resp.arrayBuffer() : await resp.text();
            }

            // HTTP(S) URLs: use Rust HTTP ops
            if (href.startsWith('http://') || href.startsWith('https://')) {
                if (wantBinary) {
                    const buffer = await op_vega_data_fetch_bytes(href);
                    return buffer.buffer;
                }
                return await op_vega_data_fetch(href);
            }

            // Filesystem path (sanitize strips file:// prefix, so href is a bare path).
            // On Windows, stripping file:// from file:///C:/path leaves /C:/path;
            // remove the leading slash so the Rust op receives a valid Windows path.
            let filePath = decodeURIComponent(href);
            if (globalThis.Deno?.build?.os === 'windows' && /^\/[A-Za-z]:/.test(filePath)) {
                filePath = filePath.slice(1);
            }
            if (wantBinary) {
                const buffer = await op_vega_file_read_bytes(filePath);
                return buffer.buffer;
            }
            return await op_vega_file_read(filePath);
        } catch (error) {
            errors.push(error.message);
            throw error;
        }
    };

    return loader;
}

function vegaToView(vgSpec, config, errors) {
    let runtime = vega.parse(vgSpec, config || {});
    const loader = buildLoader(errors);
    return new vega.View(runtime, {renderer: 'none', loader, logLevel: vega.Debug, logger: logCollector});
}

function vegaToSvg(vgSpec, formatLocale, timeFormatLocale, config, errors) {
    if (formatLocale != null) {
        vega.formatLocale(formatLocale);
    }
    if (timeFormatLocale != null) {
        vega.timeFormatLocale(timeFormatLocale);
    }
    let view = vegaToView(vgSpec, config, errors);
    let svgPromise = view.runAsync().then(() => {
        try {
            // Workaround for https://github.com/vega/vega/issues/3481
            view.signal("geo_interval_init_tick", {});
        } catch (e) {
            // No geo_interval_init_tick signal
        }
    }).then(() => {
        return view.runAsync().then(
            () => {
                if (errors != null && errors.length > 0) {
                    throw new Error(`${errors}`);
                }
                return view.toSVG();
            }
        ).finally(() => {
            view.finalize();
            vega.resetDefaultLocale();
        })
    });
    return svgPromise
}

const SCENEGRAPH_KEYS = new Set([
  'marktype', 'name', 'role', 'interactive', 'clip', 'items', 'zindex',
  'x', 'y', 'width', 'height', 'align', 'baseline',             // layout
  'fill', 'fillOpacity', 'opacity', 'blend',                    // fill
  'x1', 'y1', 'r1', 'r2', 'gradient',                           // gradient
  'stops', 'offset', 'color',
  'stroke', 'strokeOpacity', 'strokeWidth', 'strokeCap',        // stroke
  'strokeJoin',
  'strokeDash', 'strokeDashOffset',                             // stroke dash
  'strokeForeground', 'strokeOffset',                           // group
  'startAngle', 'endAngle', 'innerRadius', 'outerRadius',       // arc
  'cornerRadius', 'padAngle',                                   // arc, rect
  'cornerRadiusTopLeft', 'cornerRadiusTopRight',                // rect, group
  'cornerRadiusBottomLeft', 'cornerRadiusBottomRight',
  'interpolate', 'tension', 'orient', 'defined',                // area, line
  'url', 'aspect', 'smooth',                                    // image
  'path', 'scaleX', 'scaleY',                                   // path
  'x2', 'y2',                                                   // rule
  'size', 'shape',                                              // symbol
  'text', 'angle', 'theta', 'radius', 'dir', 'dx', 'dy',        // text
  'ellipsis', 'limit', 'lineBreak', 'lineHeight',
  'font', 'fontSize', 'fontWeight', 'fontStyle', 'fontVariant', // font
  'description', 'aria', 'ariaRole', 'ariaRoleDescription'      // aria
]);

function cloneScenegraph(obj) {
    if (typeof obj !== 'object' || obj === null) {
        return obj;
    }

    if (Array.isArray(obj)) {
        const len = obj.length;
        const clone = new Array(len);
        for (let i = 0; i < len; i++) {
            clone[i] = cloneScenegraph(obj[i]);
        }
        return clone;
    }

    const clone = {};
    const objKeys = Object.keys(obj);
    for (let i = 0; i < objKeys.length; i++) {
        const key = objKeys[i];
        const value = obj[key];

        if (key === "shape" && typeof value === "function") {
            // Convert path object to SVG path string.
            // Initialize context. This is needed for value(obj) to work.
            value.context();
            clone.shape = value(obj) ?? "";
        } else if (SCENEGRAPH_KEYS.has(key) && value !== undefined) {
            clone[key] = cloneScenegraph(value);
        }
    }

    return clone;
}

function vegaToScenegraph(vgSpec, formatLocale, timeFormatLocale, config, errors) {
    if (formatLocale != null) {
        vega.formatLocale(formatLocale);
    }
    if (timeFormatLocale != null) {
        vega.timeFormatLocale(timeFormatLocale);
    }
    let view = vegaToView(vgSpec, config, errors);
    let scenegraphPromise = view.runAsync().then(() => {
        try {
            // Workaround for https://github.com/vega/vega/issues/3481
            view.signal("geo_interval_init_tick", {});
        } catch (e) {
            // No geo_interval_init_tick signal
        }
    }).then(() => {
        return view.runAsync().then(
            () => {
                if (errors != null && errors.length > 0) {
                    throw new Error(`${errors}`);
                }
                let padding = view.padding();
                return {
                    width: Math.max(0, view._viewWidth + padding.left + padding.right),
                    height: Math.max(0, view._viewHeight + padding.top + padding.bottom),
                    origin: [
                        padding.left + view._origin[0],
                        padding.top + view._origin[1]
                    ],
                    scenegraph: cloneScenegraph(view.scenegraph().root)
                }
            }
        ).finally(() => {
            view.finalize();
            vega.resetDefaultLocale();
        })
    });
    return scenegraphPromise
}

function vegaToCanvas(vgSpec, formatLocale, timeFormatLocale, scale, config, errors) {
    if (formatLocale != null) {
        vega.formatLocale(formatLocale);
    }
    if (timeFormatLocale != null) {
        vega.timeFormatLocale(timeFormatLocale);
    }

    let view = vegaToView(vgSpec, config, errors);
    let canvasPromise = view.runAsync().then(() => {
        try {
            // Workaround for https://github.com/vega/vega/issues/3481
            view.signal("geo_interval_init_tick", {});
        } catch (e) {
            // No geo_interval_init_tick signal
        }
    }).then(() => {
        return view.runAsync()
            .then(() => {
                return view.toCanvas(scale)
                    .then((canvas) => {
                        if (errors != null && errors.length > 0) {
                            throw new Error(`${errors}`);
                        }
                        return canvas;
                    });
            })
            .finally(() => {
                view.finalize();
                vega.resetDefaultLocale();
            })
    });
    return canvasPromise;
}
"#
            .to_string();
            function_str = function_str.replace("__BASE_URL__", resolved_base_url.as_str());
            self.worker
                .js_runtime
                .execute_script("ext:<anon>", function_str)?;
            self.worker
                .js_runtime
                .run_event_loop(Default::default())
                .await?;

            // Clone to release the borrow on self.ctx before calling load_plugin.
            if let Some(plugins) = self.ctx.resolved_plugins.clone() {
                for (i, plugin) in plugins.iter().enumerate() {
                    self.load_plugin(i, &plugin.bundled_source, true).await?;
                }
            }

            // Register custom themes: replace the frozen vegaThemes module namespace
            // with a mutable copy that includes the custom themes.
            if let Some(ref themes) = self.ctx.config.themes {
                let themes_json = serde_json::to_string(themes)?;
                self.worker.js_runtime.execute_script(
                    "ext:<anon>",
                    format!("vegaThemes = Object.assign({{}}, vegaThemes, {themes_json});"),
                )?;
            }

            self.vega_initialized = true;
        }

        Ok(())
    }

    /// Load a single plugin ESM module into the V8 runtime.
    /// If `poison_on_failure` is true, sets `plugin_init_error` on any error.
    pub(crate) async fn load_plugin(
        &mut self,
        index: usize,
        source: &str,
        poison_on_failure: bool,
    ) -> Result<(), AnyError> {
        let specifier =
            deno_core::ModuleSpecifier::parse(&format!("vl-plugin:vega-plugin-{index}"))
                .expect("valid plugin specifier");

        let poison = |this: &mut Self, msg: String| -> AnyError {
            if poison_on_failure {
                this.plugin_init_error = Some(msg.clone());
            }
            anyhow!(msg)
        };

        // Load the plugin as an ES side module
        let module_id = self
            .worker
            .js_runtime
            .load_side_es_module_from_code(&specifier, source.to_string())
            .await
            .map_err(|e| poison(self, format!("Failed to load Vega plugin {index}: {e}")))?;

        // Evaluate the module
        let receiver = self.worker.js_runtime.mod_evaluate(module_id);
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await
            .map_err(|e| poison(self, format!("Vega plugin {index} event loop error: {e}")))?;
        receiver
            .await
            .map_err(|e| poison(self, format!("Vega plugin {index} evaluation failed: {e}")))?;

        // Get the module namespace and set it as a temporary global
        let namespace = self
            .worker
            .js_runtime
            .get_module_namespace(module_id)
            .map_err(|e| {
                poison(
                    self,
                    format!("Failed to get Vega plugin {index} namespace: {e}"),
                )
            })?;
        {
            deno_core::scope!(scope, self.worker.js_runtime);
            let global = scope.get_current_context().global(scope);
            let key = v8::String::new(scope, "__vlcPluginNs").unwrap();
            let ns_local = v8::Local::new(scope, &namespace);
            global.set(scope, key.into(), ns_local.into());
        }

        // Call the default export with the vega object
        let call_code = format!(
            "if (typeof __vlcPluginNs.default === 'function') {{
                __vlcPluginNs.default(vega);
            }} else {{
                throw new Error('Vega plugin {index} does not export a default function');
            }}
            delete globalThis.__vlcPluginNs;"
        );
        self.worker
            .js_runtime
            .execute_script("ext:<anon>", call_code)
            .map_err(|e| {
                poison(
                    self,
                    format!("Vega plugin {index} default export call failed: {e}"),
                )
            })?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await
            .map_err(|e| poison(self, format!("Vega plugin {index} post-call error: {e}")))?;

        Ok(())
    }

    async fn init_vl_version(&mut self, vl_version: &VlVersion) -> Result<(), AnyError> {
        if !self.initialized_vl_versions.contains(vl_version) {
            // Create and evaluate import string
            let import_code = format!(
                r#"
var {ver_name};
import('{vl_url}').then((imported) => {{
    {ver_name} = imported;
}})
"#,
                ver_name = format!("{:?}", vl_version),
                vl_url = vl_version.to_url()
            );

            self.worker
                .js_runtime
                .execute_script("ext:<anon>", import_code)?;

            self.worker
                .js_runtime
                .run_event_loop(Default::default())
                .await?;

            // Create and initialize function string
            let function_code = format!(
                r#"
function compileVegaLite_{ver_name}(vlSpec, config, theme) {{
    let options = {{}};

    // Handle config and theme
    let usermetaTheme = ((vlSpec.usermeta ?? {{}}).embedOptions ?? {{}}).theme;
    let namedTheme = theme ?? usermetaTheme;
    if (namedTheme != null) {{
        options["config"] = vega.mergeConfig(vegaThemes[namedTheme], config ?? {{}});
    }} else if (config != null) {{
        options["config"] = config;
    }}

    options["logger"] = logCollector;

    return {ver_name}.compile(vlSpec, options).spec
}}

function vegaLiteToSvg_{ver_name}(vlSpec, config, theme, formatLocale, timeFormatLocale, errors) {{
    let vgSpec = compileVegaLite_{ver_name}(vlSpec, config, theme);
    return vegaToSvg(vgSpec, formatLocale, timeFormatLocale, null, errors)
}}

function vegaLiteToScenegraph_{ver_name}(vlSpec, config, theme, formatLocale, timeFormatLocale, errors) {{
    let vgSpec = compileVegaLite_{ver_name}(vlSpec, config, theme);
    return vegaToScenegraph(vgSpec, formatLocale, timeFormatLocale, null, errors)
}}

function vegaLiteToCanvas_{ver_name}(vlSpec, config, theme, formatLocale, timeFormatLocale, scale, errors) {{
    let vgSpec = compileVegaLite_{ver_name}(vlSpec, config, theme);
    return vegaToCanvas(vgSpec, formatLocale, timeFormatLocale, scale, null, errors)
}}
"#,
                ver_name = format!("{:?}", vl_version),
            );

            self.worker
                .js_runtime
                .execute_script("ext:<anon>", function_code)?;

            self.worker
                .js_runtime
                .run_event_loop(Default::default())
                .await?;

            // Register that this Vega-Lite version has been initialized
            self.initialized_vl_versions.insert(*vl_version);
        }
        Ok(())
    }

    pub async fn try_new(
        ctx: Arc<ConverterContext>,
        initial_font_baseline: crate::text::FontBaselineSnapshot,
    ) -> Result<Self, AnyError> {
        // MainWorker's deno_tls extension panics without a global crypto provider
        let _ =
            deno_runtime::deno_tls::rustls::crypto::aws_lc_rs::default_provider().install_default();

        let module_loader = Rc::new(VlConvertModuleLoader);

        // Create a dummy main module specifier for the worker
        let main_module = ModuleSpecifier::parse("ext:vl_convert/main.js")
            .expect("Failed to parse main module specifier");

        // Create permission descriptor parser using RealSys
        let descriptor_parser = Arc::new(RuntimePermissionDescriptorParser::new(VlConvertNodeSys));

        let permissions = build_permissions(&ctx.config)?;

        // Configure WorkerServiceOptions with stub types for npm resolution (not used by vl-convert)
        let services = WorkerServiceOptions::<
            NoOpInNpmPackageChecker,
            NoOpNpmPackageFolderResolver,
            VlConvertNodeSys,
        > {
            blob_store: Arc::new(BlobStore::default()),
            broadcast_channel: InMemoryBroadcastChannel::default(),
            deno_rt_native_addon_loader: None,
            feature_checker: Arc::new(FeatureChecker::default()),
            fs: Arc::new(RealFs),
            module_loader,
            node_services: None, // vl-convert doesn't need Node.js compatibility
            npm_process_state_provider: None,
            permissions: PermissionsContainer::new(descriptor_parser, permissions),
            root_cert_store_provider: None,
            fetch_dns_resolver: Default::default(),
            shared_array_buffer_store: None,
            compiled_wasm_module_store: None,
            v8_code_cache: None,
            bundle_provider: None,
        };

        // Configure WorkerOptions with our custom extensions and V8 snapshot.
        // The snapshot contains pre-compiled deno_runtime extensions plus our extension's ESM.
        // This is required for container compatibility (manylinux, slim images).
        let create_params = if ctx.config.max_v8_heap_size_mb > 0 {
            let max_bytes = ctx.config.max_v8_heap_size_mb.saturating_mul(1024 * 1024);
            Some(v8::CreateParams::default().heap_limits(0, max_bytes))
        } else {
            None
        };

        let options = WorkerOptions {
            extensions: vec![
                // Canvas 2D extension from vl-convert-canvas2d-deno crate
                vl_convert_canvas2d_deno::vl_convert_canvas2d::init(),
                // Our runtime extension (worker-local JSON/msgpack transfer ops)
                super::vl_convert_runtime::init(),
            ],
            startup_snapshot: Some(crate::VL_CONVERT_SNAPSHOT),
            create_params,
            ..Default::default()
        };

        // Create the MainWorker with full Web API support
        let mut worker = MainWorker::bootstrap_from_options(&main_module, services, options);

        // Register a near-heap-limit callback so V8 terminates JS execution
        // instead of calling FatalProcessOutOfMemory() (which aborts the process).
        let heap_limit_data = if ctx.config.max_v8_heap_size_mb > 0 {
            let isolate = worker.js_runtime.v8_isolate();
            let cb_data = Box::new(HeapLimitCallbackData {
                handle: isolate.thread_safe_handle(),
                heap_limit_hit: std::sync::atomic::AtomicBool::new(false),
            });
            let ptr = Box::into_raw(cb_data);
            isolate.add_near_heap_limit_callback(
                near_heap_limit_callback,
                ptr as *mut std::ffi::c_void,
            );
            Some(ptr as *const HeapLimitCallbackData)
        } else {
            None
        };

        let transfer_state = Rc::new(RefCell::new(WorkerTransferState::default()));
        worker
            .js_runtime
            .op_state()
            .borrow_mut()
            .put(transfer_state.clone());

        let mut font_state = WorkerFontState::from_baseline(&initial_font_baseline);
        font_state.shared_config_epoch = font_state.baseline_version;
        let resolved = vl_convert_canvas2d::ResolvedFontConfig::from_parts(
            font_state.db.clone(),
            font_state.hinting_enabled,
        );
        let shared_config = vl_convert_canvas2d_deno::SharedFontConfig::new(
            resolved,
            font_state.shared_config_epoch,
        );
        worker.js_runtime.op_state().borrow_mut().put(shared_config);

        // Store data access policy for the Rust data loading ops
        let data_policy = crate::data_ops::DataAccessPolicy {
            allowed_base_urls: ctx.parsed_allowed_base_urls.clone(),
        };
        worker.js_runtime.op_state().borrow_mut().put(data_policy);

        let this = Self {
            worker,
            transfer_state,
            initialized_vl_versions: Default::default(),
            vega_initialized: false,
            usvg_options: build_usvg_options_with_fontdb(font_state.db.clone()),
            font_state,
            ctx,
            heap_limit_data,
            timeout_hit: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            plugin_init_error: None,
            last_log_entries: Vec::new(),
        };

        Ok(this)
    }

    pub(crate) async fn execute_script_to_json(
        &mut self,
        script: &str,
    ) -> Result<serde_json::Value, AnyError> {
        let code = script.to_string();
        let res = self.worker.js_runtime.execute_script("ext:<anon>", code)?;

        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        deno_core::scope!(scope, self.worker.js_runtime);
        let local = v8::Local::new(scope, res);

        // Deserialize a `v8` object into a Rust type using `serde_v8`,
        // in this case deserialize to a JSON `Value`.
        let deserialized_value = serde_v8::from_v8::<serde_json::Value>(scope, local);
        deserialized_value.map_err(|err| anyhow!("Failed to deserialize JavaScript value: {err}"))
    }

    pub(crate) async fn execute_script_to_string(
        &mut self,
        script: &str,
    ) -> Result<String, AnyError> {
        let code = script.to_string();
        let res = self.worker.js_runtime.execute_script("ext:<anon>", code)?;

        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        deno_core::scope!(scope, self.worker.js_runtime);
        let local = v8::Local::new(scope, res);

        // Deserialize a `v8` object into a Rust type using `serde_v8`,
        // in this case deserialize to a JSON `Value`.
        let deserialized_value = serde_v8::from_v8::<serde_json::Value>(scope, local);

        let value = match deserialized_value {
            Ok(value) => {
                let value = value.as_str();
                value.unwrap().to_string()
            }
            Err(err) => bail!("{err}"),
        };

        Ok(value)
    }

    async fn emit_js_log_messages(&mut self) {
        self.last_log_entries.clear();
        let json = match self
            .execute_script_to_string("_collapsedLogMessages()")
            .await
        {
            Ok(json) => json,
            Err(e) => {
                vl_debug!("Failed to retrieve JS log messages: {e}");
                return;
            }
        };
        if json.is_empty() {
            return;
        }
        let entries: Vec<serde_json::Value> = match serde_json::from_str(&json) {
            Ok(v) => v,
            Err(e) => {
                vl_debug!("Failed to parse JS log messages: {e}");
                return;
            }
        };
        for entry in &entries {
            let level = entry.get("level").and_then(|v| v.as_str()).unwrap_or("");
            let msg = entry.get("msg").and_then(|v| v.as_str()).unwrap_or("");
            match level {
                "error" => {
                    vl_error!("{}", msg);
                    self.last_log_entries.push(LogEntry {
                        level: LogLevel::Error,
                        message: msg.to_string(),
                    });
                }
                "warn" => {
                    vl_warn!("{}", msg);
                    self.last_log_entries.push(LogEntry {
                        level: LogLevel::Warn,
                        message: msg.to_string(),
                    });
                }
                "info" => {
                    vl_info!("{}", msg);
                    self.last_log_entries.push(LogEntry {
                        level: LogLevel::Info,
                        message: msg.to_string(),
                    });
                }
                "debug" => {
                    vl_debug!("{}", msg);
                    self.last_log_entries.push(LogEntry {
                        level: LogLevel::Debug,
                        message: msg.to_string(),
                    });
                }
                _ => {}
            }
        }
    }

    pub async fn vegalite_to_vega(
        &mut self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<VegaOutput, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_opts.vl_version).await?;
        let config = vl_opts.config.clone().unwrap_or(serde_json::Value::Null);

        let vl_spec = apply_spec_overrides(
            vl_spec.into(),
            &vl_opts.background,
            vl_opts.width,
            vl_opts.height,
        )?;
        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vl_spec)?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let code = format!(
            r#"
_clearLogMessages();
compileVegaLite_{ver_name:?}(
    JSON.parse(op_get_json_arg({spec_arg_id})),
    JSON.parse(op_get_json_arg({config_arg_id})),
    {theme_arg}
)
"#,
            ver_name = vl_opts.vl_version,
            spec_arg_id = spec_arg.id(),
            config_arg_id = config_arg.id(),
            theme_arg = theme_arg,
        );

        let spec = self.execute_script_to_json(&code).await?;
        self.emit_js_log_messages().await;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(VegaOutput { spec, logs })
    }

    pub async fn vegalite_to_svg(
        &mut self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<SvgOutput, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_opts.vl_version).await?;

        let vl_spec = apply_spec_overrides(
            vl_spec.into(),
            &vl_opts.background,
            vl_opts.width,
            vl_opts.height,
        )?;
        let config = vl_opts.config.clone().unwrap_or(serde_json::Value::Null);

        let format_locale = match vl_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vl_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vl_spec)?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let code = format!(
            r#"
var svg;
var errors = [];
_clearLogMessages();
vegaLiteToSvg_{ver_name:?}(
    JSON.parse(op_get_json_arg({spec_arg_id})),
    JSON.parse(op_get_json_arg({config_arg_id})),
    {theme_arg},
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
    errors,
).then((result) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    svg = result;
}});
"#,
            ver_name = vl_opts.vl_version,
            spec_arg_id = spec_arg.id(),
            config_arg_id = config_arg.id(),
            format_locale_id = format_locale_arg.id(),
            time_format_locale_id = time_format_locale_arg.id(),
        );
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        self.emit_js_log_messages().await;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }
        let svg = self.execute_script_to_string("svg").await?;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(SvgOutput { svg, logs })
    }

    pub async fn vegalite_to_scenegraph_msgpack(
        &mut self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<ScenegraphMsgpackOutput, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_opts.vl_version).await?;
        let vl_spec = apply_spec_overrides(
            vl_spec.into(),
            &vl_opts.background,
            vl_opts.width,
            vl_opts.height,
        )?;

        let config = vl_opts.config.clone().unwrap_or(serde_json::Value::Null);
        let format_locale = match vl_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vl_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vl_spec)?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;
        let result = MsgpackResultGuard::new(&self.transfer_state)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let code = format!(
            r#"
var errors = [];
_clearLogMessages();
vegaLiteToScenegraph_{ver_name:?}(
    JSON.parse(op_get_json_arg({spec_arg_id})),
    JSON.parse(op_get_json_arg({config_arg_id})),
    {theme_arg},
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
    errors,
).then((result) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    op_set_msgpack_result({result_id}, msgpack.encode(result));
}})
"#,
            ver_name = vl_opts.vl_version,
            spec_arg_id = spec_arg.id(),
            config_arg_id = config_arg.id(),
            format_locale_id = format_locale_arg.id(),
            time_format_locale_id = time_format_locale_arg.id(),
            result_id = result.id(),
        );
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        self.emit_js_log_messages().await;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }
        let data = result.take_result()?;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(ScenegraphMsgpackOutput { data, logs })
    }

    pub async fn vegalite_to_scenegraph(
        &mut self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<ScenegraphOutput, AnyError> {
        let sg_output = self
            .vegalite_to_scenegraph_msgpack(vl_spec, vl_opts)
            .await?;
        let scenegraph: serde_json::Value = rmp_serde::from_slice(&sg_output.data)
            .map_err(|err| anyhow!("Failed to decode MessagePack scenegraph: {err}"))?;
        Ok(ScenegraphOutput {
            scenegraph,
            logs: sg_output.logs,
        })
    }

    pub async fn vega_to_svg(
        &mut self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<SvgOutput, AnyError> {
        self.init_vega().await?;

        let vg_spec = apply_spec_overrides(
            vg_spec.into(),
            &vg_opts.background,
            vg_opts.width,
            vg_opts.height,
        )?;

        let format_locale = match vg_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vg_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let config_value = vg_opts.config.unwrap_or(serde_json::Value::Null);

        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vg_spec)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config_value)?;

        let code = format!(
            r#"
var svg;
var errors = [];
_clearLogMessages();
vegaToSvg(
    JSON.parse(op_get_json_arg({arg_id})),
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
    JSON.parse(op_get_json_arg({config_id})),
    errors,
).then((result) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    svg = result;
}})
        "#,
            arg_id = spec_arg.id(),
            format_locale_id = format_locale_arg.id(),
            time_format_locale_id = time_format_locale_arg.id(),
            config_id = config_arg.id(),
        );
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        self.emit_js_log_messages().await;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }
        let svg = self.execute_script_to_string("svg").await?;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(SvgOutput { svg, logs })
    }

    pub async fn vega_to_scenegraph_msgpack(
        &mut self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<ScenegraphMsgpackOutput, AnyError> {
        self.init_vega().await?;
        let vg_spec = apply_spec_overrides(
            vg_spec.into(),
            &vg_opts.background,
            vg_opts.width,
            vg_opts.height,
        )?;
        let format_locale = match vg_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vg_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let config_value = vg_opts.config.unwrap_or(serde_json::Value::Null);
        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vg_spec)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config_value)?;
        let result = MsgpackResultGuard::new(&self.transfer_state)?;

        let code = format!(
            r#"
var errors = [];
_clearLogMessages();
vegaToScenegraph(
    JSON.parse(op_get_json_arg({arg_id})),
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
    JSON.parse(op_get_json_arg({config_id})),
    errors,
).then((result) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    op_set_msgpack_result({result_id}, msgpack.encode(result));
}})
"#,
            arg_id = spec_arg.id(),
            format_locale_id = format_locale_arg.id(),
            time_format_locale_id = time_format_locale_arg.id(),
            config_id = config_arg.id(),
            result_id = result.id(),
        );
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        self.emit_js_log_messages().await;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }
        let data = result.take_result()?;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(ScenegraphMsgpackOutput { data, logs })
    }

    pub async fn vega_to_scenegraph(
        &mut self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<ScenegraphOutput, AnyError> {
        let sg_output = self.vega_to_scenegraph_msgpack(vg_spec, vg_opts).await?;
        let scenegraph: serde_json::Value = rmp_serde::from_slice(&sg_output.data)
            .map_err(|err| anyhow!("Failed to decode MessagePack scenegraph: {err}"))?;
        Ok(ScenegraphOutput {
            scenegraph,
            logs: sg_output.logs,
        })
    }

    pub async fn get_local_tz(&mut self) -> Result<Option<String>, AnyError> {
        let code = "var localTz = Intl.DateTimeFormat().resolvedOptions().timeZone ?? 'undefined';"
            .to_string();
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let value = self.execute_script_to_string("localTz").await?;
        if value == "undefined" {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }

    pub async fn get_themes(&mut self) -> Result<serde_json::Value, AnyError> {
        self.init_vega().await?;

        let code = r#"
var themes = Object.assign({}, vegaThemes);
delete themes.version
delete themes.default
"#
        .to_string();

        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let value = self.execute_script_to_json("themes").await?;
        Ok(value)
    }

    pub(crate) async fn execute_script_to_bytes(
        &mut self,
        script: &str,
    ) -> Result<Vec<u8>, AnyError> {
        let code = script.to_string();
        let res = self.worker.js_runtime.execute_script("ext:<anon>", code)?;

        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        deno_core::scope!(scope, self.worker.js_runtime);
        let local = v8::Local::new(scope, res);

        // Deserialize typed-array data to bytes directly.
        let bytes = serde_v8::from_v8::<serde_v8::JsBuffer>(scope, local)?;
        Ok(bytes.to_vec())
    }

    pub async fn vega_to_png(
        &mut self,
        vg_spec: &serde_json::Value,
        vg_opts: VgOpts,
        scale: f32,
        ppi: f32,
    ) -> Result<PngOutput, AnyError> {
        self.init_vega().await?;

        let vg_spec = apply_spec_overrides(
            ValueOrString::Value(vg_spec.clone()),
            &vg_opts.background,
            vg_opts.width,
            vg_opts.height,
        )?
        .to_value()?;

        let format_locale = match vg_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vg_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let config_value = vg_opts.config.unwrap_or(serde_json::Value::Null);

        let spec_arg = JsonArgGuard::from_value(&self.transfer_state, vg_spec)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config_value)?;

        let code = format!(
            r#"
var canvasPngData;
var errors = [];
_clearLogMessages();
vegaToCanvas(
    JSON.parse(op_get_json_arg({arg_id})),
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
    {scale},
    JSON.parse(op_get_json_arg({config_id})),
    errors,
).then((canvas) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    canvasPngData = canvas._toPngWithPpi({ppi});
}})
"#,
            arg_id = spec_arg.id(),
            format_locale_id = format_locale_arg.id(),
            time_format_locale_id = time_format_locale_arg.id(),
            config_id = config_arg.id(),
        );
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        self.emit_js_log_messages().await;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }
        let data = self.execute_script_to_bytes("canvasPngData").await?;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(PngOutput { data, logs })
    }

    pub async fn vegalite_to_png(
        &mut self,
        vl_spec: &serde_json::Value,
        vl_opts: VlOpts,
        scale: f32,
        ppi: f32,
    ) -> Result<PngOutput, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_opts.vl_version).await?;

        let config = vl_opts.config.clone().unwrap_or(serde_json::Value::Null);

        let format_locale = match vl_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vl_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let spec_arg = JsonArgGuard::from_value(&self.transfer_state, vl_spec.clone())?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let code = format!(
            r#"
var canvasPngData;
var errors = [];
_clearLogMessages();
vegaLiteToCanvas_{ver_name:?}(
    JSON.parse(op_get_json_arg({spec_arg_id})),
    JSON.parse(op_get_json_arg({config_arg_id})),
    {theme_arg},
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
    {scale},
    errors,
).then((canvas) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    canvasPngData = canvas._toPngWithPpi({ppi});
}})
"#,
            ver_name = vl_opts.vl_version,
            spec_arg_id = spec_arg.id(),
            config_arg_id = config_arg.id(),
            format_locale_id = format_locale_arg.id(),
            time_format_locale_id = time_format_locale_arg.id(),
        );
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        self.emit_js_log_messages().await;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }
        let data = self.execute_script_to_bytes("canvasPngData").await?;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(PngOutput { data, logs })
    }

    pub(crate) fn parse_svg_with_worker_options(
        &mut self,
        svg: &str,
        policy: &ImageAccessPolicy,
    ) -> Result<usvg::Tree, AnyError> {
        parse_svg_with_options(svg, policy, &mut self.usvg_options)
    }

    pub(crate) fn svg_to_png_with_worker_options(
        &mut self,
        svg: &str,
        scale: f32,
        ppi: Option<f32>,
        policy: &ImageAccessPolicy,
    ) -> Result<Vec<u8>, AnyError> {
        let ppi = ppi.unwrap_or(72.0);
        let scale = scale * ppi / 72.0;
        let tree = self.parse_svg_with_worker_options(svg, policy)?;

        let mut pixmap = tiny_skia::Pixmap::new(
            (tree.size().width() * scale) as u32,
            (tree.size().height() * scale) as u32,
        )
        .ok_or_else(|| anyhow!("Failed to allocate pixmap for SVG render"))?;

        let transform = tiny_skia::Transform::from_scale(scale, scale);
        render(&tree, transform, &mut pixmap.as_mut());
        encode_png(pixmap, ppi)
    }

    pub(crate) fn svg_to_jpeg_with_worker_options(
        &mut self,
        svg: &str,
        scale: f32,
        quality: Option<u8>,
        policy: &ImageAccessPolicy,
    ) -> Result<Vec<u8>, AnyError> {
        let png_bytes = self.svg_to_png_with_worker_options(svg, scale, None, policy)?;
        let img = ImageReader::new(Cursor::new(png_bytes))
            .with_guessed_format()?
            .decode()?;

        let quality = quality.unwrap_or(90);
        if quality > 100 {
            bail!(
                "JPEG quality parameter must be between 0 and 100 inclusive. Received: {quality}"
            );
        }

        let mut jpeg_bytes: Vec<u8> = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, quality);
        encoder.encode_image(&img)?;
        Ok(jpeg_bytes)
    }

    pub(crate) fn svg_to_pdf_with_worker_options(
        &mut self,
        svg: &str,
        policy: &ImageAccessPolicy,
    ) -> Result<Vec<u8>, AnyError> {
        let tree = self.parse_svg_with_worker_options(svg, policy)?;
        let pdf = svg2pdf::to_pdf(&tree, ConversionOptions::default(), PageOptions::default());
        pdf.map_err(|err| anyhow!("Failed to convert SVG to PDF: {}", err))
    }

    pub async fn vega_to_jpeg(
        &mut self,
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        scale: f32,
        quality: Option<u8>,
        policy: ImageAccessPolicy,
    ) -> Result<JpegOutput, AnyError> {
        let svg_output = self.vega_to_svg(vg_spec, vg_opts).await?;
        let data =
            self.svg_to_jpeg_with_worker_options(&svg_output.svg, scale, quality, &policy)?;
        Ok(JpegOutput {
            data,
            logs: svg_output.logs,
        })
    }

    pub async fn vegalite_to_jpeg(
        &mut self,
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        scale: f32,
        quality: Option<u8>,
        policy: ImageAccessPolicy,
    ) -> Result<JpegOutput, AnyError> {
        let svg_output = self.vegalite_to_svg(vl_spec, vl_opts).await?;
        let data =
            self.svg_to_jpeg_with_worker_options(&svg_output.svg, scale, quality, &policy)?;
        Ok(JpegOutput {
            data,
            logs: svg_output.logs,
        })
    }

    pub async fn vega_to_pdf(
        &mut self,
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        policy: ImageAccessPolicy,
    ) -> Result<PdfOutput, AnyError> {
        let svg_output = self.vega_to_svg(vg_spec, vg_opts).await?;
        let data = self.svg_to_pdf_with_worker_options(&svg_output.svg, &policy)?;
        Ok(PdfOutput {
            data,
            logs: svg_output.logs,
        })
    }

    pub async fn vegalite_to_pdf(
        &mut self,
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        policy: ImageAccessPolicy,
    ) -> Result<PdfOutput, AnyError> {
        let svg_output = self.vegalite_to_svg(vl_spec, vl_opts).await?;
        let data = self.svg_to_pdf_with_worker_options(&svg_output.svg, &policy)?;
        Ok(PdfOutput {
            data,
            logs: svg_output.logs,
        })
    }

    /// Resolve Google Fonts requests on the worker thread using the async API.
    ///
    /// Merges per-request fonts with `config.google_fonts`, deduplicates, and
    /// downloads each unique font via `GOOGLE_FONTS_CLIENT.load()`.
    pub(crate) async fn resolve_google_fonts(
        &self,
        request_fonts: Option<Vec<GoogleFontRequest>>,
    ) -> Result<Vec<LoadedFontBatch>, AnyError> {
        let merged = match (self.ctx.config.google_fonts.clone(), request_fonts) {
            (None, None) => return Ok(Vec::new()),
            (Some(c), None) => c,
            (None, Some(r)) => r,
            (Some(mut c), Some(r)) => {
                c.extend(r);
                c
            }
        };
        if merged.is_empty() {
            return Ok(Vec::new());
        }

        let mut unique: BTreeMap<String, GoogleFontRequest> = BTreeMap::new();
        for request in merged {
            let key = google_font_request_key(&request);
            unique.entry(key).or_insert(request);
        }

        let mut batches = Vec::new();
        for request in unique.into_values() {
            let batch = GOOGLE_FONTS_CLIENT
                .load(&request.family, request.variants.as_deref())
                .await
                .map_err(|err| {
                    anyhow!(
                        "Failed to load request font '{}' from Google Fonts: {err}",
                        request.family
                    )
                })?;
            batches.push(batch);
        }
        Ok(batches)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::converter::config::VlcConfig;
    use serde_json::json;

    pub(in crate::converter) const PNG_1X1_BYTES: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4,
        0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 252, 255, 15, 0, 2, 3,
        1, 128, 179, 248, 175, 217, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    pub(in crate::converter) const SVG_2X3_BASE64: &str =
        "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyIiBoZWlnaHQ9IjMiPjxyZWN0IHdpZHRoPSIyIiBoZWlnaHQ9IjMiIGZpbGw9InJlZCIvPjwvc3ZnPg==";
    pub(in crate::converter) const SVG_2X3_DATA_URL: &str = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyIiBoZWlnaHQ9IjMiPjxyZWN0IHdpZHRoPSIyIiBoZWlnaHQ9IjMiIGZpbGw9InJlZCIvPjwvc3ZnPg==";

    #[derive(Clone)]
    pub(in crate::converter) struct TestHttpResponse {
        pub(in crate::converter) status: u16,
        pub(in crate::converter) headers: Vec<(String, String)>,
        pub(in crate::converter) body: Vec<u8>,
    }

    #[allow(dead_code)]
    impl TestHttpResponse {
        pub(in crate::converter) fn ok_text(body: &str) -> Self {
            Self {
                status: 200,
                headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
                body: body.as_bytes().to_vec(),
            }
        }

        pub(in crate::converter) fn ok_png(body: &[u8]) -> Self {
            Self {
                status: 200,
                headers: vec![("Content-Type".to_string(), "image/png".to_string())],
                body: body.to_vec(),
            }
        }

        pub(in crate::converter) fn ok_svg(body: &str) -> Self {
            Self {
                status: 200,
                headers: vec![("Content-Type".to_string(), "image/svg+xml".to_string())],
                body: body.as_bytes().to_vec(),
            }
        }

        pub(in crate::converter) fn redirect(location: &str) -> Self {
            Self {
                status: 302,
                headers: vec![("Location".to_string(), location.to_string())],
                body: Vec::new(),
            }
        }
    }

    pub(in crate::converter) struct TestHttpServer {
        addr: std::net::SocketAddr,
        running: Arc<std::sync::atomic::AtomicBool>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl TestHttpServer {
        pub(in crate::converter) fn new(routes: Vec<(&str, TestHttpResponse)>) -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let addr = listener.local_addr().unwrap();

            let routes = Arc::new(
                routes
                    .into_iter()
                    .map(|(path, response)| (path.to_string(), response))
                    .collect::<std::collections::HashMap<_, _>>(),
            );
            let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
            let running_clone = running.clone();
            let routes_clone = routes.clone();

            let handle = std::thread::spawn(move || {
                while running_clone.load(std::sync::atomic::Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            handle_test_http_connection(stream, &routes_clone);
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            });

            Self {
                addr,
                running,
                handle: Some(handle),
            }
        }

        pub(in crate::converter) fn url(&self, path: &str) -> String {
            format!("http://{}{}", self.addr, path)
        }

        pub(in crate::converter) fn origin(&self) -> String {
            format!("http://{}", self.addr)
        }

        #[allow(dead_code)]
        pub(in crate::converter) fn base_url(&self) -> String {
            format!("http://{}/", self.addr)
        }
    }

    impl Drop for TestHttpServer {
        fn drop(&mut self) {
            self.running
                .store(false, std::sync::atomic::Ordering::SeqCst);
            let _ = std::net::TcpStream::connect(self.addr);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn handle_test_http_connection(
        mut stream: std::net::TcpStream,
        routes: &std::collections::HashMap<String, TestHttpResponse>,
    ) {
        use std::io::{BufRead, BufReader, Write};

        let Ok(reader_stream) = stream.try_clone() else {
            return;
        };
        let mut reader = BufReader::new(reader_stream);

        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() {
            return;
        }
        let request_target = request_line.split_whitespace().nth(1).unwrap_or("/");
        let request_path = request_target.split('?').next().unwrap_or(request_target);

        loop {
            let mut header_line = String::new();
            if reader.read_line(&mut header_line).is_err() {
                return;
            }
            if header_line == "\r\n" || header_line == "\n" || header_line.is_empty() {
                break;
            }
        }

        let response = routes
            .get(request_path)
            .cloned()
            .unwrap_or_else(|| TestHttpResponse {
                status: 404,
                headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
                body: b"not found".to_vec(),
            });

        let mut headers = response.headers;
        if !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        {
            headers.push((
                "Content-Length".to_string(),
                response.body.len().to_string(),
            ));
        }
        if !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("connection"))
        {
            headers.push(("Connection".to_string(), "close".to_string()));
        }

        let mut response_head = format!(
            "HTTP/1.1 {} {}\r\n",
            response.status,
            http_reason_phrase(response.status)
        );
        for (name, value) in headers {
            response_head.push_str(&format!("{name}: {value}\r\n"));
        }
        response_head.push_str("\r\n");

        let _ = stream.write_all(response_head.as_bytes());
        let _ = stream.write_all(&response.body);
        let _ = stream.flush();
    }

    fn http_reason_phrase(status: u16) -> &'static str {
        match status {
            200 => "OK",
            301 => "Moved Permanently",
            302 => "Found",
            303 => "See Other",
            307 => "Temporary Redirect",
            308 => "Permanent Redirect",
            404 => "Not Found",
            _ => "Status",
        }
    }

    #[tokio::test]
    async fn test_convert_context() {
        let ctx = crate::converter::VlConverter::new();
        let vl_spec: serde_json::Value = serde_json::from_str(
            r#"
    {
    "data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
    "mark": "bar",
    "encoding": {
        "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
        "y": {"aggregate": "mean", "field": "precipitation"}
    }
    }
        "#,
        )
        .unwrap();

        let vg_output = ctx
            .vegalite_to_vega(
                vl_spec,
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        println!("vg_spec: {}", vg_output.spec)
    }

    #[tokio::test]
    async fn test_multi_convert_context() {
        let vl_spec: serde_json::Value = serde_json::from_str(
            r#"
    {
    "data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
    "mark": "bar",
    "encoding": {
        "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
        "y": {"aggregate": "mean", "field": "precipitation"}
    }
    }
        "#,
        )
        .unwrap();

        let ctx1 = crate::converter::VlConverter::new();
        let vg_output1 = ctx1
            .vegalite_to_vega(
                vl_spec.clone(),
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        println!("vg_spec1: {}", vg_output1.spec);

        let ctx1 = crate::converter::VlConverter::new();
        let vg_output2 = ctx1
            .vegalite_to_vega(
                vl_spec,
                VlOpts {
                    vl_version: VlVersion::v5_8,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        println!("vg_spec2: {}", vg_output2.spec);
    }

    #[tokio::test]
    async fn test_execute_script_to_bytes_typed_array() {
        let mut ctx = InnerVlConverter::try_new(
            std::sync::Arc::new(ConverterContext {
                config: VlcConfig::default(),
                parsed_allowed_base_urls: None,
                resolved_plugins: None,
            }),
            get_font_baseline_snapshot().unwrap(),
        )
        .await
        .unwrap();
        let bytes = ctx
            .execute_script_to_bytes("new Uint8Array([1, 2, 3])")
            .await
            .unwrap();
        assert_eq!(bytes, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_canvas_png_and_image_data_are_typed_arrays() {
        let mut ctx = InnerVlConverter::try_new(
            std::sync::Arc::new(ConverterContext {
                config: VlcConfig::default(),
                parsed_allowed_base_urls: None,
                resolved_plugins: None,
            }),
            get_font_baseline_snapshot().unwrap(),
        )
        .await
        .unwrap();
        let code = r#"
    const canvas = new HTMLCanvasElement(8, 8);
    const ctx2d = canvas.getContext('2d');
    ctx2d.fillStyle = '#ff0000';
    ctx2d.fillRect(0, 0, 8, 8);

    var __pngBytes = canvas._toPngWithPpi(72);
    var __pngIsUint8Array = __pngBytes instanceof Uint8Array;

    const imageData = ctx2d.getImageData(0, 0, 1, 1);
    var __imageDataChecks = [
      imageData.data instanceof Uint8ClampedArray,
      imageData.data[0], imageData.data[1], imageData.data[2], imageData.data[3]
    ];
    "#
        .to_string();

        ctx.worker
            .js_runtime
            .execute_script("ext:<anon>", code)
            .unwrap();
        ctx.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await
            .unwrap();

        let png_is_typed = ctx
            .execute_script_to_json("__pngIsUint8Array")
            .await
            .unwrap();
        assert_eq!(png_is_typed, json!(true));

        let png_bytes = ctx.execute_script_to_bytes("__pngBytes").await.unwrap();
        assert!(png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"));

        let image_data_checks = ctx
            .execute_script_to_json("__imageDataChecks")
            .await
            .unwrap();
        assert_eq!(image_data_checks, json!([true, 255, 0, 0, 255]));
    }

    #[tokio::test]
    async fn test_image_decode_and_load_events() {
        let mut ctx = InnerVlConverter::try_new(
            std::sync::Arc::new(ConverterContext {
                config: VlcConfig::default(),
                parsed_allowed_base_urls: None,
                resolved_plugins: None,
            }),
            get_font_baseline_snapshot().unwrap(),
        )
        .await
        .unwrap();
        let image_url = serde_json::to_string(SVG_2X3_DATA_URL).unwrap();
        let code = format!(
            r#"
    var __imageDecodeLoadResult = null;
    (async () => {{
      const img = new Image();
      let onloadCount = 0;
      let listenerCount = 0;
      img.onload = () => {{ onloadCount += 1; }};
      img.addEventListener("load", () => {{ listenerCount += 1; }});
      img.src = {image_url};
      await img.decode();
      __imageDecodeLoadResult = {{
    complete: img.complete,
    naturalWidth: img.naturalWidth,
    naturalHeight: img.naturalHeight,
    onloadCount,
    listenerCount
      }};
    }})();
    "#
        );

        ctx.worker
            .js_runtime
            .execute_script("ext:<anon>", code)
            .unwrap();
        ctx.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await
            .unwrap();

        let result = ctx
            .execute_script_to_json("__imageDecodeLoadResult")
            .await
            .unwrap();
        assert_eq!(result["complete"], json!(true));
        assert_eq!(result["naturalWidth"], json!(2));
        assert_eq!(result["naturalHeight"], json!(3));
        assert_eq!(result["onloadCount"], json!(1));
        assert_eq!(result["listenerCount"], json!(1));
    }

    #[tokio::test]
    async fn test_image_decode_rejects_and_error_events_fire() {
        use super::super::ACCESS_DENIED_MARKER;
        // Use allowed_base_urls: Some(vec![]) to deny all HTTP access via ops
        let mut ctx = InnerVlConverter::try_new(
            std::sync::Arc::new(ConverterContext {
                config: VlcConfig {
                    allowed_base_urls: Some(vec![]),
                    ..Default::default()
                },
                parsed_allowed_base_urls: Some(vec![]),
                resolved_plugins: None,
            }),
            get_font_baseline_snapshot().unwrap(),
        )
        .await
        .unwrap();
        let code = r#"
    var __imageDecodeErrorResult = null;
    (async () => {
      const img = new Image();
      let onerrorCount = 0;
      let listenerCount = 0;
      img.onerror = () => { onerrorCount += 1; };
      img.addEventListener("error", () => { listenerCount += 1; });
      img.src = "https://example.com/image.png";

      let decodeMessage = "";
      try {
    await img.decode();
    decodeMessage = "resolved";
      } catch (err) {
    decodeMessage = String(err && err.message ? err.message : err);
      }

      __imageDecodeErrorResult = {
    complete: img.complete,
    naturalWidth: img.naturalWidth,
    naturalHeight: img.naturalHeight,
    onerrorCount,
    listenerCount,
    decodeMessage,
      };
    })();
    "#;

        ctx.worker
            .js_runtime
            .execute_script("ext:<anon>", code)
            .unwrap();
        ctx.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await
            .unwrap();

        let result = ctx
            .execute_script_to_json("__imageDecodeErrorResult")
            .await
            .unwrap();
        assert_eq!(result["complete"], json!(true));
        assert_eq!(result["naturalWidth"], json!(0));
        assert_eq!(result["naturalHeight"], json!(0));
        assert_eq!(result["onerrorCount"], json!(1));
        assert_eq!(result["listenerCount"], json!(1));
        let decode_msg = result["decodeMessage"].as_str().unwrap_or_default();
        assert!(
            decode_msg.contains(ACCESS_DENIED_MARKER) || decode_msg.contains("permission"),
            "decode should fail with access denied, got: {decode_msg}"
        );
    }

    #[tokio::test]
    async fn test_image_decode_ignores_stale_src_results() {
        let mut ctx = InnerVlConverter::try_new(
            std::sync::Arc::new(ConverterContext {
                config: VlcConfig::default(),
                parsed_allowed_base_urls: None,
                resolved_plugins: None,
            }),
            get_font_baseline_snapshot().unwrap(),
        )
        .await
        .unwrap();
        // Test that setting img.src twice results in only the last assignment's
        // image being loaded. Uses data: URIs to avoid needing HTTP.
        let code = format!(
            r#"
    var __imageRaceResult = null;
    (async () => {{
      const img = new Image();
      let onloadCount = 0;
      let onerrorCount = 0;
      img.onload = () => {{ onloadCount += 1; }};
      img.onerror = () => {{ onerrorCount += 1; }};

      // Set src to an invalid data URI first, then immediately to a valid one.
      // The valid one should win.
      img.src = "data:image/png;base64,INVALIDDATA";
      img.src = "data:image/svg+xml;base64,{SVG_2X3_BASE64}";
      await img.decode();
      await new Promise((resolve) => setTimeout(resolve, 50));

      __imageRaceResult = {{
    src: img.src,
    complete: img.complete,
    naturalWidth: img.naturalWidth,
    naturalHeight: img.naturalHeight,
    onloadCount,
    onerrorCount,
      }};
    }})();
    "#
        );

        ctx.worker
            .js_runtime
            .execute_script("ext:<anon>", code)
            .unwrap();
        ctx.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await
            .unwrap();

        let result = ctx
            .execute_script_to_json("__imageRaceResult")
            .await
            .unwrap();
        let src = result["src"].as_str().unwrap_or_default();
        assert!(
            src.starts_with("data:image/svg+xml"),
            "src should be the second (valid) data URI, got: {src}"
        );
        assert_eq!(result["complete"], json!(true));
        assert_eq!(result["naturalWidth"], json!(2));
        assert_eq!(result["naturalHeight"], json!(3));
        // onload should fire once for the valid image
        assert_eq!(result["onloadCount"], json!(1));
        assert_eq!(result["onerrorCount"], json!(0));
    }

    #[tokio::test]
    async fn test_polyfill_unsupported_methods_throw() {
        let mut ctx = InnerVlConverter::try_new(
            std::sync::Arc::new(ConverterContext {
                config: VlcConfig::default(),
                parsed_allowed_base_urls: None,
                resolved_plugins: None,
            }),
            get_font_baseline_snapshot().unwrap(),
        )
        .await
        .unwrap();
        let code = r#"
    var __unsupportedMessages = [];

    // Path2D.addPath is now implemented and should NOT throw
    var addPathSucceeded = false;
    try {
      new Path2D().addPath(new Path2D());
      addPathSucceeded = true;
    } catch (err) {
      addPathSucceeded = false;
    }

    const canvas = new HTMLCanvasElement(16, 16);
    const ctx2d = canvas.getContext('2d');
    try {
      ctx2d.isPointInPath(0, 0);
    } catch (err) {
      __unsupportedMessages.push(String(err && err.message ? err.message : err));
    }
    try {
      ctx2d.isPointInStroke(0, 0);
    } catch (err) {
      __unsupportedMessages.push(String(err && err.message ? err.message : err));
    }
    "#
        .to_string();

        ctx.worker
            .js_runtime
            .execute_script("ext:<anon>", code)
            .unwrap();
        ctx.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await
            .unwrap();

        let messages = ctx
            .execute_script_to_json("__unsupportedMessages")
            .await
            .unwrap()
            .as_array()
            .cloned()
            .unwrap_or_default();

        // Path2D.addPath should now succeed (no longer unsupported)
        let add_path_succeeded = ctx
            .execute_script_to_json("addPathSucceeded")
            .await
            .unwrap();
        assert_eq!(add_path_succeeded, serde_json::json!(true));

        // isPointInPath and isPointInStroke remain unsupported
        assert_eq!(messages.len(), 2);
        assert!(messages[0]
            .as_str()
            .unwrap_or_default()
            .contains("CanvasRenderingContext2D.isPointInPath"));
        assert!(messages[1]
            .as_str()
            .unwrap_or_default()
            .contains("CanvasRenderingContext2D.isPointInStroke"));
    }
}

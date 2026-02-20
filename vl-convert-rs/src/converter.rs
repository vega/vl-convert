use crate::deno_stubs::{NoOpInNpmPackageChecker, NoOpNpmPackageFolderResolver, VlConvertNodeSys};
use crate::module_loader::import_map::{msgpack_url, vega_themes_url, vega_url, VlVersion};
use crate::module_loader::{VlConvertModuleLoader, FORMATE_LOCALE_MAP, TIME_FORMATE_LOCALE_MAP};

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::{serde_v8, v8, ModuleSpecifier};
use deno_error::JsErrorBox;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_permissions::{PermissionsContainer, RuntimePermissionDescriptorParser};
use deno_runtime::deno_web::{BlobStore, InMemoryBroadcastChannel};
use deno_runtime::worker::{MainWorker, WorkerOptions, WorkerServiceOptions};
use deno_runtime::FeatureChecker;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display, Formatter};
use std::io::Cursor;
use std::rc::Rc;
use std::sync::Once;
use std::sync::{Arc, Mutex};

use std::panic;
use std::str::FromStr;
use std::thread;
use std::thread::JoinHandle;

use deno_core::anyhow::anyhow;
use futures::channel::oneshot;
use png::{PixelDimensions, Unit};
use svg2pdf::{ConversionOptions, PageOptions};
use tiny_skia::{Pixmap, PremultipliedColorU8};

use crate::html::{bundle_vega_snippet, get_vega_or_vegalite_script};
use image::codecs::jpeg::JpegEncoder;
use image::ImageReader;
use resvg::render;

use crate::text::{FONT_CONFIG, FONT_CONFIG_VERSION, USVG_OPTIONS};
use std::sync::atomic::{AtomicUsize, Ordering};

// Extension with our custom ops - MainWorker provides all Web APIs (URL, fetch, etc.)
// Canvas 2D ops are now in the separate vl_convert_canvas2d extension from vl-convert-canvas2d-deno
deno_core::extension!(
    vl_convert_runtime,
    ops = [
        op_get_json_arg,
        op_set_msgpack_result,
    ],
    esm_entry_point = "ext:vl_convert_runtime/bootstrap.js",
    esm = [
        dir "src/js",
        "bootstrap.js",
    ],
);

// Arguments are passed to V8 as JSON strings via Deno ops and parsed in JS.
// Scenegraph results are returned as MessagePack byte buffers via ops,
// avoiding JSON serialization overhead for large payloads.
struct WorkerPool {
    senders: Vec<tokio::sync::mpsc::Sender<VlConvertCommand>>,
    next_sender_idx: AtomicUsize,
    _handles: Vec<JoinHandle<()>>,
}

impl WorkerPool {
    fn next_sender(&self) -> Option<tokio::sync::mpsc::Sender<VlConvertCommand>> {
        if self.senders.is_empty() {
            return None;
        }
        let idx = self.next_sender_idx.fetch_add(1, Ordering::Relaxed) % self.senders.len();
        Some(self.senders[idx].clone())
    }

    fn is_closed(&self) -> bool {
        self.senders
            .iter()
            .all(tokio::sync::mpsc::Sender::is_closed)
    }
}

lazy_static! {
    static ref JSON_ARGS: Arc<Mutex<HashMap<i32, String>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref MSGPACK_RESULTS: Arc<Mutex<HashMap<i32, Vec<u8>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    static ref NEXT_ID: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
}

fn ensure_v8_platform_initialized() {
    static V8_INIT: Once = Once::new();
    V8_INIT.call_once(|| deno_core::JsRuntime::init_platform(None, false));
}

fn worker_queue_capacity(num_workers: usize) -> usize {
    num_workers.saturating_mul(32).max(32)
}

fn spawn_worker_pool(num_workers: usize) -> Result<WorkerPool, AnyError> {
    if num_workers < 1 {
        bail!("num_workers must be >= 1");
    }
    ensure_v8_platform_initialized();

    let total_queue_capacity = worker_queue_capacity(num_workers);
    let per_worker_queue_capacity = (total_queue_capacity / num_workers).max(1);
    let mut handles = Vec::with_capacity(num_workers);
    let mut senders = Vec::with_capacity(num_workers);
    let mut startup_receivers = Vec::with_capacity(num_workers);

    for _ in 0..num_workers {
        let (tx, mut rx) =
            tokio::sync::mpsc::channel::<VlConvertCommand>(per_worker_queue_capacity);
        senders.push(tx);
        let (startup_tx, startup_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let handle = thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    let _ = startup_tx.send(Err(format!("Failed to construct runtime: {err}")));
                    return;
                }
            };
            let local = tokio::task::LocalSet::new();
            local.block_on(&runtime, async move {
                let mut inner = match InnerVlConverter::try_new().await {
                    Ok(inner) => {
                        let _ = startup_tx.send(Ok(()));
                        inner
                    }
                    Err(err) => {
                        let _ = startup_tx.send(Err(err.to_string()));
                        return;
                    }
                };

                while let Some(cmd) = rx.recv().await {
                    if let Err(e) = inner.refresh_font_config_if_needed() {
                        cmd.send_error(e);
                        continue;
                    }
                    inner.handle_command(cmd).await;
                }
            });
        });
        handles.push(handle);
        startup_receivers.push(startup_rx);
    }

    for startup_rx in startup_receivers {
        match startup_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                drop(senders);
                for handle in handles {
                    let _ = handle.join();
                }
                bail!("Failed to initialize worker: {err}");
            }
            Err(err) => {
                drop(senders);
                for handle in handles {
                    let _ = handle.join();
                }
                bail!("Failed to receive worker startup status: {err}");
            }
        }
    }

    Ok(WorkerPool {
        senders,
        next_sender_idx: AtomicUsize::new(0),
        _handles: handles,
    })
}

/// A JSON value that may already be serialized to a string.
/// When the caller already has a JSON string (e.g. from Python), this avoids
/// a redundant parse→Value→serialize round-trip.
#[derive(Debug, Clone)]
pub enum ValueOrString {
    /// Pre-serialized JSON string — stored directly, no serialization needed
    JsonString(String),
    /// Parsed serde_json::Value — will be serialized to JSON when needed
    Value(serde_json::Value),
}

impl From<serde_json::Value> for ValueOrString {
    fn from(v: serde_json::Value) -> Self {
        ValueOrString::Value(v)
    }
}

impl From<&serde_json::Value> for ValueOrString {
    fn from(v: &serde_json::Value) -> Self {
        ValueOrString::Value(v.clone())
    }
}

impl From<String> for ValueOrString {
    fn from(s: String) -> Self {
        ValueOrString::JsonString(s)
    }
}

impl ValueOrString {
    /// Convert to a serde_json::Value, parsing if necessary.
    pub fn to_value(self) -> Result<serde_json::Value, AnyError> {
        match self {
            ValueOrString::Value(v) => Ok(v),
            ValueOrString::JsonString(s) => Ok(serde_json::from_str(&s)?),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VgOpts {
    pub allowed_base_urls: Option<Vec<String>>,
    pub format_locale: Option<FormatLocale>,
    pub time_format_locale: Option<TimeFormatLocale>,
}

impl VgOpts {
    pub fn to_embed_opts(&self, renderer: Renderer) -> Result<serde_json::Value, AnyError> {
        let mut opts_map = serde_json::Map::new();

        opts_map.insert(
            "renderer".to_string(),
            serde_json::Value::String(renderer.to_string()),
        );

        if let Some(format_locale) = &self.format_locale {
            opts_map.insert("formatLocale".to_string(), format_locale.as_object()?);
        }
        if let Some(time_format_locale) = &self.time_format_locale {
            opts_map.insert(
                "timeFormatLocale".to_string(),
                time_format_locale.as_object()?,
            );
        }

        Ok(serde_json::Value::Object(opts_map))
    }
}

#[derive(Debug, Clone)]
pub enum FormatLocale {
    Name(String),
    Object(serde_json::Value),
}

impl FormatLocale {
    pub fn as_object(&self) -> Result<serde_json::Value, AnyError> {
        match self {
            FormatLocale::Name(name) => {
                let Some(locale_str) = FORMATE_LOCALE_MAP.get(name) else {
                    return Err(anyhow!("No built-in format locale named {}", name));
                };
                Ok(serde_json::from_str(locale_str)?)
            }
            FormatLocale::Object(object) => Ok(object.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TimeFormatLocale {
    Name(String),
    Object(serde_json::Value),
}

impl TimeFormatLocale {
    pub fn as_object(&self) -> Result<serde_json::Value, AnyError> {
        match self {
            TimeFormatLocale::Name(name) => {
                let Some(locale_str) = TIME_FORMATE_LOCALE_MAP.get(name) else {
                    return Err(anyhow!("No built-in time format locale named {}", name));
                };
                Ok(serde_json::from_str(locale_str)?)
            }
            TimeFormatLocale::Object(object) => Ok(object.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Renderer {
    Svg,
    Canvas,
    Hybrid,
}

impl Display for Renderer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let r = match self {
            Renderer::Svg => "svg",
            Renderer::Canvas => "canvas",
            Renderer::Hybrid => "hybrid",
        };
        std::fmt::Display::fmt(r, f)
    }
}

impl FromStr for Renderer {
    type Err = AnyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "svg" => Self::Svg,
            "canvas" => Self::Canvas,
            "hybrid" => Self::Hybrid,
            _ => return Err(anyhow!("Unsupported renderer: {}", s)),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct VlOpts {
    pub config: Option<serde_json::Value>,
    pub theme: Option<String>,
    pub vl_version: VlVersion,
    pub show_warnings: bool,
    pub allowed_base_urls: Option<Vec<String>>,
    pub format_locale: Option<FormatLocale>,
    pub time_format_locale: Option<TimeFormatLocale>,
}

impl VlOpts {
    pub fn to_embed_opts(&self, renderer: Renderer) -> Result<serde_json::Value, AnyError> {
        let mut opts_map = serde_json::Map::new();

        opts_map.insert(
            "renderer".to_string(),
            serde_json::Value::String(renderer.to_string()),
        );

        if let Some(theme) = &self.theme {
            opts_map.insert(
                "theme".to_string(),
                serde_json::Value::String(theme.clone()),
            );
        }

        if let Some(config) = &self.config {
            opts_map.insert("config".to_string(), config.clone());
        }

        if let Some(format_locale) = &self.format_locale {
            opts_map.insert("formatLocale".to_string(), format_locale.as_object()?);
        }
        if let Some(time_format_locale) = &self.time_format_locale {
            opts_map.insert(
                "timeFormatLocale".to_string(),
                time_format_locale.as_object()?,
            );
        }

        Ok(serde_json::Value::Object(opts_map))
    }
}

fn next_id() -> Result<i32, AnyError> {
    match NEXT_ID.lock() {
        Ok(mut guard) => {
            let id = *guard;
            *guard = (*guard + 1) % i32::MAX;
            Ok(id)
        }
        Err(err) => {
            bail!("Failed to acquire lock: {err}")
        }
    }
}

fn set_json_arg(arg: serde_json::Value) -> Result<i32, AnyError> {
    set_json_str_arg(serde_json::to_string(&arg)?)
}

fn set_json_str_arg(json_str: String) -> Result<i32, AnyError> {
    let id = next_id()?;

    match JSON_ARGS.lock() {
        Ok(mut guard) => {
            guard.insert(id, json_str);
        }
        Err(err) => {
            bail!("Failed to acquire lock: {err}")
        }
    }

    Ok(id)
}

fn set_spec_arg(spec: ValueOrString) -> Result<i32, AnyError> {
    match spec {
        ValueOrString::JsonString(s) => set_json_str_arg(s),
        ValueOrString::Value(v) => set_json_arg(v),
    }
}

fn alloc_msgpack_result_id() -> Result<i32, AnyError> {
    next_id()
}

fn take_msgpack_result(result_id: i32) -> Result<Vec<u8>, AnyError> {
    match MSGPACK_RESULTS.lock() {
        Ok(mut guard) => guard
            .remove(&result_id)
            .ok_or_else(|| anyhow!("Result id not found")),
        Err(err) => bail!("Failed to acquire lock: {err}"),
    }
}

fn clear_json_arg(arg_id: i32) {
    if let Ok(mut guard) = JSON_ARGS.lock() {
        guard.remove(&arg_id);
    }
}

fn clear_msgpack_result(result_id: i32) {
    if let Ok(mut guard) = MSGPACK_RESULTS.lock() {
        guard.remove(&result_id);
    }
}

struct JsonArgGuard {
    arg_id: Option<i32>,
}

impl JsonArgGuard {
    fn from_value(value: serde_json::Value) -> Result<Self, AnyError> {
        Ok(Self {
            arg_id: Some(set_json_arg(value)?),
        })
    }

    fn from_spec(spec: ValueOrString) -> Result<Self, AnyError> {
        Ok(Self {
            arg_id: Some(set_spec_arg(spec)?),
        })
    }

    fn id(&self) -> i32 {
        self.arg_id.expect("JsonArgGuard id missing")
    }
}

impl Drop for JsonArgGuard {
    fn drop(&mut self) {
        if let Some(arg_id) = self.arg_id.take() {
            clear_json_arg(arg_id);
        }
    }
}

struct MsgpackResultGuard {
    result_id: Option<i32>,
}

impl MsgpackResultGuard {
    fn new() -> Result<Self, AnyError> {
        Ok(Self {
            result_id: Some(alloc_msgpack_result_id()?),
        })
    }

    fn id(&self) -> i32 {
        self.result_id.expect("MsgpackResultGuard id missing")
    }

    fn take_result(mut self) -> Result<Vec<u8>, AnyError> {
        let result_id = self
            .result_id
            .take()
            .expect("MsgpackResultGuard id missing");
        take_msgpack_result(result_id)
    }
}

impl Drop for MsgpackResultGuard {
    fn drop(&mut self) {
        if let Some(result_id) = self.result_id.take() {
            clear_msgpack_result(result_id);
        }
    }
}

#[op2]
#[string]
fn op_get_json_arg(arg_id: i32) -> Result<String, JsErrorBox> {
    match JSON_ARGS.lock() {
        Ok(mut guard) => {
            if let Some(arg) = guard.remove(&arg_id) {
                Ok(arg)
            } else {
                Err(JsErrorBox::generic("Arg id not found"))
            }
        }
        Err(err) => Err(JsErrorBox::generic(format!(
            "Failed to acquire lock: {}",
            err
        ))),
    }
}

#[op2(fast)]
fn op_set_msgpack_result(result_id: i32, #[buffer] data: &[u8]) -> Result<(), JsErrorBox> {
    match MSGPACK_RESULTS.lock() {
        Ok(mut guard) => {
            guard.insert(result_id, data.to_vec());
            Ok(())
        }
        Err(err) => Err(JsErrorBox::generic(format!(
            "Failed to acquire lock: {}",
            err
        ))),
    }
}

/// Struct that interacts directly with the Deno JavaScript runtime. Not Sendable
struct InnerVlConverter {
    worker: MainWorker,
    initialized_vl_versions: HashSet<VlVersion>,
    vega_initialized: bool,
    font_config_version: u64,
}

impl InnerVlConverter {
    /// Refresh the SharedFontConfig in OpState if fonts have been registered
    /// since the worker was created (or since the last refresh).
    fn refresh_font_config_if_needed(&mut self) -> Result<(), AnyError> {
        let current = FONT_CONFIG_VERSION.load(Ordering::Acquire);
        if current != self.font_config_version {
            let font_config = FONT_CONFIG
                .lock()
                .map_err(|e| anyhow!("Failed to acquire FONT_CONFIG lock: {}", e))?;
            let resolved = font_config.resolve();
            let shared_config = vl_convert_canvas2d_deno::SharedFontConfig::new(resolved, current);
            self.worker
                .js_runtime
                .op_state()
                .borrow_mut()
                .put(shared_config);
            self.font_config_version = current;
        }
        Ok(())
    }

    async fn init_vega(&mut self) -> Result<(), AnyError> {
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
class WarningCollector {
  constructor() {
    this.warningsLogs = [];
  }

  level(lvl) {
    if (lvl == null) return 0;
    return this;
  }

  error(msg) {
    console.error(msg);
    return this;
  }

  warn(msg) {
    this.warningsLogs.push(msg);
    return this;
  }

  // skip info an debug
  info() {
    return this;
  }

  debug() {
    return this;
  }
}
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
            let function_str = r#"
function vegaToView(vgSpec, allowedBaseUrls, errors) {
    let runtime = vega.parse(vgSpec);
    let baseURL = 'https://vega.github.io/vega-datasets/';
    const loader = vega.loader({ mode: 'http', baseURL });
    const originalHttp = loader.http.bind(loader);

    if (allowedBaseUrls != null) {
        loader.http = async (uri, options) => {
            if (
                allowedBaseUrls.every(
                    (allowedUrl) => !new URL(uri).href.startsWith(allowedUrl),
                )
            ) {
                errors.push(`External data url not allowed: ${uri}`);
                throw new Error(`External data url not allowed: ${uri}`);
            }
            return originalHttp(uri, options);
        };
    }

    return new vega.View(runtime, {renderer: 'none', loader});
}

function vegaToSvg(vgSpec, allowedBaseUrls, formatLocale, timeFormatLocale, errors) {
    if (formatLocale != null) {
        vega.formatLocale(formatLocale);
    }
    if (timeFormatLocale != null) {
        vega.timeFormatLocale(timeFormatLocale);
    }
    let view = vegaToView(vgSpec, allowedBaseUrls, errors);
    let svgPromise = view.runAsync().then(() => {
        try {
            // Workaround for https://github.com/vega/vega/issues/3481
            view.signal("geo_interval_init_tick", {});
        } catch (e) {
            // No geo_interval_init_tick signal
        }
    }).then(() => {
        return view.runAsync().then(
            () => view.toSVG()
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

function vegaToScenegraph(vgSpec, allowedBaseUrls, formatLocale, timeFormatLocale, errors) {
    if (formatLocale != null) {
        vega.formatLocale(formatLocale);
    }
    if (timeFormatLocale != null) {
        vega.timeFormatLocale(timeFormatLocale);
    }
    let view = vegaToView(vgSpec, allowedBaseUrls, errors);
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

function vegaToViewCanvas(vgSpec, allowedBaseUrls, errors) {
    // Use the same view setup as vegaToView, since toCanvas() creates its own renderer
    return vegaToView(vgSpec, allowedBaseUrls, errors);
}

function vegaToCanvas(vgSpec, allowedBaseUrls, formatLocale, timeFormatLocale, scale, errors) {
    if (formatLocale != null) {
        vega.formatLocale(formatLocale);
    }
    if (timeFormatLocale != null) {
        vega.timeFormatLocale(timeFormatLocale);
    }

    let view = vegaToViewCanvas(vgSpec, allowedBaseUrls, errors);
    let canvasPromise = view.runAsync().then(() => {
        try {
            // Workaround for https://github.com/vega/vega/issues/3481
            view.signal("geo_interval_init_tick", {});
        } catch (e) {
            // No geo_interval_init_tick signal
        }
    }).then(() => {
        return view.runAsync().then(
            // Pass scale factor to toCanvas
            () => view.toCanvas(scale)
        ).finally(() => {
            view.finalize();
            vega.resetDefaultLocale();
        })
    });
    return canvasPromise;
}
"#;
            self.worker.js_runtime.execute_script(
                "ext:<anon>",
                deno_core::FastString::from_static(function_str),
            )?;
            self.worker
                .js_runtime
                .run_event_loop(Default::default())
                .await?;

            self.vega_initialized = true;
        }

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
function compileVegaLite_{ver_name}(vlSpec, config, theme, warnings) {{
    let options = {{}};

    // Handle config and theme
    let usermetaTheme = ((vlSpec.usermeta ?? {{}}).embedOptions ?? {{}}).theme;
    let namedTheme = theme ?? usermetaTheme;
    if (namedTheme != null) {{
        options["config"] = vega.mergeConfig(vegaThemes[namedTheme], config ?? {{}});
    }} else if (config != null) {{
        options["config"] = config;
    }}

    if (!warnings) {{
        options["logger"] = new WarningCollector();
    }}

    return {ver_name}.compile(vlSpec, options).spec
}}

function vegaLiteToSvg_{ver_name}(vlSpec, config, theme, warnings, allowedBaseUrls, formatLocale, timeFormatLocale, errors) {{
    let vgSpec = compileVegaLite_{ver_name}(vlSpec, config, theme, warnings);
    return vegaToSvg(vgSpec, allowedBaseUrls, formatLocale, timeFormatLocale, errors)
}}

function vegaLiteToScenegraph_{ver_name}(vlSpec, config, theme, warnings, allowedBaseUrls, formatLocale, timeFormatLocale, errors) {{
    let vgSpec = compileVegaLite_{ver_name}(vlSpec, config, theme, warnings);
    return vegaToScenegraph(vgSpec, allowedBaseUrls,formatLocale, timeFormatLocale,  errors)
}}

function vegaLiteToCanvas_{ver_name}(vlSpec, config, theme, warnings, allowedBaseUrls, formatLocale, timeFormatLocale, scale, errors) {{
    let vgSpec = compileVegaLite_{ver_name}(vlSpec, config, theme, warnings);
    return vegaToCanvas(vgSpec, allowedBaseUrls, formatLocale, timeFormatLocale, scale, errors)
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

    pub async fn try_new() -> Result<Self, AnyError> {
        // MainWorker's deno_tls extension panics without a global crypto provider
        let _ =
            deno_runtime::deno_tls::rustls::crypto::aws_lc_rs::default_provider().install_default();

        let module_loader = Rc::new(VlConvertModuleLoader);

        // Create a dummy main module specifier for the worker
        let main_module = ModuleSpecifier::parse("ext:vl_convert/main.js")
            .expect("Failed to parse main module specifier");

        // Create permission descriptor parser using RealSys
        let descriptor_parser = Arc::new(RuntimePermissionDescriptorParser::new(VlConvertNodeSys));

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
            permissions: PermissionsContainer::allow_all(descriptor_parser),
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
        let options = WorkerOptions {
            extensions: vec![
                // Canvas 2D extension from vl-convert-canvas2d-deno crate
                vl_convert_canvas2d_deno::vl_convert_canvas2d::init(),
                // Our runtime extension (text width, JSON args)
                vl_convert_runtime::init(),
            ],
            startup_snapshot: Some(crate::VL_CONVERT_SNAPSHOT),
            ..Default::default()
        };

        // Create the MainWorker with full Web API support
        let worker = MainWorker::bootstrap_from_options(&main_module, services, options);

        // Add shared font config to OpState so canvas contexts use the same fonts as SVG rendering.
        // We resolve the FontConfig into a fontdb once here; each canvas context then clones
        // the cached database instead of re-scanning system fonts.
        let initial_font_version = FONT_CONFIG_VERSION.load(Ordering::Acquire);
        {
            let font_config = FONT_CONFIG
                .lock()
                .map_err(|e| anyhow!("Failed to acquire FONT_CONFIG lock: {}", e))?;
            let resolved = font_config.resolve();
            let shared_config =
                vl_convert_canvas2d_deno::SharedFontConfig::new(resolved, initial_font_version);
            worker.js_runtime.op_state().borrow_mut().put(shared_config);
        }

        let this = Self {
            worker,
            initialized_vl_versions: Default::default(),
            vega_initialized: false,
            font_config_version: initial_font_version,
        };

        Ok(this)
    }

    async fn execute_script_to_json(
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

    async fn execute_script_to_string(&mut self, script: &str) -> Result<String, AnyError> {
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

    pub async fn vegalite_to_vega(
        &mut self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<serde_json::Value, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_opts.vl_version).await?;
        let config = vl_opts.config.clone().unwrap_or(serde_json::Value::Null);

        let spec_arg = JsonArgGuard::from_spec(vl_spec.into())?;
        let config_arg = JsonArgGuard::from_value(config)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let code = format!(
            r#"
compileVegaLite_{ver_name:?}(
    JSON.parse(op_get_json_arg({spec_arg_id})),
    JSON.parse(op_get_json_arg({config_arg_id})),
    {theme_arg},
    {show_warnings}
)
"#,
            ver_name = vl_opts.vl_version,
            spec_arg_id = spec_arg.id(),
            config_arg_id = config_arg.id(),
            theme_arg = theme_arg,
            show_warnings = vl_opts.show_warnings,
        );

        let value = self.execute_script_to_json(&code).await?;
        Ok(value)
    }

    pub async fn vegalite_to_svg(
        &mut self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<String, AnyError> {
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

        let spec_arg = JsonArgGuard::from_spec(vl_spec.into())?;
        let config_arg = JsonArgGuard::from_value(config)?;
        let allowed_base_urls =
            serde_json::to_string(&serde_json::Value::from(vl_opts.allowed_base_urls))?;
        let format_locale_arg = JsonArgGuard::from_value(format_locale)?;
        let time_format_locale_arg = JsonArgGuard::from_value(time_format_locale)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let code = format!(
            r#"
var svg;
var errors = [];
vegaLiteToSvg_{ver_name:?}(
    JSON.parse(op_get_json_arg({spec_arg_id})),
    JSON.parse(op_get_json_arg({config_arg_id})),
    {theme_arg},
    {show_warnings},
    {allowed_base_urls},
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
            show_warnings = vl_opts.show_warnings,
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

        let value = self.execute_script_to_string("svg").await?;
        Ok(value)
    }

    pub async fn vegalite_to_scenegraph_msgpack(
        &mut self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<Vec<u8>, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_opts.vl_version).await?;
        let vl_spec = vl_spec.into();

        let config = vl_opts.config.clone().unwrap_or(serde_json::Value::Null);
        let format_locale = match vl_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vl_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let spec_arg = JsonArgGuard::from_spec(vl_spec)?;
        let config_arg = JsonArgGuard::from_value(config)?;
        let allowed_base_urls =
            serde_json::to_string(&serde_json::Value::from(vl_opts.allowed_base_urls))?;
        let format_locale_arg = JsonArgGuard::from_value(format_locale)?;
        let time_format_locale_arg = JsonArgGuard::from_value(time_format_locale)?;
        let result = MsgpackResultGuard::new()?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let code = format!(
            r#"
var errors = [];
vegaLiteToScenegraph_{ver_name:?}(
    JSON.parse(op_get_json_arg({spec_arg_id})),
    JSON.parse(op_get_json_arg({config_arg_id})),
    {theme_arg},
    {show_warnings},
    {allowed_base_urls},
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
            show_warnings = vl_opts.show_warnings,
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

        result.take_result()
    }

    pub async fn vegalite_to_scenegraph(
        &mut self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<serde_json::Value, AnyError> {
        let sg_msgpack = self
            .vegalite_to_scenegraph_msgpack(vl_spec, vl_opts)
            .await?;
        let value: serde_json::Value = rmp_serde::from_slice(&sg_msgpack)
            .map_err(|err| anyhow!("Failed to decode MessagePack scenegraph: {err}"))?;
        Ok(value)
    }

    pub async fn vega_to_svg(
        &mut self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<String, AnyError> {
        self.init_vega().await?;
        let allowed_base_urls =
            serde_json::to_string(&serde_json::Value::from(vg_opts.allowed_base_urls))?;

        let format_locale = match vg_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vg_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let spec_arg = JsonArgGuard::from_spec(vg_spec.into())?;
        let format_locale_arg = JsonArgGuard::from_value(format_locale)?;
        let time_format_locale_arg = JsonArgGuard::from_value(time_format_locale)?;

        let code = format!(
            r#"
var svg;
var errors = [];
vegaToSvg(
    JSON.parse(op_get_json_arg({arg_id})),
    {allowed_base_urls},
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
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
        );
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let value = self.execute_script_to_string("svg").await?;
        Ok(value)
    }

    pub async fn vega_to_scenegraph_msgpack(
        &mut self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<Vec<u8>, AnyError> {
        self.init_vega().await?;
        let vg_spec = vg_spec.into();
        let allowed_base_urls =
            serde_json::to_string(&serde_json::Value::from(vg_opts.allowed_base_urls))?;
        let format_locale = match vg_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vg_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let spec_arg = JsonArgGuard::from_spec(vg_spec)?;
        let format_locale_arg = JsonArgGuard::from_value(format_locale)?;
        let time_format_locale_arg = JsonArgGuard::from_value(time_format_locale)?;
        let result = MsgpackResultGuard::new()?;

        let code = format!(
            r#"
var errors = [];
vegaToScenegraph(
    JSON.parse(op_get_json_arg({arg_id})),
    {allowed_base_urls},
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
            arg_id = spec_arg.id(),
            format_locale_id = format_locale_arg.id(),
            time_format_locale_id = time_format_locale_arg.id(),
            result_id = result.id(),
        );
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        result.take_result()
    }

    pub async fn vega_to_scenegraph(
        &mut self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<serde_json::Value, AnyError> {
        let sg_msgpack = self.vega_to_scenegraph_msgpack(vg_spec, vg_opts).await?;
        let value: serde_json::Value = rmp_serde::from_slice(&sg_msgpack)
            .map_err(|err| anyhow!("Failed to decode MessagePack scenegraph: {err}"))?;
        Ok(value)
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

    async fn execute_script_to_bytes(&mut self, script: &str) -> Result<Vec<u8>, AnyError> {
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
    ) -> Result<Vec<u8>, AnyError> {
        self.init_vega().await?;
        let allowed_base_urls =
            serde_json::to_string(&serde_json::Value::from(vg_opts.allowed_base_urls))?;

        let format_locale = match vg_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vg_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let spec_arg = JsonArgGuard::from_value(vg_spec.clone())?;
        let format_locale_arg = JsonArgGuard::from_value(format_locale)?;
        let time_format_locale_arg = JsonArgGuard::from_value(time_format_locale)?;

        let code = format!(
            r#"
var canvasPngData;
var errors = [];
vegaToCanvas(
    JSON.parse(op_get_json_arg({arg_id})),
    {allowed_base_urls},
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
            arg_id = spec_arg.id(),
            format_locale_id = format_locale_arg.id(),
            time_format_locale_id = time_format_locale_arg.id(),
        );
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let png_data = self.execute_script_to_bytes("canvasPngData").await?;
        Ok(png_data)
    }

    pub async fn vegalite_to_png(
        &mut self,
        vl_spec: &serde_json::Value,
        vl_opts: VlOpts,
        scale: f32,
        ppi: f32,
    ) -> Result<Vec<u8>, AnyError> {
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

        let spec_arg = JsonArgGuard::from_value(vl_spec.clone())?;
        let config_arg = JsonArgGuard::from_value(config)?;
        let format_locale_arg = JsonArgGuard::from_value(format_locale)?;
        let time_format_locale_arg = JsonArgGuard::from_value(time_format_locale)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let allowed_base_urls =
            serde_json::to_string(&serde_json::Value::from(vl_opts.allowed_base_urls))?;

        let code = format!(
            r#"
var canvasPngData;
var errors = [];
vegaLiteToCanvas_{ver_name:?}(
    JSON.parse(op_get_json_arg({spec_arg_id})),
    JSON.parse(op_get_json_arg({config_arg_id})),
    {theme_arg},
    {show_warnings},
    {allowed_base_urls},
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
            show_warnings = vl_opts.show_warnings,
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

        let png_data = self.execute_script_to_bytes("canvasPngData").await?;
        Ok(png_data)
    }

    async fn handle_command(&mut self, cmd: VlConvertCommand) {
        match cmd {
            VlConvertCommand::VlToVg {
                vl_spec,
                vl_opts,
                responder,
            } => {
                let vega_spec = self.vegalite_to_vega(vl_spec, vl_opts).await;
                responder.send(vega_spec).ok();
            }
            VlConvertCommand::VgToSvg {
                vg_spec,
                vg_opts,
                responder,
            } => {
                let svg_result = self.vega_to_svg(vg_spec, vg_opts).await;
                responder.send(svg_result).ok();
            }
            VlConvertCommand::VgToSg {
                vg_spec,
                vg_opts,
                responder,
            } => {
                let sg_result = self.vega_to_scenegraph(vg_spec, vg_opts).await;
                responder.send(sg_result).ok();
            }
            VlConvertCommand::VgToSgMsgpack {
                vg_spec,
                vg_opts,
                responder,
            } => {
                let sg_result = self.vega_to_scenegraph_msgpack(vg_spec, vg_opts).await;
                responder.send(sg_result).ok();
            }
            VlConvertCommand::VlToSvg {
                vl_spec,
                vl_opts,
                responder,
            } => {
                let svg_result = self.vegalite_to_svg(vl_spec, vl_opts).await;
                responder.send(svg_result).ok();
            }
            VlConvertCommand::VlToSg {
                vl_spec,
                vl_opts,
                responder,
            } => {
                let sg_result = self.vegalite_to_scenegraph(vl_spec, vl_opts).await;
                responder.send(sg_result).ok();
            }
            VlConvertCommand::VlToSgMsgpack {
                vl_spec,
                vl_opts,
                responder,
            } => {
                let sg_result = self.vegalite_to_scenegraph_msgpack(vl_spec, vl_opts).await;
                responder.send(sg_result).ok();
            }
            VlConvertCommand::VgToPng {
                vg_spec,
                vg_opts,
                scale,
                ppi,
                responder,
            } => {
                let png_result = match vg_spec.to_value() {
                    Ok(v) => self.vega_to_png(&v, vg_opts, scale, ppi).await,
                    Err(e) => Err(e),
                };
                responder.send(png_result).ok();
            }
            VlConvertCommand::VlToPng {
                vl_spec,
                vl_opts,
                scale,
                ppi,
                responder,
            } => {
                let png_result = match vl_spec.to_value() {
                    Ok(v) => self.vegalite_to_png(&v, vl_opts, scale, ppi).await,
                    Err(e) => Err(e),
                };
                responder.send(png_result).ok();
            }
            VlConvertCommand::GetLocalTz { responder } => {
                let local_tz = self.get_local_tz().await;
                responder.send(local_tz).ok();
            }
            VlConvertCommand::GetThemes { responder } => {
                let themes = self.get_themes().await;
                responder.send(themes).ok();
            }
        }
    }
}

pub enum VlConvertCommand {
    VlToVg {
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        responder: oneshot::Sender<Result<serde_json::Value, AnyError>>,
    },
    VgToSvg {
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        responder: oneshot::Sender<Result<String, AnyError>>,
    },
    VgToSg {
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        responder: oneshot::Sender<Result<serde_json::Value, AnyError>>,
    },
    VgToSgMsgpack {
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    VlToSvg {
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        responder: oneshot::Sender<Result<String, AnyError>>,
    },
    VlToSg {
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        responder: oneshot::Sender<Result<serde_json::Value, AnyError>>,
    },
    VgToPng {
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        scale: f32,
        ppi: f32,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    VlToPng {
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        scale: f32,
        ppi: f32,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    VlToSgMsgpack {
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    GetLocalTz {
        responder: oneshot::Sender<Result<Option<String>, AnyError>>,
    },
    GetThemes {
        responder: oneshot::Sender<Result<serde_json::Value, AnyError>>,
    },
}

impl VlConvertCommand {
    /// Send an error to the command's responder, consuming the command.
    fn send_error(self, err: AnyError) {
        match self {
            Self::VlToVg { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::VgToSvg { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::VgToSg { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::VgToSgMsgpack { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::VlToSvg { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::VlToSg { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::VlToSgMsgpack { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::VgToPng { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::VlToPng { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::GetLocalTz { responder } => {
                responder.send(Err(err)).ok();
            }
            Self::GetThemes { responder } => {
                responder.send(Err(err)).ok();
            }
        }
    }
}

/// Struct for performing Vega-Lite to Vega conversions using the Deno v8 Runtime
///
/// # Examples
///
/// ```
/// use vl_convert_rs::{VlConverter, VlVersion};
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
/// }   "#).unwrap();
///
///     let vega_spec = futures::executor::block_on(
///         converter.vegalite_to_vega(vl_spec, Default::default())
///     ).expect(
///         "Failed to perform Vega-Lite to Vega conversion"
///     );
///
///     println!("{}", vega_spec)
/// ```
struct VlConverterInner {
    vegaembed_bundles: Mutex<HashMap<VlVersion, String>>,
    pool: Mutex<Option<WorkerPool>>,
    num_workers: usize,
}

#[derive(Clone)]
pub struct VlConverter {
    inner: Arc<VlConverterInner>,
}

impl VlConverter {
    pub fn new() -> Self {
        Self::with_num_workers(1).expect("default worker count must be valid")
    }

    pub fn with_num_workers(num_workers: usize) -> Result<Self, AnyError> {
        if num_workers < 1 {
            bail!("num_workers must be >= 1");
        }

        // Initialize environment logger with filter to suppress noisy SWC tree-shaker spans
        // The swc_ecma_transforms_optimization module logs tracing spans at ERROR level
        // which are not actual errors - just instrumentation.
        env_logger::Builder::from_env(env_logger::Env::default())
            .filter_module("swc_ecma_transforms_optimization", log::LevelFilter::Off)
            .try_init()
            .ok();

        Ok(Self {
            inner: Arc::new(VlConverterInner {
                vegaembed_bundles: Default::default(),
                pool: Default::default(),
                num_workers,
            }),
        })
    }

    pub fn num_workers(&self) -> usize {
        self.inner.num_workers
    }

    fn get_or_spawn_sender(&self) -> Result<tokio::sync::mpsc::Sender<VlConvertCommand>, AnyError> {
        let mut guard = self
            .inner
            .pool
            .lock()
            .map_err(|e| anyhow!("Failed to lock worker pool: {e}"))?;

        if let Some(pool) = guard.as_ref() {
            if !pool.is_closed() {
                return pool
                    .next_sender()
                    .ok_or_else(|| anyhow!("Worker pool has no senders"));
            }
            *guard = None;
        }

        let pool = spawn_worker_pool(self.inner.num_workers)?;
        let sender = pool
            .next_sender()
            .ok_or_else(|| anyhow!("Worker pool has no senders"))?;
        *guard = Some(pool);
        Ok(sender)
    }

    fn reset_pool(&self) -> Result<(), AnyError> {
        let mut guard = self
            .inner
            .pool
            .lock()
            .map_err(|e| anyhow!("Failed to lock worker pool: {e}"))?;
        *guard = None;
        Ok(())
    }

    async fn send_command_with_retry(
        &self,
        cmd: VlConvertCommand,
        request_name: &str,
    ) -> Result<(), AnyError> {
        let sender = self.get_or_spawn_sender()?;
        match sender.send(cmd).await {
            Ok(()) => Ok(()),
            Err(tokio::sync::mpsc::error::SendError(cmd)) => {
                self.reset_pool()?;
                let sender = self.get_or_spawn_sender()?;
                sender.send(cmd).await.map_err(|err| {
                    anyhow!("Failed to send {request_name} request after retry: {err}")
                })
            }
        }
    }

    async fn request<R>(
        &self,
        make_cmd: impl FnOnce(oneshot::Sender<Result<R, AnyError>>) -> VlConvertCommand,
        request_name: &str,
    ) -> Result<R, AnyError> {
        let (resp_tx, resp_rx) = oneshot::channel::<Result<R, AnyError>>();
        self.send_command_with_retry(make_cmd(resp_tx), request_name)
            .await?;
        match resp_rx.await {
            Ok(result) => result,
            Err(err) => bail!("Failed to retrieve {request_name} result: {err}"),
        }
    }

    pub async fn vegalite_to_vega(
        &self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<serde_json::Value, AnyError> {
        let vl_spec = vl_spec.into();
        self.request(
            move |responder| VlConvertCommand::VlToVg {
                vl_spec,
                vl_opts,
                responder,
            },
            "Vega-Lite to Vega conversion",
        )
        .await
    }

    pub async fn vega_to_svg(
        &self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<String, AnyError> {
        let vg_spec = vg_spec.into();
        self.request(
            move |responder| VlConvertCommand::VgToSvg {
                vg_spec,
                vg_opts,
                responder,
            },
            "Vega to SVG conversion",
        )
        .await
    }

    pub async fn vega_to_scenegraph(
        &self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<serde_json::Value, AnyError> {
        let vg_spec = vg_spec.into();
        self.request(
            move |responder| VlConvertCommand::VgToSg {
                vg_spec,
                vg_opts,
                responder,
            },
            "Vega to Scenegraph conversion",
        )
        .await
    }

    pub async fn vega_to_scenegraph_msgpack(
        &self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<Vec<u8>, AnyError> {
        let vg_spec = vg_spec.into();
        self.request(
            move |responder| VlConvertCommand::VgToSgMsgpack {
                vg_spec,
                vg_opts,
                responder,
            },
            "Vega to Scenegraph conversion",
        )
        .await
    }

    pub async fn vegalite_to_svg(
        &self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<String, AnyError> {
        let vl_spec = vl_spec.into();
        self.request(
            move |responder| VlConvertCommand::VlToSvg {
                vl_spec,
                vl_opts,
                responder,
            },
            "Vega-Lite to SVG conversion",
        )
        .await
    }

    pub async fn vegalite_to_scenegraph(
        &self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<serde_json::Value, AnyError> {
        let vl_spec = vl_spec.into();
        self.request(
            move |responder| VlConvertCommand::VlToSg {
                vl_spec,
                vl_opts,
                responder,
            },
            "Vega-Lite to Scenegraph conversion",
        )
        .await
    }

    pub async fn vegalite_to_scenegraph_msgpack(
        &self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<Vec<u8>, AnyError> {
        let vl_spec = vl_spec.into();
        self.request(
            move |responder| VlConvertCommand::VlToSgMsgpack {
                vl_spec,
                vl_opts,
                responder,
            },
            "Vega-Lite to Scenegraph conversion",
        )
        .await
    }

    pub async fn vega_to_png(
        &self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
        scale: Option<f32>,
        ppi: Option<f32>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let ppi = ppi.unwrap_or(72.0);
        let effective_scale = scale * ppi / 72.0;
        let vg_spec = vg_spec.into();

        self.request(
            move |responder| VlConvertCommand::VgToPng {
                vg_spec,
                vg_opts,
                scale: effective_scale,
                ppi,
                responder,
            },
            "Vega to PNG conversion",
        )
        .await
    }

    pub async fn vegalite_to_png(
        &self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
        scale: Option<f32>,
        ppi: Option<f32>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let ppi = ppi.unwrap_or(72.0);
        let effective_scale = scale * ppi / 72.0;
        let vl_spec = vl_spec.into();

        self.request(
            move |responder| VlConvertCommand::VlToPng {
                vl_spec,
                vl_opts,
                scale: effective_scale,
                ppi,
                responder,
            },
            "Vega-Lite to PNG conversion",
        )
        .await
    }

    pub async fn vega_to_jpeg(
        &self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
        scale: Option<f32>,
        quality: Option<u8>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let svg = self.vega_to_svg(vg_spec, vg_opts).await?;
        svg_to_jpeg(&svg, scale, quality)
    }

    pub async fn vegalite_to_jpeg(
        &self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
        scale: Option<f32>,
        quality: Option<u8>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let svg = self.vegalite_to_svg(vl_spec, vl_opts).await?;
        svg_to_jpeg(&svg, scale, quality)
    }

    pub async fn vega_to_pdf(
        &self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<Vec<u8>, AnyError> {
        let svg = self.vega_to_svg(vg_spec, vg_opts).await?;
        svg_to_pdf(&svg)
    }

    pub async fn vegalite_to_pdf(
        &self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<Vec<u8>, AnyError> {
        let svg = self.vegalite_to_svg(vl_spec, vl_opts).await?;
        svg_to_pdf(&svg)
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

        let computed_bundle = bundle_vega_snippet(
            "window.vegaEmbed=vegaEmbed; window.vega=vega; window.vegaLite=vegaLite;",
            vl_version,
        )
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

    async fn build_html(
        &self,
        code: &str,
        vl_version: VlVersion,
        bundle: bool,
    ) -> Result<String, AnyError> {
        let script_tags = if bundle {
            format!(
                r#"
    <script type="text/javascript">{}</script>
            "#,
                self.get_vegaembed_bundle(vl_version).await?
            )
        } else {
            format!(
                r#"
    <script src="https://cdn.jsdelivr.net/npm/vega@6"></script>
    <script src="https://cdn.jsdelivr.net/npm/vega-lite@{vl_ver}"></script>
    <script src="https://cdn.jsdelivr.net/npm/vega-embed@6"></script>
            "#,
                vl_ver = vl_version.to_semver()
            )
        };

        Ok(format!(
            r#"<!DOCTYPE html>
<html>
  <head>
    <style>
        vega-chart.vega-embed {{
          width: 100%;
          display: flex;
        }}
        vega-chart.vega-embed details,
        vega-chart.vega-embed details summary {{
          position: relative;
        }}
    </style>
    <meta charset="UTF-8">
    <title>Chart</title>
{script_tags}
  </head>
  <body>
    <div id="vega-chart"></div>
    <script type="text/javascript">
{code}
    </script>
  </body>
</html>
        "#
        ))
    }

    pub async fn vegalite_to_html(
        &self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
        bundle: bool,
        renderer: Renderer,
    ) -> Result<String, AnyError> {
        let vl_version = vl_opts.vl_version;
        let code = get_vega_or_vegalite_script(vl_spec, vl_opts.to_embed_opts(renderer)?)?;
        self.build_html(&code, vl_version, bundle).await
    }

    pub async fn vega_to_html(
        &self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
        bundle: bool,
        renderer: Renderer,
    ) -> Result<String, AnyError> {
        let code = get_vega_or_vegalite_script(vg_spec, vg_opts.to_embed_opts(renderer)?)?;
        self.build_html(&code, Default::default(), bundle).await
    }

    pub async fn get_local_tz(&self) -> Result<Option<String>, AnyError> {
        self.request(
            |responder| VlConvertCommand::GetLocalTz { responder },
            "get_local_tz",
        )
        .await
    }

    pub async fn get_themes(&self) -> Result<serde_json::Value, AnyError> {
        self.request(
            |responder| VlConvertCommand::GetThemes { responder },
            "get_themes",
        )
        .await
    }
}

impl Default for VlConverter {
    fn default() -> Self {
        Self::new()
    }
}

// Modified from tiny-skia-0.10.0/src/pixmap.rs to include DPI
pub fn encode_png(pixmap: Pixmap, ppi: f32) -> Result<Vec<u8>, AnyError> {
    let mut pixmap = pixmap;

    // Demultiply alpha.
    //
    // RasterPipeline is 15% faster here, but produces slightly different results
    // due to rounding. So we stick with this method for now.
    for pixel in pixmap.pixels_mut() {
        let c = pixel.demultiply();
        let alpha = c.alpha();

        // jonmmease: tiny-skia uses the private PremultipliedColorU8::from_rgba_unchecked here,
        // but we need to use from_rgba, which checks to make sure r/g/b are less then or equal
        // to alpha. Use min to ensure we don't trigger the check
        *pixel = PremultipliedColorU8::from_rgba(
            c.red().min(alpha),
            c.green().min(alpha),
            c.blue().min(alpha),
            alpha,
        )
        .expect("Failed to construct PremultipliedColorU8 from rgba");
    }

    let mut data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut data, pixmap.width(), pixmap.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let ppm = (ppi.max(0.0) / 0.0254).round() as u32;
        encoder.set_pixel_dims(Some(PixelDimensions {
            xppu: ppm,
            yppu: ppm,
            unit: Unit::Meter,
        }));

        let mut writer = encoder.write_header()?;
        writer.write_image_data(pixmap.data())?;
    }

    Ok(data)
}

pub fn svg_to_png(svg: &str, scale: f32, ppi: Option<f32>) -> Result<Vec<u8>, AnyError> {
    // default ppi to 72
    let ppi = ppi.unwrap_or(72.0);
    let scale = scale * ppi / 72.0;

    // catch_unwind so that we don't poison Mutexes
    // if usvg/resvg panics
    let response = panic::catch_unwind(|| {
        let rtree = match parse_svg(svg) {
            Ok(rtree) => rtree,
            Err(err) => return Err(err),
        };

        let mut pixmap = tiny_skia::Pixmap::new(
            (rtree.size().width() * scale) as u32,
            (rtree.size().height() * scale) as u32,
        )
        .unwrap();

        let transform = tiny_skia::Transform::from_scale(scale, scale);
        render(&rtree, transform, &mut pixmap.as_mut());
        Ok(encode_png(pixmap, ppi))
    });
    match response {
        Ok(Ok(Ok(png_result))) => Ok(png_result),
        Ok(Err(err)) => Err(err),
        err => bail!("{err:?}"),
    }
}

pub fn svg_to_jpeg(svg: &str, scale: f32, quality: Option<u8>) -> Result<Vec<u8>, AnyError> {
    let png_bytes = svg_to_png(svg, scale, None)?;
    let img = ImageReader::new(Cursor::new(png_bytes))
        .with_guessed_format()?
        .decode()?;

    let quality = quality.unwrap_or(90);
    if quality > 100 {
        bail!("JPEG quality parameter must be between 0 and 100 inclusive. Received: {quality}");
    }

    let mut jpeg_bytes: Vec<u8> = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, quality);

    // Encode the image
    encoder.encode_image(&img)?;

    Ok(jpeg_bytes)
}

pub fn svg_to_pdf(svg: &str) -> Result<Vec<u8>, AnyError> {
    let tree = parse_svg(svg)?;
    let pdf = svg2pdf::to_pdf(&tree, ConversionOptions::default(), PageOptions::default());
    pdf.map_err(|err| anyhow!("Failed to convert SVG to PDF: {}", err))
}

/// Helper to parse svg string to usvg Tree with more helpful error messages
fn parse_svg(svg: &str) -> Result<usvg::Tree, AnyError> {
    let xml_opt = usvg::roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };

    let opts = USVG_OPTIONS
        .lock()
        .map_err(|err| anyhow!("Failed to acquire usvg options lock: {err}"))?;

    let doc = usvg::roxmltree::Document::parse_with_options(svg, xml_opt)?;

    match doc.root_element().tag_name().namespace() {
        Some("http://www.w3.org/2000/svg") => {
            // All good
        }
        Some(other) => {
            bail!(
                "Invalid xmlns for SVG file. \n\
                Expected \"http://www.w3.org/2000/svg\". \n\
                Found \"{other}\""
            );
        }
        None => {
            bail!(
                "SVG file must have the xmlns attribute set to \"http://www.w3.org/2000/svg\"\n\
                For example <svg width=\"100\", height=\"100\", xmlns=\"http://www.w3.org/2000/svg\">...</svg>"
            )
        }
    }

    Ok(usvg::Tree::from_xmltree(&doc, &opts)?)
}

pub fn vegalite_to_url(
    vl_spec: impl Into<ValueOrString>,
    fullscreen: bool,
) -> Result<String, AnyError> {
    let spec_str = match vl_spec.into() {
        ValueOrString::JsonString(s) => s,
        ValueOrString::Value(v) => serde_json::to_string(&v)?,
    };
    let compressed_data = lz_str::compress_to_encoded_uri_component(&spec_str);
    let view = if fullscreen {
        "/view".to_string()
    } else {
        String::new()
    };
    Ok(format!(
        "https://vega.github.io/editor/#/url/vega-lite/{compressed_data}{view}"
    ))
}

pub fn vega_to_url(
    vg_spec: impl Into<ValueOrString>,
    fullscreen: bool,
) -> Result<String, AnyError> {
    let spec_str = match vg_spec.into() {
        ValueOrString::JsonString(s) => s,
        ValueOrString::Value(v) => serde_json::to_string(&v)?,
    };
    let compressed_data = lz_str::compress_to_encoded_uri_component(&spec_str);
    let view = if fullscreen {
        "/view".to_string()
    } else {
        String::new()
    };
    Ok(format!(
        "https://vega.github.io/editor/#/url/vega/{compressed_data}{view}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_convert_context() {
        let ctx = VlConverter::new();
        let vl_spec: serde_json::Value = serde_json::from_str(r#"
{
    "data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
    "mark": "bar",
    "encoding": {
        "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
        "y": {"aggregate": "mean", "field": "precipitation"}
    }
}
        "#).unwrap();

        let vg_spec = ctx
            .vegalite_to_vega(
                vl_spec,
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        println!("vg_spec: {}", vg_spec)
    }

    #[tokio::test]
    async fn test_multi_convert_context() {
        let vl_spec: serde_json::Value = serde_json::from_str(r#"
{
    "data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
    "mark": "bar",
    "encoding": {
        "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
        "y": {"aggregate": "mean", "field": "precipitation"}
    }
}
        "#).unwrap();

        let ctx1 = VlConverter::new();
        let vg_spec1 = ctx1
            .vegalite_to_vega(
                vl_spec.clone(),
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        println!("vg_spec1: {}", vg_spec1);

        let ctx1 = VlConverter::new();
        let vg_spec2 = ctx1
            .vegalite_to_vega(
                vl_spec,
                VlOpts {
                    vl_version: VlVersion::v5_8,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        println!("vg_spec2: {}", vg_spec2);
    }

    #[tokio::test]
    async fn test_execute_script_to_bytes_typed_array() {
        let mut ctx = InnerVlConverter::try_new().await.unwrap();
        let bytes = ctx
            .execute_script_to_bytes("new Uint8Array([1, 2, 3])")
            .await
            .unwrap();
        assert_eq!(bytes, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_canvas_png_and_image_data_are_typed_arrays() {
        let mut ctx = InnerVlConverter::try_new().await.unwrap();
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
    async fn test_polyfill_unsupported_methods_throw() {
        let mut ctx = InnerVlConverter::try_new().await.unwrap();
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

    #[test]
    fn test_convert_vegalite_to_url() {
        let vl_spec: serde_json::Value = serde_json::from_str(r#"
{
    "data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
    "mark": "bar",
    "encoding": {
        "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
        "y": {"aggregate": "mean", "field": "precipitation"}
    }
}
        "#).unwrap();

        let url = vegalite_to_url(&vl_spec, false).unwrap();
        let expected = concat!(
            "https://vega.github.io/editor/#/url/vega-lite/",
            "N4IgJghgLhIFygK4CcA28QAspQA4Gc4B6I5CAdwDoBzASyk0QCNF8BTZAYwHsA7KNv0o8AtkQBubahAlSIAWkg",
            "x2UfERER8A5ESUz20KKjbzybaJg7D84kAF8ANCA3IA1hiYRkIJ4J5haXmp4UAAPEJAoWhE2AFVeegwRPgYfEAA",
            "zWjZUMAwlNjSoAE9cArgQbmQA3gh0RxAiiIhqamQ5ASTzXjTM7Nzy3DbOWlx6aFo+ezs7IA",
        );
        println!("{url}");
        assert_eq!(url, expected);
    }

    #[test]
    fn test_convert_vega_to_url() {
        let vl_spec: serde_json::Value = serde_json::from_str(
            r#"
{
  "$schema": "https://vega.github.io/schema/vega/v5.json",
  "description": "A basic stacked bar chart example.",
  "width": 500,
  "height": 200,
  "padding": 5,

  "data": [
    {
      "name": "table",
      "values": [
        {"x": 0, "y": 28, "c": 0}, {"x": 0, "y": 55, "c": 1},
        {"x": 1, "y": 43, "c": 0}, {"x": 1, "y": 91, "c": 1},
        {"x": 2, "y": 81, "c": 0}, {"x": 2, "y": 53, "c": 1},
        {"x": 3, "y": 19, "c": 0}, {"x": 3, "y": 87, "c": 1},
        {"x": 4, "y": 52, "c": 0}, {"x": 4, "y": 48, "c": 1},
        {"x": 5, "y": 24, "c": 0}, {"x": 5, "y": 49, "c": 1},
        {"x": 6, "y": 87, "c": 0}, {"x": 6, "y": 66, "c": 1},
        {"x": 7, "y": 17, "c": 0}, {"x": 7, "y": 27, "c": 1},
        {"x": 8, "y": 68, "c": 0}, {"x": 8, "y": 16, "c": 1},
        {"x": 9, "y": 49, "c": 0}, {"x": 9, "y": 15, "c": 1}
      ],
      "transform": [
        {
          "type": "stack",
          "groupby": ["x"],
          "sort": {"field": "c"},
          "field": "y"
        }
      ]
    }
  ],

  "scales": [
    {
      "name": "x",
      "type": "band",
      "range": "width",
      "domain": {"data": "table", "field": "x"}
    },
    {
      "name": "y",
      "type": "linear",
      "range": "height",
      "nice": true, "zero": true,
      "domain": {"data": "table", "field": "y1"}
    },
    {
      "name": "color",
      "type": "ordinal",
      "range": "category",
      "domain": {"data": "table", "field": "c"}
    }
  ],

  "axes": [
    {"orient": "bottom", "scale": "x", "zindex": 1},
    {"orient": "left", "scale": "y", "zindex": 1}
  ],

  "marks": [
    {
      "type": "rect",
      "from": {"data": "table"},
      "encode": {
        "enter": {
          "x": {"scale": "x", "field": "x"},
          "width": {"scale": "x", "band": 1, "offset": -1},
          "y": {"scale": "y", "field": "y0"},
          "y2": {"scale": "y", "field": "y1"},
          "fill": {"scale": "color", "field": "c"}
        },
        "update": {
          "fillOpacity": {"value": 1}
        },
        "hover": {
          "fillOpacity": {"value": 0.5}
        }
      }
    }
  ]
}
        "#,
        )
        .unwrap();

        let url = vega_to_url(&vl_spec, true).unwrap();
        println!("{url}");
        let expected = concat!(
            "https://vega.github.io/editor/#/url/vega/",
            "N4IgJAzgxgFgpgWwIYgFwhgF0wBwqgegIDc4BzJAOjIEtMYBXAI0poHsDp5kTykSArJQBWENgDsQAGhAATONA",
            "BONHJnaT0AQQAETJBBpRtETEigBrOLN1JF22Lcza4ADyQIcAGziVpIAO40svRoAgAMYTLwNGRYaABMETI4SLK",
            "yNOJkoTKySKZoANqg4u5waCCmTN5+xEieDAqFoC5okSAAngkAHDJQrQC+Us2tMp2oAgK9aACMg8Oo06NoACwA",
            "zFOoYXMgLQtLqACciyB9C9u78ftdx6dbQzsJ+wLrJzPnaC9j0wcbd-OfaC6AHYNrN7rtlk9Lq9Nu9UJCOisej",
            "CwfNJojUPEEbc4eixssfii4QA2K4gmF-B6oUkY4k006oqnkr7knHgtDMhKst7s1DIsbE5Fs+b8mb0nnzQn4wn",
            "CqlSmbohn9AC6MkwiiQ4ggADM2IoEE0Ku0cGV0CYzOY-GRFGwGDgmGMCg9VSAxIpMGhQNqaHBPLJyn1BiBvb7",
            "-ehOiqVTJoHVGqgiiASghTQ81caU3pxP6ZBrMinAsEYH5ZGxkBlPXI8ih0JVqjIQ37yi1+tskynOmmTeVPBk4L",
            "Y-LmyCnorEPTJxIZTeqGjIAF5wG1oadwHKlpDl1CgXL5GtIKples+xvh6YgFv3NsBtiePV+TDp8p69IlTwDzVD",
            "gN5ch6jtyNcbrcq3KWsD2DI8w1eFsXSQFw4wTPUfXED10CYNhsFLPwY2qdAWjnDJ5F2RkELgJDuzgbUx1dKBY3",
            "KX9Z3w1w3hdZBFHMCBDXvLt0EUOAoEo7UbQNTdKx3Co92qIMSKgNh5ArEjMAXCtdlALCU1wsDQybM8ZALEJhNU",
            "rSZEzMNjjYbVtQgOBkIAWjBDEVOo7DEUPTTwzCbSOniCsDPDPwGwg9pTyDb1PFffTHJTaSb0UPzwIDM8gztbdT",
            "S9GhQoAeRSKA6DGUBanqU1ZiDGA2FIGLhJCzxMrMHKK3yhpWkoAQWyg-ogA/view",
        );
        assert_eq!(url, expected);
    }

    #[test]
    fn test_with_num_workers_rejects_zero() {
        let err = VlConverter::with_num_workers(0).err().unwrap();
        assert!(err.to_string().contains("num_workers must be >= 1"));
    }

    #[test]
    fn test_num_workers_reports_configured_value() {
        let converter = VlConverter::with_num_workers(4).unwrap();
        assert_eq!(converter.num_workers(), 4);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_parallel_conversions_with_shared_converter() {
        let converter = VlConverter::with_num_workers(4).unwrap();
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
                    )
                    .await
            }));
        }

        for task in tasks {
            let svg = task.await.unwrap().unwrap();
            assert!(svg.trim_start().starts_with("<svg"));
        }
    }

    #[tokio::test]
    async fn test_font_version_propagation() {
        use crate::text::{register_font_directory, FONT_CONFIG_VERSION};
        use std::sync::atomic::Ordering;

        let version_before = FONT_CONFIG_VERSION.load(Ordering::Acquire);

        // Do an initial conversion to ensure the worker is running
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

        // Register a font directory (re-registers the built-in fonts, which is harmless)
        let font_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/liberation-sans");
        register_font_directory(font_dir).unwrap();

        let version_after = FONT_CONFIG_VERSION.load(Ordering::Acquire);
        assert_eq!(
            version_after,
            version_before + 1,
            "FONT_CONFIG_VERSION should increment after register_font_directory"
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
}

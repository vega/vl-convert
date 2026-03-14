use crate::deno_stubs::{NoOpInNpmPackageChecker, NoOpNpmPackageFolderResolver, VlConvertNodeSys};
use crate::image_loading::ImageAccessPolicy;
use crate::module_loader::import_map::{msgpack_url, vega_themes_url, vega_url, VlVersion};
use crate::module_loader::{VlConvertModuleLoader, FORMATE_LOCALE_MAP, TIME_FORMATE_LOCALE_MAP};

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::{serde_v8, v8, ModuleSpecifier, OpState};
use deno_error::JsErrorBox;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_permissions::{
    Permissions, PermissionsContainer, PermissionsOptions, RuntimePermissionDescriptorParser,
};
use deno_runtime::deno_web::{BlobStore, InMemoryBroadcastChannel};
use deno_runtime::worker::{MainWorker, WorkerOptions, WorkerServiceOptions};
use deno_runtime::FeatureChecker;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::{Debug, Display, Formatter};
use std::io::Cursor;
use std::path::{Path, PathBuf};
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

use crate::html::{font_cdn_url, font_import_rule, font_link_tag, get_vega_or_vegalite_script};
use image::codecs::jpeg::JpegEncoder;
use image::ImageReader;
use resvg::render;

use crate::extract::{
    extract_fonts_from_vega, extract_text_by_font, is_available, resolve_first_fonts,
    FirstFontStatus, FontForHtml, FontInfo, FontKey, FontSource, FontVariant,
};
use crate::font_embed::{generate_font_face_css, inject_locale_chars, variants_by_family};
use crate::text::{
    build_usvg_options_with_fontdb, get_font_baseline_snapshot, registered_google_families,
    FONT_CONFIG_VERSION, GOOGLE_FONTS_CLIENT, USVG_OPTIONS,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use vl_convert_google_fonts::{
    family_to_id, FontStyle, GoogleFontsDatabaseExt, LoadedFontBatch, RegisteredFontBatch,
    VariantRequest,
};

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
    senders: Vec<tokio::sync::mpsc::Sender<QueuedCommand>>,
    // Per-worker count of requests that have been reserved for this worker but not yet
    // fully processed. This includes in-flight senders blocked on channel capacity and
    // commands currently queued/executing in the worker loop.
    outstanding: Vec<Arc<AtomicUsize>>,
    dispatch_cursor: AtomicUsize,
    _handles: Vec<JoinHandle<()>>,
}

const VEGAEMBED_GLOBAL_SNIPPET: &str =
    "window.vegaEmbed=vegaEmbed; window.vega=vega; window.vegaLite=vegaLite; window.lodashDebounce=lodashDebounce;";
pub const ACCESS_DENIED_MARKER: &str = "VLC_ACCESS_DENIED";

impl WorkerPool {
    fn next_sender(&self) -> Option<(tokio::sync::mpsc::Sender<QueuedCommand>, OutstandingTicket)> {
        if self.senders.is_empty() {
            return None;
        }

        // Choose the worker with the smallest outstanding count. Rotate scan start so ties
        // are not biased to index 0.
        let start = self.dispatch_cursor.fetch_add(1, Ordering::Relaxed) % self.senders.len();
        let mut best_idx = None;
        let mut best_outstanding = usize::MAX;

        for offset in 0..self.senders.len() {
            let idx = (start + offset) % self.senders.len();
            if self.senders[idx].is_closed() {
                continue;
            }

            let outstanding = self.outstanding[idx].load(Ordering::Relaxed);
            if outstanding < best_outstanding {
                best_idx = Some(idx);
                best_outstanding = outstanding;
                if outstanding == 0 {
                    break;
                }
            }
        }

        let idx = best_idx?;
        let ticket = OutstandingTicket::new(self.outstanding[idx].clone());
        Some((self.senders[idx].clone(), ticket))
    }

    fn is_closed(&self) -> bool {
        self.senders
            .iter()
            .all(tokio::sync::mpsc::Sender::is_closed)
    }
}

struct OutstandingTicket {
    counter: Arc<AtomicUsize>,
}

impl OutstandingTicket {
    fn new(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self { counter }
    }
}

impl Drop for OutstandingTicket {
    fn drop(&mut self) {
        let prev = self.counter.fetch_sub(1, Ordering::Relaxed);
        debug_assert!(prev > 0, "outstanding counter underflow");
    }
}

struct QueuedCommand {
    cmd: VlConvertCommand,
    ticket: OutstandingTicket,
}

impl QueuedCommand {
    fn new(cmd: VlConvertCommand, ticket: OutstandingTicket) -> Self {
        Self { cmd, ticket }
    }

    fn into_command(self) -> VlConvertCommand {
        self.cmd
    }

    fn into_parts(self) -> (VlConvertCommand, OutstandingTicket) {
        (self.cmd, self.ticket)
    }
}

#[derive(Default)]
struct WorkerTransferState {
    json_args: HashMap<i32, String>,
    msgpack_results: HashMap<i32, Vec<u8>>,
    next_id: i32,
}

type WorkerTransferStateHandle = Rc<RefCell<WorkerTransferState>>;

fn ensure_v8_platform_initialized() {
    static V8_INIT: Once = Once::new();
    V8_INIT.call_once(|| deno_core::JsRuntime::init_platform(None, false));
}

fn worker_queue_capacity(num_workers: usize) -> usize {
    num_workers.saturating_mul(32).max(32)
}

fn spawn_worker_pool(config: Arc<VlConverterConfig>) -> Result<WorkerPool, AnyError> {
    let num_workers = config.num_workers;
    if num_workers < 1 {
        bail!("num_workers must be >= 1");
    }
    ensure_v8_platform_initialized();
    let initial_font_baseline = get_font_baseline_snapshot()?;

    let total_queue_capacity = worker_queue_capacity(num_workers);
    let per_worker_queue_capacity = (total_queue_capacity / num_workers).max(1);
    let mut handles = Vec::with_capacity(num_workers);
    let mut senders = Vec::with_capacity(num_workers);
    let mut startup_receivers = Vec::with_capacity(num_workers);

    for _ in 0..num_workers {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<QueuedCommand>(per_worker_queue_capacity);
        senders.push(tx);
        let (startup_tx, startup_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let worker_config = config.clone();
        let worker_font_baseline = initial_font_baseline.clone();
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
                let mut inner =
                    match InnerVlConverter::try_new(worker_config, worker_font_baseline).await {
                        Ok(inner) => {
                            let _ = startup_tx.send(Ok(()));
                            inner
                        }
                        Err(err) => {
                            let _ = startup_tx.send(Err(err.to_string()));
                            return;
                        }
                    };

                while let Some(queued_cmd) = rx.recv().await {
                    // Keep the ticket alive for the full loop iteration so outstanding
                    // covers refresh + command execution (drop happens at iteration end).
                    let (cmd, _ticket) = queued_cmd.into_parts();
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

    let num = senders.len();
    Ok(WorkerPool {
        senders,
        outstanding: (0..num).map(|_| Arc::new(AtomicUsize::new(0))).collect(),
        dispatch_cursor: AtomicUsize::new(0),
        _handles: handles,
    })
}

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

fn normalize_converter_config(
    mut config: VlConverterConfig,
) -> Result<VlConverterConfig, AnyError> {
    if config.num_workers < 1 {
        bail!("num_workers must be >= 1");
    }

    if !config.allow_http_access && config.allowed_base_urls.is_some() {
        bail!("allowed_base_urls cannot be set when HTTP access is disabled");
    }

    config.allowed_base_urls = normalize_allowed_base_urls(config.allowed_base_urls.take())?;

    if let Some(root) = config.filesystem_root.take() {
        let canonical_root = portable_canonicalize(&root).map_err(|err| {
            anyhow!(
                "Failed to resolve filesystem_root {}: {}",
                root.display(),
                err
            )
        })?;
        if !canonical_root.is_dir() {
            bail!(
                "filesystem_root must be a directory: {}",
                canonical_root.display()
            );
        }
        config.filesystem_root = Some(canonical_root);
    }

    Ok(config)
}

fn normalize_allowed_base_urls(
    allowed_base_urls: Option<Vec<String>>,
) -> Result<Option<Vec<String>>, AnyError> {
    allowed_base_urls
        .map(|urls| {
            if urls.is_empty() {
                bail!("allowed_base_urls cannot be empty");
            }
            urls.into_iter()
                .map(|url| normalize_allowed_base_url(&url))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
}

fn normalize_allowed_base_url(allowed_base_url: &str) -> Result<String, AnyError> {
    let parsed_url = Url::parse(allowed_base_url)
        .map_err(|err| anyhow!("Invalid allowed_base_url '{}': {}", allowed_base_url, err))?;

    let scheme = parsed_url.scheme();
    if scheme != "http" && scheme != "https" {
        bail!(
            "allowed_base_url must use http or https scheme: {}",
            allowed_base_url
        );
    }

    if !parsed_url.username().is_empty() || parsed_url.password().is_some() {
        bail!(
            "allowed_base_url cannot include userinfo credentials: {}",
            allowed_base_url
        );
    }

    if parsed_url.query().is_some() {
        bail!(
            "allowed_base_url cannot include a query component: {}",
            allowed_base_url
        );
    }

    if parsed_url.fragment().is_some() {
        bail!(
            "allowed_base_url cannot include a fragment component: {}",
            allowed_base_url
        );
    }

    let mut normalized = parsed_url.to_string();
    if !normalized.ends_with('/') {
        normalized.push('/');
    }

    Ok(normalized)
}

fn build_permissions(config: &VlConverterConfig) -> Result<Permissions, AnyError> {
    let allow_read = config
        .filesystem_root
        .as_ref()
        .map(|root| vec![root.to_string_lossy().to_string()]);

    let allow_net = if config.allow_http_access {
        // Empty allowlist means unrestricted --allow-net.
        Some(vec![])
    } else {
        None
    };

    Permissions::from_options(
        &RuntimePermissionDescriptorParser::new(VlConvertNodeSys),
        &PermissionsOptions {
            allow_read,
            allow_net,
            prompt: false,
            ..Default::default()
        },
    )
    .map_err(|err| anyhow!("Failed to build Deno permissions: {err}"))
}

fn filesystem_root_file_url(filesystem_root: Option<&Path>) -> Result<Option<String>, AnyError> {
    let Some(root) = filesystem_root else {
        return Ok(None);
    };
    let url = Url::from_directory_path(root).map_err(|_| {
        anyhow!(
            "Failed to construct file URL from filesystem_root: {}",
            root.display()
        )
    })?;
    Ok(Some(url.to_string()))
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

/// How to handle fonts referenced in a spec but not available on the system.
///
/// Only the **first** non-generic font in each CSS `font-family` string is
/// checked (e.g. for `"Roboto, Arial, sans-serif"` only `Roboto` is examined).
/// This matches Vega's rendering behavior, which tries the first font and falls
/// back to system generics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MissingFontsPolicy {
    /// Silently fall back to the default font (no validation).
    #[default]
    Fallback,
    /// Log a warning for each missing first-choice font but continue rendering.
    Warn,
    /// Return an error if any first-choice font is missing.
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlConverterConfig {
    pub num_workers: usize,
    pub allow_http_access: bool,
    pub filesystem_root: Option<PathBuf>,
    /// Converter-level default HTTP allowlist. Per-request `allowed_base_urls`
    /// values override this default when provided. Must be non-empty when set.
    /// When configured, HTTP redirects are denied instead of followed.
    pub allowed_base_urls: Option<Vec<String>>,
    /// Whether to auto-download missing fonts from Google Fonts.
    pub auto_google_fonts: bool,
    /// How to handle missing first-choice fonts: silently fallback, warn, or error.
    pub missing_fonts: MissingFontsPolicy,
    /// Whether to embed locally-available fonts (system, --font-dir, vendored)
    /// as @font-face CSS in HTML output.
    ///
    pub html_embed_local_fonts: bool,
}

impl Default for VlConverterConfig {
    fn default() -> Self {
        Self {
            num_workers: 1,
            allow_http_access: true,
            filesystem_root: None,
            allowed_base_urls: None,
            auto_google_fonts: false,
            missing_fonts: MissingFontsPolicy::Fallback,
            html_embed_local_fonts: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleFontRequest {
    pub family: String,
    pub variants: Option<Vec<VariantRequest>>,
}

#[derive(Debug, Clone, Default)]
pub struct VgOpts {
    pub allowed_base_urls: Option<Vec<String>>,
    pub format_locale: Option<FormatLocale>,
    pub time_format_locale: Option<TimeFormatLocale>,
    pub google_fonts: Option<Vec<GoogleFontRequest>>,
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
    pub google_fonts: Option<Vec<GoogleFontRequest>>,
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

fn next_id(transfer_state: &WorkerTransferStateHandle) -> Result<i32, AnyError> {
    let mut guard = transfer_state
        .try_borrow_mut()
        .map_err(|err| anyhow!("Failed to borrow worker transfer state: {err}"))?;
    let id = guard.next_id;
    guard.next_id = (guard.next_id + 1) % i32::MAX;
    Ok(id)
}

fn set_json_arg(
    transfer_state: &WorkerTransferStateHandle,
    arg: serde_json::Value,
) -> Result<i32, AnyError> {
    set_json_str_arg(transfer_state, serde_json::to_string(&arg)?)
}

fn set_json_str_arg(
    transfer_state: &WorkerTransferStateHandle,
    json_str: String,
) -> Result<i32, AnyError> {
    let id = next_id(transfer_state)?;
    let mut guard = transfer_state
        .try_borrow_mut()
        .map_err(|err| anyhow!("Failed to borrow worker transfer state: {err}"))?;
    guard.json_args.insert(id, json_str);
    Ok(id)
}

fn set_spec_arg(
    transfer_state: &WorkerTransferStateHandle,
    spec: ValueOrString,
) -> Result<i32, AnyError> {
    match spec {
        ValueOrString::JsonString(s) => set_json_str_arg(transfer_state, s),
        ValueOrString::Value(v) => set_json_arg(transfer_state, v),
    }
}

fn alloc_msgpack_result_id(transfer_state: &WorkerTransferStateHandle) -> Result<i32, AnyError> {
    next_id(transfer_state)
}

fn take_msgpack_result(
    transfer_state: &WorkerTransferStateHandle,
    result_id: i32,
) -> Result<Vec<u8>, AnyError> {
    let mut guard = transfer_state
        .try_borrow_mut()
        .map_err(|err| anyhow!("Failed to borrow worker transfer state: {err}"))?;
    guard
        .msgpack_results
        .remove(&result_id)
        .ok_or_else(|| anyhow!("Result id not found"))
}

fn clear_json_arg(transfer_state: &WorkerTransferStateHandle, arg_id: i32) {
    if let Ok(mut guard) = transfer_state.try_borrow_mut() {
        guard.json_args.remove(&arg_id);
    }
}

fn clear_msgpack_result(transfer_state: &WorkerTransferStateHandle, result_id: i32) {
    if let Ok(mut guard) = transfer_state.try_borrow_mut() {
        guard.msgpack_results.remove(&result_id);
    }
}

struct JsonArgGuard {
    transfer_state: WorkerTransferStateHandle,
    arg_id: Option<i32>,
}

impl JsonArgGuard {
    fn from_value(
        transfer_state: &WorkerTransferStateHandle,
        value: serde_json::Value,
    ) -> Result<Self, AnyError> {
        Ok(Self {
            transfer_state: transfer_state.clone(),
            arg_id: Some(set_json_arg(transfer_state, value)?),
        })
    }

    fn from_spec(
        transfer_state: &WorkerTransferStateHandle,
        spec: ValueOrString,
    ) -> Result<Self, AnyError> {
        Ok(Self {
            transfer_state: transfer_state.clone(),
            arg_id: Some(set_spec_arg(transfer_state, spec)?),
        })
    }

    fn id(&self) -> i32 {
        self.arg_id.expect("JsonArgGuard id missing")
    }
}

impl Drop for JsonArgGuard {
    fn drop(&mut self) {
        if let Some(arg_id) = self.arg_id.take() {
            clear_json_arg(&self.transfer_state, arg_id);
        }
    }
}

struct MsgpackResultGuard {
    transfer_state: WorkerTransferStateHandle,
    result_id: Option<i32>,
}

impl MsgpackResultGuard {
    fn new(transfer_state: &WorkerTransferStateHandle) -> Result<Self, AnyError> {
        Ok(Self {
            transfer_state: transfer_state.clone(),
            result_id: Some(alloc_msgpack_result_id(transfer_state)?),
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
        take_msgpack_result(&self.transfer_state, result_id)
    }
}

impl Drop for MsgpackResultGuard {
    fn drop(&mut self) {
        if let Some(result_id) = self.result_id.take() {
            clear_msgpack_result(&self.transfer_state, result_id);
        }
    }
}

#[op2]
#[string]
fn op_get_json_arg(state: &mut OpState, arg_id: i32) -> Result<String, JsErrorBox> {
    let transfer_state = state
        .try_borrow::<WorkerTransferStateHandle>()
        .cloned()
        .ok_or_else(|| JsErrorBox::generic("Worker transfer state not found"))?;
    let mut guard = transfer_state.try_borrow_mut().map_err(|err| {
        JsErrorBox::generic(format!("Failed to borrow worker transfer state: {err}"))
    })?;
    if let Some(arg) = guard.json_args.remove(&arg_id) {
        Ok(arg)
    } else {
        Err(JsErrorBox::generic("Arg id not found"))
    }
}

#[op2(fast)]
fn op_set_msgpack_result(
    state: &mut OpState,
    result_id: i32,
    #[buffer] data: &[u8],
) -> Result<(), JsErrorBox> {
    let transfer_state = state
        .try_borrow::<WorkerTransferStateHandle>()
        .cloned()
        .ok_or_else(|| JsErrorBox::generic("Worker transfer state not found"))?;
    let mut guard = transfer_state.try_borrow_mut().map_err(|err| {
        JsErrorBox::generic(format!("Failed to borrow worker transfer state: {err}"))
    })?;
    guard.msgpack_results.insert(result_id, data.to_vec());
    Ok(())
}

struct WorkerFontState {
    db: fontdb::Database,
    baseline_version: u64,
    shared_config_epoch: u64,
    hinting_enabled: bool,
    overlay_registrations: Vec<RegisteredFontBatch>,
}

impl WorkerFontState {
    fn from_baseline(snapshot: &crate::text::FontBaselineSnapshot) -> Self {
        Self {
            db: snapshot.clone_fontdb(),
            baseline_version: snapshot.version(),
            shared_config_epoch: snapshot.version(),
            hinting_enabled: snapshot.hinting_enabled(),
            overlay_registrations: Vec::new(),
        }
    }
}

/// Struct that interacts directly with the Deno JavaScript runtime. Not Sendable
struct InnerVlConverter {
    worker: MainWorker,
    transfer_state: WorkerTransferStateHandle,
    initialized_vl_versions: HashSet<VlVersion>,
    vega_initialized: bool,
    font_state: WorkerFontState,
    usvg_options: usvg::Options<'static>,
    config: Arc<VlConverterConfig>,
}

impl InnerVlConverter {
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

    fn refresh_font_config_if_needed(&mut self) -> Result<(), AnyError> {
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

    fn clear_google_fonts_overlay(&mut self) {
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
            let filesystem_base_url = serde_json::to_string(&filesystem_root_file_url(
                self.config.filesystem_root.as_deref(),
            )?)?;
            let mut function_str = r#"
const DEFAULT_HTTP_BASE_URL = 'https://vega.github.io/vega-datasets/';
const CONVERTER_ALLOW_HTTP_ACCESS = __ALLOW_HTTP_ACCESS__;
const CONVERTER_FILESYSTEM_BASE_URL = __FILESYSTEM_BASE_URL__;
const ACCESS_DENIED_MARKER = __ACCESS_DENIED_MARKER__;

function accessDeniedMessage(detail) {
    return `${ACCESS_DENIED_MARKER}: ${detail}`;
}

function fileUrlToPath(urlStr) {
    const url = new URL(urlStr);
    if (url.protocol !== 'file:') {
        throw new Error('Unsupported file URL protocol: ' + url.protocol);
    }
    let path = decodeURIComponent(url.pathname);
    if (globalThis.Deno?.build?.os === 'windows' && path.startsWith('/')) {
        path = path.slice(1);
    }
    return path;
}

function resolveUriWithBase(uri, baseURL) {
    try {
        return new URL(uri, baseURL).href;
    } catch (_err) {
        return uri;
    }
}

function isAllowedHttpUrl(uri, allowedBaseUrls) {
    let normalizedUrl;
    try {
        normalizedUrl = new URL(uri).href;
    } catch (_err) {
        return false;
    }

    for (const allowedUrl of allowedBaseUrls) {
        if (normalizedUrl.startsWith(allowedUrl)) {
            return true;
        }
    }
    return false;
}

function isRedirectStatus(status) {
    return status === 301 || status === 302 || status === 303 || status === 307 || status === 308;
}

function isReadPermissionError(error) {
    const name = String(error?.name ?? '').toLowerCase();
    const message = String(error?.message ?? '').toLowerCase();
    return (
        name === 'permissiondenied' ||
        message.includes('requires read access') ||
        message.includes('permission denied')
    );
}

async function fetchWithPolicy(uri, options, allowedBaseUrls, errors) {
    const responseType = options?.response;
    const fetchOptions = Object.assign({}, options ?? {});

    let parsedUrl = null;
    try {
        parsedUrl = new URL(uri);
    } catch (_err) {
        parsedUrl = null;
    }
    const isHttpUrl =
        parsedUrl != null && (parsedUrl.protocol === 'http:' || parsedUrl.protocol === 'https:');

    if (isHttpUrl) {
        if (!CONVERTER_ALLOW_HTTP_ACCESS) {
            const message = accessDeniedMessage('HTTP access denied by converter policy: ' + uri);
            errors.push(message);
            throw new Error(message);
        }
        if (allowedBaseUrls != null && !isAllowedHttpUrl(uri, allowedBaseUrls)) {
            const message = accessDeniedMessage('External data url not allowed: ' + uri);
            errors.push(message);
            throw new Error(message);
        }
    }

    if (allowedBaseUrls != null) {
        // Keep allowlist handling simple and deterministic: deny all redirect responses.
        fetchOptions.redirect = 'manual';
    }

    const response = await fetch(uri, fetchOptions);
    if (allowedBaseUrls != null && (response.type === 'opaqueredirect' || isRedirectStatus(response.status))) {
        const message = accessDeniedMessage(
            'Redirected HTTP URLs are not allowed when allowed_base_urls is configured: ' + uri
        );
        errors.push(message);
        throw new Error(message);
    }

    if (response.ok) {
        if (responseType != null && typeof response[responseType] === 'function') {
            return await response[responseType]();
        }
        return await response.text();
    }

    throw new Error(`${response.status} ${response.statusText}`);
}

function setCanvasImagePolicy(allowedBaseUrls, errors) {
    globalThis.__vlConvertAllowHttpAccess = CONVERTER_ALLOW_HTTP_ACCESS;
    globalThis.__vlConvertAllowedBaseUrls = allowedBaseUrls;
    globalThis.__vlConvertAccessDeniedMarker = ACCESS_DENIED_MARKER;
    globalThis.__vlConvertAccessErrors = errors;
}

function clearCanvasImagePolicy() {
    delete globalThis.__vlConvertAllowHttpAccess;
    delete globalThis.__vlConvertAllowedBaseUrls;
    delete globalThis.__vlConvertAccessDeniedMarker;
    delete globalThis.__vlConvertAccessErrors;
}

function buildLoader(allowedBaseUrls, errors) {
    const allowFilesystemAccess = CONVERTER_FILESYSTEM_BASE_URL != null;
    let baseURL = DEFAULT_HTTP_BASE_URL;
    if (allowFilesystemAccess) {
        baseURL = CONVERTER_FILESYSTEM_BASE_URL;
    }

    const loaderOptions = { baseURL };
    if (allowFilesystemAccess && !CONVERTER_ALLOW_HTTP_ACCESS) {
        loaderOptions.mode = 'file';
    } else if (!allowFilesystemAccess && CONVERTER_ALLOW_HTTP_ACCESS) {
        loaderOptions.mode = 'http';
    }

    const loader = vega.loader(loaderOptions);
    loader.http = async (uri, options) => {
        return fetchWithPolicy(uri, options, allowedBaseUrls, errors);
    };

    loader.fileAccess = allowFilesystemAccess;
    loader.file = async (uri, _options) => {
        if (!allowFilesystemAccess) {
            const message = accessDeniedMessage(
                'Filesystem access denied by converter policy: ' + uri
            );
            errors.push(message);
            throw new Error(message);
        }
        const resolved = resolveUriWithBase(uri, CONVERTER_FILESYSTEM_BASE_URL);
        if (!resolved.startsWith('file://')) {
            const message = 'Invalid filesystem URI: ' + uri;
            errors.push(message);
            throw new Error(message);
        }
        const path = fileUrlToPath(resolved);
        try {
            return await Deno.readTextFile(path);
        } catch (error) {
            if (isReadPermissionError(error)) {
                const message = accessDeniedMessage(
                    'Filesystem access denied by Deno permissions: ' + uri
                );
                errors.push(message);
                throw new Error(message);
            }
            throw error;
        }
    };

    return loader;
}

function vegaToView(vgSpec, allowedBaseUrls, errors) {
    let runtime = vega.parse(vgSpec);
    const loader = buildLoader(allowedBaseUrls, errors);
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

    setCanvasImagePolicy(allowedBaseUrls, errors);

    let view = vegaToViewCanvas(vgSpec, allowedBaseUrls, errors);
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
    return canvasPromise.finally(() => {
        clearCanvasImagePolicy();
    });
}
"#
            .to_string();
            function_str = function_str.replace(
                "__ALLOW_HTTP_ACCESS__",
                if self.config.allow_http_access {
                    "true"
                } else {
                    "false"
                },
            );
            function_str =
                function_str.replace("__FILESYSTEM_BASE_URL__", filesystem_base_url.as_str());
            function_str = function_str.replace(
                "__ACCESS_DENIED_MARKER__",
                serde_json::to_string(ACCESS_DENIED_MARKER)?.as_str(),
            );
            self.worker
                .js_runtime
                .execute_script("ext:<anon>", function_str)?;
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

    pub async fn try_new(
        config: Arc<VlConverterConfig>,
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

        let permissions = build_permissions(&config)?;

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
        let options = WorkerOptions {
            extensions: vec![
                // Canvas 2D extension from vl-convert-canvas2d-deno crate
                vl_convert_canvas2d_deno::vl_convert_canvas2d::init(),
                // Our runtime extension (worker-local JSON/msgpack transfer ops)
                vl_convert_runtime::init(),
            ],
            startup_snapshot: Some(crate::VL_CONVERT_SNAPSHOT),
            ..Default::default()
        };

        // Create the MainWorker with full Web API support
        let worker = MainWorker::bootstrap_from_options(&main_module, services, options);
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

        let this = Self {
            worker,
            transfer_state,
            initialized_vl_versions: Default::default(),
            vega_initialized: false,
            usvg_options: build_usvg_options_with_fontdb(font_state.db.clone()),
            font_state,
            config,
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

        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vl_spec.into())?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config)?;

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

        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vl_spec.into())?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config)?;
        let allowed_base_urls =
            serde_json::to_string(&serde_json::Value::from(vl_opts.allowed_base_urls))?;
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

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }

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

        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vl_spec)?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config)?;
        let allowed_base_urls =
            serde_json::to_string(&serde_json::Value::from(vl_opts.allowed_base_urls))?;
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

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }

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

        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vg_spec.into())?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;

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

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }

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

        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vg_spec)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;
        let result = MsgpackResultGuard::new(&self.transfer_state)?;

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

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }

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

        let spec_arg = JsonArgGuard::from_value(&self.transfer_state, vg_spec.clone())?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;

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

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }

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

        let spec_arg = JsonArgGuard::from_value(&self.transfer_state, vl_spec.clone())?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;

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

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }

        let png_data = self.execute_script_to_bytes("canvasPngData").await?;
        Ok(png_data)
    }

    fn parse_svg_with_worker_options(
        &mut self,
        svg: &str,
        policy: &ImageAccessPolicy,
    ) -> Result<usvg::Tree, AnyError> {
        parse_svg_with_options(svg, policy, &mut self.usvg_options)
    }

    fn svg_to_png_with_worker_options(
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

    fn svg_to_jpeg_with_worker_options(
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

    fn svg_to_pdf_with_worker_options(
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
    ) -> Result<Vec<u8>, AnyError> {
        let svg = self.vega_to_svg(vg_spec, vg_opts).await?;
        self.svg_to_jpeg_with_worker_options(&svg, scale, quality, &policy)
    }

    pub async fn vegalite_to_jpeg(
        &mut self,
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        scale: f32,
        quality: Option<u8>,
        policy: ImageAccessPolicy,
    ) -> Result<Vec<u8>, AnyError> {
        let svg = self.vegalite_to_svg(vl_spec, vl_opts).await?;
        self.svg_to_jpeg_with_worker_options(&svg, scale, quality, &policy)
    }

    pub async fn vega_to_pdf(
        &mut self,
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        policy: ImageAccessPolicy,
    ) -> Result<Vec<u8>, AnyError> {
        let svg = self.vega_to_svg(vg_spec, vg_opts).await?;
        self.svg_to_pdf_with_worker_options(&svg, &policy)
    }

    pub async fn vegalite_to_pdf(
        &mut self,
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        policy: ImageAccessPolicy,
    ) -> Result<Vec<u8>, AnyError> {
        let svg = self.vegalite_to_svg(vl_spec, vl_opts).await?;
        self.svg_to_pdf_with_worker_options(&svg, &policy)
    }

    async fn handle_command(&mut self, cmd: VlConvertCommand) {
        // Apply a google fonts overlay, execute `$work`, then clear the overlay.
        macro_rules! with_font_overlay {
            ($self:expr, $batches:expr, $work:expr) => {{
                if !$batches.is_empty() {
                    $self.apply_google_fonts_overlay($batches);
                }
                let result = $work;
                $self.clear_google_fonts_overlay();
                result
            }};
        }

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
                google_font_batches,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.vega_to_svg(vg_spec, vg_opts).await
                );
                responder.send(result).ok();
            }
            VlConvertCommand::VgToSg {
                vg_spec,
                vg_opts,
                google_font_batches,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.vega_to_scenegraph(vg_spec, vg_opts).await
                );
                responder.send(result).ok();
            }
            VlConvertCommand::VgToSgMsgpack {
                vg_spec,
                vg_opts,
                google_font_batches,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.vega_to_scenegraph_msgpack(vg_spec, vg_opts).await
                );
                responder.send(result).ok();
            }
            VlConvertCommand::VlToSvg {
                vl_spec,
                vl_opts,
                google_font_batches,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.vegalite_to_svg(vl_spec, vl_opts).await
                );
                responder.send(result).ok();
            }
            VlConvertCommand::VlToSg {
                vl_spec,
                vl_opts,
                google_font_batches,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.vegalite_to_scenegraph(vl_spec, vl_opts).await
                );
                responder.send(result).ok();
            }
            VlConvertCommand::VlToSgMsgpack {
                vl_spec,
                vl_opts,
                google_font_batches,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.vegalite_to_scenegraph_msgpack(vl_spec, vl_opts).await
                );
                responder.send(result).ok();
            }
            VlConvertCommand::VgToPng {
                vg_spec,
                vg_opts,
                google_font_batches,
                scale,
                ppi,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    match vg_spec.to_value() {
                        Ok(v) => self.vega_to_png(&v, vg_opts, scale, ppi).await,
                        Err(e) => Err(e),
                    }
                );
                responder.send(result).ok();
            }
            VlConvertCommand::VgToJpeg {
                vg_spec,
                vg_opts,
                google_font_batches,
                scale,
                quality,
                image_policy,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.vega_to_jpeg(vg_spec, vg_opts, scale, quality, image_policy)
                        .await
                );
                responder.send(result).ok();
            }
            VlConvertCommand::VgToPdf {
                vg_spec,
                vg_opts,
                google_font_batches,
                image_policy,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.vega_to_pdf(vg_spec, vg_opts, image_policy).await
                );
                responder.send(result).ok();
            }
            VlConvertCommand::VlToPng {
                vl_spec,
                vl_opts,
                google_font_batches,
                scale,
                ppi,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    match vl_spec.to_value() {
                        Ok(v) => self.vegalite_to_png(&v, vl_opts, scale, ppi).await,
                        Err(e) => Err(e),
                    }
                );
                responder.send(result).ok();
            }
            VlConvertCommand::VlToJpeg {
                vl_spec,
                vl_opts,
                google_font_batches,
                scale,
                quality,
                image_policy,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.vegalite_to_jpeg(vl_spec, vl_opts, scale, quality, image_policy)
                        .await
                );
                responder.send(result).ok();
            }
            VlConvertCommand::VlToPdf {
                vl_spec,
                vl_opts,
                google_font_batches,
                image_policy,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.vegalite_to_pdf(vl_spec, vl_opts, image_policy).await
                );
                responder.send(result).ok();
            }
            VlConvertCommand::SvgToPng {
                svg,
                scale,
                ppi,
                image_policy,
                google_font_batches,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.svg_to_png_with_worker_options(&svg, scale, ppi, &image_policy)
                );
                responder.send(result).ok();
            }
            VlConvertCommand::SvgToJpeg {
                svg,
                scale,
                quality,
                image_policy,
                google_font_batches,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.svg_to_jpeg_with_worker_options(&svg, scale, quality, &image_policy)
                );
                responder.send(result).ok();
            }
            VlConvertCommand::SvgToPdf {
                svg,
                image_policy,
                google_font_batches,
                responder,
            } => {
                let result = with_font_overlay!(
                    self,
                    google_font_batches,
                    self.svg_to_pdf_with_worker_options(&svg, &image_policy)
                );
                responder.send(result).ok();
            }
            VlConvertCommand::GetLocalTz { responder } => {
                let local_tz = self.get_local_tz().await;
                responder.send(local_tz).ok();
            }
            VlConvertCommand::GetThemes { responder } => {
                let themes = self.get_themes().await;
                responder.send(themes).ok();
            }
            VlConvertCommand::ComputeVegaembedBundle {
                vl_version,
                responder,
            } => {
                let bundle =
                    crate::html::bundle_vega_snippet(VEGAEMBED_GLOBAL_SNIPPET, vl_version).await;
                responder.send(bundle).ok();
            }
            VlConvertCommand::BundleVegaSnippet {
                snippet,
                vl_version,
                responder,
            } => {
                let bundle = crate::html::bundle_vega_snippet(&snippet, vl_version).await;
                responder.send(bundle).ok();
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
        google_font_batches: Vec<LoadedFontBatch>,
        responder: oneshot::Sender<Result<String, AnyError>>,
    },
    VgToSg {
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        google_font_batches: Vec<LoadedFontBatch>,
        responder: oneshot::Sender<Result<serde_json::Value, AnyError>>,
    },
    VgToSgMsgpack {
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        google_font_batches: Vec<LoadedFontBatch>,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    VlToSvg {
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        google_font_batches: Vec<LoadedFontBatch>,
        responder: oneshot::Sender<Result<String, AnyError>>,
    },
    VlToSg {
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        google_font_batches: Vec<LoadedFontBatch>,
        responder: oneshot::Sender<Result<serde_json::Value, AnyError>>,
    },
    VgToPng {
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        google_font_batches: Vec<LoadedFontBatch>,
        scale: f32,
        ppi: f32,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    VgToJpeg {
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        google_font_batches: Vec<LoadedFontBatch>,
        scale: f32,
        quality: Option<u8>,
        image_policy: ImageAccessPolicy,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    VgToPdf {
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        google_font_batches: Vec<LoadedFontBatch>,
        image_policy: ImageAccessPolicy,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    VlToPng {
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        google_font_batches: Vec<LoadedFontBatch>,
        scale: f32,
        ppi: f32,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    VlToJpeg {
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        google_font_batches: Vec<LoadedFontBatch>,
        scale: f32,
        quality: Option<u8>,
        image_policy: ImageAccessPolicy,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    VlToPdf {
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        google_font_batches: Vec<LoadedFontBatch>,
        image_policy: ImageAccessPolicy,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    VlToSgMsgpack {
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        google_font_batches: Vec<LoadedFontBatch>,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    SvgToPng {
        svg: String,
        scale: f32,
        ppi: Option<f32>,
        image_policy: ImageAccessPolicy,
        google_font_batches: Vec<LoadedFontBatch>,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    SvgToJpeg {
        svg: String,
        scale: f32,
        quality: Option<u8>,
        image_policy: ImageAccessPolicy,
        google_font_batches: Vec<LoadedFontBatch>,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    SvgToPdf {
        svg: String,
        image_policy: ImageAccessPolicy,
        google_font_batches: Vec<LoadedFontBatch>,
        responder: oneshot::Sender<Result<Vec<u8>, AnyError>>,
    },
    GetLocalTz {
        responder: oneshot::Sender<Result<Option<String>, AnyError>>,
    },
    GetThemes {
        responder: oneshot::Sender<Result<serde_json::Value, AnyError>>,
    },
    ComputeVegaembedBundle {
        vl_version: VlVersion,
        responder: oneshot::Sender<Result<String, AnyError>>,
    },
    BundleVegaSnippet {
        snippet: String,
        vl_version: VlVersion,
        responder: oneshot::Sender<Result<String, AnyError>>,
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
            Self::VgToJpeg { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::VgToPdf { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::VlToPng { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::VlToJpeg { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::VlToPdf { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::GetLocalTz { responder } => {
                responder.send(Err(err)).ok();
            }
            Self::SvgToPng { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::SvgToJpeg { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::SvgToPdf { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::GetThemes { responder } => {
                responder.send(Err(err)).ok();
            }
            Self::ComputeVegaembedBundle { responder, .. } => {
                responder.send(Err(err)).ok();
            }
            Self::BundleVegaSnippet { responder, .. } => {
                responder.send(Err(err)).ok();
            }
        }
    }
}

/// Classify a set of CSS `font-family` strings and return Google Fonts download
/// requests for any first-choice families that should be overlaid for a render.
///
/// When `prefer_cdn` is true (HTML path), Google-catalog fonts are requested
/// even if locally available so the render uses the same face the HTML output
/// will reference. When false (SVG/PNG/PDF path), only fonts not already in
/// `fontdb` are requested.
async fn classify_and_request_fonts(
    font_strings: HashSet<String>,
    auto_google_fonts: bool,
    missing_fonts: MissingFontsPolicy,
    prefer_cdn: bool,
) -> Result<Vec<GoogleFontRequest>, AnyError> {
    if font_strings.is_empty() {
        return Ok(Vec::new());
    }

    let available = available_font_families()?;

    let font_string_vec: Vec<String> = font_strings.into_iter().collect();

    let google_fonts_set: HashSet<String> = if auto_google_fonts {
        let candidates = auto_google_probe_candidates(&font_string_vec, &available, prefer_cdn);
        google_font_catalog_matches(candidates.iter(), missing_fonts).await?
    } else {
        HashSet::new()
    };

    // Classify each font string by its first entry
    let statuses = resolve_first_fonts(&font_string_vec, &available, |family| {
        auto_google_fonts && google_fonts_set.contains(family)
    });

    // Collect unavailable fonts — report before any downloads
    let unavailable: Vec<(String, String)> = statuses
        .iter()
        .filter_map(|(css_string, status)| match status {
            FirstFontStatus::Unavailable { name } => Some((
                name.clone(),
                if name == css_string {
                    format!("'{name}'")
                } else {
                    format!("'{name}' (from \"{css_string}\")")
                },
            )),
            _ => None,
        })
        .collect();

    let unavailable_names: Vec<String> = unavailable.iter().map(|(name, _)| name.clone()).collect();
    let unavailable_details: Vec<String> = unavailable
        .iter()
        .map(|(_, detail)| detail.clone())
        .collect();
    report_unavailable_fonts(
        &unavailable_names,
        &unavailable_details,
        auto_google_fonts,
        missing_fonts,
    )?;

    if !auto_google_fonts {
        return Ok(Vec::new());
    }

    // Collect downloadable fonts as requests for the caller to add to VgOpts
    let mut requests: Vec<GoogleFontRequest> = Vec::new();
    for (_css_string, status) in &statuses {
        if let FirstFontStatus::NeedsDownload { name } = status {
            requests.push(GoogleFontRequest {
                family: name.clone(),
                variants: None,
            });
        }
    }

    Ok(requests)
}

/// Preprocess fonts from a compiled Vega specification.
///
/// Extracts font-family strings from the spec, then classifies and requests
/// fonts via [`classify_and_request_fonts`].
async fn preprocess_fonts(
    vega_spec: &serde_json::Value,
    auto_google_fonts: bool,
    missing_fonts: MissingFontsPolicy,
) -> Result<Vec<GoogleFontRequest>, AnyError> {
    if !auto_google_fonts && missing_fonts == MissingFontsPolicy::Fallback {
        return Ok(Vec::new());
    }

    let font_strings = extract_fonts_from_vega(vega_spec);
    classify_and_request_fonts(font_strings, auto_google_fonts, missing_fonts, false).await
}

fn available_font_families() -> Result<HashSet<String>, AnyError> {
    Ok(USVG_OPTIONS
        .lock()
        .map_err(|e| anyhow!("font_preprocessing: failed to lock USVG_OPTIONS: {e}"))?
        .fontdb
        .faces()
        .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
        .collect())
}

fn auto_google_probe_candidates(
    font_strings: &[String],
    available: &HashSet<String>,
    prefer_cdn: bool,
) -> BTreeSet<String> {
    font_strings
        .iter()
        .filter_map(|font_string| {
            let entries = crate::extract::parse_css_font_family(font_string);
            match entries.first() {
                Some(crate::extract::FontFamilyEntry::Named(name))
                    if (prefer_cdn || !is_available(name, available))
                        && family_to_id(name).is_some() =>
                {
                    Some(name.clone())
                }
                _ => None,
            }
        })
        .collect()
}

fn scenegraph_google_probe_candidates(
    families: &BTreeSet<String>,
    explicit_google_families: &HashSet<String>,
    pre_registered: &HashSet<String>,
) -> BTreeSet<String> {
    families
        .iter()
        .filter(|family| {
            !explicit_google_families.contains(*family) && !pre_registered.contains(*family)
        })
        .cloned()
        .collect()
}

async fn google_font_catalog_matches<'a>(
    families: impl IntoIterator<Item = &'a String>,
    missing_fonts: MissingFontsPolicy,
) -> Result<HashSet<String>, AnyError> {
    let mut google_fonts_set: HashSet<String> = HashSet::new();
    let mut api_errors: Vec<(String, String)> = Vec::new();

    for family in families {
        match GOOGLE_FONTS_CLIENT.is_known_font(family).await {
            Ok(true) => {
                google_fonts_set.insert(family.clone());
            }
            Ok(false) => {}
            Err(e) => {
                api_errors.push((family.clone(), e.to_string()));
            }
        }
    }

    report_google_catalog_errors(&api_errors, missing_fonts)?;
    Ok(google_fonts_set)
}

fn report_google_catalog_errors(
    api_errors: &[(String, String)],
    missing_fonts: MissingFontsPolicy,
) -> Result<(), AnyError> {
    if api_errors.is_empty() {
        return Ok(());
    }

    if missing_fonts == MissingFontsPolicy::Error {
        let details: Vec<String> = api_errors
            .iter()
            .map(|(name, err)| format!("'{name}': {err}"))
            .collect();
        return Err(anyhow!(
            "auto_google_fonts: could not reach the Google Fonts API to check \
             the following fonts: {}",
            details.join(", ")
        ));
    }

    if missing_fonts == MissingFontsPolicy::Warn {
        for (name, err) in api_errors {
            log::warn!("auto_google_fonts: could not reach Google Fonts API for '{name}': {err}");
        }
    }

    Ok(())
}

fn report_unavailable_fonts(
    unavailable_names: &[String],
    unavailable_details: &[String],
    auto_google_fonts: bool,
    missing_fonts: MissingFontsPolicy,
) -> Result<(), AnyError> {
    if unavailable_names.is_empty() {
        return Ok(());
    }

    if missing_fonts == MissingFontsPolicy::Error {
        if auto_google_fonts {
            return Err(anyhow!(
                "auto_google_fonts: the following fonts are not available on the system \
                 and not found in the Google Fonts catalog: {}",
                unavailable_details.join(", ")
            ));
        } else {
            return Err(anyhow!(
                "missing_fonts=error: the following fonts are not available on the system: {}. \
                 Install them with register_google_fonts_font() or enable auto_google_fonts.",
                unavailable_details.join(", ")
            ));
        }
    }

    if missing_fonts == MissingFontsPolicy::Warn {
        for name in unavailable_names {
            if auto_google_fonts {
                log::warn!(
                    "auto_google_fonts: font '{name}' is not available on the system \
                     and not found in the Google Fonts catalog, skipping"
                );
            } else {
                log::warn!("missing_fonts=warn: font '{name}' is not available on the system");
            }
        }
    }

    Ok(())
}

fn google_font_for_html(family: &str) -> Option<FontForHtml> {
    Some(FontForHtml {
        family: family.to_string(),
        source: FontSource::Google {
            font_id: family_to_id(family)?,
        },
    })
}

/// Classify font families extracted from the scenegraph into Google Fonts
/// or Local sources.
///
/// `explicit_google_families` are families provided by per-call
/// `GoogleFontRequest` entries — they are classified as Google immediately
/// without catalog probing and are excluded from missing-font reporting.
///
/// Fonts that exist in the Google Fonts catalog are sourced from Google for
/// portability (CDN links work on any machine). Remaining fonts are classified
/// as Local when `html_embed_local_fonts` is true and the font is available
/// in fontdb.
async fn classify_scenegraph_fonts(
    families: &BTreeSet<String>,
    auto_google_fonts: bool,
    html_embed_local_fonts: bool,
    missing_fonts: MissingFontsPolicy,
    explicit_google_families: &HashSet<String>,
) -> Result<Vec<FontForHtml>, AnyError> {
    let pre_registered = registered_google_families()?;

    if families.is_empty()
        || (!auto_google_fonts
            && !html_embed_local_fonts
            && missing_fonts == MissingFontsPolicy::Fallback
            && explicit_google_families.is_empty()
            && pre_registered.is_empty())
    {
        return Ok(Vec::new());
    }

    let available = available_font_families()?;

    let google_fonts_set: HashSet<String> = if auto_google_fonts {
        let candidates =
            scenegraph_google_probe_candidates(families, explicit_google_families, &pre_registered);
        google_font_catalog_matches(candidates.iter(), missing_fonts).await?
    } else {
        HashSet::new()
    };

    let mut html_fonts: Vec<FontForHtml> = Vec::new();
    let mut unavailable: Vec<String> = Vec::new();
    for family in families {
        // Explicit per-call requests win immediately
        if explicit_google_families.contains(family) {
            if let Some(font) = google_font_for_html(family) {
                html_fonts.push(font);
            }
            continue;
        }
        if auto_google_fonts && google_fonts_set.contains(family) {
            if let Some(font) = google_font_for_html(family) {
                html_fonts.push(font);
                continue;
            }
        }
        // Fonts previously registered via register_google_fonts_font()
        if pre_registered.contains(family) {
            if let Some(font) = google_font_for_html(family) {
                html_fonts.push(font);
                continue;
            }
        }
        if is_available(family, &available) {
            if html_embed_local_fonts {
                html_fonts.push(FontForHtml {
                    family: family.clone(),
                    source: FontSource::Local,
                });
            }
            // Font is locally available — not missing even if not embedded
        } else {
            unavailable.push(family.clone());
        }
    }

    // Report fonts that are neither in Google Fonts nor locally available.
    // This covers runtime-resolved families (signal/field-driven) that static
    // spec extraction cannot see.
    let unavailable_details: Vec<String> = unavailable.iter().map(|n| format!("'{n}'")).collect();
    report_unavailable_fonts(
        &unavailable,
        &unavailable_details,
        auto_google_fonts,
        missing_fonts,
    )?;

    Ok(html_fonts)
}

/// Result of analyzing a rendered Vega scenegraph for font embedding.
struct HtmlFontAnalysis {
    /// Classified font metadata (Google or Local).
    html_fonts: Vec<FontForHtml>,
    /// Characters used per (family, weight, style) — for subsetting.
    chars_by_key: HashMap<FontKey, BTreeSet<char>>,
    /// (weight, style) variants per family — for CDN URLs.
    family_variants: HashMap<String, BTreeSet<(String, String)>>,
}

struct VlConverterInner {
    vegaembed_bundles: Mutex<HashMap<VlVersion, String>>,
    pool: Mutex<Option<WorkerPool>>,
    config: Arc<VlConverterConfig>,
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
/// let vega_spec = futures::executor::block_on(
///     converter.vegalite_to_vega(
///         vl_spec,
///         VlOpts {
///             vl_version: VlVersion::default(),
///             ..Default::default()
///         }
///     )
/// ).expect("Failed to perform Vega-Lite to Vega conversion");
///
/// println!("{}", vega_spec);
/// ```
#[derive(Clone)]
pub struct VlConverter {
    inner: Arc<VlConverterInner>,
}

impl VlConverter {
    pub fn new() -> Self {
        Self::with_config(VlConverterConfig::default()).expect("default converter config is valid")
    }

    pub fn with_config(config: VlConverterConfig) -> Result<Self, AnyError> {
        let config = Arc::new(normalize_converter_config(config)?);

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
                config,
            }),
        })
    }

    pub fn config(&self) -> VlConverterConfig {
        (*self.inner.config).clone()
    }

    fn effective_allowed_base_urls(
        &self,
        requested_allowed_base_urls: Option<Vec<String>>,
    ) -> Result<Option<Vec<String>>, AnyError> {
        let requested_allowed_base_urls = normalize_allowed_base_urls(requested_allowed_base_urls)?;
        if requested_allowed_base_urls.is_some() && !self.inner.config.allow_http_access {
            bail!("allowed_base_urls cannot be set when HTTP access is disabled");
        }

        // Per-request allowlists override converter-level defaults. Converter-level values
        // are used as a fallback when requests do not provide one.
        Ok(requested_allowed_base_urls.or_else(|| self.inner.config.allowed_base_urls.clone()))
    }

    fn image_access_policy_with_allowed_base_urls(
        &self,
        allowed_base_urls: Option<Vec<String>>,
    ) -> ImageAccessPolicy {
        ImageAccessPolicy {
            allow_http_access: self.inner.config.allow_http_access,
            filesystem_root: self.inner.config.filesystem_root.clone(),
            allowed_base_urls,
        }
    }

    fn image_access_policy(&self) -> ImageAccessPolicy {
        self.image_access_policy_with_allowed_base_urls(self.inner.config.allowed_base_urls.clone())
    }

    /// Eagerly start the worker pool for this converter instance.
    ///
    /// This is optional; if not called, the pool starts lazily on first request.
    pub fn warm_up(&self) -> Result<(), AnyError> {
        let _ = self.get_or_spawn_sender()?;
        Ok(())
    }

    fn get_or_spawn_sender(
        &self,
    ) -> Result<(tokio::sync::mpsc::Sender<QueuedCommand>, OutstandingTicket), AnyError> {
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

        let pool = spawn_worker_pool(self.inner.config.clone())?;
        let sender = pool
            .next_sender()
            .ok_or_else(|| anyhow!("Worker pool has no senders"))?;
        *guard = Some(pool);
        Ok(sender)
    }

    async fn send_command_with_retry(
        &self,
        cmd: VlConvertCommand,
        request_name: &str,
    ) -> Result<(), AnyError> {
        let (sender, ticket) = self.get_or_spawn_sender()?;
        let queued = QueuedCommand::new(cmd, ticket);
        match sender.send(queued).await {
            Ok(()) => Ok(()),
            Err(tokio::sync::mpsc::error::SendError(queued)) => {
                let cmd = queued.into_command();
                let (sender, ticket) = self.get_or_spawn_sender()?;
                sender
                    .send(QueuedCommand::new(cmd, ticket))
                    .await
                    .map_err(|err| {
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

    fn request_font_key(request: &GoogleFontRequest) -> String {
        let mut key = request.family.trim().to_lowercase();
        key.push('|');
        match &request.variants {
            None => key.push_str("all"),
            Some(variants) => {
                let mut pairs: Vec<(u16, &'static str)> = variants
                    .iter()
                    .map(|variant| (variant.weight, variant.style.as_str()))
                    .collect();
                pairs.sort_unstable();
                for (idx, (weight, style)) in pairs.iter().enumerate() {
                    if idx > 0 {
                        key.push(',');
                    }
                    key.push_str(&format!("{weight}:{style}"));
                }
            }
        }
        key
    }

    async fn resolve_google_fonts(
        &self,
        request_fonts: Option<Vec<GoogleFontRequest>>,
    ) -> Result<Vec<LoadedFontBatch>, AnyError> {
        let Some(request_fonts) = request_fonts else {
            return Ok(Vec::new());
        };
        if request_fonts.is_empty() {
            return Ok(Vec::new());
        }

        let mut unique: HashMap<String, GoogleFontRequest> = HashMap::new();
        for request in request_fonts {
            let key = Self::request_font_key(&request);
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
    ) -> Result<Option<(serde_json::Value, VgOpts)>, AnyError> {
        if !self.should_preprocess_fonts() {
            return Ok(None);
        }
        let mut vg_opts = VgOpts {
            allowed_base_urls: vl_opts.allowed_base_urls.clone(),
            format_locale: vl_opts.format_locale.clone(),
            time_format_locale: vl_opts.time_format_locale.clone(),
            google_fonts: vl_opts.google_fonts.clone(),
        };
        let vega_spec = self
            .vegalite_to_vega(vl_spec.clone(), vl_opts.clone())
            .await?;
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
        Ok(Some((vega_spec, vg_opts)))
    }

    /// If font preprocessing is enabled, parse the Vega spec and process missing fonts.
    ///
    /// Note: font downloads are governed solely by `auto_google_fonts`, independently
    /// of `allow_http_access`. The two settings control different concerns:
    /// `allow_http_access` governs data-fetching URLs in specs, while `auto_google_fonts`
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
    async fn preprocess_svg_fonts(&self, svg: &str) -> Result<Vec<LoadedFontBatch>, AnyError> {
        if !self.should_preprocess_fonts() {
            return Ok(Vec::new());
        }

        let font_strings = crate::extract::extract_fonts_from_svg(svg);
        let auto_requests = classify_and_request_fonts(
            font_strings,
            self.inner.config.auto_google_fonts,
            self.inner.config.missing_fonts,
            false,
        )
        .await?;

        self.resolve_google_fonts(if auto_requests.is_empty() {
            None
        } else {
            Some(auto_requests)
        })
        .await
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
        mut vg_opts: VgOpts,
    ) -> Result<String, AnyError> {
        vg_opts.allowed_base_urls =
            self.effective_allowed_base_urls(vg_opts.allowed_base_urls.take())?;
        let vg_spec = vg_spec.into();
        let auto_requests = self.maybe_preprocess_vega_fonts(&vg_spec).await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }
        let google_font_batches = self
            .resolve_google_fonts(vg_opts.google_fonts.take())
            .await?;
        self.request(
            move |responder| VlConvertCommand::VgToSvg {
                vg_spec,
                vg_opts,
                google_font_batches,
                responder,
            },
            "Vega to SVG conversion",
        )
        .await
    }

    pub async fn vega_to_scenegraph(
        &self,
        vg_spec: impl Into<ValueOrString>,
        mut vg_opts: VgOpts,
    ) -> Result<serde_json::Value, AnyError> {
        vg_opts.allowed_base_urls =
            self.effective_allowed_base_urls(vg_opts.allowed_base_urls.take())?;
        let vg_spec = vg_spec.into();
        let auto_requests = self.maybe_preprocess_vega_fonts(&vg_spec).await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }
        let google_font_batches = self
            .resolve_google_fonts(vg_opts.google_fonts.take())
            .await?;
        self.request(
            move |responder| VlConvertCommand::VgToSg {
                vg_spec,
                vg_opts,
                google_font_batches,
                responder,
            },
            "Vega to Scenegraph conversion",
        )
        .await
    }

    pub async fn vega_to_scenegraph_msgpack(
        &self,
        vg_spec: impl Into<ValueOrString>,
        mut vg_opts: VgOpts,
    ) -> Result<Vec<u8>, AnyError> {
        vg_opts.allowed_base_urls =
            self.effective_allowed_base_urls(vg_opts.allowed_base_urls.take())?;
        let vg_spec = vg_spec.into();
        let auto_requests = self.maybe_preprocess_vega_fonts(&vg_spec).await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }
        let google_font_batches = self
            .resolve_google_fonts(vg_opts.google_fonts.take())
            .await?;
        self.request(
            move |responder| VlConvertCommand::VgToSgMsgpack {
                vg_spec,
                vg_opts,
                google_font_batches,
                responder,
            },
            "Vega to Scenegraph conversion",
        )
        .await
    }

    pub async fn vegalite_to_svg(
        &self,
        vl_spec: impl Into<ValueOrString>,
        mut vl_opts: VlOpts,
    ) -> Result<String, AnyError> {
        vl_opts.allowed_base_urls =
            self.effective_allowed_base_urls(vl_opts.allowed_base_urls.take())?;
        let vl_spec = vl_spec.into();

        if let Some((vega_spec, mut vg_opts)) = self
            .maybe_compile_vl_with_preprocessed_fonts(&vl_spec, &vl_opts)
            .await?
        {
            let google_font_batches = self
                .resolve_google_fonts(vg_opts.google_fonts.take())
                .await?;
            let vg_spec: ValueOrString = vega_spec.into();
            self.request(
                move |responder| VlConvertCommand::VgToSvg {
                    vg_spec,
                    vg_opts,
                    google_font_batches,
                    responder,
                },
                "Vega to SVG conversion",
            )
            .await
        } else {
            let google_font_batches = self
                .resolve_google_fonts(vl_opts.google_fonts.take())
                .await?;
            self.request(
                move |responder| VlConvertCommand::VlToSvg {
                    vl_spec,
                    vl_opts,
                    google_font_batches,
                    responder,
                },
                "Vega-Lite to SVG conversion",
            )
            .await
        }
    }

    pub async fn vegalite_to_scenegraph(
        &self,
        vl_spec: impl Into<ValueOrString>,
        mut vl_opts: VlOpts,
    ) -> Result<serde_json::Value, AnyError> {
        vl_opts.allowed_base_urls =
            self.effective_allowed_base_urls(vl_opts.allowed_base_urls.take())?;
        let vl_spec = vl_spec.into();

        if let Some((vega_spec, mut vg_opts)) = self
            .maybe_compile_vl_with_preprocessed_fonts(&vl_spec, &vl_opts)
            .await?
        {
            let google_font_batches = self
                .resolve_google_fonts(vg_opts.google_fonts.take())
                .await?;
            let vg_spec: ValueOrString = vega_spec.into();
            self.request(
                move |responder| VlConvertCommand::VgToSg {
                    vg_spec,
                    vg_opts,
                    google_font_batches,
                    responder,
                },
                "Vega to Scenegraph conversion",
            )
            .await
        } else {
            let google_font_batches = self
                .resolve_google_fonts(vl_opts.google_fonts.take())
                .await?;
            self.request(
                move |responder| VlConvertCommand::VlToSg {
                    vl_spec,
                    vl_opts,
                    google_font_batches,
                    responder,
                },
                "Vega-Lite to Scenegraph conversion",
            )
            .await
        }
    }

    pub async fn vegalite_to_scenegraph_msgpack(
        &self,
        vl_spec: impl Into<ValueOrString>,
        mut vl_opts: VlOpts,
    ) -> Result<Vec<u8>, AnyError> {
        vl_opts.allowed_base_urls =
            self.effective_allowed_base_urls(vl_opts.allowed_base_urls.take())?;
        let vl_spec = vl_spec.into();

        if let Some((vega_spec, mut vg_opts)) = self
            .maybe_compile_vl_with_preprocessed_fonts(&vl_spec, &vl_opts)
            .await?
        {
            let google_font_batches = self
                .resolve_google_fonts(vg_opts.google_fonts.take())
                .await?;
            let vg_spec: ValueOrString = vega_spec.into();
            self.request(
                move |responder| VlConvertCommand::VgToSgMsgpack {
                    vg_spec,
                    vg_opts,
                    google_font_batches,
                    responder,
                },
                "Vega to Scenegraph conversion",
            )
            .await
        } else {
            let google_font_batches = self
                .resolve_google_fonts(vl_opts.google_fonts.take())
                .await?;
            self.request(
                move |responder| VlConvertCommand::VlToSgMsgpack {
                    vl_spec,
                    vl_opts,
                    google_font_batches,
                    responder,
                },
                "Vega-Lite to Scenegraph conversion",
            )
            .await
        }
    }

    pub async fn vega_to_png(
        &self,
        vg_spec: impl Into<ValueOrString>,
        mut vg_opts: VgOpts,
        scale: Option<f32>,
        ppi: Option<f32>,
    ) -> Result<Vec<u8>, AnyError> {
        vg_opts.allowed_base_urls =
            self.effective_allowed_base_urls(vg_opts.allowed_base_urls.take())?;
        let vg_spec = vg_spec.into();
        let auto_requests = self.maybe_preprocess_vega_fonts(&vg_spec).await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }
        let google_font_batches = self
            .resolve_google_fonts(vg_opts.google_fonts.take())
            .await?;
        let scale = scale.unwrap_or(1.0);
        let ppi = ppi.unwrap_or(72.0);
        let effective_scale = scale * ppi / 72.0;

        self.request(
            move |responder| VlConvertCommand::VgToPng {
                vg_spec,
                vg_opts,
                google_font_batches,
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
        mut vl_opts: VlOpts,
        scale: Option<f32>,
        ppi: Option<f32>,
    ) -> Result<Vec<u8>, AnyError> {
        vl_opts.allowed_base_urls =
            self.effective_allowed_base_urls(vl_opts.allowed_base_urls.take())?;
        let vl_spec = vl_spec.into();
        let scale = scale.unwrap_or(1.0);
        let ppi = ppi.unwrap_or(72.0);
        let effective_scale = scale * ppi / 72.0;

        if let Some((vega_spec, mut vg_opts)) = self
            .maybe_compile_vl_with_preprocessed_fonts(&vl_spec, &vl_opts)
            .await?
        {
            let google_font_batches = self
                .resolve_google_fonts(vg_opts.google_fonts.take())
                .await?;
            let vg_spec: ValueOrString = vega_spec.into();
            self.request(
                move |responder| VlConvertCommand::VgToPng {
                    vg_spec,
                    vg_opts,
                    google_font_batches,
                    scale: effective_scale,
                    ppi,
                    responder,
                },
                "Vega to PNG conversion",
            )
            .await
        } else {
            let google_font_batches = self
                .resolve_google_fonts(vl_opts.google_fonts.take())
                .await?;
            self.request(
                move |responder| VlConvertCommand::VlToPng {
                    vl_spec,
                    vl_opts,
                    google_font_batches,
                    scale: effective_scale,
                    ppi,
                    responder,
                },
                "Vega-Lite to PNG conversion",
            )
            .await
        }
    }

    pub async fn vega_to_jpeg(
        &self,
        vg_spec: impl Into<ValueOrString>,
        mut vg_opts: VgOpts,
        scale: Option<f32>,
        quality: Option<u8>,
    ) -> Result<Vec<u8>, AnyError> {
        let effective_allowed_base_urls =
            self.effective_allowed_base_urls(vg_opts.allowed_base_urls.take())?;
        let scale = scale.unwrap_or(1.0);
        let vg_spec = vg_spec.into();
        let auto_requests = self.maybe_preprocess_vega_fonts(&vg_spec).await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }
        let google_font_batches = self
            .resolve_google_fonts(vg_opts.google_fonts.take())
            .await?;
        let image_policy =
            self.image_access_policy_with_allowed_base_urls(effective_allowed_base_urls.clone());
        vg_opts.allowed_base_urls = effective_allowed_base_urls;
        self.request(
            move |responder| VlConvertCommand::VgToJpeg {
                vg_spec,
                vg_opts,
                google_font_batches,
                scale,
                quality,
                image_policy,
                responder,
            },
            "Vega to JPEG conversion",
        )
        .await
    }

    pub async fn vegalite_to_jpeg(
        &self,
        vl_spec: impl Into<ValueOrString>,
        mut vl_opts: VlOpts,
        scale: Option<f32>,
        quality: Option<u8>,
    ) -> Result<Vec<u8>, AnyError> {
        let effective_allowed_base_urls =
            self.effective_allowed_base_urls(vl_opts.allowed_base_urls.take())?;
        let scale = scale.unwrap_or(1.0);
        let image_policy =
            self.image_access_policy_with_allowed_base_urls(effective_allowed_base_urls.clone());
        vl_opts.allowed_base_urls = effective_allowed_base_urls;
        let vl_spec = vl_spec.into();

        if let Some((vega_spec, mut vg_opts)) = self
            .maybe_compile_vl_with_preprocessed_fonts(&vl_spec, &vl_opts)
            .await?
        {
            let google_font_batches = self
                .resolve_google_fonts(vg_opts.google_fonts.take())
                .await?;
            let vg_spec: ValueOrString = vega_spec.into();
            self.request(
                move |responder| VlConvertCommand::VgToJpeg {
                    vg_spec,
                    vg_opts,
                    google_font_batches,
                    scale,
                    quality,
                    image_policy,
                    responder,
                },
                "Vega to JPEG conversion",
            )
            .await
        } else {
            let google_font_batches = self
                .resolve_google_fonts(vl_opts.google_fonts.take())
                .await?;
            self.request(
                move |responder| VlConvertCommand::VlToJpeg {
                    vl_spec,
                    vl_opts,
                    google_font_batches,
                    scale,
                    quality,
                    image_policy,
                    responder,
                },
                "Vega-Lite to JPEG conversion",
            )
            .await
        }
    }

    pub async fn vega_to_pdf(
        &self,
        vg_spec: impl Into<ValueOrString>,
        mut vg_opts: VgOpts,
    ) -> Result<Vec<u8>, AnyError> {
        let effective_allowed_base_urls =
            self.effective_allowed_base_urls(vg_opts.allowed_base_urls.take())?;
        let vg_spec = vg_spec.into();
        let auto_requests = self.maybe_preprocess_vega_fonts(&vg_spec).await?;
        if !auto_requests.is_empty() {
            vg_opts
                .google_fonts
                .get_or_insert_with(Vec::new)
                .extend(auto_requests);
        }
        let google_font_batches = self
            .resolve_google_fonts(vg_opts.google_fonts.take())
            .await?;
        let image_policy =
            self.image_access_policy_with_allowed_base_urls(effective_allowed_base_urls.clone());
        vg_opts.allowed_base_urls = effective_allowed_base_urls;
        self.request(
            move |responder| VlConvertCommand::VgToPdf {
                vg_spec,
                vg_opts,
                google_font_batches,
                image_policy,
                responder,
            },
            "Vega to PDF conversion",
        )
        .await
    }

    pub async fn vegalite_to_pdf(
        &self,
        vl_spec: impl Into<ValueOrString>,
        mut vl_opts: VlOpts,
    ) -> Result<Vec<u8>, AnyError> {
        let effective_allowed_base_urls =
            self.effective_allowed_base_urls(vl_opts.allowed_base_urls.take())?;
        let image_policy =
            self.image_access_policy_with_allowed_base_urls(effective_allowed_base_urls.clone());
        vl_opts.allowed_base_urls = effective_allowed_base_urls;
        let vl_spec = vl_spec.into();

        if let Some((vega_spec, mut vg_opts)) = self
            .maybe_compile_vl_with_preprocessed_fonts(&vl_spec, &vl_opts)
            .await?
        {
            let google_font_batches = self
                .resolve_google_fonts(vg_opts.google_fonts.take())
                .await?;
            let vg_spec: ValueOrString = vega_spec.into();
            self.request(
                move |responder| VlConvertCommand::VgToPdf {
                    vg_spec,
                    vg_opts,
                    google_font_batches,
                    image_policy,
                    responder,
                },
                "Vega to PDF conversion",
            )
            .await
        } else {
            let google_font_batches = self
                .resolve_google_fonts(vl_opts.google_fonts.take())
                .await?;
            self.request(
                move |responder| VlConvertCommand::VlToPdf {
                    vl_spec,
                    vl_opts,
                    google_font_batches,
                    image_policy,
                    responder,
                },
                "Vega-Lite to PDF conversion",
            )
            .await
        }
    }

    pub async fn svg_to_png(
        &self,
        svg: &str,
        scale: f32,
        ppi: Option<f32>,
    ) -> Result<Vec<u8>, AnyError> {
        let image_policy = self.image_access_policy();
        let google_font_batches = self.preprocess_svg_fonts(svg).await?;
        let svg = svg.to_string();
        self.request(
            move |responder| VlConvertCommand::SvgToPng {
                svg,
                scale,
                ppi,
                image_policy,
                google_font_batches,
                responder,
            },
            "SVG to PNG conversion",
        )
        .await
    }

    pub async fn svg_to_jpeg(
        &self,
        svg: &str,
        scale: f32,
        quality: Option<u8>,
    ) -> Result<Vec<u8>, AnyError> {
        let image_policy = self.image_access_policy();
        let google_font_batches = self.preprocess_svg_fonts(svg).await?;
        let svg = svg.to_string();
        self.request(
            move |responder| VlConvertCommand::SvgToJpeg {
                svg,
                scale,
                quality,
                image_policy,
                google_font_batches,
                responder,
            },
            "SVG to JPEG conversion",
        )
        .await
    }

    pub async fn svg_to_pdf(&self, svg: &str) -> Result<Vec<u8>, AnyError> {
        let image_policy = self.image_access_policy();
        let google_font_batches = self.preprocess_svg_fonts(svg).await?;
        let svg = svg.to_string();
        self.request(
            move |responder| VlConvertCommand::SvgToPdf {
                svg,
                image_policy,
                google_font_batches,
                responder,
            },
            "SVG to PDF conversion",
        )
        .await
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
            .request(
                move |responder| VlConvertCommand::ComputeVegaembedBundle {
                    vl_version,
                    responder,
                },
                "Vega-Embed bundle generation",
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

    pub async fn bundle_vega_snippet(
        &self,
        snippet: impl Into<String>,
        vl_version: VlVersion,
    ) -> Result<String, AnyError> {
        let snippet = snippet.into();
        self.request(
            move |responder| VlConvertCommand::BundleVegaSnippet {
                snippet,
                vl_version,
                responder,
            },
            "JavaScript bundle generation",
        )
        .await
    }

    async fn build_html(
        &self,
        code: &str,
        vl_version: VlVersion,
        bundle: bool,
        font_head_html: &str,
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
{font_head_html}{script_tags}
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

    /// Render a Vega scenegraph for HTML font analysis.
    ///
    /// Unlike the public `vega_to_scenegraph`, this uses the caller-supplied
    /// `auto_google_fonts` flag (not the converter config) to decide whether
    /// to auto-detect Google Fonts from the spec. This ensures the render
    /// and the later classification agree on the effective policy.
    async fn render_scenegraph_for_html(
        &self,
        vega_spec: serde_json::Value,
        mut vg_opts: VgOpts,
        auto_google_fonts: bool,
    ) -> Result<serde_json::Value, AnyError> {
        vg_opts.allowed_base_urls =
            self.effective_allowed_base_urls(vg_opts.allowed_base_urls.take())?;
        let missing = self.inner.config.missing_fonts;

        // Preprocess fonts using the per-call auto_google_fonts flag with
        // prefer_cdn=true: HTML output references Google CDN fonts, so the
        // render should use the Google face even when a local copy exists.
        if auto_google_fonts || missing != MissingFontsPolicy::Fallback {
            let font_strings = extract_fonts_from_vega(&vega_spec);
            let auto_requests =
                classify_and_request_fonts(font_strings, auto_google_fonts, missing, true).await?;
            if !auto_requests.is_empty() {
                vg_opts
                    .google_fonts
                    .get_or_insert_with(Vec::new)
                    .extend(auto_requests);
            }
        }

        let vg_spec: ValueOrString = vega_spec.into();
        let google_font_batches = self
            .resolve_google_fonts(vg_opts.google_fonts.take())
            .await?;
        self.request(
            move |responder| VlConvertCommand::VgToSg {
                vg_spec,
                vg_opts,
                google_font_batches,
                responder,
            },
            "Vega to Scenegraph (HTML analysis)",
        )
        .await
    }

    /// Render the Vega scenegraph, walk it in Rust to extract text-by-font
    /// data, classify fonts as Google or Local, and merge any explicit
    /// per-request Google Font overrides.
    ///
    /// This is the single point of truth for font analysis — called once per
    /// HTML generation or `vega_fonts` / `vegalite_fonts` invocation.
    async fn analyze_html_fonts(
        &self,
        vega_spec: serde_json::Value,
        vg_opts: VgOpts,
        auto_google_fonts: bool,
        html_embed_local_fonts: bool,
    ) -> Result<HtmlFontAnalysis, AnyError> {
        let missing = self.inner.config.missing_fonts;

        // Clone fields we need after vg_opts is consumed by render.
        let explicit_requests = vg_opts.google_fonts.clone();
        let format_locale_value = vg_opts
            .format_locale
            .as_ref()
            .and_then(|l| l.as_object().ok());
        let time_format_locale_value = vg_opts
            .time_format_locale
            .as_ref()
            .and_then(|l| l.as_object().ok());

        // Render scenegraph using the per-call auto_google_fonts flag.
        let sg = self
            .render_scenegraph_for_html(vega_spec, vg_opts, auto_google_fonts)
            .await?;
        let sg_root = sg.get("scenegraph").unwrap_or(&sg);

        // Walk scenegraph in Rust — no separate JS call needed.
        let mut chars_by_key = extract_text_by_font(sg_root);

        // Inject locale-aware characters so that pan/zoom interactions can
        // render axis labels with locale-specific formatting (e.g. non-ASCII
        // decimal separators, currency symbols, month/day names).
        inject_locale_chars(
            &mut chars_by_key,
            format_locale_value.as_ref(),
            time_format_locale_value.as_ref(),
        );

        // Collect unique family names from the scenegraph
        let families: BTreeSet<String> = chars_by_key.keys().map(|k| k.family.clone()).collect();

        // Build explicit Google families set from per-call requests so
        // classify_scenegraph_fonts treats them as Google immediately.
        let explicit_google_families: HashSet<String> = explicit_requests
            .as_ref()
            .map(|reqs| reqs.iter().map(|r| r.family.clone()).collect())
            .unwrap_or_default();

        // Classify families as Google Fonts or Local
        let mut html_fonts = classify_scenegraph_fonts(
            &families,
            auto_google_fonts,
            html_embed_local_fonts,
            missing,
            &explicit_google_families,
        )
        .await?;

        let mut family_variants = variants_by_family(&chars_by_key);

        // Add explicit families not found in the scenegraph, and propagate
        // explicit variant requests into family_variants.
        if let Some(ref requests) = explicit_requests {
            let known: HashSet<String> = html_fonts.iter().map(|f| f.family.clone()).collect();
            for req in requests {
                if !known.contains(&req.family) {
                    if let Some(font_id) = family_to_id(&req.family) {
                        html_fonts.push(FontForHtml {
                            family: req.family.clone(),
                            source: FontSource::Google { font_id },
                        });
                    }
                }
                if let Some(ref variants) = req.variants {
                    let entry = family_variants.entry(req.family.clone()).or_default();
                    for v in variants {
                        entry.insert((v.weight.to_string(), v.style.as_str().to_string()));
                    }
                }
            }
        }

        Ok(HtmlFontAnalysis {
            html_fonts,
            chars_by_key,
            family_variants,
        })
    }

    /// Return font information for a Vega spec in the requested format.
    ///
    /// Renders the scenegraph once to discover the exact fonts, weights, and
    /// characters used. The `auto_google_fonts` and `html_embed_local_fonts`
    /// parameters control which fonts are included.
    pub async fn vega_fonts(
        &self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
        auto_google_fonts: bool,
        html_embed_local_fonts: bool,
        include_font_face: bool,
    ) -> Result<Vec<FontInfo>, AnyError> {
        let vg_spec = vg_spec.into();
        let spec_value: serde_json::Value = match &vg_spec {
            ValueOrString::JsonString(s) => serde_json::from_str(s)?,
            ValueOrString::Value(v) => v.clone(),
        };

        let analysis = self
            .analyze_html_fonts(
                spec_value,
                vg_opts,
                auto_google_fonts,
                html_embed_local_fonts,
            )
            .await?;

        self.build_font_info(analysis, include_font_face).await
    }

    /// Build structured `FontInfo` from a completed font analysis.
    ///
    /// When `include_font_face` is true, runs the subsetting pipeline to
    /// populate `FontVariant::font_face` on each variant.
    async fn build_font_info(
        &self,
        analysis: HtmlFontAnalysis,
        include_font_face: bool,
    ) -> Result<Vec<FontInfo>, AnyError> {
        let HtmlFontAnalysis {
            html_fonts,
            chars_by_key,
            family_variants,
        } = analysis;

        // If font_face is requested, run the subsetting pipeline to get
        // per-variant CSS blocks indexed by (family, weight, style).
        let font_face_index: HashMap<FontKey, String> =
            if include_font_face && !html_fonts.is_empty() {
                // Build Google Font requests with specific variants from the
                // scenegraph — only download what the chart actually uses.
                let google_font_requests: Vec<GoogleFontRequest> = html_fonts
                    .iter()
                    .filter_map(|f| match &f.source {
                        FontSource::Google { .. } => {
                            let variants = family_variants.get(&f.family).map(|vs| {
                                vs.iter()
                                    .map(|(w, s)| VariantRequest {
                                        weight: w.parse().unwrap_or(400),
                                        style: s.parse().unwrap_or(FontStyle::Normal),
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
                let batches = if google_font_requests.is_empty() {
                    Vec::new()
                } else {
                    self.resolve_google_fonts(Some(google_font_requests))
                        .await?
                };

                let missing = self.inner.config.missing_fonts;
                let fontdb = USVG_OPTIONS
                    .lock()
                    .map_err(|e| anyhow!("failed to lock USVG_OPTIONS: {e}"))?
                    .fontdb
                    .clone();
                generate_font_face_css(&chars_by_key, &html_fonts, &missing, &fontdb, &batches)?
            } else {
                HashMap::new()
            };

        // Build FontInfo for each font family.
        let results: Vec<FontInfo> = html_fonts
            .iter()
            .map(|f| {
                let variants_set = family_variants.get(&f.family);
                let url = font_cdn_url(f, variants_set);
                let link_tag = font_link_tag(f, variants_set);
                let import_rule = font_import_rule(f, variants_set);

                let variants: Vec<FontVariant> = variants_set
                    .map(|vs| {
                        vs.iter()
                            .map(|(w, s)| {
                                let font_face = if include_font_face {
                                    let key = FontKey {
                                        family: f.family.clone(),
                                        weight: w.clone(),
                                        style: s.clone(),
                                    };
                                    font_face_index.get(&key).cloned()
                                } else {
                                    None
                                };
                                FontVariant {
                                    weight: w.clone(),
                                    style: s.clone(),
                                    font_face,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                FontInfo {
                    name: f.family.clone(),
                    source: f.source.clone(),
                    variants,
                    url,
                    link_tag,
                    import_rule,
                }
            })
            .collect();

        Ok(results)
    }

    /// Return font information for a Vega-Lite spec.
    ///
    /// Compiles the spec to Vega first, then delegates to [`vega_fonts`].
    pub async fn vegalite_fonts(
        &self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
        auto_google_fonts: bool,
        html_embed_local_fonts: bool,
        include_font_face: bool,
    ) -> Result<Vec<FontInfo>, AnyError> {
        let vega_spec = self.vegalite_to_vega(vl_spec, vl_opts.clone()).await?;
        let vg_opts = VgOpts {
            allowed_base_urls: vl_opts.allowed_base_urls,
            format_locale: vl_opts.format_locale,
            time_format_locale: vl_opts.time_format_locale,
            google_fonts: vl_opts.google_fonts,
        };
        self.vega_fonts(
            vega_spec,
            vg_opts,
            auto_google_fonts,
            html_embed_local_fonts,
            include_font_face,
        )
        .await
    }

    /// Build font `<link>` and/or `<style>` tags for HTML `<head>` injection.
    ///
    /// Uses `vega_fonts` internally so the public API is exercised by every
    /// HTML export.
    async fn build_font_head_html(
        &self,
        vega_spec: serde_json::Value,
        vg_opts: VgOpts,
        bundle: bool,
        auto_install: bool,
        embed_local: bool,
    ) -> Result<String, AnyError> {
        // CDN-only fast path: when we don't need embedded @font-face blocks,
        // build <link> tags from static spec analysis without rendering the
        // scenegraph in V8.
        if !bundle && !embed_local {
            let mut parts = Vec::new();
            let mut seen_families = HashSet::new();

            // Explicit Google fonts from per-call requests
            if let Some(ref requests) = vg_opts.google_fonts {
                for req in requests {
                    if let Some(font_id) = family_to_id(&req.family) {
                        seen_families.insert(req.family.clone());
                        let font = FontForHtml {
                            family: req.family.clone(),
                            source: FontSource::Google { font_id },
                        };
                        let variants_set: Option<BTreeSet<(String, String)>> =
                            req.variants.as_ref().map(|vs| {
                                vs.iter()
                                    .map(|v| (v.weight.to_string(), v.style.as_str().to_string()))
                                    .collect()
                            });
                        if let Some(tag) = font_link_tag(&font, variants_set.as_ref()) {
                            parts.push(format!("    {tag}\n"));
                        }
                    }
                }
            }

            // Auto-detect additional Google fonts from static spec analysis
            if auto_install {
                let font_strings = extract_fonts_from_vega(&vega_spec);
                let missing = self.inner.config.missing_fonts;
                let auto_requests =
                    classify_and_request_fonts(font_strings, true, missing, true).await?;
                for req in &auto_requests {
                    if !seen_families.insert(req.family.clone()) {
                        continue;
                    }
                    if let Some(font_id) = family_to_id(&req.family) {
                        let font = FontForHtml {
                            family: req.family.clone(),
                            source: FontSource::Google { font_id },
                        };
                        if let Some(tag) = font_link_tag(&font, None) {
                            parts.push(format!("    {tag}\n"));
                        }
                    }
                }
            } else {
                // Check pre-registered Google fonts that appear in the spec
                let pre_registered = registered_google_families()?;
                if !pre_registered.is_empty() {
                    let font_strings = extract_fonts_from_vega(&vega_spec);
                    for font_string in &font_strings {
                        let entries = crate::extract::parse_css_font_family(font_string);
                        if let Some(crate::extract::FontFamilyEntry::Named(ref name)) =
                            entries.first()
                        {
                            if pre_registered.contains(name) && seen_families.insert(name.clone()) {
                                if let Some(font_id) = family_to_id(name) {
                                    let font = FontForHtml {
                                        family: name.clone(),
                                        source: FontSource::Google { font_id },
                                    };
                                    if let Some(tag) = font_link_tag(&font, None) {
                                        parts.push(format!("    {tag}\n"));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            return Ok(parts.join(""));
        }

        // Request font_face data when we'll need embedded CSS
        let include_font_face = bundle || embed_local;
        let fonts = self
            .vega_fonts(
                vega_spec,
                vg_opts,
                auto_install,
                embed_local,
                include_font_face,
            )
            .await?;

        if bundle {
            // Bundle mode: embed all fonts as @font-face CSS
            let blocks: Vec<&str> = fonts
                .iter()
                .flat_map(|f| f.variants.iter())
                .filter_map(|v| v.font_face.as_deref())
                .collect();
            if blocks.is_empty() {
                Ok(String::new())
            } else {
                let css = blocks.join("\n");
                Ok(format!("    <style>\n{css}\n    </style>\n"))
            }
        } else {
            let mut parts = Vec::new();

            // CDN mode: emit <link> tags for Google Fonts
            for font in &fonts {
                if let Some(tag) = &font.link_tag {
                    parts.push(format!("    {tag}\n"));
                }
            }

            // Embed local-only fonts as @font-face CSS
            if embed_local {
                let local_blocks: Vec<&str> = fonts
                    .iter()
                    .filter(|f| matches!(f.source, FontSource::Local))
                    .flat_map(|f| f.variants.iter())
                    .filter_map(|v| v.font_face.as_deref())
                    .collect();
                if !local_blocks.is_empty() {
                    let css = local_blocks.join("\n");
                    parts.push(format!("    <style>\n{css}\n    </style>\n"));
                }
            }

            Ok(parts.join(""))
        }
    }

    pub async fn vegalite_to_html(
        &self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
        bundle: bool,
        renderer: Renderer,
    ) -> Result<String, AnyError> {
        let vl_version = vl_opts.vl_version;
        let vl_spec = vl_spec.into();

        let auto_install = self.inner.config.auto_google_fonts;
        let embed_local = self.inner.config.html_embed_local_fonts;

        let font_head_html = if auto_install
            || embed_local
            || self.inner.config.missing_fonts != MissingFontsPolicy::Fallback
        {
            let vega_spec = self
                .vegalite_to_vega(vl_spec.clone(), vl_opts.clone())
                .await?;
            let vg_opts = VgOpts {
                allowed_base_urls: vl_opts.allowed_base_urls.clone(),
                format_locale: vl_opts.format_locale.clone(),
                time_format_locale: vl_opts.time_format_locale.clone(),
                google_fonts: vl_opts.google_fonts.clone(),
            };
            self.build_font_head_html(vega_spec, vg_opts, bundle, auto_install, embed_local)
                .await?
        } else {
            String::new()
        };

        let code = get_vega_or_vegalite_script(vl_spec, vl_opts.to_embed_opts(renderer)?)?;
        self.build_html(&code, vl_version, bundle, &font_head_html)
            .await
    }

    pub async fn vega_to_html(
        &self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
        bundle: bool,
        renderer: Renderer,
    ) -> Result<String, AnyError> {
        let vg_spec = vg_spec.into();

        let auto_install = self.inner.config.auto_google_fonts;
        let embed_local = self.inner.config.html_embed_local_fonts;

        let font_head_html = if auto_install
            || embed_local
            || self.inner.config.missing_fonts != MissingFontsPolicy::Fallback
        {
            let spec_value: serde_json::Value = match &vg_spec {
                ValueOrString::JsonString(s) => serde_json::from_str(s)?,
                ValueOrString::Value(v) => v.clone(),
            };
            self.build_font_head_html(
                spec_value,
                vg_opts.clone(),
                bundle,
                auto_install,
                embed_local,
            )
            .await?
        } else {
            String::new()
        };

        let code = get_vega_or_vegalite_script(vg_spec, vg_opts.to_embed_opts(renderer)?)?;
        self.build_html(&code, Default::default(), bundle, &font_head_html)
            .await
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

fn default_image_access_policy() -> ImageAccessPolicy {
    ImageAccessPolicy {
        allow_http_access: true,
        filesystem_root: None,
        allowed_base_urls: None,
    }
}

pub fn svg_to_png(svg: &str, scale: f32, ppi: Option<f32>) -> Result<Vec<u8>, AnyError> {
    svg_to_png_with_policy(svg, scale, ppi, &default_image_access_policy())
}

fn svg_to_png_with_policy(
    svg: &str,
    scale: f32,
    ppi: Option<f32>,
    policy: &ImageAccessPolicy,
) -> Result<Vec<u8>, AnyError> {
    // default ppi to 72
    let ppi = ppi.unwrap_or(72.0);
    let scale = scale * ppi / 72.0;
    let policy = policy.clone();

    // catch_unwind so that we don't poison Mutexes
    // if usvg/resvg panics
    let response = panic::catch_unwind(|| {
        let rtree = match parse_svg(svg, &policy) {
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
    svg_to_jpeg_with_policy(svg, scale, quality, &default_image_access_policy())
}

fn svg_to_jpeg_with_policy(
    svg: &str,
    scale: f32,
    quality: Option<u8>,
    policy: &ImageAccessPolicy,
) -> Result<Vec<u8>, AnyError> {
    let png_bytes = svg_to_png_with_policy(svg, scale, None, policy)?;
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
    svg_to_pdf_with_policy(svg, &default_image_access_policy())
}

fn svg_to_pdf_with_policy(svg: &str, policy: &ImageAccessPolicy) -> Result<Vec<u8>, AnyError> {
    let tree = parse_svg(svg, policy)?;
    let pdf = svg2pdf::to_pdf(&tree, ConversionOptions::default(), PageOptions::default());
    pdf.map_err(|err| anyhow!("Failed to convert SVG to PDF: {}", err))
}

/// Helper to parse svg string to usvg Tree with more helpful error messages
fn parse_svg(svg: &str, policy: &ImageAccessPolicy) -> Result<usvg::Tree, AnyError> {
    let mut opts = USVG_OPTIONS
        .lock()
        .map_err(|err| anyhow!("Failed to acquire usvg options lock: {err}"))?;
    parse_svg_with_options(svg, policy, &mut opts)
}

fn parse_svg_with_options(
    svg: &str,
    policy: &ImageAccessPolicy,
    opts: &mut usvg::Options<'static>,
) -> Result<usvg::Tree, AnyError> {
    let xml_opt = usvg::roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };

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

    let previous_resources_dir = opts.resources_dir.clone();
    opts.resources_dir = policy.filesystem_root.clone();
    let (result, access_errors) =
        crate::image_loading::with_image_access_policy(policy.clone(), || {
            usvg::Tree::from_xmltree(&doc, opts)
        });
    opts.resources_dir = previous_resources_dir;

    if !access_errors.is_empty() {
        bail!("{}", access_errors.join("\n"));
    }

    Ok(result?)
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
    use std::collections::HashMap;
    use std::future::Future;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    const PNG_1X1_BYTES: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4,
        0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 252, 255, 15, 0, 2, 3,
        1, 128, 179, 248, 175, 217, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    const SVG_2X3_BASE64: &str =
        "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyIiBoZWlnaHQ9IjMiPjxyZWN0IHdpZHRoPSIyIiBoZWlnaHQ9IjMiIGZpbGw9InJlZCIvPjwvc3ZnPg==";
    const SVG_2X3_DATA_URL: &str = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyIiBoZWlnaHQ9IjMiPjxyZWN0IHdpZHRoPSIyIiBoZWlnaHQ9IjMiIGZpbGw9InJlZCIvPjwvc3ZnPg==";

    fn make_test_command() -> VlConvertCommand {
        let (responder, _rx) =
            futures::channel::oneshot::channel::<Result<Option<String>, AnyError>>();
        VlConvertCommand::GetLocalTz { responder }
    }

    fn assert_send_future<F: Future + Send>(_: F) {}

    #[derive(Clone)]
    struct TestHttpResponse {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl TestHttpResponse {
        fn ok_text(body: &str) -> Self {
            Self {
                status: 200,
                headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
                body: body.as_bytes().to_vec(),
            }
        }

        fn ok_png(body: &[u8]) -> Self {
            Self {
                status: 200,
                headers: vec![("Content-Type".to_string(), "image/png".to_string())],
                body: body.to_vec(),
            }
        }

        fn ok_svg(body: &str) -> Self {
            Self {
                status: 200,
                headers: vec![("Content-Type".to_string(), "image/svg+xml".to_string())],
                body: body.as_bytes().to_vec(),
            }
        }

        fn redirect(location: &str) -> Self {
            Self {
                status: 302,
                headers: vec![("Location".to_string(), location.to_string())],
                body: Vec::new(),
            }
        }
    }

    struct TestHttpServer {
        addr: SocketAddr,
        running: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl TestHttpServer {
        fn new(routes: Vec<(&str, TestHttpResponse)>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let addr = listener.local_addr().unwrap();

            let routes = Arc::new(
                routes
                    .into_iter()
                    .map(|(path, response)| (path.to_string(), response))
                    .collect::<HashMap<_, _>>(),
            );
            let running = Arc::new(AtomicBool::new(true));
            let running_clone = running.clone();
            let routes_clone = routes.clone();

            let handle = thread::spawn(move || {
                while running_clone.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            handle_test_http_connection(stream, &routes_clone);
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
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

        fn url(&self, path: &str) -> String {
            format!("http://{}{}", self.addr, path)
        }

        fn origin(&self) -> String {
            format!("http://{}", self.addr)
        }

        fn base_url(&self) -> String {
            format!("http://{}/", self.addr)
        }
    }

    impl Drop for TestHttpServer {
        fn drop(&mut self) {
            self.running.store(false, Ordering::SeqCst);
            let _ = TcpStream::connect(self.addr);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn handle_test_http_connection(
        mut stream: TcpStream,
        routes: &HashMap<String, TestHttpResponse>,
    ) {
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
        let mut ctx = InnerVlConverter::try_new(
            std::sync::Arc::new(VlConverterConfig::default()),
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
            std::sync::Arc::new(VlConverterConfig::default()),
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
            std::sync::Arc::new(VlConverterConfig::default()),
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
        let mut ctx = InnerVlConverter::try_new(
            std::sync::Arc::new(VlConverterConfig::default()),
            get_font_baseline_snapshot().unwrap(),
        )
        .await
        .unwrap();
        let marker = serde_json::to_string(ACCESS_DENIED_MARKER).unwrap();
        let code = format!(
            r#"
var __imageDecodeErrorResult = null;
(async () => {{
  globalThis.__vlConvertAllowHttpAccess = false;
  globalThis.__vlConvertAllowedBaseUrls = null;
  globalThis.__vlConvertAccessDeniedMarker = {marker};
  globalThis.__vlConvertAccessErrors = [];

  const img = new Image();
  let onerrorCount = 0;
  let listenerCount = 0;
  img.onerror = () => {{ onerrorCount += 1; }};
  img.addEventListener("error", () => {{ listenerCount += 1; }});
  img.src = "https://example.com/image.png";

  let decodeMessage = "";
  try {{
    await img.decode();
    decodeMessage = "resolved";
  }} catch (err) {{
    decodeMessage = String(err && err.message ? err.message : err);
  }}

  __imageDecodeErrorResult = {{
    complete: img.complete,
    naturalWidth: img.naturalWidth,
    naturalHeight: img.naturalHeight,
    onerrorCount,
    listenerCount,
    decodeMessage,
    accessErrors: globalThis.__vlConvertAccessErrors.slice(),
  }};

  delete globalThis.__vlConvertAllowHttpAccess;
  delete globalThis.__vlConvertAllowedBaseUrls;
  delete globalThis.__vlConvertAccessDeniedMarker;
  delete globalThis.__vlConvertAccessErrors;
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
            .execute_script_to_json("__imageDecodeErrorResult")
            .await
            .unwrap();
        assert_eq!(result["complete"], json!(true));
        assert_eq!(result["naturalWidth"], json!(0));
        assert_eq!(result["naturalHeight"], json!(0));
        assert_eq!(result["onerrorCount"], json!(1));
        assert_eq!(result["listenerCount"], json!(1));
        assert!(result["decodeMessage"]
            .as_str()
            .unwrap_or_default()
            .contains(ACCESS_DENIED_MARKER));
        assert!(result["accessErrors"]
            .as_array()
            .and_then(|values| values.first())
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .contains(ACCESS_DENIED_MARKER));
    }

    #[tokio::test]
    async fn test_image_decode_ignores_stale_src_results() {
        let mut ctx = InnerVlConverter::try_new(
            std::sync::Arc::new(VlConverterConfig::default()),
            get_font_baseline_snapshot().unwrap(),
        )
        .await
        .unwrap();
        let code = format!(
            r#"
var __imageRaceResult = null;
(async () => {{
  const validImageBytes = (() => {{
    const binary = atob({SVG_2X3_BASE64:?});
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {{
      bytes[i] = binary.charCodeAt(i);
    }}
    return bytes;
  }})();
  const invalidBytes = new TextEncoder().encode("not-a-valid-image");
  const toArrayBuffer = (bytes) =>
    bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);

  const originalFetch = globalThis.fetch;
  globalThis.fetch = async (url) => {{
    const asString = String(url);
    if (asString.includes("slow")) {{
      await new Promise((resolve) => setTimeout(resolve, 30));
      return {{
        ok: true,
        status: 200,
        url: asString,
        type: "basic",
        redirected: false,
        arrayBuffer: async () => toArrayBuffer(invalidBytes),
      }};
    }}
    return {{
      ok: true,
      status: 200,
      url: asString,
      type: "basic",
      redirected: false,
      arrayBuffer: async () => toArrayBuffer(validImageBytes),
    }};
  }};

  try {{
    const img = new Image();
    let onloadCount = 0;
    let onerrorCount = 0;
    img.onload = () => {{ onloadCount += 1; }};
    img.onerror = () => {{ onerrorCount += 1; }};

    img.src = "https://example.com/slow.png";
    img.src = "https://example.com/fast.png";
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
  }} finally {{
    globalThis.fetch = originalFetch;
  }}
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
        assert_eq!(result["src"], json!("https://example.com/fast.png"));
        assert_eq!(result["complete"], json!(true));
        assert_eq!(result["naturalWidth"], json!(2));
        assert_eq!(result["naturalHeight"], json!(3));
        assert_eq!(result["onloadCount"], json!(1));
        assert_eq!(result["onerrorCount"], json!(0));
    }

    #[tokio::test]
    async fn test_polyfill_unsupported_methods_throw() {
        let mut ctx = InnerVlConverter::try_new(
            std::sync::Arc::new(VlConverterConfig::default()),
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
    fn test_with_config_rejects_zero_num_workers() {
        let err = VlConverter::with_config(VlConverterConfig {
            num_workers: 0,
            ..Default::default()
        })
        .err()
        .unwrap();
        assert!(err.to_string().contains("num_workers must be >= 1"));
    }

    #[test]
    fn test_config_reports_configured_num_workers() {
        let converter = VlConverter::with_config(VlConverterConfig {
            num_workers: 4,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(converter.config().num_workers, 4);
    }

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
    fn test_allowed_base_url_normalization_and_validation() {
        assert_eq!(
            normalize_allowed_base_url("https://example.com").unwrap(),
            "https://example.com/"
        );
        assert_eq!(
            normalize_allowed_base_url("https://example.com/data").unwrap(),
            "https://example.com/data/"
        );

        assert!(normalize_allowed_base_url("ftp://example.com/").is_err());
        assert!(normalize_allowed_base_url("https://user@example.com/").is_err());
        assert!(normalize_allowed_base_url("https://example.com/?q=1").is_err());
        assert!(normalize_allowed_base_url("https://example.com/#fragment").is_err());
        assert!(normalize_allowed_base_urls(Some(vec![])).is_err());
    }

    #[test]
    fn test_with_config_rejects_allowed_base_urls_when_http_disabled() {
        let err = VlConverter::with_config(VlConverterConfig {
            allow_http_access: false,
            allowed_base_urls: Some(vec!["https://example.com".to_string()]),
            ..Default::default()
        })
        .err()
        .unwrap();
        assert!(err
            .to_string()
            .contains("allowed_base_urls cannot be set when HTTP access is disabled"));
    }

    #[test]
    fn test_with_config_rejects_empty_allowed_base_urls() {
        let err = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: Some(vec![]),
            ..Default::default()
        })
        .err()
        .unwrap();
        assert!(err
            .to_string()
            .contains("allowed_base_urls cannot be empty"));
    }

    #[test]
    fn test_effective_allowed_base_urls_override_behavior() {
        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: Some(vec!["https://config.example/".to_string()]),
            ..Default::default()
        })
        .unwrap();

        let fallback = converter
            .effective_allowed_base_urls(None)
            .unwrap()
            .unwrap();
        assert_eq!(fallback, vec!["https://config.example/".to_string()]);

        let request_override = converter
            .effective_allowed_base_urls(Some(vec!["https://request.example/".to_string()]))
            .unwrap()
            .unwrap();
        assert_eq!(
            request_override,
            vec!["https://request.example/".to_string()]
        );
    }

    #[test]
    fn test_effective_allowed_base_urls_rejects_empty_override() {
        let converter = VlConverter::new();
        let err = converter
            .effective_allowed_base_urls(Some(vec![]))
            .err()
            .unwrap();
        assert!(err
            .to_string()
            .contains("allowed_base_urls cannot be empty"));
    }

    #[tokio::test]
    async fn test_svg_helper_denies_subdomain_and_userinfo_url_confusion() {
        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: Some(vec!["https://example.com".to_string()]),
            ..Default::default()
        })
        .unwrap();

        let subdomain_err = converter
            .svg_to_png(
                &svg_with_href("https://example.com.evil.test/image.png"),
                1.0,
                None,
            )
            .await
            .unwrap_err();
        assert!(subdomain_err
            .to_string()
            .contains("External data url not allowed"));

        let userinfo_err = converter
            .svg_to_png(
                &svg_with_href("https://example.com@evil.test/image.png"),
                1.0,
                None,
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

        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: false,
            filesystem_root: None,
            ..Default::default()
        })
        .unwrap();

        let err = converter
            .svg_to_png(&svg_with_href(&href), 1.0, None)
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

        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: false,
            filesystem_root: Some(root.clone()),
            ..Default::default()
        })
        .unwrap();

        let allowed = converter
            .svg_to_png(&svg_with_href("inside.png"), 1.0, None)
            .await;
        assert!(allowed.is_ok());

        let outside_href = Url::from_file_path(&outside_path).unwrap().to_string();
        let err = converter
            .svg_to_png(&svg_with_href(&outside_href), 1.0, None)
            .await
            .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("filesystem_root") || message.contains("access denied"));

        let err = converter
            .svg_to_png(&svg_with_href("../outside.png"), 1.0, None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("filesystem_root"));
    }

    #[tokio::test]
    async fn test_svg_helper_enforces_http_access_and_allowed_base_urls() {
        let remote_svg = svg_with_href("https://example.com/image.png");

        let no_http_converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: false,
            ..Default::default()
        })
        .unwrap();
        let err = no_http_converter
            .svg_to_png(&remote_svg, 1.0, None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("HTTP access denied"));

        let allowlisted_converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: Some(vec!["https://allowed.example/".to_string()]),
            ..Default::default()
        })
        .unwrap();
        let err = allowlisted_converter
            .svg_to_png(&remote_svg, 1.0, None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("External data url not allowed"));
    }

    #[tokio::test]
    async fn test_svg_helper_allows_data_uri_when_http_disabled() {
        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: false,
            ..Default::default()
        })
        .unwrap();
        let svg = svg_with_href(
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/w8AAgMBgLP4r9kAAAAASUVORK5CYII=",
        );
        let png = converter.svg_to_png(&svg, 1.0, None).await.unwrap();
        assert!(png.starts_with(&[137, 80, 78, 71]));
    }

    #[tokio::test]
    async fn test_vega_to_pdf_denies_http_access() {
        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: false,
            ..Default::default()
        })
        .unwrap();
        let spec = vega_spec_with_data_url("https://example.com/data.csv");

        let err = converter
            .vega_to_pdf(spec, VgOpts::default())
            .await
            .unwrap_err();
        let message = err.to_string().to_ascii_lowercase();
        assert!(
            message.contains("http access denied")
                || message.contains("requires net access")
                || message.contains("permission")
        );
    }

    #[tokio::test]
    async fn test_vega_loader_allows_data_uri_when_http_disabled() {
        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: false,
            ..Default::default()
        })
        .unwrap();
        let spec = vega_spec_with_data_url("data:text/csv,a,b%0A1,2");

        let svg = converter
            .vega_to_svg(spec, VgOpts::default())
            .await
            .unwrap();
        assert!(svg.contains("<svg"));
    }

    #[tokio::test]
    async fn test_vega_loader_denies_redirect_when_allowlist_configured() {
        let disallowed_server =
            TestHttpServer::new(vec![("/data.csv", TestHttpResponse::ok_text("a,b\n1,2\n"))]);
        let allowed_server = TestHttpServer::new(vec![(
            "/redirect.csv",
            TestHttpResponse::redirect(&disallowed_server.url("/data.csv")),
        )]);

        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: Some(vec![allowed_server.origin()]),
            ..Default::default()
        })
        .unwrap();
        let spec = vega_spec_with_data_url(&allowed_server.url("/redirect.csv"));

        let err = converter
            .vega_to_svg(spec, VgOpts::default())
            .await
            .unwrap_err();
        assert!(err
            .to_string()
            .contains("Redirected HTTP URLs are not allowed"));
    }

    #[tokio::test]
    async fn test_vega_loader_allows_redirect_when_allowlist_is_not_configured() {
        let target_server =
            TestHttpServer::new(vec![("/data.csv", TestHttpResponse::ok_text("a,b\n1,2\n"))]);
        let redirect_server = TestHttpServer::new(vec![(
            "/redirect.csv",
            TestHttpResponse::redirect(&target_server.url("/data.csv")),
        )]);

        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: None,
            ..Default::default()
        })
        .unwrap();

        let svg = converter
            .vega_to_svg(
                vega_spec_with_data_url(&redirect_server.url("/redirect.csv")),
                VgOpts::default(),
            )
            .await
            .unwrap();
        assert!(svg.contains("<svg"));
    }

    #[tokio::test]
    async fn test_vegalite_to_png_canvas_image_denies_http_access() {
        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: false,
            ..Default::default()
        })
        .unwrap();
        let spec = vegalite_spec_with_image_url("https://example.com/image.png");

        let err = converter
            .vegalite_to_png(
                spec,
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
                Some(1.0),
                Some(72.0),
            )
            .await
            .unwrap_err();
        assert!(err
            .to_string()
            .contains(&format!("{ACCESS_DENIED_MARKER}: HTTP access denied")));
    }

    #[tokio::test]
    async fn test_vegalite_to_png_canvas_image_enforces_allowed_base_urls() {
        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: Some(vec!["https://allowed.example/".to_string()]),
            ..Default::default()
        })
        .unwrap();
        let spec = vegalite_spec_with_image_url("https://example.com/image.png");

        let err = converter
            .vegalite_to_png(
                spec,
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
                Some(1.0),
                Some(72.0),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains(&format!(
            "{ACCESS_DENIED_MARKER}: External data url not allowed"
        )));
    }

    #[tokio::test]
    async fn test_vegalite_to_png_canvas_image_denies_redirect_when_allowlist_configured() {
        let disallowed_server = TestHttpServer::new(vec![(
            "/image.png",
            TestHttpResponse::ok_png(PNG_1X1_BYTES),
        )]);
        let allowed_server = TestHttpServer::new(vec![(
            "/redirect.png",
            TestHttpResponse::redirect(&disallowed_server.url("/image.png")),
        )]);

        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: Some(vec![allowed_server.origin()]),
            ..Default::default()
        })
        .unwrap();
        let spec = vegalite_spec_with_image_url(&allowed_server.url("/redirect.png"));

        let err = converter
            .vegalite_to_png(
                spec,
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
                Some(1.0),
                Some(72.0),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains(&format!(
            "{ACCESS_DENIED_MARKER}: Redirected HTTP URLs are not allowed"
        )));
    }

    #[tokio::test]
    async fn test_vegalite_to_png_canvas_image_allows_redirect_without_allowlist() {
        let target_server = TestHttpServer::new(vec![(
            "/image.svg",
            TestHttpResponse::ok_svg(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#,
            ),
        )]);
        let redirect_server = TestHttpServer::new(vec![(
            "/redirect.svg",
            TestHttpResponse::redirect(&target_server.url("/image.svg")),
        )]);

        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: None,
            ..Default::default()
        })
        .unwrap();

        let png = converter
            .vegalite_to_png(
                vegalite_spec_with_image_url(&redirect_server.url("/redirect.svg")),
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
                Some(1.0),
                Some(72.0),
            )
            .await
            .unwrap();
        assert!(png.starts_with(&[137, 80, 78, 71]));
    }

    #[tokio::test]
    async fn test_svg_helper_denies_redirect_when_allowlist_configured() {
        let disallowed_server = TestHttpServer::new(vec![(
            "/image.png",
            TestHttpResponse::ok_png(PNG_1X1_BYTES),
        )]);
        let allowed_server = TestHttpServer::new(vec![(
            "/redirect.png",
            TestHttpResponse::redirect(&disallowed_server.url("/image.png")),
        )]);

        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: Some(vec![allowed_server.base_url()]),
            ..Default::default()
        })
        .unwrap();

        let err = converter
            .svg_to_png(
                &svg_with_href(&allowed_server.url("/redirect.png")),
                1.0,
                None,
            )
            .await
            .unwrap_err();
        assert!(err
            .to_string()
            .contains("Redirected HTTP URLs are not allowed"));
    }

    #[tokio::test]
    async fn test_svg_helper_allows_redirect_without_allowlist() {
        let target_server = TestHttpServer::new(vec![(
            "/image.svg",
            TestHttpResponse::ok_svg(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="blue"/></svg>"#,
            ),
        )]);
        let redirect_server = TestHttpServer::new(vec![(
            "/redirect.svg",
            TestHttpResponse::redirect(&target_server.url("/image.svg")),
        )]);

        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: None,
            ..Default::default()
        })
        .unwrap();

        let png = converter
            .svg_to_png(
                &svg_with_href(&redirect_server.url("/redirect.svg")),
                1.0,
                None,
            )
            .await
            .unwrap();
        assert!(png.starts_with(&[137, 80, 78, 71]));
    }

    #[tokio::test]
    async fn test_vega_to_pdf_denies_disallowed_base_url() {
        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: Some(vec!["https://allowed.example/".to_string()]),
            ..Default::default()
        })
        .unwrap();
        let spec = vega_spec_with_data_url("https://example.com/data.csv");

        let err = converter
            .vega_to_pdf(spec, VgOpts::default())
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

        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: false,
            filesystem_root: Some(root),
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

        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: false,
            filesystem_root: Some(root),
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
            )
            .await
            .unwrap_err();
        assert!(err
            .to_string()
            .contains(&format!("{ACCESS_DENIED_MARKER}: Filesystem access denied")));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_vegalite_to_pdf_uses_per_request_allowlist_for_svg_rasterization() {
        let server = TestHttpServer::new(vec![(
            "/image.svg",
            TestHttpResponse::ok_svg(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#,
            ),
        )]);
        let converter = VlConverter::with_config(VlConverterConfig {
            allow_http_access: true,
            allowed_base_urls: Some(vec!["https://blocked.example/".to_string()]),
            ..Default::default()
        })
        .unwrap();

        let pdf = converter
            .vegalite_to_pdf(
                vegalite_spec_with_image_url(&server.url("/image.svg")),
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    allowed_base_urls: Some(vec![server.origin()]),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(pdf.starts_with(b"%PDF"));
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
            true,
            Renderer::Svg,
        ));
        assert_send_future(converter.vega_to_html(vg_spec, VgOpts::default(), true, Renderer::Svg));
    }

    #[tokio::test]
    async fn test_get_vegaembed_bundle_caches_result() {
        let converter = VlConverter::with_config(VlConverterConfig {
            num_workers: 1,
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
        let converter = VlConverter::with_config(VlConverterConfig {
            num_workers: 1,
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

    #[test]
    fn test_worker_pool_next_sender_balances_outstanding_reservations() {
        let mut senders = Vec::new();
        let mut _receivers = Vec::new();
        for _ in 0..3 {
            let (tx, rx) = tokio::sync::mpsc::channel::<QueuedCommand>(1);
            senders.push(tx);
            _receivers.push(rx);
        }

        let pool = WorkerPool {
            senders,
            outstanding: (0..3)
                .map(|_| std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)))
                .collect(),
            dispatch_cursor: std::sync::atomic::AtomicUsize::new(0),
            _handles: Vec::new(),
        };

        let mut tickets = Vec::new();
        for _ in 0..30 {
            let (_, ticket) = pool
                .next_sender()
                .expect("pool with open senders should produce a sender");
            tickets.push(ticket);

            let loads: Vec<usize> = pool
                .outstanding
                .iter()
                .map(|outstanding| outstanding.load(std::sync::atomic::Ordering::Relaxed))
                .collect();
            let min = *loads.iter().min().expect("loads should not be empty");
            let max = *loads.iter().max().expect("loads should not be empty");
            assert!(
                max - min <= 1,
                "expected balanced outstanding counts, got {loads:?}"
            );
        }

        drop(tickets);
        for outstanding in &pool.outstanding {
            assert_eq!(outstanding.load(std::sync::atomic::Ordering::Relaxed), 0);
        }
    }

    #[test]
    fn test_worker_pool_next_sender_skips_closed_senders() {
        let (closed_sender, closed_receiver) = tokio::sync::mpsc::channel::<QueuedCommand>(1);
        drop(closed_receiver);

        let (open_sender, mut open_receiver) = tokio::sync::mpsc::channel::<QueuedCommand>(1);

        let pool = WorkerPool {
            senders: vec![closed_sender, open_sender],
            outstanding: (0..2)
                .map(|_| std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)))
                .collect(),
            dispatch_cursor: std::sync::atomic::AtomicUsize::new(0),
            _handles: Vec::new(),
        };

        for _ in 0..4 {
            let (sender, ticket) = pool
                .next_sender()
                .expect("pool should return the open sender");
            sender
                .try_send(QueuedCommand::new(make_test_command(), ticket))
                .expect("dispatch should use open sender, not closed sender");
            let queued = open_receiver
                .try_recv()
                .expect("open receiver should receive dispatched command");
            drop(queued);
        }
    }

    #[tokio::test]
    async fn test_worker_pool_cancellation_releases_outstanding_ticket() {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<QueuedCommand>(1);
        let pool = WorkerPool {
            senders: vec![sender],
            outstanding: vec![std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0))],
            dispatch_cursor: std::sync::atomic::AtomicUsize::new(0),
            _handles: Vec::new(),
        };

        let (sender, ticket) = pool.next_sender().unwrap();
        sender
            .send(QueuedCommand::new(make_test_command(), ticket))
            .await
            .unwrap();
        assert_eq!(
            pool.outstanding[0].load(std::sync::atomic::Ordering::Relaxed),
            1
        );

        let (sender, ticket) = pool.next_sender().unwrap();
        let blocked_send = tokio::spawn(async move {
            sender
                .send(QueuedCommand::new(make_test_command(), ticket))
                .await
        });
        tokio::task::yield_now().await;

        assert_eq!(
            pool.outstanding[0].load(std::sync::atomic::Ordering::Relaxed),
            2
        );

        blocked_send.abort();
        let _ = blocked_send.await;
        tokio::task::yield_now().await;

        assert_eq!(
            pool.outstanding[0].load(std::sync::atomic::Ordering::Relaxed),
            1
        );

        let queued = receiver
            .recv()
            .await
            .expect("first queued command should still be in the channel");
        drop(queued);

        assert_eq!(
            pool.outstanding[0].load(std::sync::atomic::Ordering::Relaxed),
            0
        );
    }

    #[test]
    fn test_get_or_spawn_sender_respawns_closed_pool_without_explicit_reset() {
        let num_workers = 2;
        let converter = VlConverter::with_config(VlConverterConfig {
            num_workers,
            ..Default::default()
        })
        .unwrap();

        let mut closed_senders = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            let (sender, receiver) = tokio::sync::mpsc::channel::<QueuedCommand>(1);
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
        let converter = VlConverter::with_config(VlConverterConfig {
            num_workers: 2,
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
        let converter = VlConverter::with_config(VlConverterConfig {
            num_workers: 2,
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

        let svg = converter
            .vegalite_to_svg(
                vl_spec,
                VlOpts {
                    vl_version: VlVersion::v5_16,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(svg.trim_start().starts_with("<svg"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_parallel_conversions_with_shared_converter() {
        let converter = VlConverter::with_config(VlConverterConfig {
            num_workers: 4,
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

    #[test]
    fn test_scenegraph_google_probe_candidates_skip_explicit_and_preregistered() {
        let families = BTreeSet::from([
            "Alpha".to_string(),
            "Bravo".to_string(),
            "Charlie".to_string(),
        ]);
        let explicit = HashSet::from(["Bravo".to_string()]);
        let pre_registered = HashSet::from(["Charlie".to_string()]);

        let candidates = scenegraph_google_probe_candidates(&families, &explicit, &pre_registered);

        assert_eq!(candidates, BTreeSet::from(["Alpha".to_string()]));
    }

    #[tokio::test]
    async fn test_classify_scenegraph_fonts_uses_case_insensitive_local_match() {
        let available = available_font_families().unwrap();
        let family = available
            .iter()
            .find(|name| name.chars().any(|c| c.is_ascii_alphabetic()))
            .cloned()
            .expect("expected at least one alphabetic font family in fontdb");
        let alt_family = if family.to_ascii_uppercase() != family {
            family.to_ascii_uppercase()
        } else {
            family.to_ascii_lowercase()
        };

        assert_ne!(
            family, alt_family,
            "test requires a case-changed family name"
        );
        assert!(is_available(&alt_family, &available));

        let families = BTreeSet::from([alt_family.clone()]);
        let result = classify_scenegraph_fonts(
            &families,
            false,
            true,
            MissingFontsPolicy::Fallback,
            &HashSet::new(),
        )
        .await
        .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].family, alt_family);
        assert!(matches!(result[0].source, FontSource::Local));
    }
}

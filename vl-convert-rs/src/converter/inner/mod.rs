mod conversions;
mod fonts;
mod init;
mod rendering;
mod runtime;

use crate::deno_stubs::{NoOpInNpmPackageChecker, NoOpNpmPackageFolderResolver, VlConvertNodeSys};
use crate::module_loader::import_map::VlVersion;
use crate::module_loader::VlConvertModuleLoader;
use crate::text::build_usvg_options_with_fontdb;
use deno_core::error::AnyError;
use deno_core::{v8, ModuleSpecifier};
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_permissions::{PermissionsContainer, RuntimePermissionDescriptorParser};
use deno_runtime::deno_web::{BlobStore, InMemoryBroadcastChannel};
use deno_runtime::worker::{MainWorker, WorkerOptions, WorkerServiceOptions};
use deno_runtime::FeatureChecker;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use super::config::{ConverterContext, ResolvedPlugin, VlcConfig};
use super::fonts::WorkerFontState;
use super::permissions::build_permissions;
use super::transfer::{WorkerTransferState, WorkerTransferStateHandle};
use super::types::LogEntry;

use std::collections::HashMap;
use std::sync::Mutex;

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
    /// `max_v8_heap_size_mb` is `None` (no cap).
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
        let create_params = ctx.config.max_v8_heap_size_mb.map(|n| {
            let max_bytes: usize = n
                .get()
                .saturating_mul(1024 * 1024)
                .try_into()
                .unwrap_or(usize::MAX);
            v8::CreateParams::default().heap_limits(0, max_bytes)
        });

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
        let heap_limit_data = if ctx.config.max_v8_heap_size_mb.is_some() {
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

        // Store data access policy for the Rust data loading ops.
        // The ConverterContext tracks `parsed_allowed_base_urls` as a
        // `Vec<...>` (empty = block all), so we always wrap in `Some(...)`
        // to engage the allowlist enforcer.
        let data_policy = crate::data_ops::DataAccessPolicy {
            allowed_base_urls: Some(ctx.parsed_allowed_base_urls.clone()),
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
}

pub(crate) struct VlConverterInner {
    pub(super) vegaembed_bundles: Mutex<HashMap<VlVersion, String>>,
    pub(super) pool: Mutex<Option<super::worker_pool::WorkerPool>>,
    pub(crate) config: Arc<VlcConfig>,
    /// Resolved plugins populated when the worker pool is first spawned.
    /// Separate from config because spawn_worker_pool() creates a new Arc
    /// but VlConverterInner.config is set at with_config() time.
    /// Empty = no plugins resolved yet / no plugins configured.
    pub(crate) resolved_plugins: Mutex<Vec<ResolvedPlugin>>,
    /// Semaphore limiting concurrent ephemeral workers for per-request plugins.
    /// None when max_ephemeral_workers is None (no limit).
    pub(super) ephemeral_semaphore: Option<Arc<tokio::sync::Semaphore>>,
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::converter::config::VlcConfig;
    use crate::text::get_font_baseline_snapshot;
    use serde_json::json;

    use super::super::types::VlOpts;

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

        let _ = stream.shutdown(std::net::Shutdown::Write);
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
        let mut drain_buf = [0u8; 256];
        loop {
            use std::io::Read;
            match stream.read(&mut drain_buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => continue,
            }
        }
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
                parsed_allowed_base_urls: Vec::new(),
                resolved_plugins: Vec::new(),
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
                parsed_allowed_base_urls: Vec::new(),
                resolved_plugins: Vec::new(),
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
                parsed_allowed_base_urls: Vec::new(),
                resolved_plugins: Vec::new(),
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
        // Use allowed_base_urls: vec![] to deny all HTTP access via ops
        let mut ctx = InnerVlConverter::try_new(
            std::sync::Arc::new(ConverterContext {
                config: VlcConfig {
                    allowed_base_urls: vec![],
                    ..Default::default()
                },
                parsed_allowed_base_urls: Vec::new(),
                resolved_plugins: Vec::new(),
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
                parsed_allowed_base_urls: Vec::new(),
                resolved_plugins: Vec::new(),
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
                parsed_allowed_base_urls: Vec::new(),
                resolved_plugins: Vec::new(),
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

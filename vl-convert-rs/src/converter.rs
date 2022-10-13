use crate::module_loader::import_map::{url_for_path, vega_url, VlVersion};
use crate::module_loader::VlConvertModuleLoader;
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use deno_runtime::deno_core::anyhow::bail;
use deno_runtime::deno_core::error::AnyError;
use deno_runtime::deno_core::{serde_v8, v8, Extension};

use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_core;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::Permissions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::BootstrapOptions;

use std::thread;
use std::thread::JoinHandle;

use futures::channel::{mpsc, mpsc::Sender, oneshot};
use futures_util::{SinkExt, StreamExt};

use crate::text::{op_text_width, USVG_OPTIONS};

lazy_static! {
    pub static ref TOKIO_RUNTIME: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
}

fn get_error_class_name(e: &AnyError) -> &'static str {
    deno_runtime::errors::get_error_class_name(e).unwrap_or("Error")
}

/// Struct that interacts directly with the Deno JavaScript runtime. Not Sendable
struct InnerVlConverter {
    worker: MainWorker,
    initialized_vl_versions: HashSet<VlVersion>,
    vega_initialized: bool,
    module_loader: Rc<VlConvertModuleLoader>,
}

impl InnerVlConverter {
    async fn init_vega(&mut self) -> Result<(), AnyError> {
        if !self.vega_initialized {
            let import_str = format!(
                r#"
var vega;
import('{vega_url}').then((imported) => {{
    vega = imported;
}})"#,
                vega_url = vega_url()
            );

            self.worker.execute_script("<anon>", &import_str)?;
            self.worker.run_event_loop(false).await?;

            // Override text width measurement in vega-scenegraph
            for path in self.module_loader.import_map.keys() {
                if path.ends_with("vega-scenegraph.js") {
                    let script = format!(
                        r#"
import('{url}').then((sg) => {{
    sg.textMetrics.width = (item, text) => {{
        let style = item.fontStyle;
        let variant = item.fontVariant;
        let weight = item.fontWeight;
        let size = sg.fontSize(item);
        let family = sg.fontFamily(item);

        let text_info = JSON.stringify({{
            style, variant, weight, size, family, text
        }}, null, 2);

        return Deno.core.ops.op_text_width(text_info)
    }};
}})
"#,
                        url = url_for_path(path)
                    );
                    self.worker.execute_script("<anon>", &script)?;
                    self.worker.run_event_loop(false).await?;
                }
            }

            // Create and initialize svg function string
            let function_str = r#"
function vegaToSvg(vgSpec) {
    let runtime = vega.parse(vgSpec);
    let view = new vega.View(runtime, {renderer: 'none'});
    let svgPromise = view.toSVG();
    return svgPromise
}
"#;

            self.worker.execute_script("<anon>", function_str)?;
            self.worker.run_event_loop(false).await?;

            self.vega_initialized = true;
        }

        Ok(())
    }

    async fn init_vl_version(&mut self, vl_version: &VlVersion) -> Result<(), AnyError> {
        if !self.initialized_vl_versions.contains(vl_version) {
            // Create and evaluate import string
            let import_str = format!(
                r#"
var {ver_name};
import('{vl_url}').then((imported) => {{
    {ver_name} = imported;
}})
"#,
                ver_name = format!("{:?}", vl_version),
                vl_url = vl_version.to_url()
            );

            self.worker.execute_script("<anon>", &import_str)?;

            self.worker.run_event_loop(false).await?;

            // Create and initialize function string
            let function_str = format!(
                r#"
function compileVegaLite_{ver_name}(vlSpec, pretty) {{
    let options = {{}};
    let vgSpec = {ver_name}.compile(vlSpec, options).spec;
    if (pretty) {{
        return JSON.stringify(vgSpec, null, 2)
    }} else {{
        return JSON.stringify(vgSpec)
    }}
}}

function vegaLiteToSvg_{ver_name}(vlSpec) {{
    let options = {{}};
    let vgSpec = {ver_name}.compile(vlSpec, options).spec;
    return vegaToSvg(vgSpec)
}}
"#,
                ver_name = format!("{:?}", vl_version),
            );

            self.worker.execute_script("<anon>", &function_str)?;

            self.worker.run_event_loop(false).await?;

            // Register that this Vega-Lite version has been initialized
            self.initialized_vl_versions.insert(*vl_version);
        }
        Ok(())
    }

    pub async fn try_new() -> Result<Self, AnyError> {
        let module_loader = Rc::new(VlConvertModuleLoader::new());

        let ext = Extension::builder()
            .ops(vec![
                // Op to measure text width with resvg
                op_text_width::decl(),
            ])
            .build();

        let create_web_worker_cb = Arc::new(|_| {
            todo!("Web workers are not supported");
        });
        let web_worker_event_cb = Arc::new(|_| {
            todo!("Web workers are not supported");
        });

        let options = WorkerOptions {
            bootstrap: BootstrapOptions {
                args: vec![],
                cpu_count: 1,
                debug_flag: false,
                enable_testing_features: false,
                location: None,
                no_color: false,
                is_tty: false,
                runtime_version: "x".to_string(),
                ts_version: "x".to_string(),
                unstable: false,
                user_agent: "hello_runtime".to_string(),
                inspect: false,
            },
            extensions: vec![ext],
            unsafely_ignore_certificate_errors: None,
            root_cert_store: None,
            seed: None,
            source_map_getter: None,
            format_js_error_fn: None,
            web_worker_preload_module_cb: web_worker_event_cb.clone(),
            web_worker_pre_execute_module_cb: web_worker_event_cb,
            create_web_worker_cb,
            maybe_inspector_server: None,
            should_break_on_first_statement: false,
            module_loader: module_loader.clone(),
            npm_resolver: None,
            get_error_class_fn: Some(&get_error_class_name),
            cache_storage_dir: None,
            origin_storage_dir: None,
            blob_store: BlobStore::default(),
            broadcast_channel: InMemoryBroadcastChannel::default(),
            shared_array_buffer_store: None,
            compiled_wasm_module_store: None,
            stdio: Default::default(),
        };

        let js_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("vl-convert-rs.js");
        let main_module = deno_core::resolve_path(&js_path.to_string_lossy())?;
        let permissions = Permissions::allow_all();

        let mut worker =
            MainWorker::bootstrap_from_options(main_module.clone(), permissions, options);
        worker.execute_main_module(&main_module).await?;
        worker.run_event_loop(false).await?;

        let this = Self {
            worker,
            initialized_vl_versions: Default::default(),
            vega_initialized: false,
            module_loader,
        };

        Ok(this)
    }

    async fn execute_script_to_string(&mut self, script: &str) -> Result<String, AnyError> {
        let res = self.worker.js_runtime.execute_script("<anon>", script)?;

        self.worker.run_event_loop(false).await?;

        let scope = &mut self.worker.js_runtime.handle_scope();
        let local = v8::Local::new(scope, res);

        // Deserialize a `v8` object into a Rust type using `serde_v8`,
        // in this case deserialize to a JSON `Value`.
        let deserialized_value = serde_v8::from_v8::<serde_json::Value>(scope, local);

        let value = match deserialized_value {
            Ok(value) => {
                let value = value.as_str();
                value.unwrap().to_string()
            }
            Err(err) => bail!("{}", err.to_string()),
        };

        Ok(value)
    }

    pub async fn vegalite_to_vega(
        &mut self,
        vl_spec: &serde_json::Value,
        vl_version: VlVersion,
        pretty: bool,
    ) -> Result<String, AnyError> {
        self.init_vl_version(&vl_version).await?;

        let vl_spec_str = serde_json::to_string(vl_spec)?;
        let code = format!(
            r#"
compileVegaLite_{ver_name:?}(
    {vl_spec_str},
    {pretty}
)
"#,
            ver_name = vl_version,
            vl_spec_str = vl_spec_str,
            pretty = pretty,
        );

        let value = self.execute_script_to_string(&code).await?;
        Ok(value)
    }

    pub async fn vegalite_to_svg(
        &mut self,
        vl_spec: &serde_json::Value,
        vl_version: VlVersion,
    ) -> Result<String, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_version).await?;

        let vl_spec_str = serde_json::to_string(vl_spec)?;

        let code = format!(
            r#"
var svg;
vegaLiteToSvg_{ver_name:?}(
    {vl_spec_str}
).then((result) => {{
    svg = result;
}});
"#,
            ver_name = vl_version,
            vl_spec_str = vl_spec_str,
        );
        self.worker.execute_script("<anon>", &code)?;
        self.worker.run_event_loop(false).await?;

        let value = self.execute_script_to_string("svg").await?;
        Ok(value)
    }

    pub async fn vega_to_svg(&mut self, vg_spec: &serde_json::Value) -> Result<String, AnyError> {
        self.init_vega().await?;

        let vg_spec_str = serde_json::to_string(vg_spec)?;
        let code = format!(
            r#"
var svg;
vegaToSvg(
    {vg_spec_str}
).then((result) => {{
    svg = result;
}})
"#,
            vg_spec_str = vg_spec_str,
        );
        self.worker.execute_script("<anon>", &code)?;
        self.worker.run_event_loop(false).await?;

        let value = self.execute_script_to_string("svg").await?;
        Ok(value)
    }
}

pub enum VlConvertCommand {
    VlToVg {
        vl_spec: serde_json::Value,
        vl_version: VlVersion,
        pretty: bool,
        responder: oneshot::Sender<Result<String, AnyError>>,
    },
    VgToSvg {
        vg_spec: serde_json::Value,
        responder: oneshot::Sender<Result<String, AnyError>>,
    },
    VlToSvg {
        vl_spec: serde_json::Value,
        vl_version: VlVersion,
        responder: oneshot::Sender<Result<String, AnyError>>,
    },
}

/// Struct for performing Vega-Lite to Vega conversions using the Deno v8 Runtime
///
/// # Examples
///
/// ```
/// use vl_convert_rs::{VlConverter, VlVersion};
/// let mut converter = VlConverter::new();
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
///         converter.vegalite_to_vega(vl_spec, VlVersion::v5_5, true)
///     ).expect(
///         "Failed to perform Vega-Lite to Vega conversion"
///     );
///
///     println!("{}", vega_spec)
/// ```
#[derive(Clone)]
pub struct VlConverter {
    sender: Sender<VlConvertCommand>,
    _handle: Arc<JoinHandle<Result<(), AnyError>>>,
}

impl VlConverter {
    pub fn new() -> Self {
        let (sender, mut receiver) = mpsc::channel::<VlConvertCommand>(32);

        let handle = Arc::new(thread::spawn(move || {
            let mut inner = TOKIO_RUNTIME.block_on(InnerVlConverter::try_new())?;

            while let Some(cmd) = TOKIO_RUNTIME.block_on(receiver.next()) {
                match cmd {
                    VlConvertCommand::VlToVg {
                        vl_spec,
                        vl_version,
                        pretty,
                        responder,
                    } => {
                        let vega_spec = TOKIO_RUNTIME
                            .block_on(inner.vegalite_to_vega(&vl_spec, vl_version, pretty));
                        responder.send(vega_spec).ok();
                    }
                    VlConvertCommand::VgToSvg { vg_spec, responder } => {
                        let svg_result = TOKIO_RUNTIME.block_on(inner.vega_to_svg(&vg_spec));
                        responder.send(svg_result).ok();
                    }
                    VlConvertCommand::VlToSvg {
                        vl_spec,
                        vl_version,
                        responder,
                    } => {
                        let svg_result =
                            TOKIO_RUNTIME.block_on(inner.vegalite_to_svg(&vl_spec, vl_version));
                        responder.send(svg_result).ok();
                    }
                }
            }
            Ok(())
        }));

        Self {
            sender,
            _handle: handle,
        }
    }

    pub async fn vegalite_to_vega(
        &mut self,
        vl_spec: serde_json::Value,
        vl_version: VlVersion,
        pretty: bool,
    ) -> Result<String, AnyError> {
        let (resp_tx, resp_rx) = oneshot::channel::<Result<String, AnyError>>();
        let cmd = VlConvertCommand::VlToVg {
            vl_spec,
            vl_version,
            pretty,
            responder: resp_tx,
        };

        // Send request
        match self.sender.send(cmd).await {
            Ok(_) => {
                // All good
            }
            Err(err) => {
                bail!("Failed to send conversion request: {}", err.to_string())
            }
        }

        // Wait for result
        match resp_rx.await {
            Ok(vega_spec_result) => vega_spec_result,
            Err(err) => bail!("Failed to retrieve conversion result: {}", err.to_string()),
        }
    }

    pub async fn vega_to_svg(&mut self, vg_spec: serde_json::Value) -> Result<String, AnyError> {
        let (resp_tx, resp_rx) = oneshot::channel::<Result<String, AnyError>>();
        let cmd = VlConvertCommand::VgToSvg {
            vg_spec,
            responder: resp_tx,
        };

        // Send request
        match self.sender.send(cmd).await {
            Ok(_) => {
                // All good
            }
            Err(err) => {
                bail!("Failed to send SVG conversion request: {}", err.to_string())
            }
        }

        // Wait for result
        match resp_rx.await {
            Ok(svg_result) => svg_result,
            Err(err) => bail!("Failed to retrieve conversion result: {}", err.to_string()),
        }
    }

    pub async fn vegalite_to_svg(
        &mut self,
        vl_spec: serde_json::Value,
        vl_version: VlVersion,
    ) -> Result<String, AnyError> {
        let (resp_tx, resp_rx) = oneshot::channel::<Result<String, AnyError>>();
        let cmd = VlConvertCommand::VlToSvg {
            vl_spec,
            vl_version,
            responder: resp_tx,
        };

        // Send request
        match self.sender.send(cmd).await {
            Ok(_) => {
                // All good
            }
            Err(err) => {
                bail!("Failed to send SVG conversion request: {}", err.to_string())
            }
        }

        // Wait for result
        match resp_rx.await {
            Ok(svg_result) => svg_result,
            Err(err) => bail!("Failed to retrieve conversion result: {}", err.to_string()),
        }
    }

    pub async fn vega_to_png(
        &mut self,
        vg_spec: serde_json::Value,
        scale: Option<f32>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let svg = self.vega_to_svg(vg_spec).await?;
        Self::svg_to_png(&svg, scale)
    }

    pub async fn vegalite_to_png(
        &mut self,
        vl_spec: serde_json::Value,
        vl_version: VlVersion,
        scale: Option<f32>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let svg = self.vegalite_to_svg(vl_spec, vl_version).await?;
        Self::svg_to_png(&svg, scale)
    }

    fn svg_to_png(svg: &str, scale: f32) -> Result<Vec<u8>, AnyError> {
        let rtree = match usvg::Tree::from_str(svg, &USVG_OPTIONS.to_ref()) {
            Ok(rtree) => rtree,
            Err(err) => {
                bail!("Failed to parse SVG string: {}", err.to_string())
            }
        };

        let pixmap_size = rtree.svg_node().size.to_screen_size();
        let mut pixmap = tiny_skia::Pixmap::new(
            (pixmap_size.width() as f32 * scale) as u32,
            (pixmap_size.height() as f32 * scale) as u32,
        )
        .unwrap();
        resvg::render(
            &rtree,
            usvg::FitTo::Zoom(scale),
            tiny_skia::Transform::default(),
            pixmap.as_mut(),
        )
        .unwrap();

        match pixmap.encode_png() {
            Ok(png_data) => Ok(png_data),
            Err(err) => {
                bail!("Failed to encode PNG: {}", err.to_string())
            }
        }
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

    #[tokio::test]
    async fn test_convert_context() {
        let mut ctx = VlConverter::new();
        let vl_spec: serde_json::Value = serde_json::from_str(r##"
{
    "data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
    "mark": "bar",
    "encoding": {
        "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
        "y": {"aggregate": "mean", "field": "precipitation"}
    }
}
        "##).unwrap();

        let vg_spec = ctx
            .vegalite_to_vega(vl_spec, VlVersion::v4_17, true)
            .await
            .unwrap();
        println!("vg_spec: {}", vg_spec)
    }

    #[tokio::test]
    async fn test_multi_convert_context() {
        let vl_spec: serde_json::Value = serde_json::from_str(r##"
{
    "data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
    "mark": "bar",
    "encoding": {
        "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
        "y": {"aggregate": "mean", "field": "precipitation"}
    }
}
        "##).unwrap();

        let mut ctx1 = VlConverter::new();
        let vg_spec1 = ctx1
            .vegalite_to_vega(vl_spec.clone(), VlVersion::v4_17, true)
            .await
            .unwrap();
        println!("vg_spec1: {}", vg_spec1);

        let mut ctx1 = VlConverter::new();
        let vg_spec2 = ctx1
            .vegalite_to_vega(vl_spec, VlVersion::v5_5, true)
            .await
            .unwrap();
        println!("vg_spec2: {}", vg_spec2);
    }
}

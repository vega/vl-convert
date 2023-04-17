use crate::module_loader::import_map::{url_for_path, vega_themes_url, vega_url, VlVersion};
use crate::module_loader::VlConvertModuleLoader;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use deno_runtime::deno_core::anyhow::bail;
use deno_runtime::deno_core::error::AnyError;
use deno_runtime::deno_core::{serde_v8, v8, Extension};

use deno_core::op;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_core;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::{Permissions, PermissionsContainer};
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;

use std::thread;
use std::thread::JoinHandle;

use crate::anyhow::anyhow;
use futures::channel::{mpsc, mpsc::Sender, oneshot};
use futures_util::{SinkExt, StreamExt};
use resvg::FitTo;
use usvg::{TreeParsing, TreeTextToPath};

use crate::text::{op_text_width, FONT_DB, USVG_OPTIONS};

lazy_static! {
    pub static ref TOKIO_RUNTIME: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
    static ref JSON_ARGS: Arc<Mutex<HashMap<i32, String>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref NEXT_ARG_ID: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
}

#[derive(Debug, Clone, Default)]
pub struct VlOpts {
    pub config: Option<serde_json::Value>,
    pub theme: Option<String>,
    pub vl_version: VlVersion,
}

fn set_json_arg(arg: serde_json::Value) -> Result<i32, AnyError> {
    // Increment arg id
    let id = match NEXT_ARG_ID.lock() {
        Ok(mut guard) => {
            let id = *guard;
            *guard = (*guard + 1) % i32::MAX;
            id
        }
        Err(err) => {
            bail!("Failed to acquire lock: {}", err.to_string())
        }
    };

    // Add Arg at id to args
    match JSON_ARGS.lock() {
        Ok(mut guard) => {
            guard.insert(id, serde_json::to_string(&arg).unwrap());
        }
        Err(err) => {
            bail!("Failed to acquire lock: {}", err.to_string())
        }
    }

    Ok(id)
}

#[op]
fn op_get_json_arg(arg_id: i32) -> Result<String, AnyError> {
    match JSON_ARGS.lock() {
        Ok(mut guard) => {
            if let Some(arg) = guard.remove(&arg_id) {
                Ok(arg)
            } else {
                bail!("Arg id not found")
            }
        }
        Err(err) => {
            bail!("Failed to acquire lock: {}", err.to_string())
        }
    }
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
"#,
                vega_url = vega_url(),
                vega_themes_url = vega_themes_url(),
            );

            self.worker.execute_script("<anon>", &import_code)?;
            self.worker.run_event_loop(false).await?;

            // Override text width measurement in vega-scenegraph
            for path in self.module_loader.import_map.keys() {
                if path.ends_with("vega-scenegraph.js") {
                    let script_code = format!(
                        r#"
import('{url}').then((sg) => {{
    sg.textMetrics.width = (item, text) => {{
        let style = item.fontStyle;
        let variant = item.fontVariant;

        // weight may be string like "bold" or number like 600.
        // Convert number form to string
        let weight = String(item.fontWeight);
        let size = sg.fontSize(item);
        let family = sg.fontFamily(item);

        let text_info = JSON.stringify({{
            style, variant, weight, size, family, text
        }}, null, 2);

        return Deno[Deno.internal].core.ops.op_text_width(text_info)
    }};
}})
"#,
                        url = url_for_path(path)
                    );
                    self.worker.execute_script("<anon>", &script_code)?;
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

            self.worker.execute_script("<anon>", &import_code)?;

            self.worker.run_event_loop(false).await?;

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

    return {ver_name}.compile(vlSpec, options).spec
}}

function vegaLiteToSvg_{ver_name}(vlSpec, config, theme) {{
    let options = {{}};

    // Handle config and theme
    let usermetaTheme = ((vlSpec.usermeta ?? {{}}).embedOptions ?? {{}}).theme;
    let namedTheme = theme ?? usermetaTheme;
    if (namedTheme != null) {{
        options["config"] = vega.mergeConfig(vegaThemes[namedTheme], config ?? {{}});
    }} else if (config != null) {{
        options["config"] = config;
    }}

    let vgSpec = {ver_name}.compile(vlSpec, options).spec;
    return vegaToSvg(vgSpec)
}}
"#,
                ver_name = format!("{:?}", vl_version),
            );

            self.worker.execute_script("<anon>", &function_code)?;

            self.worker.run_event_loop(false).await?;

            // Register that this Vega-Lite version has been initialized
            self.initialized_vl_versions.insert(*vl_version);
        }
        Ok(())
    }

    pub async fn try_new() -> Result<Self, AnyError> {
        let module_loader = Rc::new(VlConvertModuleLoader::new());

        let ext = Extension::builder("vl_convert_extensions")
            .ops(vec![
                // Op to measure text width with resvg
                op_text_width::decl(),
                op_get_json_arg::decl(),
            ])
            .build();

        let create_web_worker_cb = Arc::new(|_| {
            todo!("Web workers are not supported");
        });
        let web_worker_event_cb = Arc::new(|_| {
            todo!("Web workers are not supported");
        });

        let options = WorkerOptions {
            bootstrap: Default::default(),
            extensions: vec![ext],
            extensions_with_js: vec![],
            startup_snapshot: None,
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
            should_wait_for_inspector_session: false,
        };

        let js_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("vl-convert-rs.js");
        let main_module = deno_core::resolve_path(&js_path.to_string_lossy())?;
        let permissions = PermissionsContainer::new(Permissions::allow_all());

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

    async fn execute_script_to_json(
        &mut self,
        script: &str,
    ) -> Result<serde_json::Value, AnyError> {
        let res = self.worker.js_runtime.execute_script("<anon>", script)?;

        self.worker.run_event_loop(false).await?;

        let scope = &mut self.worker.js_runtime.handle_scope();
        let local = v8::Local::new(scope, res);

        // Deserialize a `v8` object into a Rust type using `serde_v8`,
        // in this case deserialize to a JSON `Value`.
        let deserialized_value = serde_v8::from_v8::<serde_json::Value>(scope, local);
        deserialized_value.map_err(|err| {
            anyhow!(
                "Failed to deserialize JavaScript value: {}",
                err.to_string()
            )
        })
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
        vl_opts: VlOpts,
    ) -> Result<serde_json::Value, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_opts.vl_version).await?;
        let config = vl_opts.config.clone().unwrap_or(serde_json::Value::Null);

        let spec_arg_id = set_json_arg(vl_spec.clone())?;
        let config_arg_id = set_json_arg(config)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let code = format!(
            r#"
compileVegaLite_{ver_name:?}(
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({spec_arg_id})),
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({config_arg_id})),
    {theme_arg}
)
"#,
            ver_name = vl_opts.vl_version,
            spec_arg_id = spec_arg_id,
            config_arg_id = config_arg_id,
            theme_arg = theme_arg,
        );

        let value = self.execute_script_to_json(&code).await?;
        Ok(value)
    }

    pub async fn vegalite_to_svg(
        &mut self,
        vl_spec: &serde_json::Value,
        vl_opts: VlOpts,
    ) -> Result<String, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_opts.vl_version).await?;

        let config = vl_opts.config.clone().unwrap_or(serde_json::Value::Null);
        let spec_arg_id = set_json_arg(vl_spec.clone())?;
        let config_arg_id = set_json_arg(config)?;
        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let code = format!(
            r#"
var svg;
vegaLiteToSvg_{ver_name:?}(
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({spec_arg_id})),
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({config_arg_id})),
    {theme_arg}
).then((result) => {{
    svg = result;
}});
"#,
            ver_name = vl_opts.vl_version,
            spec_arg_id = spec_arg_id,
            config_arg_id = config_arg_id,
            theme_arg = theme_arg,
        );
        self.worker.execute_script("<anon>", &code)?;
        self.worker.run_event_loop(false).await?;

        let value = self.execute_script_to_string("svg").await?;
        Ok(value)
    }

    pub async fn vega_to_svg(&mut self, vg_spec: &serde_json::Value) -> Result<String, AnyError> {
        self.init_vega().await?;

        let arg_id = set_json_arg(vg_spec.clone())?;
        let code = format!(
            r#"
var svg;
vegaToSvg(
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({arg_id}))
).then((result) => {{
    svg = result;
}})
"#,
            arg_id = arg_id
        );
        self.worker.execute_script("<anon>", &code)?;
        self.worker.run_event_loop(false).await?;

        let value = self.execute_script_to_string("svg").await?;
        Ok(value)
    }

    pub async fn get_local_tz(&mut self) -> Result<Option<String>, AnyError> {
        let code = "var localTz = Intl.DateTimeFormat().resolvedOptions().timeZone ?? 'undefined';";
        self.worker.execute_script("<anon>", code)?;
        self.worker.run_event_loop(false).await?;

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
"#;
        self.worker.execute_script("<anon>", code)?;
        self.worker.run_event_loop(false).await?;

        let value = self.execute_script_to_json("themes").await?;
        Ok(value)
    }
}

pub enum VlConvertCommand {
    VlToVg {
        vl_spec: serde_json::Value,
        vl_opts: VlOpts,
        responder: oneshot::Sender<Result<serde_json::Value, AnyError>>,
    },
    VgToSvg {
        vg_spec: serde_json::Value,
        responder: oneshot::Sender<Result<String, AnyError>>,
    },
    VlToSvg {
        vl_spec: serde_json::Value,
        vl_opts: VlOpts,
        responder: oneshot::Sender<Result<String, AnyError>>,
    },
    GetLocalTz {
        responder: oneshot::Sender<Result<Option<String>, AnyError>>,
    },
    GetThemes {
        responder: oneshot::Sender<Result<serde_json::Value, AnyError>>,
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
///         converter.vegalite_to_vega(vl_spec, Default::default())
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
                        vl_opts,
                        responder,
                    } => {
                        let vega_spec =
                            TOKIO_RUNTIME.block_on(inner.vegalite_to_vega(&vl_spec, vl_opts));
                        responder.send(vega_spec).ok();
                    }
                    VlConvertCommand::VgToSvg { vg_spec, responder } => {
                        let svg_result = TOKIO_RUNTIME.block_on(inner.vega_to_svg(&vg_spec));
                        responder.send(svg_result).ok();
                    }
                    VlConvertCommand::VlToSvg {
                        vl_spec,
                        vl_opts,
                        responder,
                    } => {
                        let svg_result =
                            TOKIO_RUNTIME.block_on(inner.vegalite_to_svg(&vl_spec, vl_opts));
                        responder.send(svg_result).ok();
                    }
                    VlConvertCommand::GetLocalTz { responder } => {
                        let local_tz = TOKIO_RUNTIME.block_on(inner.get_local_tz());
                        responder.send(local_tz).ok();
                    }
                    VlConvertCommand::GetThemes { responder } => {
                        let themes = TOKIO_RUNTIME.block_on(inner.get_themes());
                        responder.send(themes).ok();
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
        vl_opts: VlOpts,
    ) -> Result<serde_json::Value, AnyError> {
        let (resp_tx, resp_rx) = oneshot::channel::<Result<serde_json::Value, AnyError>>();
        let cmd = VlConvertCommand::VlToVg {
            vl_spec,
            vl_opts,
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
        vl_opts: VlOpts,
    ) -> Result<String, AnyError> {
        let (resp_tx, resp_rx) = oneshot::channel::<Result<String, AnyError>>();
        let cmd = VlConvertCommand::VlToSvg {
            vl_spec,
            vl_opts,
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
        vl_opts: VlOpts,
        scale: Option<f32>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let svg = self.vegalite_to_svg(vl_spec, vl_opts).await?;
        Self::svg_to_png(&svg, scale)
    }

    fn svg_to_png(svg: &str, scale: f32) -> Result<Vec<u8>, AnyError> {
        let opts = USVG_OPTIONS
            .lock()
            .map_err(|err| anyhow!("Failed to acquire usvg options lock: {}", err.to_string()))?;

        let font_database = FONT_DB
            .lock()
            .map_err(|err| anyhow!("Failed to acquire fontdb lock: {}", err.to_string()))?;

        let mut rtree = match usvg::Tree::from_str(svg, &opts) {
            Ok(rtree) => rtree,
            Err(err) => {
                bail!("Failed to parse SVG string: {}", err.to_string())
            }
        };

        rtree.convert_text(&font_database);

        let pixmap_size = rtree.size.to_screen_size();
        let mut pixmap = tiny_skia::Pixmap::new(
            (pixmap_size.width() as f32 * scale) as u32,
            (pixmap_size.height() as f32 * scale) as u32,
        )
        .unwrap();
        resvg::render(
            &rtree,
            FitTo::Zoom(scale),
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

    pub async fn get_local_tz(&mut self) -> Result<Option<String>, AnyError> {
        let (resp_tx, resp_rx) = oneshot::channel::<Result<Option<String>, AnyError>>();
        let cmd = VlConvertCommand::GetLocalTz { responder: resp_tx };

        // Send request
        match self.sender.send(cmd).await {
            Ok(_) => {
                // All good
            }
            Err(err) => {
                bail!("Failed to send get_local_tz request: {}", err.to_string())
            }
        }

        // Wait for result
        match resp_rx.await {
            Ok(local_tz_result) => local_tz_result,
            Err(err) => bail!(
                "Failed to retrieve get_local_tz result: {}",
                err.to_string()
            ),
        }
    }

    pub async fn get_themes(&mut self) -> Result<serde_json::Value, AnyError> {
        let (resp_tx, resp_rx) = oneshot::channel::<Result<serde_json::Value, AnyError>>();
        let cmd = VlConvertCommand::GetThemes { responder: resp_tx };

        // Send request
        match self.sender.send(cmd).await {
            Ok(_) => {
                // All good
            }
            Err(err) => {
                bail!("Failed to send get_themes request: {}", err.to_string())
            }
        }

        // Wait for result
        match resp_rx.await {
            Ok(themes_result) => themes_result,
            Err(err) => bail!("Failed to retrieve get_themes result: {}", err.to_string()),
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
            .vegalite_to_vega(
                vl_spec,
                VlOpts {
                    vl_version: VlVersion::v4_17,
                    ..Default::default()
                },
            )
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
            .vegalite_to_vega(
                vl_spec.clone(),
                VlOpts {
                    vl_version: VlVersion::v4_17,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        println!("vg_spec1: {}", vg_spec1);

        let mut ctx1 = VlConverter::new();
        let vg_spec2 = ctx1
            .vegalite_to_vega(
                vl_spec,
                VlOpts {
                    vl_version: VlVersion::v5_5,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        println!("vg_spec2: {}", vg_spec2);
    }
}

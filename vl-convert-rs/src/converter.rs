use crate::module_loader::import_map::{url_for_path, vega_themes_url, vega_url, VlVersion};
use crate::module_loader::{
    VlConvertModuleLoader, FORMATE_LOCALE_MAP, IMPORT_MAP, TIME_FORMATE_LOCALE_MAP,
};
use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display, Formatter};
use std::io::Cursor;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use deno_runtime::deno_core::anyhow::bail;
use deno_runtime::deno_core::error::AnyError;
use deno_runtime::deno_core::{serde_v8, v8, Extension};

use deno_core::{op2, ModuleCode, Op};
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_core;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::{Permissions, PermissionsContainer};
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;

use deno_runtime::deno_fs::RealFs;
use std::panic;
use std::str::FromStr;
use std::thread;
use std::thread::JoinHandle;

use crate::anyhow::anyhow;
use futures::channel::{mpsc, mpsc::Sender, oneshot};
use futures_util::{SinkExt, StreamExt};
use png::{PixelDimensions, Unit};
use tiny_skia::{Pixmap, PremultipliedColorU8};
use usvg::{TreeParsing, TreeTextToPath};

use crate::html::{bundle_vega_snippet, get_vega_or_vegalite_script};
use image::io::Reader as ImageReader;

use crate::text::{op_text_width, FONT_DB, USVG_OPTIONS};

deno_core::extension!(vl_convert_converter_runtime, ops = [op_get_json_arg]);

lazy_static! {
    pub static ref TOKIO_RUNTIME: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
    static ref JSON_ARGS: Arc<Mutex<HashMap<i32, String>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref NEXT_ARG_ID: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
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

#[op2]
#[string]
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
}

impl InnerVlConverter {
    async fn init_vega(&mut self) -> Result<(), AnyError> {
        if !self.vega_initialized {
            let import_code = ModuleCode::from(format!(
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
            ));

            self.worker.execute_script("<anon>", import_code)?;

            let logger_code = ModuleCode::from(
                r#"""
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
                .to_string(),
            );
            self.worker.execute_script("<anon>", logger_code)?;
            self.worker.run_event_loop(false).await?;

            // Override text width measurement in vega-scenegraph
            for path in IMPORT_MAP.keys() {
                if path.ends_with("vega-scenegraph.js") {
                    let script_code = ModuleCode::from(format!(
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
                    ));
                    self.worker.execute_script("<anon>", script_code)?;
                    self.worker.run_event_loop(false).await?;
                }
            }

            // Create and initialize svg function string
            let function_str = r#"
function vegaToView(vgSpec, allowedBaseUrls, errors) {
    let runtime = vega.parse(vgSpec);
    let baseURL = 'https://vega.github.io/vega-datasets/';
    const loader = vega.loader({ mode: 'http', baseURL });
    const originalHttp = loader.http.bind(loader);

    if (allowedBaseUrls != null) {
        loader.http = async (uri, options) => {
            const parsedUri = new URL(uri);
            if (
                allowedBaseUrls.every(
                    (allowedUrl) => !parsedUri.href.startsWith(allowedUrl),
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

function cloneScenegraph(obj) {
    const keys = [
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
    ];

    // Check if the input is an object (including an array) or null
    if (typeof obj !== 'object' || obj === null) {
        return obj;
    }

    // Initialize the clone as an array or object based on the input type
    const clone = Array.isArray(obj) ? [] : {};

    // If the object is an array, iterate over its elements
    if (Array.isArray(obj)) {
        for (let i = 0; i < obj.length; i++) {
            // Apply the function recursively to each element
            clone.push(cloneScenegraph(obj[i]));
        }
    } else {
        // If the object is not an array, iterate over its keys
        for (const key in obj) {
            // Clone only the properties with specified keys
            if (key === "shape" && typeof obj[key] === "function") {
                // Convert path object to SVG path string.
                // Initialize context. This is needed for obj.shape(obj) to work.
                obj.shape.context();
                clone["shape"] = obj.shape(obj) ?? "";
            } else if (keys.includes(key)) {
                clone[key] = cloneScenegraph(obj[key]);
            }
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
"#;
            self.worker
                .execute_script("<anon>", deno_core::FastString::Static(function_str))?;
            self.worker.run_event_loop(false).await?;

            self.vega_initialized = true;
        }

        Ok(())
    }

    async fn init_vl_version(&mut self, vl_version: &VlVersion) -> Result<(), AnyError> {
        if !self.initialized_vl_versions.contains(vl_version) {
            // Create and evaluate import string
            let import_code = ModuleCode::from(format!(
                r#"
var {ver_name};
import('{vl_url}').then((imported) => {{
    {ver_name} = imported;
}})
"#,
                ver_name = format!("{:?}", vl_version),
                vl_url = vl_version.to_url()
            ));

            self.worker.execute_script("<anon>", import_code)?;

            self.worker.run_event_loop(false).await?;

            // Create and initialize function string
            let function_code = ModuleCode::from(format!(
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
"#,
                ver_name = format!("{:?}", vl_version),
            ));

            self.worker.execute_script("<anon>", function_code)?;

            self.worker.run_event_loop(false).await?;

            // Register that this Vega-Lite version has been initialized
            self.initialized_vl_versions.insert(*vl_version);
        }
        Ok(())
    }

    pub async fn try_new() -> Result<Self, AnyError> {
        let module_loader = Rc::new(VlConvertModuleLoader);

        let ext = Extension {
            name: "vl_convert_extensions",
            ops: Cow::Owned(vec![
                // Op to measure text width with resvg,
                op_text_width::DECL,
                op_get_json_arg::DECL,
            ]),
            ..Default::default()
        };

        let create_web_worker_cb = Arc::new(|_| {
            todo!("Web workers are not supported");
        });

        let options = WorkerOptions {
            bootstrap: Default::default(),
            extensions: vec![ext],
            startup_snapshot: None,
            skip_op_registration: false,
            create_params: None,
            unsafely_ignore_certificate_errors: None,
            root_cert_store_provider: None,
            seed: None,
            source_map_getter: None,
            format_js_error_fn: None,
            create_web_worker_cb,
            maybe_inspector_server: None,
            should_break_on_first_statement: false,
            module_loader: module_loader.clone(),
            npm_resolver: None,
            get_error_class_fn: Some(&get_error_class_name),
            cache_storage_dir: None,
            origin_storage_dir: None,
            blob_store: Arc::new(BlobStore::default()),
            broadcast_channel: InMemoryBroadcastChannel::default(),
            shared_array_buffer_store: None,
            compiled_wasm_module_store: None,
            stdio: Default::default(),
            should_wait_for_inspector_session: false,
            fs: Arc::new(RealFs),
            feature_checker: Arc::new(Default::default()),
            strace_ops: None,
        };

        let main_module =
            deno_core::resolve_path("vl-convert-rs.js", Path::new(env!("CARGO_MANIFEST_DIR")))?;
        let permissions = PermissionsContainer::new(Permissions::allow_all());

        let mut worker =
            MainWorker::bootstrap_from_options(main_module.clone(), permissions, options);
        worker.execute_main_module(&main_module).await?;
        worker.run_event_loop(false).await?;

        let this = Self {
            worker,
            initialized_vl_versions: Default::default(),
            vega_initialized: false,
        };

        Ok(this)
    }

    async fn execute_script_to_json(
        &mut self,
        script: &str,
    ) -> Result<serde_json::Value, AnyError> {
        let code = ModuleCode::from(script.to_string());
        let res = self.worker.js_runtime.execute_script("<anon>", code)?;

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
        let code = ModuleCode::from(script.to_string());
        let res = self.worker.js_runtime.execute_script("<anon>", code)?;

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

        let allowed_base_urls =
            serde_json::to_string(&serde_json::Value::from(vl_opts.allowed_base_urls))?;

        let code = format!(
            r#"
compileVegaLite_{ver_name:?}(
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({spec_arg_id})),
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({config_arg_id})),
    {theme_arg},
    {show_warnings},
    {allowed_base_urls},
)
"#,
            ver_name = vl_opts.vl_version,
            spec_arg_id = spec_arg_id,
            config_arg_id = config_arg_id,
            theme_arg = theme_arg,
            show_warnings = vl_opts.show_warnings,
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

        let format_locale = match vl_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vl_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let spec_arg_id = set_json_arg(vl_spec.clone())?;
        let config_arg_id = set_json_arg(config)?;
        let format_locale_id = set_json_arg(format_locale)?;
        let time_format_locale_id = set_json_arg(time_format_locale)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let allowed_base_urls =
            serde_json::to_string(&serde_json::Value::from(vl_opts.allowed_base_urls))?;

        let code = ModuleCode::from(format!(
            r#"
var svg;
var errors = [];
vegaLiteToSvg_{ver_name:?}(
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({spec_arg_id})),
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({config_arg_id})),
    {theme_arg},
    {show_warnings},
    {allowed_base_urls},
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({format_locale_id})),
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({time_format_locale_id})),
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
        ));
        self.worker.execute_script("<anon>", code)?;
        self.worker.run_event_loop(false).await?;

        let value = self.execute_script_to_string("svg").await?;
        Ok(value)
    }

    pub async fn vegalite_to_scenegraph(
        &mut self,
        vl_spec: &serde_json::Value,
        vl_opts: VlOpts,
    ) -> Result<serde_json::Value, AnyError> {
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

        let spec_arg_id = set_json_arg(vl_spec.clone())?;
        let config_arg_id = set_json_arg(config)?;
        let format_locale_id = set_json_arg(format_locale)?;
        let time_format_locale_id = set_json_arg(time_format_locale)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let allowed_base_urls =
            serde_json::to_string(&serde_json::Value::from(vl_opts.allowed_base_urls))?;

        let code = ModuleCode::from(format!(
            r#"
var sg;
var errors = [];
vegaLiteToScenegraph_{ver_name:?}(
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({spec_arg_id})),
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({config_arg_id})),
    {theme_arg},
    {show_warnings},
    {allowed_base_urls},
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({format_locale_id})),
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({time_format_locale_id})),
    errors,
).then((result) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    sg = result;
}})
"#,
            ver_name = vl_opts.vl_version,
            show_warnings = vl_opts.show_warnings,
        ));
        self.worker.execute_script("<anon>", code)?;
        self.worker.run_event_loop(false).await?;

        let value = self.execute_script_to_json("sg").await?;
        Ok(value)
    }

    pub async fn vega_to_svg(
        &mut self,
        vg_spec: &serde_json::Value,
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

        let arg_id = set_json_arg(vg_spec.clone())?;
        let format_locale_id = set_json_arg(format_locale)?;
        let time_format_locale_id = set_json_arg(time_format_locale)?;

        let code = ModuleCode::from(format!(
            r#"
var svg;
var errors = [];
vegaToSvg(
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({arg_id})),
    {allowed_base_urls},
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({format_locale_id})),
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({time_format_locale_id})),
    errors,
).then((result) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    svg = result;
}})
"#
        ));
        self.worker.execute_script("<anon>", code)?;
        self.worker.run_event_loop(false).await?;

        let value = self.execute_script_to_string("svg").await?;
        Ok(value)
    }

    pub async fn vega_to_scenegraph(
        &mut self,
        vg_spec: &serde_json::Value,
        vg_opts: VgOpts,
    ) -> Result<serde_json::Value, AnyError> {
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

        let arg_id = set_json_arg(vg_spec.clone())?;
        let format_locale_id = set_json_arg(format_locale)?;
        let time_format_locale_id = set_json_arg(time_format_locale)?;

        let code = ModuleCode::from(format!(
            r#"
var sg;
var errors = [];
vegaToScenegraph(
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({arg_id})),
    {allowed_base_urls},
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({format_locale_id})),
    JSON.parse(Deno[Deno.internal].core.ops.op_get_json_arg({time_format_locale_id})),
    errors,
).then((result) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    sg = result;
}})
"#
        ));
        self.worker.execute_script("<anon>", code)?;
        self.worker.run_event_loop(false).await?;

        let value = self.execute_script_to_json("sg").await?;
        Ok(value)
    }

    pub async fn get_local_tz(&mut self) -> Result<Option<String>, AnyError> {
        let code = ModuleCode::from(
            "var localTz = Intl.DateTimeFormat().resolvedOptions().timeZone ?? 'undefined';"
                .to_string(),
        );
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

        let code = ModuleCode::from(
            r#"
var themes = Object.assign({}, vegaThemes);
delete themes.version
delete themes.default
"#
            .to_string(),
        );
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
        vg_opts: VgOpts,
        responder: oneshot::Sender<Result<String, AnyError>>,
    },
    VgToSg {
        vg_spec: serde_json::Value,
        vg_opts: VgOpts,
        responder: oneshot::Sender<Result<serde_json::Value, AnyError>>,
    },
    VlToSvg {
        vl_spec: serde_json::Value,
        vl_opts: VlOpts,
        responder: oneshot::Sender<Result<String, AnyError>>,
    },
    VlToSg {
        vl_spec: serde_json::Value,
        vl_opts: VlOpts,
        responder: oneshot::Sender<Result<serde_json::Value, AnyError>>,
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
    _vegaembed_bundles: HashMap<VlVersion, String>,
}

impl VlConverter {
    pub fn new() -> Self {
        // Initialize environment logger
        env_logger::try_init().ok();

        let (sender, mut receiver) = mpsc::channel::<VlConvertCommand>(32);

        let handle = Arc::new(thread::spawn(move || {
            TOKIO_RUNTIME.block_on(async {
                let mut inner = InnerVlConverter::try_new().await?;
                while let Some(cmd) = receiver.next().await {
                    match cmd {
                        VlConvertCommand::VlToVg {
                            vl_spec,
                            vl_opts,
                            responder,
                        } => {
                            let vega_spec = inner.vegalite_to_vega(&vl_spec, vl_opts).await;
                            responder.send(vega_spec).ok();
                        }
                        VlConvertCommand::VgToSvg {
                            vg_spec,
                            vg_opts,
                            responder,
                        } => {
                            let svg_result = inner.vega_to_svg(&vg_spec, vg_opts).await;
                            responder.send(svg_result).ok();
                        }
                        VlConvertCommand::VgToSg {
                            vg_spec,
                            vg_opts,
                            responder,
                        } => {
                            let sg_result = inner.vega_to_scenegraph(&vg_spec, vg_opts).await;
                            responder.send(sg_result).ok();
                        }
                        VlConvertCommand::VlToSvg {
                            vl_spec,
                            vl_opts,
                            responder,
                        } => {
                            let svg_result = inner.vegalite_to_svg(&vl_spec, vl_opts).await;
                            responder.send(svg_result).ok();
                        }
                        VlConvertCommand::VlToSg {
                            vl_spec,
                            vl_opts,
                            responder,
                        } => {
                            let sg_result = inner.vegalite_to_scenegraph(&vl_spec, vl_opts).await;
                            responder.send(sg_result).ok();
                        }
                        VlConvertCommand::GetLocalTz { responder } => {
                            let local_tz = inner.get_local_tz().await;
                            responder.send(local_tz).ok();
                        }
                        VlConvertCommand::GetThemes { responder } => {
                            let themes = inner.get_themes().await;
                            responder.send(themes).ok();
                        }
                    }
                }
                Ok::<(), AnyError>(())
            })?;

            Ok(())
        }));

        Self {
            sender,
            _handle: handle,
            _vegaembed_bundles: Default::default(),
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

    pub async fn vega_to_svg(
        &mut self,
        vg_spec: serde_json::Value,
        vg_opts: VgOpts,
    ) -> Result<String, AnyError> {
        let (resp_tx, resp_rx) = oneshot::channel::<Result<String, AnyError>>();
        let cmd = VlConvertCommand::VgToSvg {
            vg_spec,
            vg_opts,
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

    pub async fn vega_to_scenegraph(
        &mut self,
        vg_spec: serde_json::Value,
        vg_opts: VgOpts,
    ) -> Result<serde_json::Value, AnyError> {
        let (resp_tx, resp_rx) = oneshot::channel::<Result<serde_json::Value, AnyError>>();
        let cmd = VlConvertCommand::VgToSg {
            vg_spec,
            vg_opts,
            responder: resp_tx,
        };

        // Send request
        match self.sender.send(cmd).await {
            Ok(_) => {
                // All good
            }
            Err(err) => {
                bail!(
                    "Failed to send Scenegraph conversion request: {}",
                    err.to_string()
                )
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

    pub async fn vegalite_to_scenegraph(
        &mut self,
        vl_spec: serde_json::Value,
        vl_opts: VlOpts,
    ) -> Result<serde_json::Value, AnyError> {
        let (resp_tx, resp_rx) = oneshot::channel::<Result<serde_json::Value, AnyError>>();
        let cmd = VlConvertCommand::VlToSg {
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
                bail!(
                    "Failed to send Scenegraph conversion request: {}",
                    err.to_string()
                )
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
        vg_opts: VgOpts,
        scale: Option<f32>,
        ppi: Option<f32>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let svg = self.vega_to_svg(vg_spec, vg_opts).await?;
        svg_to_png(&svg, scale, ppi)
    }

    pub async fn vegalite_to_png(
        &mut self,
        vl_spec: serde_json::Value,
        vl_opts: VlOpts,
        scale: Option<f32>,
        ppi: Option<f32>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let svg = self.vegalite_to_svg(vl_spec, vl_opts).await?;
        svg_to_png(&svg, scale, ppi)
    }

    pub async fn vega_to_jpeg(
        &mut self,
        vg_spec: serde_json::Value,
        vg_opts: VgOpts,
        scale: Option<f32>,
        quality: Option<u8>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let svg = self.vega_to_svg(vg_spec, vg_opts).await?;
        svg_to_jpeg(&svg, scale, quality)
    }

    pub async fn vegalite_to_jpeg(
        &mut self,
        vl_spec: serde_json::Value,
        vl_opts: VlOpts,
        scale: Option<f32>,
        quality: Option<u8>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let svg = self.vegalite_to_svg(vl_spec, vl_opts).await?;
        svg_to_jpeg(&svg, scale, quality)
    }

    pub async fn vega_to_pdf(
        &mut self,
        vg_spec: serde_json::Value,
        vg_opts: VgOpts,
        scale: Option<f32>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let svg = self.vega_to_svg(vg_spec, vg_opts).await?;
        svg_to_pdf(&svg, scale)
    }

    pub async fn vegalite_to_pdf(
        &mut self,
        vl_spec: serde_json::Value,
        vl_opts: VlOpts,
        scale: Option<f32>,
    ) -> Result<Vec<u8>, AnyError> {
        let scale = scale.unwrap_or(1.0);
        let svg = self.vegalite_to_svg(vl_spec, vl_opts).await?;
        svg_to_pdf(&svg, scale)
    }

    pub async fn get_vegaembed_bundle(
        &mut self,
        vl_version: VlVersion,
    ) -> Result<String, AnyError> {
        let bundle = match self._vegaembed_bundles.entry(vl_version) {
            Entry::Occupied(occupied) => occupied.get().clone(),
            Entry::Vacant(vacant) => {
                let bundle = bundle_vega_snippet(
                    "window.vegaEmbed=vegaEmbed; window.vega=vega; window.vegaLite=vegaLite;",
                    vl_version,
                )
                .await?;
                vacant.insert(bundle.clone());
                bundle
            }
        };

        Ok(bundle)
    }

    async fn build_html(
        &mut self,
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
    <script src="https://cdn.jsdelivr.net/npm/vega@5"></script>
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
        &mut self,
        vl_spec: serde_json::Value,
        vl_opts: VlOpts,
        bundle: bool,
        renderer: Renderer,
    ) -> Result<String, AnyError> {
        let vl_version = vl_opts.vl_version;
        let code = get_vega_or_vegalite_script(vl_spec, vl_opts.to_embed_opts(renderer)?)?;
        self.build_html(&code, vl_version, bundle).await
    }

    pub async fn vega_to_html(
        &mut self,
        vg_spec: serde_json::Value,
        vg_opts: VgOpts,
        bundle: bool,
        renderer: Renderer,
    ) -> Result<String, AnyError> {
        let code = get_vega_or_vegalite_script(vg_spec, vg_opts.to_embed_opts(renderer)?)?;
        self.build_html(&code, Default::default(), bundle).await
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
    let font_database = FONT_DB
        .lock()
        .map_err(|err| anyhow!("Failed to acquire fontdb lock: {}", err.to_string()))?;

    // catch_unwind so that we don't poison Mutexes
    // if usvg/resvg panics
    let response = panic::catch_unwind(|| {
        let mut rtree = match parse_svg(svg) {
            Ok(rtree) => rtree,
            Err(err) => return Err(err),
        };
        rtree.convert_text(&font_database);

        let rtree = resvg::Tree::from_usvg(&rtree);

        let mut pixmap = tiny_skia::Pixmap::new(
            (rtree.size.width() * scale) as u32,
            (rtree.size.height() * scale) as u32,
        )
        .unwrap();

        let transform = tiny_skia::Transform::from_scale(scale, scale);
        resvg::Tree::render(&rtree, transform, &mut pixmap.as_mut());

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
    img.write_to(
        &mut Cursor::new(&mut jpeg_bytes),
        image::ImageOutputFormat::Jpeg(quality),
    )?;
    Ok(jpeg_bytes)
}

pub fn svg_to_pdf(svg: &str, scale: f32) -> Result<Vec<u8>, AnyError> {
    // Load system fonts
    let font_db = FONT_DB
        .lock()
        .map_err(|err| anyhow!("Failed to acquire fontdb lock: {}", err.to_string()))?;

    let tree = parse_svg(svg)?;
    vl_convert_pdf::svg_to_pdf(&tree, &font_db, scale)
}

/// Helper to parse svg string to usvg Tree with more helpful error messages
fn parse_svg(svg: &str) -> Result<usvg::Tree, AnyError> {
    let xml_opt = usvg::roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };

    let opts = USVG_OPTIONS
        .lock()
        .map_err(|err| anyhow!("Failed to acquire usvg options lock: {}", err.to_string()))?;

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

pub fn vegalite_to_url(vl_spec: &serde_json::Value, fullscreen: bool) -> Result<String, AnyError> {
    let spec_str = serde_json::to_string(vl_spec)?;
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

pub fn vega_to_url(vg_spec: &serde_json::Value, fullscreen: bool) -> Result<String, AnyError> {
    let spec_str = serde_json::to_string(vg_spec)?;
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

    #[tokio::test]
    async fn test_convert_context() {
        let mut ctx = VlConverter::new();
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

        let mut ctx1 = VlConverter::new();
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

        let mut ctx1 = VlConverter::new();
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
}

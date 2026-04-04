use super::InnerVlConverter;
use crate::module_loader::import_map::{msgpack_url, vega_themes_url, vega_url, VlVersion};
use deno_core::anyhow::{anyhow, bail};
use deno_core::error::AnyError;
use deno_core::v8;

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

    pub(super) async fn init_vl_version(&mut self, vl_version: &VlVersion) -> Result<(), AnyError> {
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
}

use crate::module_loader::import_map::VlVersion;
use crate::module_loader::VegaFusionModuleLoader;
use deno_core::error::AnyError;
use deno_core::{serde_v8, v8, JsRuntime, RuntimeOptions};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::anyhow::bail;
use std::thread;
use std::thread::JoinHandle;

use futures::channel::{mpsc, mpsc::Sender, oneshot};
use futures::executor::block_on;
use futures_util::{SinkExt, StreamExt};

/// Struct that interacts directly with the Deno JavaScript runtime. Not Sendable
struct InnerVlConverter {
    js_runtime: JsRuntime,
    initialized_vl_versions: HashSet<VlVersion>,
}

impl InnerVlConverter {
    async fn init_version(&mut self, vl_version: &VlVersion) -> Result<(), AnyError> {
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

            self.js_runtime.execute_script("<anon>", &import_str)?;

            self.js_runtime.run_event_loop(false).await?;

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
"#,
                ver_name = format!("{:?}", vl_version),
            );

            self.js_runtime.execute_script("<anon>", &function_str)?;

            self.js_runtime.run_event_loop(false).await?;

            // Register that this Vega-Lite version has been initialized
            self.initialized_vl_versions.insert(*vl_version);
        }
        Ok(())
    }

    pub fn new() -> Self {
        let module_loader = Rc::new(VegaFusionModuleLoader::new());
        let js_runtime = JsRuntime::new(RuntimeOptions {
            module_loader: Some(module_loader),
            ..Default::default()
        });

        Self {
            js_runtime,
            initialized_vl_versions: Default::default(),
        }
    }

    pub async fn vegalite_to_vega(
        &mut self,
        vl_spec: &serde_json::Value,
        vl_version: VlVersion,
        pretty: bool,
    ) -> Result<String, AnyError> {
        self.init_version(&vl_version).await?;

        let vl_spec_str = serde_json::to_string(vl_spec)?;
        let res = self.js_runtime.execute_script(
            "<anon>",
            &format!(
                r#"
compileVegaLite_{ver_name:?}(
    {vl_spec_str},
    {pretty}
)
"#,
                ver_name = vl_version,
                vl_spec_str = vl_spec_str,
                pretty = pretty,
            ),
        )?;

        self.js_runtime.run_event_loop(false).await?;

        let scope = &mut self.js_runtime.handle_scope();
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
}

pub enum VlConvertCommand {
    VlToVg {
        vl_spec: serde_json::Value,
        vl_version: VlVersion,
        pretty: bool,
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
            let mut inner = InnerVlConverter::new();

            while let Some(cmd) = block_on(receiver.next()) {
                match cmd {
                    VlConvertCommand::VlToVg {
                        vl_spec,
                        vl_version,
                        pretty,
                        responder,
                    } => {
                        let vega_spec =
                            block_on(inner.vegalite_to_vega(&vl_spec, vl_version, pretty));
                        responder.send(vega_spec).ok();
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

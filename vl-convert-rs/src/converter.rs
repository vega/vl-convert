use crate::module_loader::import_map::{VL_VERSIONS, VlVersion};
use crate::module_loader::VegaFusionModuleLoader;
use deno_core::error::AnyError;
use deno_core::{serde_v8, v8, JsRuntime, RuntimeOptions};
use std::rc::Rc;
use std::sync::Arc;

use std::thread;
use std::thread::JoinHandle;
use deno_core::anyhow::bail;

use futures::channel::{mpsc, mpsc::Sender, oneshot};
use futures::channel::mpsc::SendError;
use futures::channel::oneshot::Canceled;
use futures::executor::block_on;
use futures_util::{SinkExt, StreamExt};


/// Struct that interacts directly with the Deno JavaScript runtime. Not Sendable
struct InnerVlConverter {
    js_runtime: JsRuntime,
}

impl InnerVlConverter {
    pub async fn try_new() -> Result<Self, AnyError> {
        let module_loader = Rc::new(VegaFusionModuleLoader::new());
        let mut js_runtime = JsRuntime::new(RuntimeOptions {
            module_loader: Some(module_loader),
            ..Default::default()
        });

        // Imports
        let mut import_str = String::new();

        for vl_version in VL_VERSIONS {
            import_str.push_str(&format!(
                r#"
var {ver_name};
import('{vl_url}').then((imported) => {{
    {ver_name} = imported;
}})
"#,
                ver_name = format!("{:?}", vl_version),
                vl_url = vl_version.to_url()))
        }

        js_runtime
            .execute_script(
                "<anon>",
                &import_str,
            )
            .expect("Failed to import vega-lite");

        js_runtime.run_event_loop(false).await?;

        // Define functions
        let mut functions_str = String::new();
        for vl_version in VL_VERSIONS {
            functions_str.push_str(&format!(
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
            ))
        }

        js_runtime
            .execute_script(
                "<anon>",
                &functions_str,
            )
            .expect("Failed to import vega-lite");
        js_runtime.run_event_loop(false).await?;

        Ok(Self { js_runtime })
    }

    pub async fn vegalite_to_vega(
        &mut self,
        vl_spec: &serde_json::Value,
        vl_version: VlVersion,
        pretty: bool,
    ) -> Result<String, AnyError> {
        let vl_spec_str = serde_json::to_string(vl_spec)?;
        let res = self
            .js_runtime
            .execute_script(
                "<anon>",
                &format!(
                    r#"
compileVegaLite_{ver_name}(
    {vl_spec_str},
    {pretty}
)
"#,
                    ver_name = format!("{:?}", vl_version),
                    vl_spec_str = vl_spec_str,
                    pretty=pretty,
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
            },
            Err(err) => bail!("{}", err.to_string())
        };
        Ok(value)
    }
}

pub enum VlConvertCommand {
    VlToVg {
        vl_spec: serde_json::Value,
        vl_version: VlVersion,
        pretty: bool,
        responder: oneshot::Sender<String>,
    }
}


/// Public struct for performing Vega-Lite to Vega conversions
#[derive(Clone)]
pub struct VlConverter {
    sender: Sender<VlConvertCommand>,
    handle: Arc<JoinHandle<Result<(), AnyError>>>,
}

impl VlConverter {
    pub fn new() -> Self {
        let (sender, mut receiver) = mpsc::channel::<VlConvertCommand>(32);

        let handle = Arc::new(thread::spawn(move || {
            let mut inner = block_on(InnerVlConverter::try_new())?;

            while let Some(cmd) = block_on(receiver.next()) {
                match cmd {
                    VlConvertCommand::VlToVg { vl_spec, vl_version, pretty, responder } => {
                        let vega_spec = block_on(inner.vegalite_to_vega(&vl_spec, vl_version, pretty))?;
                        responder.send(vega_spec);
                    }
                }
            }
            Ok(())
        }));

        Self {
            sender,
            handle,
        }
    }

    pub async fn vegalite_to_vega(
        &mut self,
        vl_spec: serde_json::Value,
        vl_version: VlVersion,
        pretty: bool,
    ) -> Result<String, AnyError> {
        let (resp_tx, resp_rx) = oneshot::channel::<String>();
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
            Ok(vega_spec) => Ok(vega_spec),
            Err(err) => bail!("Failed to retrieve conversion result: {}", err.to_string())
        }
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

        let vg_spec = ctx.vegalite_to_vega(vl_spec, VlVersion::v4_17, true).await.unwrap();
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
        let vg_spec1 = ctx1.vegalite_to_vega(vl_spec.clone(), VlVersion::v4_17, true).await.unwrap();
        println!("vg_spec1: {}", vg_spec1);

        let mut ctx1 = VlConverter::new();
        let vg_spec2 = ctx1.vegalite_to_vega(vl_spec, VlVersion::v5_5, true).await.unwrap();
        println!("vg_spec2: {}", vg_spec2);
    }
}

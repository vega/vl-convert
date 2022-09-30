use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use deno_runtime::BootstrapOptions;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_core::{serde_json, serde_v8, v8};
use deno_runtime::deno_core::error::AnyError;
use deno_runtime::deno_web::{BlobStore, get_error_class_name};
use deno_runtime::permissions::Permissions;
use deno_runtime::worker::{MainWorker, WorkerOptions};
use crate::module_loader::VegaFusionModuleLoader;

pub mod module_loader;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

async fn convert() -> Result<(), AnyError> {
    let module_loader = Rc::new(VegaFusionModuleLoader::new());
    let create_web_worker_cb = Arc::new(|_| {
        todo!("Web workers are not supported in the example");
    });
    let web_worker_preload_module_cb = Arc::new(|_| {
        todo!("Web workers are not supported in the example");
    });
    let web_worker_pre_execute_module_cb = Arc::new(|_| {
        todo!("Web workers are not supported in the example");
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
            user_agent: "vegafusion-convert".to_string(),
            inspect: false
        },
        extensions: vec![],
        unsafely_ignore_certificate_errors: None,
        root_cert_store: None,
        seed: None,
        web_worker_preload_module_cb,
        web_worker_pre_execute_module_cb,
        format_js_error_fn: None,
        create_web_worker_cb,
        maybe_inspector_server: None,
        should_break_on_first_statement: false,
        module_loader,
        // get_error_class_fn: Some(&get_error_class_name),
        get_error_class_fn: None,
        cache_storage_dir: None,
        origin_storage_dir: None,
        blob_store: BlobStore::default(),
        broadcast_channel: InMemoryBroadcastChannel::default(),
        shared_array_buffer_store: None,
        compiled_wasm_module_store: None,
        npm_resolver: None,
        source_map_getter: None,
        stdio: Default::default(),
    };

    let js_path = Path::new("vegafusion-convert.js");
    let main_module = deno_runtime::deno_core::resolve_path(&js_path.to_string_lossy())?;
    let permissions = Permissions::allow_all();

    let mut worker = MainWorker::bootstrap_from_options(
        main_module.clone(),
        permissions,
        options,
    );
    worker.execute_main_module(&main_module).await?;
    worker.run_event_loop(false).await?;

    // Imports
    worker.js_runtime.execute_script("<anon>", r#"
var vl;
import('https://cdn.skypack.dev/pin/vega-lite@v5.2.0-0lbC9JVxwLSC3btqiwR4/mode=imports,min/optimized/vega-lite.js').then((imported) => {
    vl = imported;
})
"#).expect("Failed to import vega-lite");
    worker.run_event_loop(false).await?;

    // eval something
    let res = worker.js_runtime.execute_script("<anon>", r#"
let vlSpec = {
    "data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
    "mark": "bar",
    "encoding": {
        "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
        "y": {"aggregate": "mean", "field": "precipitation"}
    }
};
let options = {};
let vgSpec = vl.compile(vlSpec, options).spec;
JSON.stringify(vgSpec)
"#).unwrap();
    worker.run_event_loop(false).await?;

    let scope = &mut worker.js_runtime.handle_scope();
    let local = v8::Local::new(scope, res);
    // Deserialize a `v8` object into a Rust type using `serde_v8`,
    // in this case deserialize to a JSON `Value`.
    let deserialized_value =
        serde_v8::from_v8::<serde_json::Value>(scope, local);

    let value = deserialized_value.unwrap().as_str().unwrap().to_string();
    println!("result: {:?}", value);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let m = module_loader::import_map::build_import_map();
        println!("{:#?}", m.keys());

        let source = m.get("/-/vega-force@v4.0.7-PSUFEGG7pO0gjWmlkXJl/dist=es2020,mode=imports,min/optimized/vega-force.js").unwrap();
        println!("{}", source);
    }

    #[tokio::test]
    async fn test_convert() {
        convert().await.unwrap();
    }
}

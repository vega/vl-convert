use std::path::Path;
use deno_runtime::deno_permissions::{Permissions, PermissionsContainer};
use deno_runtime::worker::{MainWorker, WorkerOptions};


#[tokio::main]
async fn main() {
    convert().await;
    convert().await;
}

async fn convert() {

    println!("convert()");

    let options = WorkerOptions {
        ..Default::default()
    };

    let main_module =
        deno_core::resolve_path("empty_main.js", Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();

    let permissions = PermissionsContainer::new(Permissions::allow_all());

    let mut worker =
        MainWorker::bootstrap_from_options(main_module.clone(), permissions, options);

    worker.execute_main_module(&main_module).await.unwrap();
    worker.run_event_loop(false).await.unwrap();

    let code = r"1 + 1".to_string();
    worker.execute_script("ext:<anon>", code.into()).unwrap();

    // println!("CARGO_MANIFEST_DIR: {:?}", env!("CARGO_MANIFEST_DIR"));
    // let main_module =
    //     deno_core::resolve_path("vendor_imports.js", Path::new(env!("CARGO_MANIFEST_DIR")))
    //         .unwrap();

    // println!("main_module: {:?}", main_module);

    // println!("VlConverter::new()");
    // let mut converter = VlConverter::new();
    // println!("converter.does_it_crash()");
    // converter.does_it_crash().await.unwrap();

    // converter
    //     .vegalite_to_svg(
    //         vl_spec,
    //         VlOpts {
    //             vl_version: VlVersion::v5_8,
    //             ..Default::default()
    //         },
    //     )
    //     .await
    //     .expect("Failed to perform Vega-Lite to Vega conversion")
}

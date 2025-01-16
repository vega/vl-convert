use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use deno_runtime::deno_permissions::{Permissions, PermissionsContainer};
use deno_runtime::worker::{MainWorker, WorkerOptions};
use futures::channel::{mpsc, mpsc::Sender};
use futures_util::{SinkExt, StreamExt};
use vl_convert_rs::converter::{VlConvertCommand, TOKIO_RUNTIME};
use vl_convert_rs::module_loader::VlConvertModuleLoader;
use vl_convert_rs::VlConverter;

#[tokio::main]
async fn main() {
    // convert().await;
    // convert().await;
    // convert2();
    // convert2();

    convert3().await;
    convert3().await;
}

pub struct Converter {
    sender: Sender<()>,
    handle: Arc<JoinHandle<()>>
}

impl Converter {
    pub fn new() -> Self {
        let (sender, mut receiver) = mpsc::channel::<()>(32);

        println!("TOKIO_RUNTIME.block_on");
        let handle = Arc::new(thread::spawn(move || {
            TOKIO_RUNTIME.block_on(async {
                println!("in block_on");
                let mut inner = InnerConverter::new().await;
                while let Some(cmd) = receiver.next().await {
                    inner.convert()
                }
            })
        }));

        Self {
            sender,
            handle
        }
    }

    pub async fn convert(&mut self) {
        println!("Send convert");
        // Send request
        self.sender.send(()).await.unwrap();

        // // Wait for result
        // match resp_rx.await {
        //     Ok(result) => result,
        //     Err(err) => bail!("Failed to retrieve get_themes result: {}", err.to_string()),
        // }
    }
}
pub struct InnerConverter {
    worker: MainWorker
}

impl InnerConverter {
    pub async fn new() -> Self {
        println!("build inner converter");
        let module_loader = Rc::new(VlConvertModuleLoader);
        let options = WorkerOptions {
            module_loader,
            ..Default::default()
        };

        let main_module =
            deno_core::resolve_path("vl-convert-rs.js", Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();

        let permissions = PermissionsContainer::new(Permissions::allow_all());

        let mut worker =
            MainWorker::bootstrap_from_options(main_module.clone(), permissions, options);

        worker.execute_main_module(&main_module).await.unwrap();
        worker.run_event_loop(false).await.unwrap();
        Self {
            worker
        }
    }

    fn convert(&mut self) {
        let code = r"1 + 1".to_string();
        self.worker.execute_script("ext:<anon>", code.into()).unwrap();
    }
}

async fn convert3() {
    let mut converter = Converter::new();
    converter.convert().await;
}

fn convert2() {
    println!("convert2");
    let handle = Arc::new(thread::spawn(move || {
        TOKIO_RUNTIME.block_on(async {
            let module_loader = Rc::new(VlConvertModuleLoader);
            let options = WorkerOptions {
                module_loader,
                ..Default::default()
            };

            let main_module =
                deno_core::resolve_path("vl-convert-rs.js", Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();

            let permissions = PermissionsContainer::new(Permissions::allow_all());

            let mut worker =
                MainWorker::bootstrap_from_options(main_module.clone(), permissions, options);

            worker.execute_main_module(&main_module).await.unwrap();
            worker.run_event_loop(false).await.unwrap();

            let code = r"1 + 1".to_string();
            worker.execute_script("ext:<anon>", code.into()).unwrap();
        })
    }));
}

async fn convert() {

    // println!("convert()");
    //
    // let module_loader = Rc::new(VlConvertModuleLoader);
    // let options = WorkerOptions {
    //     module_loader,
    //     ..Default::default()
    // };
    //
    // let main_module =
    //     deno_core::resolve_path("vl-convert-rs.js", Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    //
    // let permissions = PermissionsContainer::new(Permissions::allow_all());
    //
    // let mut worker =
    //     MainWorker::bootstrap_from_options(main_module.clone(), permissions, options);
    //
    // worker.execute_main_module(&main_module).await.unwrap();
    // worker.run_event_loop(false).await.unwrap();
    //
    // let code = r"1 + 1".to_string();
    // worker.execute_script("ext:<anon>", code.into()).unwrap();
    //
    // // println!("CARGO_MANIFEST_DIR: {:?}", env!("CARGO_MANIFEST_DIR"));
    // let main_module =
    //     deno_core::resolve_path("vendor_imports.js", Path::new(env!("CARGO_MANIFEST_DIR")))
    //         .unwrap();

    // println!("main_module: {:?}", main_module);

    println!("VlConverter::new()");
    let mut converter = VlConverter::new();
    println!("converter.does_it_crash()");
    converter.does_it_crash().await.unwrap();

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

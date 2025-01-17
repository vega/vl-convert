use deno_core::error::AnyError;
use deno_runtime::deno_permissions::{Permissions, PermissionsContainer};
use deno_runtime::worker::{MainWorker, WorkerOptions};
use futures::channel::{mpsc, mpsc::Sender, oneshot};
use futures_util::{SinkExt, StreamExt};
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use tokio::io::AsyncWriteExt;
use vl_convert_rs::converter::TOKIO_RUNTIME;

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() {
    convert3().await;
    convert3().await;
}

struct ConvertCommand {
    responder: oneshot::Sender<Result<(), AnyError>>,
    shutdown: bool,
}

pub struct Converter {
    sender: Option<Sender<ConvertCommand>>,
    handle: Arc<JoinHandle<()>>,
}

impl Converter {
    pub fn new() -> Self {
        let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();

        let (sender, mut receiver) = mpsc::channel::<ConvertCommand>(32);

        // println!("TOKIO_RUNTIME.block_on");
        let handle = Arc::new(thread::spawn(move || {
            tokio_runtime.block_on(async {
                println!("in block_on");
                let mut inner = InnerConverter::new().await;
                while let Some(cmd) = receiver.next().await {
                    println!("receiver.next()");
                    if cmd.shutdown {
                        cmd.responder.send(Ok(())).unwrap();
                        break;
                    }
                    inner.convert();
                    cmd.responder.send(Ok(())).unwrap()
                }
                println!("Worker thread shutting down");
            })
        }));

        Self {
            sender: Some(sender),
            handle,
        }
    }

    pub async fn convert(&mut self) {
        println!("Send convert");

        let (resp_tx, resp_rx) = oneshot::channel::<Result<(), AnyError>>();
        let cmd = ConvertCommand {
            responder: resp_tx,
            shutdown: false,
        };

        // Send request
        match self.sender.as_mut().unwrap().send(cmd).await {
            Ok(_) => {
                // All good
            }
            Err(err) => {
                panic!("Failed to send get_themes request: {}", err.to_string())
            }
        }

        // Wait for result
        resp_rx.await.unwrap().unwrap()
    }
}

impl Drop for Converter {
    fn drop(&mut self) {
        if let Some(mut sender) = self.sender.take() {
            // Send shutdown command
            let (resp_tx, resp_rx) = oneshot::channel::<Result<(), AnyError>>();
            let cmd = ConvertCommand {
                responder: resp_tx,
                shutdown: true,
            };

            // Use std::sync channels for shutdown
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                futures::executor::block_on(async {
                    sender.send(cmd).await.unwrap();
                    resp_rx.await.unwrap().unwrap();
                });
                tx.send(()).unwrap();
            });
            rx.recv().unwrap();
        }

        // Wait for the thread to finish
        if Arc::strong_count(&self.handle) == 1 {
            let handle = std::mem::replace(&mut self.handle, Arc::new(thread::spawn(|| {})));
            if let Ok(thread_handle) = Arc::try_unwrap(handle) {
                thread_handle.join().unwrap();
            }
        }
    }
}

pub struct InnerConverter {
    worker: MainWorker,
}

impl InnerConverter {
    pub async fn new() -> Self {
        println!("build inner converter");
        let options = WorkerOptions {
            ..Default::default()
        };

        let main_module =
            deno_core::resolve_path("empty_main.js", Path::new(env!("CARGO_MANIFEST_DIR")))
                .unwrap();

        let permissions = PermissionsContainer::new(Permissions::allow_all());

        let mut worker =
            MainWorker::bootstrap_from_options(main_module.clone(), permissions, options);

        worker.execute_main_module(&main_module).await.unwrap();
        worker.run_event_loop(false).await.unwrap();
        Self { worker }
    }

    fn convert(&mut self) {
        println!("inner convert");
        let code = r"1 + 1".to_string();
        self.worker
            .execute_script("ext:<anon>", code.into())
            .unwrap();
    }
}

impl Drop for InnerConverter {
    fn drop(&mut self) {
        println!("drop inner converter");
    }
}

async fn convert3() {
    let mut converter = Converter::new();
    converter.convert().await;
}

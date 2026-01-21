// Minimal example to debug op registration with deno_runtime
use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_runtime::deno_core;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_permissions::{Permissions, PermissionsContainer};
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::worker::{MainWorker, WorkerOptions, WorkerServiceOptions};
use deno_snapshots::CLI_SNAPSHOT;
use node_resolver::{InNpmPackageChecker, NpmPackageFolderResolver};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use sys_traits::impls::RealSys;

// Define a simple test op
deno_core::extension!(test_ops, ops = [op_test_echo]);

#[op2]
#[string]
fn op_test_echo(#[string] msg: String) -> Result<String, JsErrorBox> {
    Ok(format!("Echo: {}", msg))
}

// Stub implementations for npm (not used)
#[derive(Clone)]
pub struct NeverInNpmPackageChecker;
impl InNpmPackageChecker for NeverInNpmPackageChecker {
    fn in_npm_package(&self, _specifier: &deno_core::url::Url) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct NeverNpmPackageFolderResolver;
impl NpmPackageFolderResolver for NeverNpmPackageFolderResolver {
    fn resolve_package_folder_from_package(
        &self,
        specifier: &str,
        referrer: &node_resolver::UrlOrPathRef<'_>,
    ) -> Result<PathBuf, node_resolver::errors::PackageFolderResolveError> {
        use node_resolver::errors::*;
        use node_resolver::UrlOrPath;
        let referrer_owned = if let Ok(url) = referrer.url() {
            UrlOrPath::Url(url.clone())
        } else if let Ok(path) = referrer.path() {
            UrlOrPath::Path(path.to_path_buf())
        } else {
            UrlOrPath::Path(PathBuf::new())
        };
        Err(PackageFolderResolveError(Box::new(
            PackageFolderResolveErrorKind::PackageNotFound(PackageNotFoundError {
                package_name: specifier.to_string(),
                referrer: referrer_owned,
                referrer_extra: None,
            }),
        )))
    }
}

struct NoopModuleLoader;
impl deno_core::ModuleLoader for NoopModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: deno_core::ResolutionKind,
    ) -> Result<deno_core::ModuleSpecifier, JsErrorBox> {
        deno_core::resolve_import(specifier, referrer)
            .map_err(|e| JsErrorBox::generic(e.to_string()))
    }

    fn load(
        &self,
        module_specifier: &deno_core::ModuleSpecifier,
        _maybe_referrer: Option<&deno_core::ModuleLoadReferrer>,
        _is_dyn_import: bool,
        _requested_module_type: deno_core::RequestedModuleType,
    ) -> deno_core::ModuleLoadResponse {
        let specifier = module_specifier.to_string();
        // Return JavaScript code that checks the environment
        let code = if specifier.ends_with("debug_ops.js") {
            r#"
            // Check Deno.internal which might have access to core
            console.log("[Module] Has Deno.internal:", typeof Deno.internal !== 'undefined');
            if (typeof Deno.internal !== 'undefined') {
                console.log("[Module] Deno.internal keys:", Object.keys(Deno.internal).sort());
            }

            // Try importing from ext:core/ops
            console.log("[Module] Trying dynamic import of ext:core/ops...");
            try {
                const coreOps = await import("ext:core/ops");
                console.log("[Module] ext:core/ops keys:", Object.keys(coreOps).sort());
                console.log("[Module] Has op_test_echo in ext:core/ops:", 'op_test_echo' in coreOps);
            } catch (e) {
                console.log("[Module] Error importing ext:core/ops:", e.message);
            }

            // Try accessing through globalThis
            console.log("[Module] globalThis.Deno?.core:", globalThis.Deno?.core);
            "#
        } else {
            ""
        };
        deno_core::ModuleLoadResponse::Sync(Ok(deno_core::ModuleSource::new(
            deno_core::ModuleType::JavaScript,
            deno_core::ModuleSourceCode::String(code.to_string().into()),
            module_specifier,
            None,
        )))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating worker with CLI_SNAPSHOT and custom extension...");
    println!("CLI_SNAPSHOT is Some: {}", CLI_SNAPSHOT.is_some());
    if let Some(snapshot) = CLI_SNAPSHOT {
        println!("CLI_SNAPSHOT size: {} bytes", snapshot.len());
    }

    let module_loader = Rc::new(NoopModuleLoader);
    let options = WorkerOptions {
        extensions: vec![test_ops::init()],
        startup_snapshot: CLI_SNAPSHOT,
        skip_op_registration: false,
        ..Default::default()
    };

    let main_module = deno_core::resolve_path("debug_ops.js", Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let fs: Arc<dyn deno_runtime::deno_fs::FileSystem> = Arc::new(RealFs);
    let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(RealSys));
    let permissions = Permissions::allow_all();

    let mut worker = MainWorker::bootstrap_from_options::<
        NeverInNpmPackageChecker,
        NeverNpmPackageFolderResolver,
        RealSys,
    >(
        &main_module,
        WorkerServiceOptions {
            module_loader,
            permissions: PermissionsContainer::new(permission_desc_parser, permissions),
            blob_store: Default::default(),
            broadcast_channel: Default::default(),
            feature_checker: Default::default(),
            node_services: None,
            npm_process_state_provider: Default::default(),
            root_cert_store_provider: Default::default(),
            fetch_dns_resolver: Default::default(),
            shared_array_buffer_store: Default::default(),
            compiled_wasm_module_store: Default::default(),
            v8_code_cache: Default::default(),
            fs,
            bundle_provider: None,
            deno_rt_native_addon_loader: None,
        },
        options,
    );

    worker.execute_main_module(&main_module).await?;
    worker.run_event_loop(false).await?;

    println!("\nListing all ops in Deno.core.ops...");

    // Check what ops are available
    let result = worker.js_runtime.execute_script(
        "check_ops.js",
        r#"
        // First check what's available at each level
        const hasDeno = typeof Deno !== 'undefined';
        console.log("Has Deno:", hasDeno);

        const hasCore = hasDeno && typeof Deno.core !== 'undefined';
        console.log("Has Deno.core:", hasCore);

        const hasOps = hasCore && typeof Deno.core.ops !== 'undefined';
        console.log("Has Deno.core.ops:", hasOps);

        if (hasCore) {
            console.log("Deno.core keys:", Object.keys(Deno.core).sort());
        }

        let result = { hasDeno, hasCore, hasOps };

        if (hasOps) {
            const ops = Object.keys(Deno.core.ops).sort();
            console.log("Total ops:", ops.length);

            // Check for our custom op
            const hasTestOp = 'op_test_echo' in Deno.core.ops;
            console.log("Has op_test_echo:", hasTestOp);

            result.total = ops.length;
            result.hasTestOp = hasTestOp;
        }

        JSON.stringify(result)
        "#.to_string(),
    )?;

    worker.run_event_loop(false).await?;

    deno_core::scope!(scope, &mut worker.js_runtime);
    let local = v8::Local::new(scope, result);
    let result_str = local.to_rust_string_lossy(scope);
    println!("Result: {}", result_str);

    Ok(())
}

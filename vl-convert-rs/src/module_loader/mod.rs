pub mod import_map;

use crate::module_loader::import_map::build_import_map;
use deno_core::{ModuleCode, ResolutionKind};
use deno_runtime::deno_core::anyhow::Error;
use deno_runtime::deno_core::futures::FutureExt;
use deno_runtime::deno_core::{
    resolve_import, ModuleLoader, ModuleSource, ModuleSourceFuture, ModuleSpecifier, ModuleType,
};
use std::collections::HashMap;
use std::pin::Pin;

lazy_static! {
    pub static ref IMPORT_MAP: HashMap<String, String> = build_import_map();
}

pub struct VlConvertModuleLoader;

impl Default for VlConvertModuleLoader {
    fn default() -> Self {
        Self
    }
}

impl ModuleLoader for VlConvertModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, Error> {
        let resolved = resolve_import(specifier, referrer).unwrap();
        Ok(resolved)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
    ) -> Pin<Box<ModuleSourceFuture>> {
        let module_specifier = module_specifier.clone();
        let string_specifier = module_specifier.to_string();
        // println!("load: {}", string_specifier);

        let code = if string_specifier.ends_with("vl-convert-rs.js") {
            // Load vl-convert-rs.js as an empty file
            // This is the main module, which is required, but we don't need to
            // run any code here
            "".to_string()
        } else {
            IMPORT_MAP
                .get(module_specifier.path())
                .unwrap_or_else(|| {
                    panic!(
                        "Unexpected source file with path: {}",
                        module_specifier.path()
                    )
                })
                .clone()
        };

        futures::future::ready(Ok(ModuleSource::new(
            ModuleType::JavaScript,
            ModuleCode::from(code),
            &module_specifier,
        )))
        .boxed_local()
    }
}

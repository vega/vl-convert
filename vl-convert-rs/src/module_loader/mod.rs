pub mod import_map;

use crate::module_loader::import_map::build_import_map;
use deno_core::anyhow::Error;
use deno_core::futures::FutureExt;
use deno_core::{
    resolve_import, ModuleLoader, ModuleSource, ModuleSourceFuture, ModuleSpecifier, ModuleType,
};
use std::collections::HashMap;
use std::pin::Pin;

pub struct VegaFusionModuleLoader {
    import_map: HashMap<String, String>,
}

impl VegaFusionModuleLoader {
    pub fn new() -> Self {
        Self {
            import_map: build_import_map(),
        }
    }
}

impl Default for VegaFusionModuleLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleLoader for VegaFusionModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
    ) -> Result<ModuleSpecifier, Error> {
        let resolved = resolve_import(specifier, referrer).unwrap();
        Ok(resolved)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
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
            self.import_map
                .get(module_specifier.path())
                .unwrap_or_else(|| {
                    panic!(
                        "Unexpected source file with path: {}",
                        module_specifier.path()
                    )
                })
                .clone()
        };

        async {
            Ok(ModuleSource {
                code: code.into_boxed_str().into_boxed_bytes(),
                module_type: ModuleType::JavaScript,
                module_url_specified: string_specifier.clone(),
                module_url_found: string_specifier,
            })
        }
        .boxed_local()
    }
}

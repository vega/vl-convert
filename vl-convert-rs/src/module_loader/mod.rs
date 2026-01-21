pub mod import_map;

use crate::module_loader::import_map::{
    build_format_locale_map, build_import_map, build_time_format_locale_map, JSDELIVR_URL,
};
use deno_core::{
    ModuleLoadResponse, ModuleLoadReferrer, ModuleSourceCode, RequestedModuleType, ResolutionKind,
};
use deno_error::JsErrorBox;
use deno_runtime::deno_core::{
    resolve_import, ModuleLoader, ModuleSource, ModuleSpecifier, ModuleType,
};
use std::collections::HashMap;

lazy_static! {
    pub static ref IMPORT_MAP: HashMap<String, String> = build_import_map();
    pub static ref FORMATE_LOCALE_MAP: HashMap<String, String> = build_format_locale_map();
    pub static ref TIME_FORMATE_LOCALE_MAP: HashMap<String, String> =
        build_time_format_locale_map();
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
    ) -> Result<ModuleSpecifier, JsErrorBox> {
        let resolved = resolve_import(specifier, referrer)
            .map_err(|e| JsErrorBox::generic(e.to_string()))?;
        Ok(resolved)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _is_dyn_import: bool,
        _requested_module_type: RequestedModuleType,
    ) -> ModuleLoadResponse {
        let module_specifier = module_specifier.clone();
        let string_specifier = module_specifier.to_string();

        let code = if string_specifier.ends_with("vl-convert-rs.js") {
            // Load vl-convert-rs.js as an empty file
            // This is the main module, which is required, but we don't need to
            // run any code here
            "".to_string()
        } else {
            let path = module_specifier.path();
            let path = path.strip_prefix(JSDELIVR_URL).unwrap_or(path);
            // strip the .js extension if it exists
            let path = path.strip_suffix(".js").unwrap_or(path).to_string();
            IMPORT_MAP
                .get(&path)
                .unwrap_or_else(|| panic!("Unexpected source file with path: {}", path))
                .clone()
        };

        ModuleLoadResponse::Sync(Ok(ModuleSource::new(
            ModuleType::JavaScript,
            ModuleSourceCode::String(code.into()),
            &module_specifier,
            None,
        )))
    }
}

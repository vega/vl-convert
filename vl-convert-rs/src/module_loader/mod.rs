pub mod import_map;

use crate::module_loader::import_map::{
    build_format_locale_map, build_import_map, build_time_format_locale_map, JSDELIVR_URL,
    VEGA_PATH, VEGA_THEMES_PATH,
};
use crate::VlVersion;
use deno_core::url::Url;
use deno_core::{ModuleLoadResponse, ModuleSourceCode, RequestedModuleType, ResolutionKind};
use deno_emit::{LoadFuture, LoadOptions, Loader};
use deno_graph::source::LoadResponse;
use deno_runtime::deno_core::anyhow::Error;
use deno_runtime::deno_core::{
    resolve_import, ModuleLoader, ModuleSource, ModuleSpecifier, ModuleType,
};
use regex::Regex;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

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
    ) -> Result<ModuleSpecifier, Error> {
        let resolved = resolve_import(specifier, referrer).unwrap();
        Ok(resolved)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
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

/// Loader implementation used by deno_emit to bundle Vega and dependencies
///
/// The loader has some special logic for vega-embed. When vega-embed is vendored from
/// skypack, it has references to the latest versions of vega, vega-lite, and vega-themes that
/// existed at the time that version was published. The bundle loader overrides the vega and
/// vega-themes versions to match what's used in the rest of vl-convert, and it overrides the
/// vega-lite version with a version passed to the constructor.
pub struct VlConvertBundleLoader {
    pub index_js: String,
    pub name_version_re: Regex,
    pub vegalite_re: Regex,
    pub vega_re: Regex,
    pub vega_themes_re: Regex,
    pub embed_vl_version: VlVersion,
}

impl VlConvertBundleLoader {
    pub fn new(index_js: String, embed_vl_version: VlVersion) -> Self {
        let name_version_re =
            Regex::new(r"(?P<name>[^@]+)@(?P<version>[0-9]+\.[0-9]+\.[0-9]+)").unwrap();
        let vegalite_re = Regex::new(r#"("/npm/vega-lite@[0-9]+\.[0-9]+\.[0-9]+/\+esm")"#).unwrap();
        let vega_re = Regex::new(r#"("/npm/vega@[0-9]+\.[0-9]+\.[0-9]+/\+esm")"#).unwrap();
        let vega_themes_re =
            Regex::new(r#"("/npm/vega-themes@[0-9]+\.[0-9]+\.[0-9]+/\+esm")"#).unwrap();
        Self {
            index_js,
            name_version_re,
            vegalite_re,
            vega_re,
            vega_themes_re,
            embed_vl_version,
        }
    }
}

impl Loader for VlConvertBundleLoader {
    fn load(&self, module_specifier: &ModuleSpecifier, _options: LoadOptions) -> LoadFuture {
        let module_specifier = module_specifier.clone();
        let last_path_part = module_specifier.path().split('/').next_back().unwrap();
        let path = module_specifier.path();
        let path_no_js = path.strip_suffix(".js").unwrap_or(path).to_string();

        let code = if last_path_part == "vl-convert-index.js" {
            self.index_js.clone()
        } else {
            let mut src = IMPORT_MAP
                .get(&path_no_js)
                .unwrap_or_else(|| panic!("Unexpected source file with path: {}", path))
                .clone();

            if let Some(caps) = self.name_version_re.captures(module_specifier.path()) {
                // Drop any leading slash segments
                let name = caps["name"].rsplit('/').next().unwrap();
                if name == "vega-embed" {
                    // Replace vega-lite
                    src = self
                        .vegalite_re
                        .replace_all(&src, format!("\"{}\"", self.embed_vl_version.to_path()))
                        .into_owned();

                    // Replace vega
                    src = self
                        .vega_re
                        .replace_all(&src, format!("\"{}\"", VEGA_PATH))
                        .into_owned();

                    // Replace vega-themes
                    src = self
                        .vega_themes_re
                        .replace_all(&src, format!("\"{}\"", VEGA_THEMES_PATH))
                        .into_owned();
                }
            }

            src
        };

        let code_bytes = code.into_bytes();
        let content: Arc<[u8]> = code_bytes.into_boxed_slice().into();

        // Make new specifier with .js extension, so deno bundle knows the media type
        let url = module_specifier.to_string();
        let url_no_js = url.strip_suffix(".js").unwrap_or(&url).to_string();

        let return_specifier =
            Url::from_str(&format!("{}{}", url_no_js, ".js")).unwrap_or_else(|_| {
                panic!("Failed to parse module specifier {url_no_js} with .js extension")
            });

        Box::pin(async move {
            Ok(Some(LoadResponse::Module {
                specifier: return_specifier,
                maybe_headers: None,
                content,
            }))
        })
    }
}

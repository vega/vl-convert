pub mod import_map;

use crate::converter::domain_matches_patterns;
use crate::deno_emit::{LoadFuture, LoadOptions, Loader};
use crate::module_loader::import_map::{
    build_format_locale_map, build_import_map, build_time_format_locale_map, JSDELIVR_URL,
    VEGA_PATH, VEGA_THEMES_PATH,
};
use crate::VlVersion;
use deno_core::url::Url;
use deno_core::{
    resolve_import, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
    ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, ResolutionKind,
};
use deno_error::JsErrorBox;
use deno_graph::source::{LoadError, LoadResponse};
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
    ) -> Result<ModuleSpecifier, JsErrorBox> {
        let resolved =
            resolve_import(specifier, referrer).map_err(|e| JsErrorBox::generic(e.to_string()))?;
        Ok(resolved)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _options: ModuleLoadOptions,
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

/// Loader for bundling JavaScript with deno_emit.
/// Serves vendored modules from IMPORT_MAP and handles version substitution.
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
        let path = module_specifier.path();

        // Skip source map files - return None to indicate not found
        if path.ends_with(".map") || path.starts_with("/sm/") {
            return Box::pin(async move { Ok(None) });
        }

        let last_path_part = path.split('/').next_back().unwrap();
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
                mtime: None,
            }))
        })
    }
}

/// Loader for bundling plugin ESM modules with HTTP import support.
/// Used at startup (spawn_worker_pool) to bundle plugin dependencies,
/// not at V8 runtime.
pub struct PluginBundleLoader {
    /// The plugin entry module source code.
    pub entry_source: String,
    /// The entry specifier string (used to match the entry module request).
    /// For URL plugins this is the original URL; for inline/file plugins
    /// this is the synthetic vl-plugin-entry.js path.
    pub entry_specifier: String,
    /// Domain allowlist for HTTP fetches.
    pub allowed_domains: Vec<String>,
}

impl Loader for PluginBundleLoader {
    fn load(&self, module_specifier: &ModuleSpecifier, _options: LoadOptions) -> LoadFuture {
        // Serve the entry module source directly
        if module_specifier.to_string() == self.entry_specifier {
            let content: Arc<[u8]> = self.entry_source.as_bytes().into();
            let specifier = module_specifier.clone();
            return Box::pin(async move {
                Ok(Some(LoadResponse::Module {
                    specifier,
                    maybe_headers: Some(std::collections::HashMap::from([(
                        "content-type".to_string(),
                        "application/javascript".to_string(),
                    )])),
                    content,
                    mtime: None,
                }))
            });
        }

        // Handle HTTP/HTTPS imports
        if module_specifier.scheme() == "https" || module_specifier.scheme() == "http" {
            let domain = module_specifier.host_str().unwrap_or("").to_string();
            let allowed = self.allowed_domains.clone();
            let url = module_specifier.to_string();
            let specifier = module_specifier.clone();

            if !domain_matches_patterns(&domain, &allowed) {
                return Box::pin(async move {
                    Err(LoadError::Other(Arc::new(JsErrorBox::generic(format!(
                        "HTTP import blocked: domain '{domain}' not in \
                         allowed_plugin_import_domains. URL: {url}"
                    )))))
                });
            }

            return Box::pin(async move {
                let client = reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .map_err(|e| {
                        LoadError::Other(Arc::new(JsErrorBox::generic(format!(
                            "HTTP client error: {e}"
                        ))))
                    })?;
                let resp = client.get(&url).send().await.map_err(|e| {
                    LoadError::Other(Arc::new(JsErrorBox::generic(format!(
                        "Failed to fetch module {url}: {e}"
                    ))))
                })?;

                // Handle redirects: resolve Location (absolute or relative),
                // check target domain against allowlist
                if resp.status().is_redirection() {
                    if let Some(location) = resp.headers().get("location") {
                        let target = location.to_str().unwrap_or("");
                        // Try absolute URL first, fall back to resolving against request URL
                        let target_url = Url::parse(target)
                            .or_else(|_| specifier.join(target))
                            .map_err(|e| {
                                LoadError::Other(Arc::new(JsErrorBox::generic(format!(
                                    "Invalid redirect URL '{target}' from {url}: {e}"
                                ))))
                            })?;
                        let target_domain = target_url.host_str().unwrap_or("");
                        if !domain_matches_patterns(target_domain, &allowed) {
                            return Err(LoadError::Other(Arc::new(JsErrorBox::generic(format!(
                                "HTTP import redirect blocked: {url} redirected to \
                                     domain '{target_domain}' which is not in \
                                     allowed_plugin_import_domains"
                            )))));
                        }
                        // Return redirect for deno_graph to follow
                        return Ok(Some(LoadResponse::Redirect {
                            specifier: target_url,
                        }));
                    }
                    return Err(LoadError::Other(Arc::new(JsErrorBox::generic(format!(
                        "HTTP import redirect with no valid location: {url}"
                    )))));
                }

                if !resp.status().is_success() {
                    return Err(LoadError::Other(Arc::new(JsErrorBox::generic(format!(
                        "HTTP {} fetching module {url}",
                        resp.status()
                    )))));
                }

                // Forward the response content-type so deno_graph can identify
                // the media type (JavaScript, TypeScript, etc.). Fall back to
                // application/javascript if no header is present.
                let content_type = resp
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("application/javascript")
                    .to_string();

                let code = resp.bytes().await.map_err(|e| {
                    LoadError::Other(Arc::new(JsErrorBox::generic(format!(
                        "Failed to read module {url}: {e}"
                    ))))
                })?;
                let content: Arc<[u8]> = code.to_vec().into_boxed_slice().into();

                Ok(Some(LoadResponse::Module {
                    specifier,
                    maybe_headers: Some(std::collections::HashMap::from([(
                        "content-type".to_string(),
                        content_type,
                    )])),
                    content,
                    mtime: None,
                }))
            });
        }

        // Unknown scheme — return None
        Box::pin(async move { Ok(None) })
    }
}

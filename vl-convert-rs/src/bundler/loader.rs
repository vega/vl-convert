//! Module loader for vl-convert's bundler.
//!
//! Implements deno_graph's Loader trait to serve vendored JavaScript modules
//! from the IMPORT_MAP.

use crate::module_loader::import_map::{VlVersion, VEGA_PATH, VEGA_THEMES_PATH};
use crate::module_loader::IMPORT_MAP;
use deno_graph::source::{LoadError, LoadFuture, LoadOptions, LoadResponse, Loader};
use deno_graph::ModuleSpecifier;
use regex::Regex;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;

/// Loader implementation for serving vendored modules during bundling.
///
/// This loader serves JavaScript modules from vl-convert's vendored IMPORT_MAP,
/// handling version substitution for vega-embed to ensure the correct Vega/Vega-Lite
/// versions are bundled together.
pub struct VlConvertGraphLoader {
    /// The entry point JavaScript code (e.g., the vega snippet to bundle)
    pub entry_js: String,
    /// The Vega-Lite version to use for bundling
    pub vl_version: VlVersion,
    /// Regex for matching package name@version patterns
    name_version_re: Regex,
    /// Regex for matching vega-lite version strings in vega-embed
    vegalite_re: Regex,
    /// Regex for matching vega version strings in vega-embed
    vega_re: Regex,
    /// Regex for matching vega-themes version strings in vega-embed
    vega_themes_re: Regex,
}

impl VlConvertGraphLoader {
    /// Creates a new loader with the given entry point code and Vega-Lite version.
    pub fn new(entry_js: String, vl_version: VlVersion) -> Self {
        // Regexes for version substitution in vega-embed
        let name_version_re =
            Regex::new(r"(?P<name>[^@]+)@(?P<version>[0-9]+\.[0-9]+\.[0-9]+)").unwrap();
        let vegalite_re = Regex::new(r#"("/npm/vega-lite@[0-9]+\.[0-9]+\.[0-9]+/\+esm")"#).unwrap();
        let vega_re = Regex::new(r#"("/npm/vega@[0-9]+\.[0-9]+\.[0-9]+/\+esm")"#).unwrap();
        let vega_themes_re =
            Regex::new(r#"("/npm/vega-themes@[0-9]+\.[0-9]+\.[0-9]+/\+esm")"#).unwrap();

        Self {
            entry_js,
            vl_version,
            name_version_re,
            vegalite_re,
            vega_re,
            vega_themes_re,
        }
    }

    /// Loads a module's source code, applying version substitution if needed.
    fn load_module(&self, specifier: &ModuleSpecifier) -> Result<String, io::Error> {
        // Check if this is our entry point by looking at the last path segment
        let last_path_part = specifier.path().split('/').next_back().unwrap_or("");
        if last_path_part == "vl-convert-index.js" {
            return Ok(self.entry_js.clone());
        }

        // Get the path from the specifier and strip .js extension
        // The IMPORT_MAP stores paths without the .js extension
        let path = specifier.path();
        let path_no_js = path.strip_suffix(".js").unwrap_or(path);

        // Look up in IMPORT_MAP
        let mut src = IMPORT_MAP
            .get(path_no_js)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Module not found in IMPORT_MAP: {}", path_no_js),
                )
            })?
            .clone();

        // Apply version substitution for vega-embed
        if let Some(caps) = self.name_version_re.captures(path_no_js) {
            let name = caps.name("name").map(|m| m.as_str()).unwrap_or("");
            // Extract just the package name from the path
            let name = name.rsplit('/').next().unwrap_or(name);

            if name == "vega-embed" {
                // Get the paths for the target versions
                let vegalite_path = self.vl_version.to_path();
                let vega_path = VEGA_PATH;
                let vega_themes_path = VEGA_THEMES_PATH;

                // Replace vega-lite version
                src = self
                    .vegalite_re
                    .replace_all(&src, format!("\"{}\"", vegalite_path))
                    .into_owned();

                // Replace vega version
                src = self
                    .vega_re
                    .replace_all(&src, format!("\"{}\"", vega_path))
                    .into_owned();

                // Replace vega-themes version
                src = self
                    .vega_themes_re
                    .replace_all(&src, format!("\"{}\"", vega_themes_path))
                    .into_owned();
            }
        }

        Ok(src)
    }
}

impl Loader for VlConvertGraphLoader {
    fn load(&self, specifier: &ModuleSpecifier, _options: LoadOptions) -> LoadFuture {
        let specifier = specifier.clone();
        let result = self.load_module(&specifier);

        Box::pin(async move {
            match result {
                Ok(content) => {
                    // Provide content-type header so deno_graph knows this is JavaScript
                    let mut headers = HashMap::new();
                    headers.insert(
                        "content-type".to_string(),
                        "application/javascript".to_string(),
                    );

                    Ok(Some(LoadResponse::Module {
                        specifier,
                        maybe_headers: Some(headers),
                        content: Arc::from(content.into_bytes()),
                        mtime: None,
                    }))
                }
                Err(e) => Err(LoadError::Other(Arc::new(e))),
            }
        })
    }
}

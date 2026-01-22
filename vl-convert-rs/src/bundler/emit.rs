//! Core bundling logic using SWC bundler.
//!
//! This module implements the JavaScript bundling process by:
//! 1. Building a module graph using deno_graph
//! 2. Transpiling modules using deno_ast
//! 3. Bundling into a single output using SWC bundler

use std::collections::HashMap;
use std::rc::Rc;

use deno_ast::swc::ast::EsVersion;
use deno_ast::swc::bundler::{Bundler, Config as BundlerConfig, Load, ModuleData, Resolve};
use deno_ast::swc::codegen::text_writer::JsWriter;
use deno_ast::swc::codegen::{Config as CodegenConfig, Emitter};
use deno_ast::swc::common::comments::SingleThreadedComments;
use deno_ast::swc::common::sync::Lrc;
use deno_ast::swc::common::{FileName, Globals, SourceMap, GLOBALS};
use deno_ast::swc::loader::resolve::Resolution;
use deno_ast::swc::parser::lexer::Lexer;
use deno_ast::swc::parser::{Parser, StringInput, Syntax};
use deno_ast::MediaType;
use deno_core::anyhow::{self, anyhow, bail};
use deno_graph::{Module, ModuleGraph, ModuleSpecifier};

use super::bundle_hook::BundleHook;
use super::loader::VlConvertGraphLoader;
use super::text::{strip_bom, transform_json_source};
use crate::module_loader::import_map::VlVersion;

/// Options for JavaScript bundling.
#[derive(Debug, Clone)]
pub struct BundleOptions {
    /// The type of bundle output.
    pub bundle_type: BundleType,
    /// Whether to minify the output.
    pub minify: bool,
}

impl Default for BundleOptions {
    fn default() -> Self {
        Self {
            bundle_type: BundleType::Module,
            minify: true,
        }
    }
}

/// The type of bundle output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleType {
    /// ES Module output
    Module,
    /// Classic IIFE output
    Classic,
}

impl From<BundleType> for deno_ast::swc::bundler::ModuleType {
    fn from(bt: BundleType) -> Self {
        match bt {
            BundleType::Module => deno_ast::swc::bundler::ModuleType::Es,
            BundleType::Classic => deno_ast::swc::bundler::ModuleType::Iife,
        }
    }
}

/// The result of a bundle operation.
#[derive(Debug)]
pub struct BundleEmit {
    /// The bundled JavaScript code.
    pub code: String,
}

/// Bundles JavaScript/TypeScript code into a single file.
///
/// # Arguments
/// * `entry_js` - The entry point JavaScript code
/// * `entry_specifier` - The specifier for the entry point
/// * `vl_version` - The Vega-Lite version to bundle
/// * `options` - Bundle configuration options
///
/// # Returns
/// The bundled JavaScript code
pub async fn bundle(
    entry_js: String,
    entry_specifier: ModuleSpecifier,
    vl_version: VlVersion,
    options: BundleOptions,
) -> Result<BundleEmit, anyhow::Error> {
    // Create the graph loader
    let loader = VlConvertGraphLoader::new(entry_js, vl_version);

    // Build the module graph
    let mut graph = ModuleGraph::new(deno_graph::GraphKind::CodeOnly);

    // Build the graph starting from the entry specifier
    graph
        .build(
            vec![entry_specifier.clone()],
            vec![], // No additional imports
            &loader,
            deno_graph::BuildOptions::default(),
        )
        .await;

    // Check for errors in the graph
    graph.valid()?;

    // Bundle the graph
    bundle_graph(&graph, options)
}

/// Bundles a module graph into a single JavaScript file.
fn bundle_graph(graph: &ModuleGraph, options: BundleOptions) -> Result<BundleEmit, anyhow::Error> {
    let globals = Globals::new();

    GLOBALS.set(&globals, || {
        let source_map = Lrc::new(SourceMap::default());

        // Create the bundle loader (implements swc::bundler::Load)
        let bundle_loader = SWCBundleLoader {
            graph,
            source_map: source_map.clone(),
        };

        // Create the bundle resolver (implements swc::bundler::Resolve)
        let bundle_resolver = SWCBundleResolver { graph };

        // Create the hook for import.meta
        let hook = Box::new(BundleHook);

        // Collect external modules (we don't support external, npm, or node modules)
        let external_modules: Vec<_> = graph
            .modules()
            .filter_map(|m| match m {
                Module::External(_) | Module::Node(_) | Module::Npm(_) => {
                    Some(m.specifier().to_string().into())
                }
                _ => None,
            })
            .collect();

        let config = BundlerConfig {
            module: options.bundle_type.into(),
            external_modules,
            ..Default::default()
        };

        let mut bundler = Bundler::new(
            &globals,
            source_map.clone(),
            bundle_loader,
            bundle_resolver,
            config,
            hook,
        );

        // Create entry point
        let mut entries = HashMap::new();
        if let Some(root) = graph.roots.first() {
            entries.insert("bundle".to_string(), FileName::Url(root.clone()));
        } else {
            bail!("No root module in graph");
        }

        // Bundle
        let bundles = bundler.bundle(entries)?;

        if bundles.is_empty() {
            bail!("Bundler produced no output");
        }

        // Generate code
        let mut buf = Vec::new();
        {
            let cfg = CodegenConfig::default()
                .with_minify(options.minify)
                .with_target(EsVersion::Es2020)
                .with_omit_last_semi(false);

            let mut emitter = Emitter {
                cfg,
                cm: source_map.clone(),
                comments: None,
                wr: Box::new(JsWriter::new(source_map.clone(), "\n", &mut buf, None)),
            };

            emitter.emit_module(&bundles[0].module)?;
        }

        let code = String::from_utf8(buf)?;

        Ok(BundleEmit { code })
    })
}

/// SWC bundler Load trait implementation that loads modules from the graph.
struct SWCBundleLoader<'a> {
    graph: &'a ModuleGraph,
    source_map: Lrc<SourceMap>,
}

impl<'a> Load for SWCBundleLoader<'a> {
    fn load(&self, file: &FileName) -> Result<ModuleData, anyhow::Error> {
        let specifier = match file {
            FileName::Url(url) => url,
            _ => bail!("Unsupported file name: {:?}", file),
        };

        let module = self
            .graph
            .get(specifier)
            .ok_or_else(|| anyhow!("Module not found in graph: {}", specifier))?;

        let (source, media_type) = match module {
            Module::Js(m) => (m.source.text.as_ref(), m.media_type),
            Module::Json(m) => (m.source.text.as_ref(), m.media_type),
            Module::Wasm(_) => bail!("WebAssembly modules are not supported for bundling"),
            Module::Npm(_) => bail!("NPM modules are not supported for bundling"),
            Module::Node(_) => bail!("Node built-in modules are not supported for bundling"),
            Module::External(_) => bail!("External modules are not supported for bundling"),
        };

        // Transpile the module
        let (source_file, swc_module) =
            transpile_module(specifier, source, media_type, &self.source_map)?;

        Ok(ModuleData {
            fm: source_file,
            module: swc_module,
            helpers: Default::default(),
        })
    }
}

/// SWC bundler Resolve trait implementation that resolves specifiers using the graph.
struct SWCBundleResolver<'a> {
    graph: &'a ModuleGraph,
}

impl<'a> Resolve for SWCBundleResolver<'a> {
    fn resolve(
        &self,
        base: &FileName,
        module_specifier: &str,
    ) -> Result<Resolution, anyhow::Error> {
        let base_specifier = match base {
            FileName::Url(url) => url,
            _ => bail!("Unsupported base file name: {:?}", base),
        };

        // Use the graph to resolve the dependency
        let resolved = self
            .graph
            .resolve_dependency(module_specifier, base_specifier, false)
            .ok_or_else(|| {
                anyhow!(
                    "Failed to resolve '{}' from '{}'",
                    module_specifier,
                    base_specifier
                )
            })?;

        Ok(Resolution {
            filename: FileName::Url(resolved.clone()),
            slug: None,
        })
    }
}

/// Transpiles a module source into SWC AST.
fn transpile_module(
    specifier: &ModuleSpecifier,
    source: &str,
    media_type: MediaType,
    source_map: &Lrc<SourceMap>,
) -> Result<
    (
        Rc<deno_ast::swc::common::SourceFile>,
        deno_ast::swc::ast::Module,
    ),
    anyhow::Error,
> {
    // Strip BOM if present
    let source = strip_bom(source);

    // Transform JSON to JavaScript if needed
    let source = if media_type == MediaType::Json {
        transform_json_source(source)
    } else {
        source.to_string()
    };

    // Create source file
    let source_file =
        source_map.new_source_file(FileName::Url(specifier.clone()).into(), source.clone());

    // Determine syntax based on media type
    let syntax = match media_type {
        MediaType::TypeScript
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Dts
        | MediaType::Dmts
        | MediaType::Dcts => Syntax::Typescript(deno_ast::swc::parser::TsSyntax {
            tsx: false,
            decorators: true,
            dts: media_type == MediaType::Dts
                || media_type == MediaType::Dmts
                || media_type == MediaType::Dcts,
            no_early_errors: true,
            disallow_ambiguous_jsx_like: false,
        }),
        MediaType::Tsx => Syntax::Typescript(deno_ast::swc::parser::TsSyntax {
            tsx: true,
            decorators: true,
            dts: false,
            no_early_errors: true,
            disallow_ambiguous_jsx_like: false,
        }),
        MediaType::Jsx => Syntax::Es(deno_ast::swc::parser::EsSyntax {
            jsx: true,
            ..Default::default()
        }),
        _ => Syntax::Es(deno_ast::swc::parser::EsSyntax::default()),
    };

    // Parse
    let comments = SingleThreadedComments::default();
    let input = StringInput::from(&*source_file);
    let lexer = Lexer::new(syntax, EsVersion::Es2020, input, Some(&comments));
    let mut parser = Parser::new_from(lexer);

    let module = parser
        .parse_module()
        .map_err(|e| anyhow!("Parse error: {:?}", e))?;

    // For vendored JavaScript, we typically don't need to transpile
    // The source is already in a usable form
    // If TypeScript transpilation is needed, it would be done here using deno_ast

    Ok((Rc::new((*source_file).clone()), module))
}

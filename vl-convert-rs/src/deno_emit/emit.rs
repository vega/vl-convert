// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use base64::Engine;
use deno_ast::get_syntax;
use deno_ast::swc;
use deno_ast::swc::common::comments::SingleThreadedComments;
use deno_ast::swc::common::Mark;
use deno_ast::swc::parser::lexer::Lexer;
use deno_ast::swc::parser::StringInput;
use deno_ast::EmitOptions;
use deno_ast::Marks;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseDiagnostic;
use deno_ast::SourceMap;
use deno_ast::SourceTextInfo;
use deno_ast::TranspileOptions;
use deno_core::anyhow::{anyhow, Context, Result};
use deno_graph::Module;
use std::collections::HashMap;
use std::rc::Rc;

use super::bundle_hook::BundleHook;
use super::text::strip_bom;
use super::text::transform_json_source;

const IGNORE_DIRECTIVES: &[&str] = &[
    "// deno-fmt-ignore-file",
    "// deno-lint-ignore-file",
    "// This code was bundled using `deno bundle` and it's not recommended to edit it manually",
];

pub enum BundleType {
    /// Return the emitted contents of the program as a single "flattened" ES
    /// module.
    Module,
    /// Return the emitted contents of the program as a single script that
    /// executes the program using an immediately invoked function execution
    /// (IIFE).
    Classic,
}

impl From<BundleType> for swc::bundler::ModuleType {
    fn from(bundle_type: BundleType) -> Self {
        match bundle_type {
            BundleType::Classic => Self::Iife,
            BundleType::Module => Self::Es,
        }
    }
}

pub struct BundleOptions {
    pub bundle_type: BundleType,
    pub transpile_options: TranspileOptions,
    pub emit_options: EmitOptions,
    pub emit_ignore_directives: bool,
    pub minify: bool,
}

#[derive(Debug)]
pub struct BundleEmit {
    pub code: String,
    pub maybe_map: Option<String>,
}

struct BundleLoader<'a> {
    cm: &'a SourceMap,
    transpile_options: &'a TranspileOptions,
    graph: &'a deno_graph::ModuleGraph,
}

impl swc::bundler::Load for BundleLoader<'_> {
    fn load(&self, file_name: &swc::common::FileName) -> Result<swc::bundler::ModuleData> {
        match file_name {
            swc::common::FileName::Url(specifier) => {
                let (source, media_type) = match self.graph.get(specifier) {
                    Some(Module::Js(m)) => (m.source.text.as_ref(), m.media_type),
                    Some(Module::Json(m)) => (m.source.text.as_ref(), m.media_type),
                    Some(
                        Module::Npm(_) | Module::Node(_) | Module::External(_) | Module::Wasm(_),
                    ) => {
                        return Err(anyhow!(
                            "Module \"{}\" was an unsupported module kind.",
                            specifier
                        ));
                    }
                    None => {
                        return Err(anyhow!(
                            "Module \"{}\" unexpectedly missing when bundling.",
                            specifier
                        ));
                    }
                };
                let (fm, module) = transpile_module(
                    specifier,
                    source,
                    media_type,
                    self.transpile_options,
                    self.cm,
                )?;
                Ok(swc::bundler::ModuleData {
                    fm,
                    module,
                    helpers: Default::default(),
                })
            }
            _ => unreachable!(
                "Received a request for unsupported filename {:?}",
                file_name
            ),
        }
    }
}

struct BundleResolver<'a>(&'a deno_graph::ModuleGraph);

impl swc::bundler::Resolve for BundleResolver<'_> {
    fn resolve(
        &self,
        referrer: &swc::common::FileName,
        specifier: &str,
    ) -> Result<swc::loader::resolve::Resolution> {
        let referrer = if let swc::common::FileName::Url(referrer) = referrer {
            referrer
        } else {
            unreachable!(
                "An unexpected referrer was passed when bundling: {:?}",
                referrer
            );
        };

        if let Some(specifier) = self.0.resolve_dependency(specifier, referrer, false) {
            Ok(swc::loader::resolve::Resolution {
                filename: deno_ast::swc::common::FileName::Url(specifier.clone()),
                slug: None,
            })
        } else {
            Err(anyhow!(
                "Cannot resolve \"{}\" from \"{}\".",
                specifier,
                referrer
            ))
        }
    }
}

/// Given a module graph, generate and return a bundle of the graph and
/// optionally its source map in memory.
pub fn bundle_graph(graph: &deno_graph::ModuleGraph, options: BundleOptions) -> Result<BundleEmit> {
    let globals = swc::common::Globals::new();
    deno_ast::swc::common::GLOBALS.set(&globals, || {
        let source_map_config = deno_ast::SourceMapConfig {
            inline_sources: options.emit_options.inline_sources,
            maybe_base: None,
        };

        let cm = SourceMap::default();
        let loader = BundleLoader {
            graph,
            transpile_options: &options.transpile_options,
            cm: &cm,
        };
        let resolver = BundleResolver(graph);
        let config = swc::bundler::Config {
            module: options.bundle_type.into(),
            external_modules: graph
                .modules()
                .filter_map(|m| match m {
                    Module::External(_) | Module::Node(_) | Module::Npm(_) | Module::Wasm(_) => {
                        Some(m.specifier().to_string().into())
                    }
                    Module::Js(_) | Module::Json(_) => None,
                })
                .collect(),
            ..Default::default()
        };
        // This hook will rewrite the `import.meta` when bundling to give a consistent
        // behavior between bundled and unbundled code.
        let hook = Box::new(BundleHook);
        let mut bundler = swc::bundler::Bundler::new(
            &globals,
            cm.inner().clone(),
            loader,
            resolver,
            config,
            hook,
        );
        let mut entries = HashMap::new();
        entries.insert(
            "bundle".to_string(),
            swc::common::FileName::Url(graph.roots[0].clone()),
        );
        let output = bundler
            .bundle(entries)
            .context("Unable to output during bundling")?;
        let mut buf = Vec::new();
        let mut srcmap = Vec::new();
        {
            // can't use struct expr because Config has #[non_exhaustive]
            let mut cfg = swc::codegen::Config::default();
            cfg.minify = options.minify;
            cfg.ascii_only = false;
            cfg.target = deno_ast::ES_VERSION;
            cfg.omit_last_semi = false;
            cfg.emit_assert_for_import_attributes = false;
            let mut wr = Box::new(swc::codegen::text_writer::JsWriter::new(
                cm.inner().clone(),
                "\n",
                &mut buf,
                Some(&mut srcmap),
            ));

            if options.emit_ignore_directives {
                // write leading comments in bundled file
                use swc::codegen::text_writer::WriteJs;
                let cmt = IGNORE_DIRECTIVES.join("\n") + "\n\n";
                wr.write_comment(&cmt)?;
            }

            let mut emitter = swc::codegen::Emitter {
                cfg,
                cm: cm.inner().clone(),
                comments: None,
                wr,
            };
            emitter
                .emit_module(&output[0].module)
                .context("Unable to emit during bundling.")?;
        }
        let mut code = shebang_file(graph)
            .map(|shebang| format!("{shebang}\n"))
            .unwrap_or_default();
        code.push_str(&String::from_utf8(buf).context("Emitted code is an invalid string.")?);
        let mut maybe_map: Option<String> = None;
        {
            let mut buf = Vec::new();
            cm.inner()
                .build_source_map(&srcmap, None, source_map_config)
                .to_writer(&mut buf)?;
            match options.emit_options.source_map {
                deno_ast::SourceMapOption::Inline => {
                    code.push_str("//# sourceMappingURL=data:application/json;base64,");
                    base64::prelude::BASE64_STANDARD.encode_string(buf, &mut code);
                }
                deno_ast::SourceMapOption::Separate => {
                    maybe_map = Some(String::from_utf8(buf)?);
                }
                deno_ast::SourceMapOption::None => {}
            }
        }

        Ok(BundleEmit { code, maybe_map })
    })
}

fn shebang_file(graph: &deno_graph::ModuleGraph) -> Option<String> {
    let module = graph.get(graph.roots.first()?)?.js()?;
    let source = &module.source;
    let first_line = source.text.lines().next()?;
    if first_line.starts_with("#!") {
        Some(first_line.to_string())
    } else {
        None
    }
}

/// Transpiles a source module into an swc SourceFile.
fn transpile_module(
    specifier: &ModuleSpecifier,
    source: &str,
    media_type: MediaType,
    options: &deno_ast::TranspileOptions,
    cm: &SourceMap,
) -> Result<(Rc<swc::common::SourceFile>, swc::ast::Module)> {
    let source = strip_bom(source);
    let source = if media_type == MediaType::Json {
        transform_json_source(source)
    } else {
        source.to_string()
    };
    let source_file = cm.new_source_file(specifier.clone(), source);
    let input = StringInput::from(&*source_file);
    let comments = SingleThreadedComments::default();
    let syntax = if media_type == MediaType::Json {
        get_syntax(MediaType::JavaScript)
    } else {
        get_syntax(media_type)
    };
    let lexer = Lexer::new(syntax, deno_ast::ES_VERSION, input, Some(&comments));
    let mut parser = swc::parser::Parser::new_from(lexer);
    let module = parser.parse_module().map_err(|e| {
        ParseDiagnostic::from_swc_error(
            e,
            specifier,
            SourceTextInfo::from_string(source_file.src.to_string()),
        )
    })?;
    let diagnostics = {
        let diagnostics = parser.take_errors();
        if diagnostics.is_empty() {
            Vec::new()
        } else {
            let info = SourceTextInfo::from_string(source_file.src.to_string());
            diagnostics
                .into_iter()
                .map(|e| ParseDiagnostic::from_swc_error(e, specifier, info.clone()))
                .collect::<Vec<_>>()
        }
    };

    let marks = Marks {
        top_level: Mark::fresh(Mark::root()),
        unresolved: Mark::new(),
    };
    let program = deno_ast::fold_program(
        swc::ast::Program::Module(module),
        options,
        cm,
        &comments,
        &marks,
        Box::new(diagnostics.iter()),
    )?;
    let module = match program {
        swc::ast::Program::Module(module) => module,
        _ => unreachable!(),
    };

    Ok((source_file, module))
}

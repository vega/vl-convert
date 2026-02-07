// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Adapted from deno_emit 0.46.0 for vl-convert

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]

mod bundle_hook;
mod emit;
mod text;

use deno_core::anyhow::Result;
use deno_graph::source::ResolveError;
use deno_graph::BuildOptions;
use deno_graph::GraphKind;
use deno_graph::ModuleGraph;
use deno_graph::Range;

pub use emit::bundle_graph;
pub use emit::BundleEmit;
pub use emit::BundleOptions;
pub use emit::BundleType;

pub use deno_ast::EmitOptions;
pub use deno_ast::ModuleSpecifier;
pub use deno_ast::SourceMapOption;
pub use deno_ast::TranspileOptions;
pub use deno_graph::source::LoadFuture;
pub use deno_graph::source::LoadOptions;
pub use deno_graph::source::Loader;

/// Bundle JavaScript/TypeScript modules into a single file.
pub async fn bundle(
    root: ModuleSpecifier,
    loader: &mut dyn Loader,
    options: BundleOptions,
) -> Result<BundleEmit> {
    let resolver = DefaultResolver;
    let mut graph = ModuleGraph::new(GraphKind::CodeOnly);
    graph
        .build(
            vec![root],
            vec![],
            loader,
            BuildOptions {
                resolver: Some(resolver.as_resolver()),
                ..Default::default()
            },
        )
        .await;

    bundle_graph(&graph, options)
}

#[derive(Debug)]
struct DefaultResolver;

impl DefaultResolver {
    pub fn as_resolver(&self) -> &dyn deno_graph::source::Resolver {
        self
    }
}

impl deno_graph::source::Resolver for DefaultResolver {
    fn resolve(
        &self,
        specifier: &str,
        referrer_range: &Range,
        _mode: deno_graph::source::ResolutionKind,
    ) -> Result<ModuleSpecifier, ResolveError> {
        deno_graph::resolve_import(specifier, &referrer_range.specifier).map_err(|err| err.into())
    }
}

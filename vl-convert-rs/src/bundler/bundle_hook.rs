//! Bundle hook for import.meta rewriting.
//!
//! Implements the SWC bundler Hook trait to handle import.meta properties
//! during the bundling process.

use deno_ast::swc::ast::{
    Bool, Expr, KeyValueProp, Lit, MemberExpr, MemberProp, MetaPropExpr, MetaPropKind, PropName,
    Str,
};
use deno_ast::swc::bundler::{Hook, ModuleRecord};
use deno_ast::swc::common::Span;
use deno_core::anyhow;

/// Hook implementation for handling import.meta during bundling.
///
/// This hook rewrites import.meta.url and import.meta.main properties
/// to appropriate values for bundled modules.
pub struct BundleHook;

impl Hook for BundleHook {
    fn get_import_meta_props(
        &self,
        span: Span,
        module_record: &ModuleRecord,
    ) -> Result<Vec<KeyValueProp>, anyhow::Error> {
        Ok(vec![
            // import.meta.url = "<module file name>"
            KeyValueProp {
                key: PropName::Ident("url".into()),
                value: Box::new(Expr::Lit(Lit::Str(Str {
                    span,
                    value: module_record.file_name.to_string().into(),
                    raw: None,
                }))),
            },
            // import.meta.main = true/false based on whether this is the entry module
            KeyValueProp {
                key: PropName::Ident("main".into()),
                value: if module_record.is_entry {
                    // For entry module, preserve the import.meta.main expression
                    Box::new(Expr::Member(MemberExpr {
                        span,
                        obj: Box::new(Expr::MetaProp(MetaPropExpr {
                            span,
                            kind: MetaPropKind::ImportMeta,
                        })),
                        prop: MemberProp::Ident("main".into()),
                    }))
                } else {
                    // For non-entry modules, set to false
                    Box::new(Expr::Lit(Lit::Bool(Bool { span, value: false })))
                },
            },
        ])
    }
}

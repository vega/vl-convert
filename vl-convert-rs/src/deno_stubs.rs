//! Stub implementations for deno_runtime WorkerServiceOptions type parameters.
//!
//! These are minimal no-op implementations used because vl-convert doesn't need
//! npm/node package resolution. When `node_services: None` is passed to MainWorker,
//! these types satisfy the generic bounds but are never actually invoked.

use std::path::PathBuf;

use deno_core::url;
use node_resolver::{InNpmPackageChecker, NpmPackageFolderResolver};

/// No-op implementation of InNpmPackageChecker.
/// Always returns false since vl-convert doesn't use npm packages.
#[derive(Debug, Clone)]
pub struct NoOpInNpmPackageChecker;

impl InNpmPackageChecker for NoOpInNpmPackageChecker {
    fn in_npm_package(&self, _specifier: &url::Url) -> bool {
        false
    }
}

/// No-op implementation of NpmPackageFolderResolver.
/// Always returns an error since vl-convert doesn't resolve npm packages.
#[derive(Debug, Clone)]
pub struct NoOpNpmPackageFolderResolver;

impl NpmPackageFolderResolver for NoOpNpmPackageFolderResolver {
    fn resolve_package_folder_from_package(
        &self,
        _specifier: &str,
        _referrer: &node_resolver::UrlOrPathRef,
    ) -> Result<PathBuf, node_resolver::errors::PackageFolderResolveError> {
        Err(node_resolver::errors::PackageFolderResolveError::from(
            node_resolver::errors::PackageFolderResolveErrorKind::PackageNotFound(
                node_resolver::errors::PackageNotFoundError {
                    package_name: String::new(),
                    referrer: node_resolver::UrlOrPath::Path(PathBuf::new()),
                    referrer_extra: None,
                },
            ),
        ))
    }

    fn resolve_types_package_folder(
        &self,
        _types_package_name: &str,
        _maybe_package_version: Option<&deno_semver::Version>,
        _maybe_referrer: Option<&node_resolver::UrlOrPathRef>,
    ) -> Option<PathBuf> {
        None
    }
}

/// Re-export sys_traits::impls::RealSys as our ExtNodeSys implementation.
/// RealSys implements all required traits: NodeResolverSys + EnvCurrentDir + Clone
/// where NodeResolverSys = FsCanonicalize + FsMetadata + FsRead + FsReadDir + FsOpen
pub use sys_traits::impls::RealSys as VlConvertNodeSys;

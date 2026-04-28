use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use std::path::Path;

use super::config::{ResolvedPlugin, VlcConfig};

/// Bundle a URL plugin using the URL as the deno_emit entry specifier.
pub(crate) async fn bundle_url_plugin(
    url: &str,
    allowed_domains: &[String],
) -> Result<String, AnyError> {
    use crate::deno_emit::{bundle, BundleOptions, BundleType, EmitOptions, SourceMapOption};
    use crate::module_loader::PluginBundleLoader;

    let entry = deno_core::url::Url::parse(url)?;
    let mut loader = PluginBundleLoader {
        entry_source: String::new(),
        entry_specifier: String::new(),
        allowed_domains: allowed_domains.to_vec(),
    };
    let bundled = bundle(
        entry,
        &mut loader,
        BundleOptions {
            bundle_type: BundleType::Module,
            transpile_options: Default::default(),
            emit_options: EmitOptions {
                source_map: SourceMapOption::None,
                ..Default::default()
            },
            emit_ignore_directives: false,
            minify: false,
        },
    )
    .await?;
    Ok(bundled.code)
}

/// Bundle a file/inline plugin into a self-contained ESM string.
pub(crate) async fn bundle_source_plugin(
    source: &str,
    allowed_domains: &[String],
) -> Result<String, AnyError> {
    use crate::deno_emit::{bundle, BundleOptions, BundleType, EmitOptions, SourceMapOption};
    use crate::module_loader::PluginBundleLoader;

    let entry =
        deno_core::resolve_path("vl-plugin-entry.js", Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let entry_str = entry.to_string();
    let mut loader = PluginBundleLoader {
        entry_source: source.to_string(),
        entry_specifier: entry_str,
        allowed_domains: allowed_domains.to_vec(),
    };
    let bundled = bundle(
        entry,
        &mut loader,
        BundleOptions {
            bundle_type: BundleType::Module,
            transpile_options: Default::default(),
            emit_options: EmitOptions {
                source_map: SourceMapOption::None,
                ..Default::default()
            },
            emit_ignore_directives: false,
            minify: false,
        },
    )
    .await?;
    Ok(bundled.code)
}

/// Bundle a single plugin entry (URL or source) into a ResolvedPlugin.
pub(crate) async fn resolve_plugin(
    entry: &str,
    allowed_domains: &[String],
) -> Result<ResolvedPlugin, AnyError> {
    let is_url = entry.starts_with("http://") || entry.starts_with("https://");
    if is_url {
        let bundled = bundle_url_plugin(entry, allowed_domains).await?;
        Ok(ResolvedPlugin {
            original_url: Some(entry.to_string()),
            bundled_source: bundled,
        })
    } else {
        let bundled = bundle_source_plugin(entry, allowed_domains).await?;
        Ok(ResolvedPlugin {
            original_url: None,
            bundled_source: bundled,
        })
    }
}

/// Resolve and bundle all plugins. Runs async on a dedicated thread.
/// Returns the resolved plugins (empty if no plugins configured).
pub(crate) async fn resolve_and_bundle_plugins(
    config: &VlcConfig,
) -> Result<Vec<ResolvedPlugin>, AnyError> {
    if config.vega_plugins.is_empty() {
        return Ok(Vec::new());
    }
    let mut resolved = Vec::new();
    for (i, entry) in config.vega_plugins.iter().enumerate() {
        let plugin = resolve_plugin(entry, &config.plugin_import_domains)
            .await
            .map_err(|e| anyhow!("Vega plugin {i} bundling failed: {e}"))?;
        resolved.push(plugin);
    }
    Ok(resolved)
}

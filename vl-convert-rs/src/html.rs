use crate::converter::{
    classify_and_request_fonts, classify_scenegraph_fonts, GoogleFontRequest, HtmlFontAnalysis,
    MissingFontsPolicy, Renderer, ResolvedPlugin, ValueOrString, VgOpts, VlConvertCommand,
    VlConverter, VlOpts,
};
use crate::deno_emit::{bundle, BundleOptions, BundleType, EmitOptions, SourceMapOption};
use crate::extract::{
    extract_fonts_from_vega, extract_text_by_font, FontForHtml, FontInfo, FontKey, FontSource,
    FontVariant,
};
use crate::font_embed::{generate_font_face_css, inject_locale_chars, variants_by_family};
use crate::module_loader::import_map::{DEBOUNCE_PATH, JSDELIVR_URL, VEGA_EMBED_PATH, VEGA_PATH};
use crate::module_loader::VlConvertBundleLoader;
use crate::text::{GOOGLE_FONTS_CLIENT, USVG_OPTIONS};
use crate::VlVersion;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use vl_convert_google_fonts::{family_to_id, FontStyle, VariantRequest};

/// Escape a string for safe embedding inside a JavaScript template literal.
///
/// Handles backslashes, backticks, `${` interpolation sequences, and
/// `</script>` sequences that would prematurely close the HTML script element.
fn escape_for_template_literal(s: &str) -> String {
    use std::sync::OnceLock;
    static SCRIPT_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = SCRIPT_RE.get_or_init(|| {
        regex::RegexBuilder::new(r"</script")
            .case_insensitive(true)
            .build()
            .unwrap()
    });

    let s = s
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${");
    // HTML safety: prevent </script> from terminating the script element
    re.replace_all(&s, "<\\/script").into_owned()
}

/// Generate JavaScript lines that import and execute resolved plugins.
///
/// - `bundle=true` or file/inline plugins: embed source via blob URL using `__vlcLoadPlugin()`
/// - `bundle=false` + URL plugin: use `import('{original_url}')` directly
fn generate_plugin_imports(plugins: &[ResolvedPlugin], bundle: bool) -> String {
    plugins
        .iter()
        .enumerate()
        .map(|(i, plugin)| {
            if !bundle && plugin.original_url.is_some() {
                // bundle=false + URL plugin: import directly from CDN
                // (browser fetches natively, handles caching)
                let url = plugin.original_url.as_ref().unwrap();
                format!(
                    "        const __vlcPlugin{i} = await import('{url}');\n        __vlcPlugin{i}.default(window.vega);"
                )
            } else {
                // bundle=true (all plugins), or bundle=false + file/inline plugin:
                // embed bundled source via blob URL
                let escaped = escape_for_template_literal(&plugin.bundled_source);
                format!(
                    "        const __vlcPlugin{i} = await __vlcLoadPlugin(`{escaped}`);\n        __vlcPlugin{i}.default(window.vega);"
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn get_vega_or_vegalite_script(
    spec: impl Into<ValueOrString>,
    opts: serde_json::Value,
    resolved_plugins: Option<&[ResolvedPlugin]>,
    bundle: bool,
) -> Result<String, AnyError> {
    let chart_id = "vega-chart";
    let spec_json = match spec.into() {
        ValueOrString::JsonString(s) => s,
        ValueOrString::Value(v) => serde_json::to_string(&v)?,
    };

    // Setup embed opts
    let opts = format!("const opts = {}", serde_json::to_string(&opts)?);

    let has_plugins = resolved_plugins.map_or(false, |p| !p.is_empty());

    let index_js = if has_plugins {
        let plugins = resolved_plugins.unwrap();
        let plugin_imports = generate_plugin_imports(plugins, bundle);
        format!(
            r##"
    try {{
        async function __vlcLoadPlugin(src) {{
            const blob = new Blob([src], {{type: 'text/javascript'}});
            const url = URL.createObjectURL(blob);
            const mod = await import(url);
            URL.revokeObjectURL(url);
            return mod;
        }}
        // Wait for all synchronous scripts (e.g. bundled Vega in <head>) to
        // finish executing before reading window.vega. An `await import()`
        // above yields back to the event loop, which can run before a large
        // classic <script> in <head> completes. A setTimeout(0) macrotask
        // schedules after all pending synchronous script execution.
        await new Promise(r => setTimeout(r, 0));
{plugin_imports}
        const spec = {spec_json};
        {opts}
        await Promise.all([...document.fonts].map(f => f.load()));
        await vegaEmbed('#{chart_id}', spec, opts);
    }} catch(e) {{
        console.error(e);
    }}
"##,
        )
    } else {
        format!(
            r##"
{{
    const spec = {spec_json};
    {opts}
    Promise.all([...document.fonts].map(f => f.load()))
        .then(() => vegaEmbed('#{chart_id}', spec, opts))
        .catch(console.error);
}}
"##,
        )
    };
    Ok(index_js)
}

pub async fn bundle_script(script: String, vl_version: VlVersion) -> Result<String, AnyError> {
    // Bundle dependencies
    let bundle_entry_point =
        deno_core::resolve_path("vl-convert-index.js", Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let mut loader = VlConvertBundleLoader::new(script, vl_version);
    let bundled = bundle(
        bundle_entry_point,
        &mut loader,
        BundleOptions {
            bundle_type: BundleType::Module,
            transpile_options: Default::default(),
            emit_options: EmitOptions {
                source_map: SourceMapOption::None,
                ..Default::default()
            },
            emit_ignore_directives: false,
            minify: true,
        },
    )
    .await?;
    Ok(bundled.code)
}

/// Bundle a JavaScript snippet that may contain references to vegaEmbed, vegaLite, or vega
pub async fn bundle_vega_snippet(snippet: &str, vl_version: VlVersion) -> Result<String, AnyError> {
    let script = format!(
        r#"
import vegaEmbed from "{JSDELIVR_URL}{VEGA_EMBED_PATH}.js"
import * as vega from "{JSDELIVR_URL}{VEGA_PATH}.js"
import * as vegaLite from "{JSDELIVR_URL}{VEGA_LITE_PATH}.js"
import lodashDebounce from "{JSDELIVR_URL}{DEBOUNCE_PATH}.js"
{snippet}
"#,
        VEGA_LITE_PATH = vl_version.to_path()
    );

    bundle_script(script.to_string(), vl_version).await
}

/// Format (weight, style) pairs as CSS2 API axis tuples.
///
/// Requests `ital,wght@{ital},{weight}` tuples when italics are present,
/// or `wght@{weight}` when only normal styles are used.
/// Omits the `ital` axis when no italics are requested to avoid errors
/// on fonts without italic support.
fn format_css2_axis(variants: &BTreeSet<(String, String)>) -> String {
    let has_italic = variants.iter().any(|(_, s)| s == "italic");
    if has_italic {
        let mut parts: Vec<String> = variants
            .iter()
            .map(|(weight, style)| {
                let ital = if style == "italic" { "1" } else { "0" };
                format!("{ital},{weight}")
            })
            .collect();
        parts.sort();
        parts.dedup();
        format!("ital,wght@{}", parts.join(";"))
    } else {
        let mut weights: Vec<String> = variants.iter().map(|(w, _)| w.clone()).collect();
        weights.sort();
        weights.dedup();
        format!("wght@{}", weights.join(";"))
    }
}

/// Return the CDN stylesheet URL for a font.
///
/// Requests exactly the given (weight, style) tuples from the Google Fonts
/// CSS2 API. When `text` is provided, appends `&text=` so Google returns
/// only the glyphs needed — significantly smaller than full unicode-range
/// responses.
///
/// Returns `None` for local fonts.
pub fn font_cdn_url(
    font: &FontForHtml,
    variants: &BTreeSet<(String, String)>,
    text: Option<&str>,
) -> Option<String> {
    match &font.source {
        FontSource::Google { .. } => {
            let name = font.family.replace(' ', "+");
            let axis = format_css2_axis(variants);
            let mut url =
                format!("https://fonts.googleapis.com/css2?family={name}:{axis}&display=swap");
            if let Some(t) = text {
                if !t.is_empty() {
                    url.push_str("&text=");
                    url.push_str(&urlencoding::encode(t));
                }
            }
            Some(url)
        }
        FontSource::Local => None,
    }
}

/// Return an HTML `<link rel="stylesheet">` tag for a font.
/// Returns `None` for local fonts.
pub fn font_link_tag(
    font: &FontForHtml,
    variants: &BTreeSet<(String, String)>,
    text: Option<&str>,
) -> Option<String> {
    let url = font_cdn_url(font, variants, text)?;
    Some(format!(r#"<link rel="stylesheet" href="{url}">"#))
}

/// Return a CSS `@import` rule for a font.
/// Returns `None` for local fonts.
pub fn font_import_rule(
    font: &FontForHtml,
    variants: &BTreeSet<(String, String)>,
    text: Option<&str>,
) -> Option<String> {
    let url = font_cdn_url(font, variants, text)?;
    Some(format!(r#"@import url("{url}");"#))
}

// ---------------------------------------------------------------------------
// HTML export methods on VlConverter
// ---------------------------------------------------------------------------

impl VlConverter {
    async fn build_html(
        &self,
        code: &str,
        vl_version: VlVersion,
        bundle: bool,
        font_head_html: &str,
        has_plugins: bool,
    ) -> Result<String, AnyError> {
        let script_tags = if bundle {
            format!(
                r#"
    <script type="text/javascript">{}</script>
            "#,
                self.get_vegaembed_bundle(vl_version).await?
            )
        } else {
            format!(
                r#"
    <script src="https://cdn.jsdelivr.net/npm/vega@6"></script>
    <script src="https://cdn.jsdelivr.net/npm/vega-lite@{vl_ver}"></script>
    <script src="https://cdn.jsdelivr.net/npm/vega-embed@6"></script>
            "#,
                vl_ver = vl_version.to_semver()
            )
        };

        // Use module script when plugins are present (required for dynamic import());
        // otherwise keep the classic script for backward compatibility.
        let script_type = if has_plugins {
            "module"
        } else {
            "text/javascript"
        };

        Ok(format!(
            r#"<!DOCTYPE html>
<html>
  <head>
    <style>
        vega-chart.vega-embed {{
          width: 100%;
          display: flex;
        }}
        vega-chart.vega-embed details,
        vega-chart.vega-embed details summary {{
          position: relative;
        }}
    </style>
    <meta charset="UTF-8">
    <title>Chart</title>
{font_head_html}{script_tags}
  </head>
  <body>
    <div id="vega-chart"></div>
    <script type="{script_type}">
{code}
    </script>
  </body>
</html>
        "#
        ))
    }

    /// Render a Vega scenegraph for HTML font analysis via msgpack.
    ///
    /// Unlike the public `vega_to_scenegraph`, this uses the caller-supplied
    /// `auto_google_fonts` flag (not the converter config) to decide whether
    /// to auto-detect Google Fonts from the spec. This ensures the render
    /// and the later classification agree on the effective policy.
    ///
    /// Returns msgpack bytes that are deserialized to `serde_json::Value` by
    /// the caller, avoiding the overhead of JSON string serialization in V8.
    async fn render_scenegraph_for_html(
        &self,
        vega_spec: serde_json::Value,
        mut vg_opts: VgOpts,
        auto_google_fonts: bool,
    ) -> Result<serde_json::Value, AnyError> {
        vg_opts.allowed_base_urls =
            self.effective_allowed_base_urls(vg_opts.allowed_base_urls.take())?;
        let missing = self.inner.config.missing_fonts;

        if auto_google_fonts || missing != MissingFontsPolicy::Fallback {
            let font_strings = extract_fonts_from_vega(&vega_spec);
            let auto_requests =
                classify_and_request_fonts(font_strings, auto_google_fonts, missing, true).await?;
            if !auto_requests.is_empty() {
                vg_opts
                    .google_fonts
                    .get_or_insert_with(Vec::new)
                    .extend(auto_requests);
            }
        }

        let vg_spec: ValueOrString = vega_spec.into();
        let msgpack_bytes: Vec<u8> = self
            .request(
                move |responder| VlConvertCommand::VgToSgMsgpack {
                    vg_spec,
                    vg_opts,
                    responder,
                },
                "Vega to Scenegraph msgpack (HTML analysis)",
            )
            .await?;
        let sg: serde_json::Value = rmp_serde::from_slice(&msgpack_bytes)?;
        Ok(sg)
    }

    /// Render the Vega scenegraph, walk it in Rust to extract text-by-font
    /// data, classify fonts as Google or Local, and merge any explicit
    /// per-request Google Font overrides.
    ///
    /// This is the single point of truth for font analysis — called once per
    /// HTML generation or `vega_fonts` / `vegalite_fonts` invocation.
    async fn analyze_html_fonts(
        &self,
        vega_spec: serde_json::Value,
        vg_opts: VgOpts,
        auto_google_fonts: bool,
        embed_local_fonts: bool,
    ) -> Result<HtmlFontAnalysis, AnyError> {
        let missing = self.inner.config.missing_fonts;

        let explicit_requests = vg_opts.google_fonts.clone();
        let format_locale_value = vg_opts
            .format_locale
            .as_ref()
            .and_then(|l| l.as_object().ok());
        let time_format_locale_value = vg_opts
            .time_format_locale
            .as_ref()
            .and_then(|l| l.as_object().ok());

        let sg = self
            .render_scenegraph_for_html(vega_spec, vg_opts, auto_google_fonts)
            .await?;
        let sg_root = sg.get("scenegraph").unwrap_or(&sg);

        let mut chars_by_key = extract_text_by_font(sg_root);

        inject_locale_chars(
            &mut chars_by_key,
            format_locale_value.as_ref(),
            time_format_locale_value.as_ref(),
        );

        let families: BTreeSet<String> = chars_by_key.keys().map(|k| k.family.clone()).collect();

        let explicit_google_families: HashSet<String> = explicit_requests
            .as_ref()
            .map(|reqs| reqs.iter().map(|r| r.family.clone()).collect())
            .unwrap_or_default();

        let mut html_fonts = classify_scenegraph_fonts(
            &families,
            auto_google_fonts,
            embed_local_fonts,
            missing,
            &explicit_google_families,
        )
        .await?;

        let mut family_variants = variants_by_family(&chars_by_key);

        if let Some(ref requests) = explicit_requests {
            let known: HashSet<String> = html_fonts.iter().map(|f| f.family.clone()).collect();
            for req in requests {
                if !known.contains(&req.family) {
                    if let Some(font_id) = family_to_id(&req.family) {
                        html_fonts.push(FontForHtml {
                            family: req.family.clone(),
                            source: FontSource::Google { font_id },
                        });
                    }
                }
                if let Some(ref variants) = req.variants {
                    let entry = family_variants.entry(req.family.clone()).or_default();
                    for v in variants {
                        entry.insert((v.weight.to_string(), v.style.as_str().to_string()));
                    }
                }
            }
        }

        Ok(HtmlFontAnalysis {
            html_fonts,
            chars_by_key,
            family_variants,
        })
    }

    /// Return font information for a Vega spec in the requested format.
    ///
    /// Renders the scenegraph once to discover the exact fonts, weights, and
    /// characters used. The `auto_google_fonts` and `embed_local_fonts`
    /// parameters control which fonts are included.
    pub async fn vega_fonts(
        &self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
        auto_google_fonts: bool,
        embed_local_fonts: bool,
        include_font_face: bool,
        subset_fonts: bool,
    ) -> Result<Vec<FontInfo>, AnyError> {
        let vg_spec = vg_spec.into();
        let spec_value: serde_json::Value = match &vg_spec {
            ValueOrString::JsonString(s) => serde_json::from_str(s)?,
            ValueOrString::Value(v) => v.clone(),
        };

        let analysis = self
            .analyze_html_fonts(spec_value, vg_opts, auto_google_fonts, embed_local_fonts)
            .await?;

        self.build_font_info(analysis, include_font_face, subset_fonts)
            .await
    }

    /// Build structured `FontInfo` from a completed font analysis.
    ///
    /// When `include_font_face` is true, runs the subsetting pipeline to
    /// populate `FontVariant::font_face` on each variant.
    /// When `subset_fonts` is false, embedded fonts include all glyphs and
    /// CDN URLs omit the `&text=` parameter.
    async fn build_font_info(
        &self,
        analysis: HtmlFontAnalysis,
        include_font_face: bool,
        subset_fonts: bool,
    ) -> Result<Vec<FontInfo>, AnyError> {
        let HtmlFontAnalysis {
            html_fonts,
            chars_by_key,
            family_variants,
        } = analysis;

        let font_face_index: HashMap<FontKey, String> =
            if include_font_face && !html_fonts.is_empty() {
                let google_font_requests: Vec<GoogleFontRequest> = html_fonts
                    .iter()
                    .filter_map(|f| match &f.source {
                        FontSource::Google { .. } => {
                            let variants = family_variants.get(&f.family).map(|vs| {
                                vs.iter()
                                    .map(|(w, s)| VariantRequest {
                                        weight: w.parse().unwrap_or(400),
                                        style: s.parse().unwrap_or(FontStyle::Normal),
                                    })
                                    .collect::<Vec<_>>()
                            });
                            Some(GoogleFontRequest {
                                family: f.family.clone(),
                                variants,
                            })
                        }
                        _ => None,
                    })
                    .collect();
                let batches = if google_font_requests.is_empty() {
                    Vec::new()
                } else {
                    self.request(
                        move |responder| VlConvertCommand::ResolveGoogleFonts {
                            google_fonts: google_font_requests,
                            responder,
                        },
                        "Resolve Google Fonts for font-face CSS",
                    )
                    .await?
                };

                let missing = self.inner.config.missing_fonts;
                let fontdb = USVG_OPTIONS
                    .lock()
                    .map_err(|e| anyhow!("failed to lock USVG_OPTIONS: {e}"))?
                    .fontdb
                    .clone();
                generate_font_face_css(
                    &chars_by_key,
                    &html_fonts,
                    &missing,
                    &fontdb,
                    &batches,
                    subset_fonts,
                )?
            } else {
                HashMap::new()
            };

        let mut cdn_variants: HashMap<String, BTreeSet<(String, String)>> = HashMap::new();
        for f in &html_fonts {
            if let FontSource::Google { .. } = &f.source {
                if let Some(vs) = family_variants.get(&f.family) {
                    let requested: Vec<VariantRequest> = vs
                        .iter()
                        .map(|(w, s)| VariantRequest {
                            weight: w.parse().unwrap_or(400),
                            style: s.parse().unwrap_or(FontStyle::Normal),
                        })
                        .collect();
                    match GOOGLE_FONTS_CLIENT
                        .resolve_available_variants(&f.family, &requested)
                        .await
                    {
                        Ok(resolved) => {
                            let set: BTreeSet<(String, String)> = resolved
                                .into_iter()
                                .map(|v| (v.weight.to_string(), v.style.as_str().to_string()))
                                .collect();
                            cdn_variants.insert(f.family.clone(), set);
                        }
                        Err(e) => {
                            log::warn!(
                                "Failed to resolve variants for '{}': {e}, skipping CDN URL",
                                f.family
                            );
                        }
                    }
                }
            }
        }

        let family_chars: HashMap<String, BTreeSet<char>> = if subset_fonts {
            let mut map: HashMap<String, BTreeSet<char>> = HashMap::new();
            for (key, chars) in &chars_by_key {
                map.entry(key.family.clone()).or_default().extend(chars);
            }
            map
        } else {
            HashMap::new()
        };

        let results: Vec<FontInfo> = html_fonts
            .iter()
            .map(|f| {
                let text: Option<String> = family_chars
                    .get(&f.family)
                    .map(|chars| chars.iter().collect());
                let text_ref = text.as_deref();

                let (url, link_tag, import_rule) =
                    if let Some(cdn_set) = cdn_variants.get(&f.family) {
                        (
                            font_cdn_url(f, cdn_set, text_ref),
                            font_link_tag(f, cdn_set, text_ref),
                            font_import_rule(f, cdn_set, text_ref),
                        )
                    } else {
                        (None, None, None)
                    };
                let variants_set = family_variants.get(&f.family);

                let variants: Vec<FontVariant> = variants_set
                    .map(|vs| {
                        vs.iter()
                            .map(|(w, s)| {
                                let font_face = if include_font_face {
                                    let key = FontKey {
                                        family: f.family.clone(),
                                        weight: w.clone(),
                                        style: s.clone(),
                                    };
                                    font_face_index.get(&key).cloned()
                                } else {
                                    None
                                };
                                FontVariant {
                                    weight: w.clone(),
                                    style: s.clone(),
                                    font_face,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                FontInfo {
                    name: f.family.clone(),
                    source: f.source.clone(),
                    variants,
                    url,
                    link_tag,
                    import_rule,
                }
            })
            .collect();

        Ok(results)
    }

    /// Return font information for a Vega-Lite spec.
    ///
    /// Compiles the spec to Vega first, then delegates to [`vega_fonts`].
    pub async fn vegalite_fonts(
        &self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
        auto_google_fonts: bool,
        embed_local_fonts: bool,
        include_font_face: bool,
        subset_fonts: bool,
    ) -> Result<Vec<FontInfo>, AnyError> {
        let vega_spec = self.vegalite_to_vega(vl_spec, vl_opts.clone()).await?;
        let vg_opts = VgOpts {
            allowed_base_urls: vl_opts.allowed_base_urls,
            format_locale: vl_opts.format_locale,
            time_format_locale: vl_opts.time_format_locale,
            google_fonts: vl_opts.google_fonts,
        };
        self.vega_fonts(
            vega_spec,
            vg_opts,
            auto_google_fonts,
            embed_local_fonts,
            include_font_face,
            subset_fonts,
        )
        .await
    }

    /// Build font `<link>` and/or `<style>` tags for HTML `<head>` injection.
    ///
    /// Uses `vega_fonts` internally so the public API is exercised by every
    /// HTML export.
    async fn build_font_head_html(
        &self,
        vega_spec: serde_json::Value,
        vg_opts: VgOpts,
        bundle: bool,
        auto_install: bool,
        embed_local: bool,
        subset_fonts: bool,
    ) -> Result<String, AnyError> {
        let include_font_face = bundle || embed_local;
        let fonts = self
            .vega_fonts(
                vega_spec,
                vg_opts,
                auto_install,
                embed_local,
                include_font_face,
                subset_fonts,
            )
            .await?;

        if bundle {
            let blocks: Vec<&str> = fonts
                .iter()
                .flat_map(|f| f.variants.iter())
                .filter_map(|v| v.font_face.as_deref())
                .collect();
            if blocks.is_empty() {
                Ok(String::new())
            } else {
                let css = blocks.join("\n");
                Ok(format!("    <style>\n{css}\n    </style>\n"))
            }
        } else {
            let mut parts = Vec::new();

            for font in &fonts {
                if let Some(tag) = &font.link_tag {
                    parts.push(format!("    {tag}\n"));
                }
            }

            if embed_local {
                let local_blocks: Vec<&str> = fonts
                    .iter()
                    .filter(|f| matches!(f.source, FontSource::Local))
                    .flat_map(|f| f.variants.iter())
                    .filter_map(|v| v.font_face.as_deref())
                    .collect();
                if !local_blocks.is_empty() {
                    let css = local_blocks.join("\n");
                    parts.push(format!("    <style>\n{css}\n    </style>\n"));
                }
            }

            Ok(parts.join(""))
        }
    }

    /// Convert a Vega-Lite spec to a self-contained HTML page.
    ///
    /// # `bundle` flag
    ///
    /// Controls how **Vega/vega-embed** are delivered:
    /// - `true` — all Vega JS is inlined in a `<script>` tag; the page works offline.
    /// - `false` — Vega/vega-embed are loaded from the jsDelivr CDN via `<script src>`.
    ///
    /// **Plugins are always bundled** (HTTP imports inlined via deno_emit) regardless
    /// of this flag, with one exception: URL-backed plugins (e.g. `https://esm.sh/…`)
    /// are fetched live from their original URL when `bundle=false`, so the browser
    /// benefits from CDN caching. With `bundle=true` their source is inlined too.
    pub async fn vegalite_to_html(
        &self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
        bundle: bool,
        embed_local_fonts: bool,
        subset_fonts: bool,
        renderer: Renderer,
    ) -> Result<String, AnyError> {
        let vl_version = vl_opts.vl_version;
        let vl_spec = vl_spec.into();

        let auto_install = self.inner.config.auto_google_fonts;
        let embed_local = embed_local_fonts;

        let has_font_work = auto_install || embed_local || vl_opts.google_fonts.is_some();
        let font_head_html = if has_font_work {
            let vega_spec = self
                .vegalite_to_vega(vl_spec.clone(), vl_opts.clone())
                .await?;
            let vg_opts = VgOpts {
                allowed_base_urls: vl_opts.allowed_base_urls.clone(),
                format_locale: vl_opts.format_locale.clone(),
                time_format_locale: vl_opts.time_format_locale.clone(),
                google_fonts: vl_opts.google_fonts.clone(),
            };
            self.build_font_head_html(
                vega_spec,
                vg_opts,
                bundle,
                auto_install,
                embed_local,
                subset_fonts,
            )
            .await?
        } else {
            String::new()
        };

        // Ensure plugins are resolved (triggers pool spawn if not yet started)
        if self.inner.config.vega_plugins.is_some() {
            self.warm_up()?;
        }
        let resolved_plugins_owned = self.inner.resolved_plugins.lock().unwrap().clone();
        let resolved_plugins = resolved_plugins_owned.as_deref();
        let has_plugins = resolved_plugins.map_or(false, |p| !p.is_empty());
        let code = get_vega_or_vegalite_script(
            vl_spec,
            vl_opts.to_embed_opts(renderer)?,
            resolved_plugins,
            bundle,
        )?;
        self.build_html(&code, vl_version, bundle, &font_head_html, has_plugins)
            .await
    }

    /// Convert a Vega spec to a self-contained HTML page.
    ///
    /// # `bundle` flag
    ///
    /// Controls how **Vega/vega-embed** are delivered:
    /// - `true` — all Vega JS is inlined in a `<script>` tag; the page works offline.
    /// - `false` — Vega/vega-embed are loaded from the jsDelivr CDN via `<script src>`.
    ///
    /// **Plugins are always bundled** (HTTP imports inlined via deno_emit) regardless
    /// of this flag, with one exception: URL-backed plugins (e.g. `https://esm.sh/…`)
    /// are fetched live from their original URL when `bundle=false`, so the browser
    /// benefits from CDN caching. With `bundle=true` their source is inlined too.
    pub async fn vega_to_html(
        &self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
        bundle: bool,
        embed_local_fonts: bool,
        subset_fonts: bool,
        renderer: Renderer,
    ) -> Result<String, AnyError> {
        let vg_spec = vg_spec.into();

        let auto_install = self.inner.config.auto_google_fonts;
        let embed_local = embed_local_fonts;

        let has_font_work = auto_install || embed_local || vg_opts.google_fonts.is_some();
        let font_head_html = if has_font_work {
            let spec_value: serde_json::Value = match &vg_spec {
                ValueOrString::JsonString(s) => serde_json::from_str(s)?,
                ValueOrString::Value(v) => v.clone(),
            };
            self.build_font_head_html(
                spec_value,
                vg_opts.clone(),
                bundle,
                auto_install,
                embed_local,
                subset_fonts,
            )
            .await?
        } else {
            String::new()
        };

        // Ensure plugins are resolved (triggers pool spawn if not yet started)
        if self.inner.config.vega_plugins.is_some() {
            self.warm_up()?;
        }
        let resolved_plugins_owned = self.inner.resolved_plugins.lock().unwrap().clone();
        let resolved_plugins = resolved_plugins_owned.as_deref();
        let has_plugins = resolved_plugins.map_or(false, |p| !p.is_empty());
        let code = get_vega_or_vegalite_script(
            vg_spec,
            vg_opts.to_embed_opts(renderer)?,
            resolved_plugins,
            bundle,
        )?;
        self.build_html(
            &code,
            Default::default(),
            bundle,
            &font_head_html,
            has_plugins,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn google_font(family: &str) -> FontForHtml {
        FontForHtml {
            family: family.to_string(),
            source: FontSource::Google {
                font_id: family.to_lowercase().replace(' ', "-"),
            },
        }
    }

    fn local_font(family: &str) -> FontForHtml {
        FontForHtml {
            family: family.to_string(),
            source: FontSource::Local,
        }
    }

    // format_css2_axis tests

    #[test]
    fn test_format_css2_axis_normal_only() {
        let vs = BTreeSet::from([("300".to_string(), "normal".to_string())]);
        assert_eq!(format_css2_axis(&vs), "wght@300");
    }

    #[test]
    fn test_format_css2_axis_mixed() {
        let vs = BTreeSet::from([
            ("400".to_string(), "normal".to_string()),
            ("700".to_string(), "italic".to_string()),
            ("300".to_string(), "normal".to_string()),
        ]);
        assert_eq!(format_css2_axis(&vs), "ital,wght@0,300;0,400;1,700");
    }

    // font_cdn_url tests

    #[test]
    fn test_cdn_url_specific_variants() {
        let font = google_font("Roboto");
        let vs = BTreeSet::from([
            ("300".to_string(), "normal".to_string()),
            ("600".to_string(), "italic".to_string()),
        ]);
        let url = font_cdn_url(&font, &vs, None).unwrap();
        assert_eq!(
            url,
            "https://fonts.googleapis.com/css2?family=Roboto:ital,wght@0,300;1,600&display=swap"
        );
    }

    #[test]
    fn test_cdn_url_with_text_subset() {
        let font = google_font("Roboto");
        let vs = BTreeSet::from([("400".to_string(), "normal".to_string())]);
        let url = font_cdn_url(&font, &vs, Some("Hello World")).unwrap();
        assert!(url.ends_with("&text=Hello%20World"));
    }

    #[test]
    fn test_cdn_url_google_font_multi_word() {
        let font = google_font("Playfair Display");
        let vs = BTreeSet::from([("400".to_string(), "normal".to_string())]);
        let url = font_cdn_url(&font, &vs, None).unwrap();
        assert!(url.contains("family=Playfair+Display:wght@"));
    }

    #[test]
    fn test_cdn_url_local_font() {
        let vs = BTreeSet::from([("400".to_string(), "normal".to_string())]);
        assert!(font_cdn_url(&local_font("Arial"), &vs, None).is_none());
    }
}

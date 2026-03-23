#![allow(clippy::too_many_arguments)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::uninlined_format_args)]

use pyo3::exceptions::{PyPermissionError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyModule};
use pythonize::{depythonize, pythonize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use vl_convert_rs::configure_font_cache as configure_font_cache_rs;
use vl_convert_rs::converter::{
    BaseUrlSetting, FormatLocale, GoogleFontRequest, HtmlOpts, JpegOpts, MissingFontsPolicy,
    PdfOpts, PngOpts, Renderer, SvgOpts, TimeFormatLocale, ValueOrString, VgOpts,
    VlConverterConfig, VlOpts, ACCESS_DENIED_MARKER,
};
use vl_convert_rs::module_loader::import_map::{
    VlVersion, VEGA_EMBED_VERSION, VEGA_THEMES_VERSION, VEGA_VERSION, VL_VERSIONS,
};
use vl_convert_rs::module_loader::{FORMATE_LOCALE_MAP, TIME_FORMATE_LOCALE_MAP};
use vl_convert_rs::serde_json;
use vl_convert_rs::text::register_font_directory as register_font_directory_rs;
use vl_convert_rs::VlConverter as VlConverterRs;
use vl_convert_rs::{FontStyle, VariantRequest};

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref VL_CONVERTER: RwLock<Arc<VlConverterRs>> =
        RwLock::new(Arc::new(VlConverterRs::new()));
    static ref PYTHON_RUNTIME: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
    static ref CONFIGURED_GOOGLE_FONTS: RwLock<Option<Vec<GoogleFontRequest>>> = RwLock::new(None);
}

/// Return the configured google_fonts list, merged with any per-call overrides.
fn effective_google_fonts(
    per_call: Option<Vec<GoogleFontRequest>>,
) -> Option<Vec<GoogleFontRequest>> {
    let configured = CONFIGURED_GOOGLE_FONTS
        .read()
        .ok()
        .and_then(|guard| guard.clone());
    match (configured, per_call) {
        (None, None) => None,
        (Some(c), None) => Some(c),
        (None, Some(p)) => Some(p),
        (Some(mut c), Some(p)) => {
            c.extend(p);
            Some(c)
        }
    }
}

fn converter_read_handle() -> Result<Arc<VlConverterRs>, vl_convert_rs::anyhow::Error> {
    VL_CONVERTER
        .read()
        .map_err(|e| vl_convert_rs::anyhow::anyhow!("Failed to acquire converter read lock: {e}"))
        .map(|guard| guard.clone())
}

fn converter_config() -> Result<VlConverterConfig, vl_convert_rs::anyhow::Error> {
    VL_CONVERTER
        .read()
        .map_err(|e| vl_convert_rs::anyhow::anyhow!("Failed to acquire converter read lock: {e}"))
        .map(|guard| guard.config())
}

fn converter_config_json(config: &VlConverterConfig) -> serde_json::Value {
    let base_url_value = match &config.base_url {
        BaseUrlSetting::Default => serde_json::Value::Bool(true),
        BaseUrlSetting::Disabled => serde_json::Value::Bool(false),
        BaseUrlSetting::Custom(url) => serde_json::Value::String(url.clone()),
    };
    serde_json::json!({
        "num_workers": config.num_workers,
        "base_url": base_url_value,
        "allowed_base_urls": config.allowed_base_urls,
        "auto_google_fonts": config.auto_google_fonts,
        "embed_local_fonts": config.embed_local_fonts,
        "missing_fonts": match config.missing_fonts {
            MissingFontsPolicy::Fallback => "fallback",
            MissingFontsPolicy::Warn => "warn",
            MissingFontsPolicy::Error => "error",
        },
        "google_fonts_cache_dir": vl_convert_rs::google_fonts_cache_dir()
            .map(|p| p.to_string_lossy().into_owned()),
        "max_v8_heap_size_mb": config.max_v8_heap_size_mb,
        "max_v8_execution_time_secs": config.max_v8_execution_time_secs,
        "gc_after_conversion": config.gc_after_conversion,
        "vega_plugins": config.vega_plugins,
        "plugin_import_domains": config.plugin_import_domains,
        "allow_per_request_plugins": config.allow_per_request_plugins,
        "per_request_plugin_import_domains": config.per_request_plugin_import_domains,
        "default_theme": config.default_theme,
        "default_format_locale": config.default_format_locale.as_ref().map(|l| match l {
            FormatLocale::Name(n) => serde_json::Value::String(n.clone()),
            FormatLocale::Object(o) => serde_json::to_value(o).unwrap_or(serde_json::Value::Null),
        }),
        "default_time_format_locale": config.default_time_format_locale.as_ref().map(|l| match l {
            TimeFormatLocale::Name(n) => serde_json::Value::String(n.clone()),
            TimeFormatLocale::Object(o) => serde_json::to_value(o).unwrap_or(serde_json::Value::Null),
        }),
        "themes": config.themes,
    })
}

#[derive(Default)]
struct ConverterConfigOverrides {
    num_workers: Option<usize>,
    base_url: Option<BaseUrlSetting>,
    // None => no change, Some(None) => clear, Some(Some(urls)) => set
    allowed_base_urls: Option<Option<Vec<String>>>,
    google_fonts_cache_size_mb: Option<u64>,
    auto_google_fonts: Option<bool>,
    embed_local_fonts: Option<bool>,
    missing_fonts: Option<MissingFontsPolicy>,
    // None => no change, Some(None) => clear, Some(Some(fonts)) => set
    google_fonts: Option<Option<Vec<GoogleFontRequest>>>,
    max_v8_heap_size_mb: Option<usize>,
    max_v8_execution_time_secs: Option<u64>,
    gc_after_conversion: Option<bool>,
    // None => no change, Some(None) => clear, Some(Some(plugins)) => set
    vega_plugins: Option<Option<Vec<String>>>,
    plugin_import_domains: Option<Vec<String>>,
    allow_per_request_plugins: Option<bool>,
    per_request_plugin_import_domains: Option<Vec<String>>,
    // None => no change, Some(None) => clear, Some(Some(theme)) => set
    default_theme: Option<Option<String>>,
    // None => no change, Some(None) => clear, Some(Some(locale)) => set
    default_format_locale: Option<Option<FormatLocale>>,
    // None => no change, Some(None) => clear, Some(Some(locale)) => set
    default_time_format_locale: Option<Option<TimeFormatLocale>>,
    themes: Option<Option<HashMap<String, serde_json::Value>>>,
}

fn parse_config_overrides(
    kwargs: Option<&Bound<'_, PyDict>>,
) -> Result<ConverterConfigOverrides, vl_convert_rs::anyhow::Error> {
    let mut overrides = ConverterConfigOverrides::default();
    let Some(kwargs) = kwargs else {
        return Ok(overrides);
    };

    for (key, value) in kwargs.iter() {
        let key_str: String = key.extract().map_err(|err| {
            vl_convert_rs::anyhow::anyhow!("configure keyword parsing failed: {err}")
        })?;
        match key_str.as_str() {
            "num_workers" => {
                if !value.is_none() {
                    overrides.num_workers = Some(value.extract::<usize>().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid num_workers value for configure: {err}"
                        )
                    })?);
                }
            }
            "base_url" => {
                if value.is_none() {
                    overrides.base_url = Some(BaseUrlSetting::Default);
                } else if let Ok(b) = value.extract::<bool>() {
                    overrides.base_url = Some(if b {
                        BaseUrlSetting::Default
                    } else {
                        BaseUrlSetting::Disabled
                    });
                } else {
                    let s = value.extract::<String>().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid base_url value for configure: {err}. \
                             Expected None, True, False, or a string URL"
                        )
                    })?;
                    overrides.base_url = Some(BaseUrlSetting::Custom(s));
                }
            }
            "allowed_base_urls" => {
                if value.is_none() {
                    overrides.allowed_base_urls = Some(None);
                } else {
                    overrides.allowed_base_urls =
                        Some(Some(value.extract::<Vec<String>>().map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid allowed_base_urls value for configure: {err}"
                            )
                        })?));
                }
            }
            "google_fonts_cache_size_mb" => {
                if !value.is_none() {
                    overrides.google_fonts_cache_size_mb =
                        Some(value.extract::<u64>().map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid google_fonts_cache_size_mb value for configure: {err}"
                            )
                        })?);
                }
            }
            "auto_google_fonts" => {
                if !value.is_none() {
                    overrides.auto_google_fonts = Some(value.extract::<bool>().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid auto_google_fonts value for configure: {err}"
                        )
                    })?);
                }
            }
            "embed_local_fonts" => {
                if !value.is_none() {
                    overrides.embed_local_fonts = Some(value.extract::<bool>().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid embed_local_fonts value for configure: {err}"
                        )
                    })?);
                }
            }
            "missing_fonts" => {
                if !value.is_none() {
                    let s = value.extract::<String>().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid missing_fonts value for configure: {err}"
                        )
                    })?;
                    overrides.missing_fonts = Some(match s.as_str() {
                        "fallback" => MissingFontsPolicy::Fallback,
                        "warn" => MissingFontsPolicy::Warn,
                        "error" => MissingFontsPolicy::Error,
                        _ => {
                            return Err(vl_convert_rs::anyhow::anyhow!(
                                "Invalid missing_fonts value: {s}. Expected 'fallback', 'warn', or 'error'"
                            ));
                        }
                    });
                }
            }
            "google_fonts" => {
                if value.is_none() {
                    overrides.google_fonts = Some(None);
                } else {
                    let fonts: Vec<PyObject> = value.extract().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid google_fonts value for configure: {err}"
                        )
                    })?;
                    let parsed = parse_google_fonts_arg(Some(fonts)).map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid google_fonts value for configure: {err}"
                        )
                    })?;
                    overrides.google_fonts = Some(parsed);
                }
            }
            "max_v8_heap_size_mb" => {
                if !value.is_none() {
                    overrides.max_v8_heap_size_mb =
                        Some(value.extract::<usize>().map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid max_v8_heap_size_mb value for configure: {err}"
                            )
                        })?);
                }
            }
            "max_v8_execution_time_secs" => {
                if !value.is_none() {
                    overrides.max_v8_execution_time_secs =
                        Some(value.extract::<u64>().map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid max_v8_execution_time_secs value for configure: {err}"
                            )
                        })?);
                }
            }
            "gc_after_conversion" => {
                if !value.is_none() {
                    overrides.gc_after_conversion =
                        Some(value.extract::<bool>().map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid gc_after_conversion value for configure: {err}"
                            )
                        })?);
                }
            }
            "vega_plugins" => {
                if value.is_none() {
                    overrides.vega_plugins = Some(None);
                } else {
                    let raw: Vec<String> = value.extract().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid vega_plugins value for configure: {err}"
                        )
                    })?;
                    overrides.vega_plugins = Some(Some(raw));
                }
            }
            "plugin_import_domains" => {
                if value.is_none() {
                    overrides.plugin_import_domains = Some(vec![]);
                } else {
                    overrides.plugin_import_domains =
                        Some(value.extract::<Vec<String>>().map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid plugin_import_domains value for configure: {err}"
                            )
                        })?);
                }
            }
            "allow_per_request_plugins" => {
                if !value.is_none() {
                    overrides.allow_per_request_plugins =
                        Some(value.extract::<bool>().map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid allow_per_request_plugins value for configure: {err}"
                            )
                        })?);
                }
            }
            "per_request_plugin_import_domains" => {
                if value.is_none() {
                    overrides.per_request_plugin_import_domains = Some(vec![]);
                } else {
                    overrides.per_request_plugin_import_domains =
                        Some(value.extract::<Vec<String>>().map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid per_request_plugin_import_domains value for configure: {err}"
                            )
                        })?);
                }
            }
            "default_theme" => {
                if value.is_none() {
                    overrides.default_theme = Some(None);
                } else {
                    overrides.default_theme =
                        Some(Some(value.extract::<String>().map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid default_theme value for configure: {err}"
                            )
                        })?));
                }
            }
            "default_format_locale" => {
                if value.is_none() {
                    overrides.default_format_locale = Some(None);
                } else {
                    let locale = parse_format_locale(value.clone().unbind()).map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid default_format_locale for configure: {err}"
                        )
                    })?;
                    overrides.default_format_locale = Some(Some(locale));
                }
            }
            "default_time_format_locale" => {
                if value.is_none() {
                    overrides.default_time_format_locale = Some(None);
                } else {
                    let locale =
                        parse_time_format_locale(value.clone().unbind()).map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid default_time_format_locale for configure: {err}"
                            )
                        })?;
                    overrides.default_time_format_locale = Some(Some(locale));
                }
            }
            "themes" => {
                if value.is_none() {
                    overrides.themes = Some(None);
                } else {
                    let py_dict: &Bound<'_, PyDict> = value.downcast().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid themes value for configure (expected dict): {err}"
                        )
                    })?;
                    let mut themes_map = HashMap::new();
                    for (k, v) in py_dict.iter() {
                        let key: String = k.extract().map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid theme name (expected string): {err}"
                            )
                        })?;
                        let val: serde_json::Value = depythonize(&v).map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid theme config for '{}': {err}",
                                key
                            )
                        })?;
                        themes_map.insert(key, val);
                    }
                    overrides.themes = Some(Some(themes_map));
                }
            }
            // Read-only config fields returned by get_config() are
            // silently ignored so that `configure(**get_config())` works.
            "google_fonts_cache_dir" => {}
            other => {
                return Err(vl_convert_rs::anyhow::anyhow!(
                    "Unknown configure argument: {other}"
                ));
            }
        }
    }

    Ok(overrides)
}

fn apply_config_overrides(
    config: &mut VlConverterConfig,
    overrides: ConverterConfigOverrides,
) -> Result<(), vl_convert_rs::anyhow::Error> {
    if let Some(num_workers) = overrides.num_workers {
        config.num_workers = num_workers;
    }
    if let Some(base_url) = overrides.base_url {
        config.base_url = base_url;
    }
    if let Some(allowed_base_urls) = overrides.allowed_base_urls {
        config.allowed_base_urls = allowed_base_urls;
    }
    if let Some(mb) = overrides.google_fonts_cache_size_mb {
        let bytes = mb.saturating_mul(1024 * 1024);
        configure_font_cache_rs(Some(bytes))?;
    }
    if let Some(auto_google_fonts) = overrides.auto_google_fonts {
        config.auto_google_fonts = auto_google_fonts;
    }
    if let Some(embed_local_fonts) = overrides.embed_local_fonts {
        config.embed_local_fonts = embed_local_fonts;
    }
    if let Some(missing_fonts) = overrides.missing_fonts {
        config.missing_fonts = missing_fonts;
    }
    if let Some(google_fonts) = overrides.google_fonts {
        let mut guard = CONFIGURED_GOOGLE_FONTS
            .write()
            .map_err(|e| vl_convert_rs::anyhow::anyhow!("Failed to write google_fonts: {e}"))?;
        *guard = google_fonts;
    }
    if let Some(max_v8_heap_size_mb) = overrides.max_v8_heap_size_mb {
        config.max_v8_heap_size_mb = max_v8_heap_size_mb;
    }
    if let Some(max_v8_execution_time_secs) = overrides.max_v8_execution_time_secs {
        config.max_v8_execution_time_secs = max_v8_execution_time_secs;
    }
    if let Some(gc_after_conversion) = overrides.gc_after_conversion {
        config.gc_after_conversion = gc_after_conversion;
    }
    if let Some(vega_plugins) = overrides.vega_plugins {
        config.vega_plugins = vega_plugins;
    }
    if let Some(plugin_import_domains) = overrides.plugin_import_domains {
        config.plugin_import_domains = plugin_import_domains;
    }
    if let Some(allow_per_request_plugins) = overrides.allow_per_request_plugins {
        config.allow_per_request_plugins = allow_per_request_plugins;
    }
    if let Some(per_request_plugin_import_domains) = overrides.per_request_plugin_import_domains {
        config.per_request_plugin_import_domains = per_request_plugin_import_domains;
    }
    if let Some(default_theme) = overrides.default_theme {
        config.default_theme = default_theme;
    }
    if let Some(default_format_locale) = overrides.default_format_locale {
        config.default_format_locale = default_format_locale;
    }
    if let Some(default_time_format_locale) = overrides.default_time_format_locale {
        config.default_time_format_locale = default_time_format_locale;
    }
    if let Some(themes) = overrides.themes {
        config.themes = themes;
    }
    Ok(())
}

fn configure_converter_with_config_overrides(
    overrides: ConverterConfigOverrides,
) -> Result<(), vl_convert_rs::anyhow::Error> {
    let mut guard = VL_CONVERTER.write().map_err(|e| {
        vl_convert_rs::anyhow::anyhow!("Failed to acquire converter write lock: {e}")
    })?;

    let mut config = guard.config();
    apply_config_overrides(&mut config, overrides)?;
    let converter = VlConverterRs::with_config(config)?;
    *guard = Arc::new(converter);
    Ok(())
}

fn run_converter_future<R, Fut, F>(make_future: F) -> Result<R, vl_convert_rs::anyhow::Error>
where
    F: FnOnce(Arc<VlConverterRs>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<R, vl_convert_rs::anyhow::Error>> + 'static,
    R: Send + 'static,
{
    let converter = converter_read_handle()?;
    Python::with_gil(|py| py.allow_threads(move || PYTHON_RUNTIME.block_on(make_future(converter))))
}

fn is_permission_denied_message(message: &str) -> bool {
    if message.contains(ACCESS_DENIED_MARKER) {
        return true;
    }

    let lowercase = message.to_ascii_lowercase();
    lowercase.contains("permission denied")
        || lowercase.contains("access denied")
        || lowercase.contains("requires read access")
        || lowercase.contains("requires net access")
}

fn prefixed_py_error(prefix: &'static str, err: impl std::fmt::Display) -> PyErr {
    let message = format!("{prefix}:\n{err}");
    if is_permission_denied_message(&message) {
        PyPermissionError::new_err(message)
    } else {
        PyValueError::new_err(message)
    }
}

fn future_into_py_object<'py, Fut>(py: Python<'py>, fut: Fut) -> PyResult<Bound<'py, PyAny>>
where
    Fut: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    pyo3_async_runtimes::tokio::future_into_py::<_, PyObject>(py, fut)
}

fn run_converter_future_async<'py, R, Fut, F, C>(
    py: Python<'py>,
    make_future: F,
    error_prefix: &'static str,
    convert: C,
) -> PyResult<Bound<'py, PyAny>>
where
    F: FnOnce(Arc<VlConverterRs>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<R, vl_convert_rs::anyhow::Error>> + Send + 'static,
    R: Send + 'static,
    C: FnOnce(Python<'_>, R) -> PyResult<PyObject> + Send + 'static,
{
    let converter = converter_read_handle().map_err(|err| prefixed_py_error(error_prefix, err))?;

    future_into_py_object(py, async move {
        let value = make_future(converter)
            .await
            .map_err(|err| prefixed_py_error(error_prefix, err))?;
        Python::with_gil(|py| convert(py, value))
    })
}

macro_rules! async_variant_doc {
    ($name:literal) => {
        concat!(
            "Async awaitable variant of `vl_convert.",
            $name,
            "`.\n\nSee `vl_convert.",
            $name,
            "` for full arguments and behavior."
        )
    };
}

/// Convert a Vega-Lite spec to a Vega spec using a particular
/// version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str | None): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     show_warnings (bool | None): Whether to print Vega-Lite compilation warnings (default false)
/// Returns:
///     dict: Vega JSON specification dict
#[pyfunction]
#[pyo3(signature = (vl_spec, vl_version=None, config=None, theme=None, show_warnings=None))]
fn vegalite_to_vega(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
) -> PyResult<PyObject> {
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;

    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),

        format_locale: None,
        time_format_locale: None,
        google_fonts: effective_google_fonts(None),
        vega_plugin: None,
    };

    let vega_spec = match run_converter_future(move |converter| async move {
        converter.vegalite_to_vega(vl_spec, vl_opts).await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(prefixed_py_error(
                "Vega-Lite to Vega conversion failed",
                err,
            ))
        }
    };
    Python::with_gil(|py| -> PyResult<PyObject> {
        pythonize(py, &vega_spec)
            .map_err(|err| PyValueError::new_err(err.to_string()))
            .map(|obj| obj.into())
    })
}

/// Convert a Vega spec to an SVG image string
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     str: SVG image string
#[pyfunction]
#[pyo3(signature = (vg_spec, format_locale=None, time_format_locale=None, vega_plugin=None, bundle=None, subset_fonts=None))]
fn vega_to_svg(
    vg_spec: PyObject,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    bundle: Option<bool>,
    subset_fonts: Option<bool>,
) -> PyResult<String> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    let svg_opts = SvgOpts {
        bundle: bundle.unwrap_or(false),
        subset_fonts: subset_fonts.unwrap_or(true),
    };

    let svg = match run_converter_future(move |converter| async move {
        converter.vega_to_svg(vg_spec, vg_opts, svg_opts).await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega to SVG conversion failed", err)),
    };
    Ok(svg)
}

/// Convert a Vega spec to a Vega Scenegraph
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     format (str): Output format, either "dict" (default) or "msgpack"
/// Returns:
///     dict | bytes: scenegraph as dict (format="dict") or msgpack bytes (format="msgpack")
#[pyfunction]
#[pyo3(signature = (vg_spec, format_locale=None, time_format_locale=None, format="dict", vega_plugin=None))]
fn vega_to_scenegraph(
    vg_spec: PyObject,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    format: &str,
    vega_plugin: Option<String>,
) -> PyResult<PyObject> {
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    match format {
        "dict" => {
            let vg_spec = parse_json_spec(vg_spec)?;
            let sg = run_converter_future(move |converter| async move {
                converter.vega_to_scenegraph(vg_spec, vg_opts).await
            })
            .map_err(|err| prefixed_py_error("Vega to Scenegraph conversion failed", err))?;
            Python::with_gil(|py| -> PyResult<PyObject> {
                pythonize(py, &sg)
                    .map_err(|err| PyValueError::new_err(err.to_string()))
                    .map(|obj| obj.into())
            })
        }
        "msgpack" => {
            let vg_spec = parse_spec_to_value_or_string(vg_spec)?;
            let sg_bytes = run_converter_future(move |converter| async move {
                converter.vega_to_scenegraph_msgpack(vg_spec, vg_opts).await
            })
            .map_err(|err| prefixed_py_error("Vega to Scenegraph conversion failed", err))?;
            Ok(Python::with_gil(|py| -> PyObject {
                PyBytes::new(py, sg_bytes.as_slice()).into()
            }))
        }
        _ => Err(PyValueError::new_err(format!(
            "Invalid format '{format}'. Expected 'dict' or 'msgpack'"
        ))),
    }
}

/// Convert a Vega-Lite spec to an SVG image string using a
/// particular version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str | None): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     show_warnings (bool | None): Whether to print Vega-Lite compilation warnings (default false)
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     str: SVG image string
#[pyfunction]
#[pyo3(
    signature = (vl_spec, vl_version=None, config=None, theme=None, show_warnings=None, format_locale=None, time_format_locale=None, vega_plugin=None, bundle=None, subset_fonts=None)
)]
fn vegalite_to_svg(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    bundle: Option<bool>,
    subset_fonts: Option<bool>,
) -> PyResult<String> {
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    let svg_opts = SvgOpts {
        bundle: bundle.unwrap_or(false),
        subset_fonts: subset_fonts.unwrap_or(true),
    };

    let svg = match run_converter_future(move |converter| async move {
        converter.vegalite_to_svg(vl_spec, vl_opts, svg_opts).await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega-Lite to SVG conversion failed", err)),
    };
    Ok(svg)
}

/// Convert a Vega-Lite spec to a Vega Scenegraph using a
/// particular version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str | None): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     show_warnings (bool | None): Whether to print Vega-Lite compilation warnings (default false)
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     format (str): Output format, either "dict" (default) or "msgpack"
/// Returns:
///     dict | bytes: scenegraph as dict (format="dict") or msgpack bytes (format="msgpack")
#[pyfunction]
#[pyo3(
    signature = (vl_spec, vl_version=None, config=None, theme=None, show_warnings=None, format_locale=None, time_format_locale=None, format="dict", vega_plugin=None)
)]
fn vegalite_to_scenegraph(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    format: &str,
    vega_plugin: Option<String>,
) -> PyResult<PyObject> {
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    match format {
        "dict" => {
            let vl_spec = parse_json_spec(vl_spec)?;
            let sg = run_converter_future(move |converter| async move {
                converter.vegalite_to_scenegraph(vl_spec, vl_opts).await
            })
            .map_err(|err| prefixed_py_error("Vega-Lite to Scenegraph conversion failed", err))?;
            Python::with_gil(|py| -> PyResult<PyObject> {
                pythonize(py, &sg)
                    .map_err(|err| PyValueError::new_err(err.to_string()))
                    .map(|obj| obj.into())
            })
        }
        "msgpack" => {
            let vl_spec = parse_spec_to_value_or_string(vl_spec)?;
            let sg_bytes = run_converter_future(move |converter| async move {
                converter
                    .vegalite_to_scenegraph_msgpack(vl_spec, vl_opts)
                    .await
            })
            .map_err(|err| prefixed_py_error("Vega-Lite to Scenegraph conversion failed", err))?;
            Ok(Python::with_gil(|py| -> PyObject {
                PyBytes::new(py, sg_bytes.as_slice()).into()
            }))
        }
        _ => Err(PyValueError::new_err(format!(
            "Invalid format '{format}'. Expected 'dict' or 'msgpack'"
        ))),
    }
}

/// Convert a Vega spec to PNG image data.
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     scale (float): Image scale factor (default 1.0)
///     ppi (float): Pixels per inch (default 72)
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(
    signature = (vg_spec, scale=None, ppi=None, format_locale=None, time_format_locale=None, vega_plugin=None)
)]
fn vega_to_png(
    vg_spec: PyObject,
    scale: Option<f32>,
    ppi: Option<f32>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
) -> PyResult<PyObject> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    let png_opts = PngOpts { scale, ppi };
    let png_data = match run_converter_future(move |converter| async move {
        converter.vega_to_png(vg_spec, vg_opts, png_opts).await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega to PNG conversion failed", err)),
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, png_data.as_slice()).into()
    }))
}

/// Convert a Vega-Lite spec to PNG image data using a particular
/// version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     scale (float): Image scale factor (default 1.0)
///     ppi (float): Pixels per inch (default 72)
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     show_warnings (bool | None): Whether to print Vega-Lite compilation warnings (default false)
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(
    signature = (vl_spec, vl_version=None, scale=None, ppi=None, config=None, theme=None, show_warnings=None, format_locale=None, time_format_locale=None, vega_plugin=None)
)]
fn vegalite_to_png(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    ppi: Option<f32>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
) -> PyResult<PyObject> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    let png_opts = PngOpts { scale, ppi };
    let png_data = match run_converter_future(move |converter| async move {
        converter.vegalite_to_png(vl_spec, vl_opts, png_opts).await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega-Lite to PNG conversion failed", err)),
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, png_data.as_slice()).into()
    }))
}

/// Convert a Vega spec to JPEG image data.
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     scale (float): Image scale factor (default 1.0)
///     quality (int): JPEG Quality between 0 (worst) and 100 (best). Default 90
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: JPEG image data
#[pyfunction]
#[pyo3(
    signature = (vg_spec, scale=None, quality=None, format_locale=None, time_format_locale=None, vega_plugin=None)
)]
fn vega_to_jpeg(
    vg_spec: PyObject,
    scale: Option<f32>,
    quality: Option<u8>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
) -> PyResult<PyObject> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    let jpeg_opts = JpegOpts { scale, quality };
    let jpeg_data = match run_converter_future(move |converter| async move {
        converter.vega_to_jpeg(vg_spec, vg_opts, jpeg_opts).await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega to JPEG conversion failed", err)),
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, jpeg_data.as_slice()).into()
    }))
}

/// Convert a Vega-Lite spec to JPEG image data using a particular
/// version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     scale (float): Image scale factor (default 1.0)
///     quality (int): JPEG Quality between 0 (worst) and 100 (best). Default 90
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     show_warnings (bool | None): Whether to print Vega-Lite compilation warnings (default false)
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: JPEG image data
#[pyfunction]
#[pyo3(
    signature = (vl_spec, vl_version=None, scale=None, quality=None, config=None, theme=None, show_warnings=None, format_locale=None, time_format_locale=None, vega_plugin=None)
)]
fn vegalite_to_jpeg(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    quality: Option<u8>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
) -> PyResult<PyObject> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    let jpeg_opts = JpegOpts { scale, quality };
    let jpeg_data = match run_converter_future(move |converter| async move {
        converter
            .vegalite_to_jpeg(vl_spec, vl_opts, jpeg_opts)
            .await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(prefixed_py_error(
                "Vega-Lite to JPEG conversion failed",
                err,
            ))
        }
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, jpeg_data.as_slice()).into()
    }))
}

/// Convert a Vega spec to PDF format
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     scale (float): Image scale factor (default 1.0)
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: PDF file bytes
#[pyfunction]
#[pyo3(signature = (vg_spec, scale=None, format_locale=None, time_format_locale=None, vega_plugin=None))]
fn vega_to_pdf(
    vg_spec: PyObject,
    scale: Option<f32>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
) -> PyResult<PyObject> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    let pdf_bytes = match run_converter_future(move |converter| async move {
        converter
            .vega_to_pdf(vg_spec, vg_opts, PdfOpts::default())
            .await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega to PDF conversion failed", err)),
    };
    Ok(Python::with_gil(|py| -> PyObject {
        PyObject::from(PyBytes::new(py, pdf_bytes.as_slice()))
    }))
}

/// Convert a Vega-Lite spec to PDF image data using a particular
/// version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     scale (float): Image scale factor (default 1.0)
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: PDF image data
#[pyfunction]
#[pyo3(
    signature = (vl_spec, vl_version=None, scale=None, config=None, theme=None, format_locale=None, time_format_locale=None, vega_plugin=None)
)]
fn vegalite_to_pdf(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    config: Option<PyObject>,
    theme: Option<String>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
) -> PyResult<PyObject> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: false,
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    let pdf_data = match run_converter_future(move |converter| async move {
        converter
            .vegalite_to_pdf(vl_spec, vl_opts, PdfOpts::default())
            .await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega-Lite to PDF conversion failed", err)),
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyObject::from(PyBytes::new(py, pdf_data.as_slice()))
    }))
}

/// Convert a Vega-Lite spec to a URL that opens the chart in the Vega editor
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     fullscreen (bool): Whether to open the chart in full screen in the editor
/// Returns:
///     str: URL string
#[pyfunction]
#[pyo3(signature = (vl_spec, fullscreen=None))]
fn vegalite_to_url(vl_spec: PyObject, fullscreen: Option<bool>) -> PyResult<String> {
    let vl_spec = parse_json_spec(vl_spec)?;
    Ok(vl_convert_rs::converter::vegalite_to_url(
        &vl_spec,
        fullscreen.unwrap_or(false),
    )?)
}

/// Convert a Vega spec to a URL that opens the chart in the Vega editor
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     fullscreen (bool): Whether to open the chart in full screen in the editor
/// Returns:
///     str: URL string
#[pyfunction]
#[pyo3(signature = (vg_spec, fullscreen=None))]
fn vega_to_url(vg_spec: PyObject, fullscreen: Option<bool>) -> PyResult<String> {
    let vg_spec = parse_json_spec(vg_spec)?;
    Ok(vl_convert_rs::converter::vega_to_url(
        &vg_spec,
        fullscreen.unwrap_or(false),
    )?)
}

/// Convert a Vega-Lite spec to self-contained HTML document using a particular
/// version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     bundle (bool): If True, bundle all dependencies in HTML file
///         If False (default), HTML file will load dependencies from only CDN
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     renderer (str): Vega renderer. One of 'svg' (default), 'canvas',
///         or 'hybrid' (where text is svg and other marks are canvas)
/// Returns:
///     string: HTML document
#[pyfunction]
#[pyo3(
    signature = (vl_spec, vl_version=None, bundle=None, subset_fonts=None, google_fonts=None, config=None, theme=None, format_locale=None, time_format_locale=None, renderer=None, vega_plugin=None)
)]
fn vegalite_to_html(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    bundle: Option<bool>,
    subset_fonts: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    theme: Option<String>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    renderer: Option<String>,
    vega_plugin: Option<String>,
) -> PyResult<String> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let renderer = renderer.unwrap_or_else(|| "svg".to_string());
    let renderer = Renderer::from_str(&renderer)?;
    let subset_fonts = subset_fonts.unwrap_or(true);
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: false,

        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(google_fonts),
        vega_plugin,
    };

    let html_opts = HtmlOpts {
        bundle: bundle.unwrap_or(false),
        subset_fonts,
        renderer,
    };
    run_converter_future(move |converter| async move {
        converter
            .vegalite_to_html(vl_spec, vl_opts, html_opts)
            .await
    })
    .map_err(|err| prefixed_py_error("Vega-Lite to HTML conversion failed", err))
}

/// Convert a Vega spec to a self-contained HTML document
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     bundle (bool): If True, bundle all dependencies in HTML file
///         If False (default), HTML file will load dependencies from only CDN
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     renderer (str): Vega renderer. One of 'svg' (default), 'canvas',
///         or 'hybrid' (where text is svg and other marks are canvas)
/// Returns:
///     string: HTML document
#[pyfunction]
#[pyo3(signature = (vg_spec, bundle=None, subset_fonts=None, google_fonts=None, format_locale=None, time_format_locale=None, renderer=None, vega_plugin=None))]
fn vega_to_html(
    vg_spec: PyObject,
    bundle: Option<bool>,
    subset_fonts: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    renderer: Option<String>,
    vega_plugin: Option<String>,
) -> PyResult<String> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let renderer = renderer.unwrap_or_else(|| "svg".to_string());
    let renderer = Renderer::from_str(&renderer)?;
    let subset_fonts = subset_fonts.unwrap_or(true);
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(google_fonts),
        vega_plugin,
    };
    let html_opts = HtmlOpts {
        bundle: bundle.unwrap_or(false),
        subset_fonts,
        renderer,
    };
    run_converter_future(move |converter| async move {
        converter.vega_to_html(vg_spec, vg_opts, html_opts).await
    })
    .map_err(|err| prefixed_py_error("Vega to HTML conversion failed", err))
}

/// Return font information for a rendered Vega-Lite spec
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str): Vega-Lite library version string (e.g. '5.15')
///         (default to latest)
///     config (str | dict): Chart configuration object or JSON string
///     theme (str): Named theme (e.g. "dark")
///     auto_google_fonts (bool): Override auto-download from Google Fonts
///         (default: use converter config)
///     include_font_face (bool): Whether to run the font subsetting pipeline
///         and populate the font_face field on each variant (default False)
/// Returns:
///     list[FontInfo]: Structured font metadata for each font used by the chart
#[pyfunction]
#[pyo3(signature = (vl_spec, vl_version=None, config=None, theme=None, auto_google_fonts=None, include_font_face=false, subset_fonts=None, google_fonts=None, format_locale=None, time_format_locale=None))]
fn vegalite_fonts(
    py: Python<'_>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    auto_google_fonts: Option<bool>,
    include_font_face: bool,
    subset_fonts: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<PyObject> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: false,

        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(google_fonts),
        vega_plugin: None,
    };

    let result = run_converter_future(move |converter| async move {
        let config = converter.config();
        let auto_gf = auto_google_fonts.unwrap_or(config.auto_google_fonts);
        let embed_lf = config.embed_local_fonts;
        converter
            .vegalite_fonts(
                vl_spec,
                vl_opts,
                auto_gf,
                embed_lf,
                include_font_face,
                subset_fonts.unwrap_or(true),
            )
            .await
    })
    .map_err(|err| prefixed_py_error("vegalite_fonts request failed", err))?;

    pythonize(py, &result)
        .map_err(|err| PyValueError::new_err(err.to_string()))
        .map(|obj| obj.into())
}

/// Return font information for a rendered Vega spec
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     auto_google_fonts (bool): Override auto-download from Google Fonts
///         (default: use converter config)
///     include_font_face (bool): Whether to run the font subsetting pipeline
///         and populate the font_face field on each variant (default False)
/// Returns:
///     list[FontInfo]: Structured font metadata for each font used by the chart
#[pyfunction]
#[pyo3(signature = (vg_spec, auto_google_fonts=None, include_font_face=false, subset_fonts=None, google_fonts=None, format_locale=None, time_format_locale=None))]
fn vega_fonts(
    py: Python<'_>,
    vg_spec: PyObject,
    auto_google_fonts: Option<bool>,
    include_font_face: bool,
    subset_fonts: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<PyObject> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(google_fonts),
        vega_plugin: None,
    };

    let result = run_converter_future(move |converter| async move {
        let config = converter.config();
        let auto_gf = auto_google_fonts.unwrap_or(config.auto_google_fonts);
        let embed_lf = config.embed_local_fonts;
        converter
            .vega_fonts(
                vg_spec,
                vg_opts,
                auto_gf,
                embed_lf,
                include_font_face,
                subset_fonts.unwrap_or(true),
            )
            .await
    })
    .map_err(|err| prefixed_py_error("vega_fonts request failed", err))?;

    pythonize(py, &result)
        .map_err(|err| PyValueError::new_err(err.to_string()))
        .map(|obj| obj.into())
}

/// Convert an SVG image string to PNG image data
///
/// Args:
///     svg (str): SVG image string
///     scale (float): Image scale factor (default 1.0)
///     ppi (float): Pixels per inch (default 72)
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(signature = (svg, scale=None, ppi=None))]
fn svg_to_png(svg: &str, scale: Option<f32>, ppi: Option<f32>) -> PyResult<PyObject> {
    let svg = svg.to_string();
    let png_opts = PngOpts { scale, ppi };
    let png_data =
        run_converter_future(
            move |converter| async move { converter.svg_to_png(&svg, png_opts).await },
        )
        .map_err(|err| prefixed_py_error("SVG to PNG conversion failed", err))?;
    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, png_data.as_slice()).into()
    }))
}

/// Convert an SVG image string to JPEG image data
///
/// Args:
///     svg (str): SVG image string
///     scale (float): Image scale factor (default 1.0)
///     quality (int): JPEG Quality between 0 (worst) and 100 (best). Default 90
/// Returns:
///     bytes: JPEG image data
#[pyfunction]
#[pyo3(signature = (svg, scale=None, quality=None))]
fn svg_to_jpeg(svg: &str, scale: Option<f32>, quality: Option<u8>) -> PyResult<PyObject> {
    let svg = svg.to_string();
    let jpeg_opts = JpegOpts { scale, quality };
    let jpeg_data = run_converter_future(move |converter| async move {
        converter.svg_to_jpeg(&svg, jpeg_opts).await
    })
    .map_err(|err| prefixed_py_error("SVG to JPEG conversion failed", err))?;
    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, jpeg_data.as_slice()).into()
    }))
}

/// Convert an SVG image string to PDF document data
///
/// Args:
///     svg (str): SVG image string
///     scale (float): Image scale factor (default 1.0)
/// Returns:
///     bytes: PDF document data
#[pyfunction]
#[pyo3(signature = (svg, scale=None))]
fn svg_to_pdf(svg: &str, scale: Option<f32>) -> PyResult<PyObject> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let svg = svg.to_string();
    let pdf_data = run_converter_future(move |converter| async move {
        converter.svg_to_pdf(&svg, PdfOpts::default()).await
    })
    .map_err(|err| prefixed_py_error("SVG to PDF conversion failed", err))?;
    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, pdf_data.as_slice()).into()
    }))
}

/// Helper function to parse an input Python string or dict as a serde_json::Value
fn parse_json_spec(vl_spec: PyObject) -> PyResult<serde_json::Value> {
    Python::with_gil(|py| -> PyResult<serde_json::Value> {
        if let Ok(vl_spec) = vl_spec.extract::<Cow<str>>(py) {
            match serde_json::from_str::<serde_json::Value>(vl_spec.as_ref()) {
                Ok(vl_spec) => Ok(vl_spec),
                Err(err) => Err(PyValueError::new_err(format!(
                    "Failed to parse vl_spec string as JSON: {}",
                    err
                ))),
            }
        } else if let Ok(vl_spec) = vl_spec.downcast_bound::<PyDict>(py) {
            match depythonize(vl_spec.as_any()) {
                Ok(vl_spec) => Ok(vl_spec),
                Err(err) => Err(PyValueError::new_err(format!(
                    "Failed to parse vl_spec dict as JSON: {}",
                    err
                ))),
            }
        } else {
            Err(PyValueError::new_err("vl_spec must be a string or dict"))
        }
    })
}

fn parse_optional_config(config: Option<PyObject>) -> PyResult<Option<serde_json::Value>> {
    config.map(parse_json_spec).transpose()
}

/// Helper function to parse a Python string or dict as a ValueOrString.
/// When input is a string, returns ValueOrString::JsonString to avoid
/// the serde_json::from_str/to_string round-trip.
fn parse_spec_to_value_or_string(vl_spec: PyObject) -> PyResult<ValueOrString> {
    Python::with_gil(|py| -> PyResult<ValueOrString> {
        if let Ok(vl_spec) = vl_spec.extract::<String>(py) {
            Ok(ValueOrString::JsonString(vl_spec))
        } else if let Ok(vl_spec) = vl_spec.downcast_bound::<PyDict>(py) {
            match depythonize(vl_spec.as_any()) {
                Ok(vl_spec) => Ok(ValueOrString::Value(vl_spec)),
                Err(err) => Err(PyValueError::new_err(format!(
                    "Failed to parse vl_spec dict as JSON: {}",
                    err
                ))),
            }
        } else {
            Err(PyValueError::new_err("vl_spec must be a string or dict"))
        }
    })
}

/// Helper function to parse an input Python string or dict as a FormatLocale
fn parse_format_locale(v: PyObject) -> PyResult<FormatLocale> {
    Python::with_gil(|py| -> PyResult<FormatLocale> {
        if let Ok(name) = v.extract::<Cow<str>>(py) {
            let format_locale = FormatLocale::Name(name.as_ref().to_string());
            if format_locale.as_object().is_err() {
                Err(PyValueError::new_err(
                    format!("Invalid format_locale name: {name}\nSee https://github.com/d3/d3-format/tree/main/locale for available names")
                ))
            } else {
                Ok(format_locale)
            }
        } else if let Ok(obj) = v.downcast_bound::<PyDict>(py) {
            match depythonize(obj.as_any()) {
                Ok(obj) => Ok(FormatLocale::Object(obj)),
                Err(err) => Err(PyValueError::new_err(format!(
                    "Failed to parse format_locale dict as JSON: {}",
                    err
                ))),
            }
        } else {
            Err(PyValueError::new_err(
                "format_locale must be a string or dict",
            ))
        }
    })
}
fn parse_option_format_locale(v: Option<PyObject>) -> PyResult<Option<FormatLocale>> {
    match v {
        None => Ok(None),
        Some(v) => Ok(Some(parse_format_locale(v)?)),
    }
}

/// Helper function to parse an input Python string or dict as a TimeFormatLocale
fn parse_time_format_locale(v: PyObject) -> PyResult<TimeFormatLocale> {
    Python::with_gil(|py| -> PyResult<TimeFormatLocale> {
        if let Ok(name) = v.extract::<Cow<str>>(py) {
            let time_format_locale = TimeFormatLocale::Name(name.as_ref().to_string());
            if time_format_locale.as_object().is_err() {
                Err(PyValueError::new_err(
                    format!("Invalid time_format_locale name: {name}\nSee https://github.com/d3/d3-time-format/tree/main/locale for available names")
                ))
            } else {
                Ok(time_format_locale)
            }
        } else if let Ok(obj) = v.downcast_bound::<PyDict>(py) {
            match depythonize(obj.as_any()) {
                Ok(obj) => Ok(TimeFormatLocale::Object(obj)),
                Err(err) => Err(PyValueError::new_err(format!(
                    "Failed to parse time_format_locale dict as JSON: {}",
                    err
                ))),
            }
        } else {
            Err(PyValueError::new_err(
                "time_format_locale must be a string or dict",
            ))
        }
    })
}

fn parse_option_time_format_locale(v: Option<PyObject>) -> PyResult<Option<TimeFormatLocale>> {
    match v {
        None => Ok(None),
        Some(v) => Ok(Some(parse_time_format_locale(v)?)),
    }
}

fn parse_embedded_locale_json(raw: &str, kind: &str) -> PyResult<serde_json::Value> {
    serde_json::from_str(raw).map_err(|err| {
        PyValueError::new_err(format!("Failed to parse internal {kind} as JSON: {err}"))
    })
}

/// Register a directory of fonts for use in subsequent conversions
///
/// Args:
///     font_dir (str): Absolute path to a directory containing font files
///
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(signature = (font_dir))]
fn register_font_directory(font_dir: &str) -> PyResult<()> {
    register_font_directory_rs(font_dir).map_err(|err| {
        PyValueError::new_err(format!("Failed to register font directory: {}", err))
    })?;
    Ok(())
}

/// Parse Python variant tuples into VariantRequest vec.
fn parse_variant_args(
    variants: Option<Vec<(u16, String)>>,
) -> PyResult<Option<Vec<VariantRequest>>> {
    match variants {
        None => Ok(None),
        Some(tuples) => {
            let mut requests = Vec::with_capacity(tuples.len());
            for (weight, style_str) in tuples {
                let style = match style_str.as_str() {
                    "normal" => FontStyle::Normal,
                    "italic" => FontStyle::Italic,
                    other => {
                        return Err(PyValueError::new_err(format!(
                            "Invalid font style '{}'. Must be 'normal' or 'italic'",
                            other
                        )))
                    }
                };
                requests.push(VariantRequest { weight, style });
            }
            Ok(Some(requests))
        }
    }
}

/// Parse a `google_fonts` list argument into `Vec<GoogleFontRequest>`.
///
/// Accepts `list[dict]` where each dict has `"family"` (str, required) and
/// optionally `"variants"` (list of `(weight, style)` tuples).
/// Parse a `google_fonts` argument into `Vec<GoogleFontRequest>`.
///
/// Each entry can be:
/// - A `str` — interpreted as a font family name (all variants)
/// - A `dict` with `"family"` (str, required) and optionally `"variants"`
///   (list of `(weight, style)` tuples)
fn parse_google_fonts_arg(
    fonts: Option<Vec<PyObject>>,
) -> PyResult<Option<Vec<GoogleFontRequest>>> {
    let Some(fonts) = fonts else {
        return Ok(None);
    };
    if fonts.is_empty() {
        return Ok(None);
    }
    Python::with_gil(|py| {
        let mut requests = Vec::with_capacity(fonts.len());
        for obj in &fonts {
            let bound = obj.bind(py);
            if let Ok(family) = bound.extract::<String>() {
                requests.push(GoogleFontRequest {
                    family,
                    variants: None,
                });
            } else if let Ok(dict) = bound.downcast::<PyDict>() {
                let family: String = dict
                    .get_item("family")?
                    .ok_or_else(|| {
                        PyValueError::new_err("google_fonts dict entry missing 'family' key")
                    })?
                    .extract()?;
                let variants: Option<Vec<(u16, String)>> = dict
                    .get_item("variants")?
                    .map(|v| v.extract())
                    .transpose()?;
                let variants = parse_variant_args(variants)?;
                requests.push(GoogleFontRequest { family, variants });
            } else {
                return Err(PyValueError::new_err(
                    "Each google_fonts entry must be a str or dict with 'family' key",
                ));
            }
        }
        Ok(Some(requests))
    })
}

/// Configure converter options for subsequent requests
#[pyfunction]
#[pyo3(signature = (**kwargs))]
fn configure(kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    let overrides = parse_config_overrides(kwargs)
        .map_err(|err| prefixed_py_error("Failed to configure converter", err))?;
    configure_converter_with_config_overrides(overrides)
        .map_err(|err| prefixed_py_error("Failed to configure converter", err))
}

/// Get the currently configured converter options.
#[pyfunction(name = "get_config")]
#[pyo3(signature = ())]
fn get_config() -> PyResult<PyObject> {
    let config = converter_config()
        .map_err(|err| prefixed_py_error("Failed to read converter config", err))?;
    Python::with_gil(|py| {
        pythonize(py, &converter_config_json(&config))
            .map_err(|err| PyValueError::new_err(err.to_string()))
            .map(|obj| obj.into())
    })
}

/// Eagerly start converter workers for the current converter configuration
#[pyfunction]
#[pyo3(signature = ())]
fn warm_up_workers() -> PyResult<()> {
    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("warm_up_workers request failed", err))?;

    Python::with_gil(|py| py.allow_threads(move || converter.warm_up()))
        .map_err(|err| prefixed_py_error("warm_up_workers request failed", err))?;
    Ok(())
}

/// Get the named local timezone that Vega uses to perform timezone calculations
///
/// Returns:
///     str: Named local timezone (e.g. "America/New_York"),
///          or None if the local timezone cannot be determined
#[pyfunction]
#[pyo3(signature = ())]
fn get_local_tz() -> PyResult<Option<String>> {
    run_converter_future(|converter| async move { converter.get_local_tz().await })
        .map_err(|err| prefixed_py_error("get_local_tz request failed", err))
}

/// Get V8 memory usage for each worker in the converter pool.
///
/// Returns:
///     list[dict]: List of dicts with keys ``worker_index``, ``used_heap_size``,
///         ``total_heap_size``, ``heap_size_limit``, and ``external_memory``
///         (all sizes in bytes).
#[pyfunction]
#[pyo3(signature = ())]
fn get_worker_memory_usage() -> PyResult<PyObject> {
    let stats =
        run_converter_future(|converter| async move { converter.get_worker_memory_usage().await })
            .map_err(|err| prefixed_py_error("get_worker_memory_usage request failed", err))?;

    Python::with_gil(|py| {
        let list = pyo3::types::PyList::empty(py);
        for s in &stats {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("worker_index", s.worker_index)?;
            dict.set_item("used_heap_size", s.used_heap_size)?;
            dict.set_item("total_heap_size", s.total_heap_size)?;
            dict.set_item("heap_size_limit", s.heap_size_limit)?;
            dict.set_item("external_memory", s.external_memory)?;
            list.append(dict)?;
        }
        Ok(list.into())
    })
}

/// Get the config dict for each built-in theme
///
/// Returns:
///     dict: dict from theme name to config object
#[pyfunction]
#[pyo3(signature = ())]
fn get_themes() -> PyResult<PyObject> {
    let themes = run_converter_future(|converter| async move { converter.get_themes().await })
        .map_err(|err| prefixed_py_error("get_themes request failed", err))?;
    Python::with_gil(|py| -> PyResult<PyObject> {
        pythonize(py, &themes)
            .map_err(|err| PyValueError::new_err(err.to_string()))
            .map(|obj| obj.into())
    })
}

/// Get the d3-format locale dict for a named locale
///
/// See https://github.com/d3/d3-format/tree/main/locale for available names
///
/// Args:
///     name (str): d3-format locale name (e.g. 'it-IT')
///
/// Returns:
///     dict: d3-format locale dict
#[pyfunction]
#[pyo3(signature = (name))]
fn get_format_locale(name: &str) -> PyResult<PyObject> {
    match FORMATE_LOCALE_MAP.get(name) {
        None => {
            Err(PyValueError::new_err(format!(
                "Invalid format locale name: {name}\nSee https://github.com/d3/d3-format/tree/main/locale for available names"
            )))
        }
        Some(locale) => {
            let locale = parse_embedded_locale_json(locale, "format locale")?;
            Python::with_gil(|py| -> PyResult<PyObject> {
                pythonize(py, &locale).map_err(|err| PyValueError::new_err(err.to_string())).map(|obj| obj.into())
            })
        }
    }
}

/// Get the d3-time-format locale dict for a named locale
///
/// See https://github.com/d3/d3-time-format/tree/main/locale for available names
///
/// Args:
///     name (str): d3-time-format locale name (e.g. 'it-IT')
///
/// Returns:
///     dict: d3-time-format locale dict
#[pyfunction]
#[pyo3(signature = (name))]
fn get_time_format_locale(name: &str) -> PyResult<PyObject> {
    match TIME_FORMATE_LOCALE_MAP.get(name) {
        None => {
            Err(PyValueError::new_err(format!(
                "Invalid time format locale name: {name}\nSee https://github.com/d3/d3-time-format/tree/main/locale for available names"
            )))
        }
        Some(locale) => {
            let locale = parse_embedded_locale_json(locale, "time format locale")?;
            Python::with_gil(|py| -> PyResult<PyObject> {
                pythonize(py, &locale).map_err(|err| PyValueError::new_err(err.to_string())).map(|obj| obj.into())
            })
        }
    }
}

/// Create a JavaScript bundle containing the Vega Embed, Vega-Lite, and Vega libraries
///
/// Optionally, a JavaScript snippet may be provided that references Vega Embed
/// as `vegaEmbed`, Vega-Lite as `vegaLite`, Vega and `vega`, and the lodash.debounce
/// function as `lodashDebounce`.
///
/// The resulting string will include these JavaScript libraries and all of their dependencies.
/// This bundle result is suitable for inclusion in an HTML <script> tag with no external
/// dependencies required. The default snippet assigns `vegaEmbed`, `vegaLite`, and `vega`
/// to the global window object, making them available globally to other script tags.
///
/// Args:
///     snippet (str): An ES6 JavaScript snippet which includes no imports
///     vl_version (str): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
/// Returns:
///     str: Bundled snippet with all dependencies
#[pyfunction]
#[pyo3(signature = (snippet=None, vl_version=None))]
fn javascript_bundle(snippet: Option<String>, vl_version: Option<&str>) -> PyResult<String> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    if let Some(snippet) = snippet {
        run_converter_future(move |converter| async move {
            converter.bundle_vega_snippet(snippet, vl_version).await
        })
        .map_err(|err| prefixed_py_error("javascript_bundle request failed", err))
    } else {
        run_converter_future(move |converter| async move {
            converter.get_vegaembed_bundle(vl_version).await
        })
        .map_err(|err| prefixed_py_error("javascript_bundle request failed", err))
    }
}

/// Get the bundled version of Vega
///
/// Returns:
///     str: Vega version string (e.g. "5.30.0")
#[pyfunction]
#[pyo3(signature = ())]
fn get_vega_version() -> String {
    VEGA_VERSION.to_string()
}

/// Get the bundled version of Vega-Themes
///
/// Returns:
///     str: Vega-Themes version string (e.g. "2.14.0")
#[pyfunction]
#[pyo3(signature = ())]
fn get_vega_themes_version() -> String {
    VEGA_THEMES_VERSION.to_string()
}

/// Get the bundled version of Vega-Embed
///
/// Returns:
///     str: Vega-Embed version string (e.g. "6.26.0")
#[pyfunction]
#[pyo3(signature = ())]
fn get_vega_embed_version() -> String {
    VEGA_EMBED_VERSION.to_string()
}

/// Get the bundled versions of Vega-Lite
///
/// Returns:
///     list: Vega-Lite version strings (e.g. ["5.8", "5.9", ..., "5.21"])
#[pyfunction]
#[pyo3(signature = ())]
fn get_vegalite_versions() -> Vec<String> {
    VL_VERSIONS
        .iter()
        .map(|v| v.to_semver().to_string())
        .collect()
}

#[doc = async_variant_doc!("vegalite_to_vega")]
#[pyfunction(name = "vegalite_to_vega")]
#[pyo3(signature = (vl_spec, vl_version=None, config=None, theme=None, show_warnings=None))]
fn vegalite_to_vega_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
) -> PyResult<Bound<'py, PyAny>> {
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),

        format_locale: None,
        time_format_locale: None,
        google_fonts: effective_google_fonts(None),
        vega_plugin: None,
    };

    run_converter_future_async(
        py,
        move |converter| async move { converter.vegalite_to_vega(vl_spec, vl_opts).await },
        "Vega-Lite to Vega conversion failed",
        |py, value| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        },
    )
}

#[doc = async_variant_doc!("vega_to_svg")]
#[pyfunction(name = "vega_to_svg")]
#[pyo3(signature = (vg_spec, format_locale=None, time_format_locale=None, vega_plugin=None, bundle=None, subset_fonts=None))]
fn vega_to_svg_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    bundle: Option<bool>,
    subset_fonts: Option<bool>,
) -> PyResult<Bound<'py, PyAny>> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };
    let svg_opts = SvgOpts {
        bundle: bundle.unwrap_or(false),
        subset_fonts: subset_fonts.unwrap_or(true),
    };

    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("Vega to SVG conversion failed", err))?;

    let error_prefix = "Vega to SVG conversion failed";
    future_into_py_object(py, async move {
        let value = converter
            .vega_to_svg(vg_spec, vg_opts, svg_opts)
            .await
            .map_err(|err| prefixed_py_error(error_prefix, err))?;
        Python::with_gil(|py| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("vega_to_scenegraph")]
#[pyfunction(name = "vega_to_scenegraph")]
#[pyo3(signature = (vg_spec, format_locale=None, time_format_locale=None, format="dict", vega_plugin=None))]
fn vega_to_scenegraph_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    format: &str,
    vega_plugin: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    match format {
        "dict" => {
            let vg_spec = parse_json_spec(vg_spec)?;
            run_converter_future_async(
                py,
                move |converter| async move { converter.vega_to_scenegraph(vg_spec, vg_opts).await },
                "Vega to Scenegraph conversion failed",
                |py, value| {
                    pythonize(py, &value)
                        .map_err(|err| PyValueError::new_err(err.to_string()))
                        .map(|obj| obj.into())
                },
            )
        }
        "msgpack" => {
            let vg_spec = parse_spec_to_value_or_string(vg_spec)?;
            run_converter_future_async(
                py,
                move |converter| async move {
                    converter.vega_to_scenegraph_msgpack(vg_spec, vg_opts).await
                },
                "Vega to Scenegraph conversion failed",
                |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
            )
        }
        _ => Err(PyValueError::new_err(format!(
            "Invalid format '{format}'. Expected 'dict' or 'msgpack'"
        ))),
    }
}

#[doc = async_variant_doc!("vegalite_to_svg")]
#[pyfunction(name = "vegalite_to_svg")]
#[pyo3(
    signature = (vl_spec, vl_version=None, config=None, theme=None, show_warnings=None, format_locale=None, time_format_locale=None, vega_plugin=None, bundle=None, subset_fonts=None)
)]
fn vegalite_to_svg_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    bundle: Option<bool>,
    subset_fonts: Option<bool>,
) -> PyResult<Bound<'py, PyAny>> {
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };
    let svg_opts = SvgOpts {
        bundle: bundle.unwrap_or(false),
        subset_fonts: subset_fonts.unwrap_or(true),
    };

    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("Vega-Lite to SVG conversion failed", err))?;

    let error_prefix = "Vega-Lite to SVG conversion failed";
    future_into_py_object(py, async move {
        let value = converter
            .vegalite_to_svg(vl_spec, vl_opts, svg_opts)
            .await
            .map_err(|err| prefixed_py_error(error_prefix, err))?;
        Python::with_gil(|py| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("vegalite_to_scenegraph")]
#[pyfunction(name = "vegalite_to_scenegraph")]
#[pyo3(
    signature = (vl_spec, vl_version=None, config=None, theme=None, show_warnings=None, format_locale=None, time_format_locale=None, format="dict", vega_plugin=None)
)]
fn vegalite_to_scenegraph_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    format: &str,
    vega_plugin: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    match format {
        "dict" => {
            let vl_spec = parse_json_spec(vl_spec)?;
            run_converter_future_async(
                py,
                move |converter| async move { converter.vegalite_to_scenegraph(vl_spec, vl_opts).await },
                "Vega-Lite to Scenegraph conversion failed",
                |py, value| {
                    pythonize(py, &value)
                        .map_err(|err| PyValueError::new_err(err.to_string()))
                        .map(|obj| obj.into())
                },
            )
        }
        "msgpack" => {
            let vl_spec = parse_spec_to_value_or_string(vl_spec)?;
            run_converter_future_async(
                py,
                move |converter| async move {
                    converter
                        .vegalite_to_scenegraph_msgpack(vl_spec, vl_opts)
                        .await
                },
                "Vega-Lite to Scenegraph conversion failed",
                |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
            )
        }
        _ => Err(PyValueError::new_err(format!(
            "Invalid format '{format}'. Expected 'dict' or 'msgpack'"
        ))),
    }
}

#[doc = async_variant_doc!("vega_to_png")]
#[pyfunction(name = "vega_to_png")]
#[pyo3(
    signature = (vg_spec, scale=None, ppi=None, format_locale=None, time_format_locale=None, vega_plugin=None)
)]
fn vega_to_png_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    scale: Option<f32>,
    ppi: Option<f32>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vega_to_png(vg_spec, vg_opts, PngOpts { scale, ppi })
                .await
        },
        "Vega to PNG conversion failed",
        |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("vegalite_to_png")]
#[pyfunction(name = "vegalite_to_png")]
#[pyo3(
    signature = (vl_spec, vl_version=None, scale=None, ppi=None, config=None, theme=None, show_warnings=None, format_locale=None, time_format_locale=None, vega_plugin=None)
)]
fn vegalite_to_png_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    ppi: Option<f32>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vegalite_to_png(vl_spec, vl_opts, PngOpts { scale, ppi })
                .await
        },
        "Vega-Lite to PNG conversion failed",
        |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("vega_to_jpeg")]
#[pyfunction(name = "vega_to_jpeg")]
#[pyo3(
    signature = (vg_spec, scale=None, quality=None, format_locale=None, time_format_locale=None, vega_plugin=None)
)]
fn vega_to_jpeg_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    scale: Option<f32>,
    quality: Option<u8>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vega_to_jpeg(vg_spec, vg_opts, JpegOpts { scale, quality })
                .await
        },
        "Vega to JPEG conversion failed",
        |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("vegalite_to_jpeg")]
#[pyfunction(name = "vegalite_to_jpeg")]
#[pyo3(
    signature = (vl_spec, vl_version=None, scale=None, quality=None, config=None, theme=None, show_warnings=None, format_locale=None, time_format_locale=None, vega_plugin=None)
)]
fn vegalite_to_jpeg_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    quality: Option<u8>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vegalite_to_jpeg(vl_spec, vl_opts, JpegOpts { scale, quality })
                .await
        },
        "Vega-Lite to JPEG conversion failed",
        |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("vega_to_pdf")]
#[pyfunction(name = "vega_to_pdf")]
#[pyo3(signature = (vg_spec, scale=None, format_locale=None, time_format_locale=None, vega_plugin=None))]
fn vega_to_pdf_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    scale: Option<f32>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vega_to_pdf(vg_spec, vg_opts, PdfOpts::default())
                .await
        },
        "Vega to PDF conversion failed",
        |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("vegalite_to_pdf")]
#[pyfunction(name = "vegalite_to_pdf")]
#[pyo3(
    signature = (vl_spec, vl_version=None, scale=None, config=None, theme=None, format_locale=None, time_format_locale=None, vega_plugin=None)
)]
fn vegalite_to_pdf_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    config: Option<PyObject>,
    theme: Option<String>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: false,
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(None),
        vega_plugin,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vegalite_to_pdf(vl_spec, vl_opts, PdfOpts::default())
                .await
        },
        "Vega-Lite to PDF conversion failed",
        |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("vegalite_to_url")]
#[pyfunction(name = "vegalite_to_url")]
#[pyo3(signature = (vl_spec, fullscreen=None))]
fn vegalite_to_url_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    fullscreen: Option<bool>,
) -> PyResult<Bound<'py, PyAny>> {
    let vl_spec = parse_json_spec(vl_spec)?;
    let url = vl_convert_rs::converter::vegalite_to_url(&vl_spec, fullscreen.unwrap_or(false))?;
    future_into_py_object(py, async move {
        Python::with_gil(|py| {
            pythonize(py, &url)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("vega_to_url")]
#[pyfunction(name = "vega_to_url")]
#[pyo3(signature = (vg_spec, fullscreen=None))]
fn vega_to_url_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    fullscreen: Option<bool>,
) -> PyResult<Bound<'py, PyAny>> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let url = vl_convert_rs::converter::vega_to_url(&vg_spec, fullscreen.unwrap_or(false))?;
    future_into_py_object(py, async move {
        Python::with_gil(|py| {
            pythonize(py, &url)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("vegalite_to_html")]
#[pyfunction(name = "vegalite_to_html")]
#[pyo3(
    signature = (vl_spec, vl_version=None, bundle=None, subset_fonts=None, google_fonts=None, config=None, theme=None, format_locale=None, time_format_locale=None, renderer=None, vega_plugin=None)
)]
fn vegalite_to_html_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    bundle: Option<bool>,
    subset_fonts: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    theme: Option<String>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    renderer: Option<String>,
    vega_plugin: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let renderer = renderer.unwrap_or_else(|| "svg".to_string());
    let renderer = Renderer::from_str(&renderer)?;
    let subset_fonts = subset_fonts.unwrap_or(true);
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: false,

        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(google_fonts),
        vega_plugin,
    };

    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("Vega-Lite to HTML conversion failed", err))?;

    let error_prefix = "Vega-Lite to HTML conversion failed";
    future_into_py_object(py, async move {
        let value = converter
            .vegalite_to_html(
                vl_spec,
                vl_opts,
                HtmlOpts {
                    bundle: bundle.unwrap_or(false),
                    subset_fonts,
                    renderer,
                },
            )
            .await
            .map_err(|err| prefixed_py_error(error_prefix, err))?;
        Python::with_gil(|py| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("vega_to_html")]
#[pyfunction(name = "vega_to_html")]
#[pyo3(signature = (vg_spec, bundle=None, subset_fonts=None, google_fonts=None, format_locale=None, time_format_locale=None, renderer=None, vega_plugin=None))]
fn vega_to_html_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    bundle: Option<bool>,
    subset_fonts: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    renderer: Option<String>,
    vega_plugin: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let renderer = renderer.unwrap_or_else(|| "svg".to_string());
    let renderer = Renderer::from_str(&renderer)?;
    let subset_fonts = subset_fonts.unwrap_or(true);
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(google_fonts),
        vega_plugin,
    };

    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("Vega to HTML conversion failed", err))?;

    let error_prefix = "Vega to HTML conversion failed";
    future_into_py_object(py, async move {
        let value = converter
            .vega_to_html(
                vg_spec,
                vg_opts,
                HtmlOpts {
                    bundle: bundle.unwrap_or(false),
                    subset_fonts,
                    renderer,
                },
            )
            .await
            .map_err(|err| prefixed_py_error(error_prefix, err))?;
        Python::with_gil(|py| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("vegalite_fonts")]
#[pyfunction(name = "vegalite_fonts")]
#[pyo3(signature = (vl_spec, vl_version=None, config=None, theme=None, auto_google_fonts=None, include_font_face=false, subset_fonts=None, google_fonts=None, format_locale=None, time_format_locale=None))]
fn vegalite_fonts_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    auto_google_fonts: Option<bool>,
    include_font_face: bool,
    subset_fonts: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<Bound<'py, PyAny>> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: false,

        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(google_fonts),
        vega_plugin: None,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            let config = converter.config();
            let auto_gf = auto_google_fonts.unwrap_or(config.auto_google_fonts);
            let embed_lf = config.embed_local_fonts;
            converter
                .vegalite_fonts(
                    vl_spec,
                    vl_opts,
                    auto_gf,
                    embed_lf,
                    include_font_face,
                    subset_fonts.unwrap_or(true),
                )
                .await
        },
        "vegalite_fonts request failed",
        |py, value| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        },
    )
}

#[doc = async_variant_doc!("vega_fonts")]
#[pyfunction(name = "vega_fonts")]
#[pyo3(signature = (vg_spec, auto_google_fonts=None, include_font_face=false, subset_fonts=None, google_fonts=None, format_locale=None, time_format_locale=None))]
fn vega_fonts_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    auto_google_fonts: Option<bool>,
    include_font_face: bool,
    subset_fonts: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<Bound<'py, PyAny>> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts: effective_google_fonts(google_fonts),
        vega_plugin: None,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            let config = converter.config();
            let auto_gf = auto_google_fonts.unwrap_or(config.auto_google_fonts);
            let embed_lf = config.embed_local_fonts;
            converter
                .vega_fonts(
                    vg_spec,
                    vg_opts,
                    auto_gf,
                    embed_lf,
                    include_font_face,
                    subset_fonts.unwrap_or(true),
                )
                .await
        },
        "vega_fonts request failed",
        |py, value| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        },
    )
}

#[doc = async_variant_doc!("svg_to_png")]
#[pyfunction(name = "svg_to_png")]
#[pyo3(signature = (svg, scale=None, ppi=None))]
fn svg_to_png_asyncio<'py>(
    py: Python<'py>,
    svg: &str,
    scale: Option<f32>,
    ppi: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    let svg = svg.to_string();
    run_converter_future_async(
        py,
        move |converter| async move { converter.svg_to_png(&svg, PngOpts { scale, ppi }).await },
        "SVG to PNG conversion failed",
        |py, data| Ok(PyBytes::new(py, data.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("svg_to_jpeg")]
#[pyfunction(name = "svg_to_jpeg")]
#[pyo3(signature = (svg, scale=None, quality=None))]
fn svg_to_jpeg_asyncio<'py>(
    py: Python<'py>,
    svg: &str,
    scale: Option<f32>,
    quality: Option<u8>,
) -> PyResult<Bound<'py, PyAny>> {
    let svg = svg.to_string();
    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .svg_to_jpeg(&svg, JpegOpts { scale, quality })
                .await
        },
        "SVG to JPEG conversion failed",
        |py, data| Ok(PyBytes::new(py, data.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("svg_to_pdf")]
#[pyfunction(name = "svg_to_pdf")]
#[pyo3(signature = (svg, scale=None))]
fn svg_to_pdf_asyncio<'py>(
    py: Python<'py>,
    svg: &str,
    scale: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let svg = svg.to_string();
    run_converter_future_async(
        py,
        move |converter| async move { converter.svg_to_pdf(&svg, PdfOpts::default()).await },
        "SVG to PDF conversion failed",
        |py, data| Ok(PyBytes::new(py, data.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("register_font_directory")]
#[pyfunction(name = "register_font_directory")]
#[pyo3(signature = (font_dir))]
fn register_font_directory_asyncio<'py>(
    py: Python<'py>,
    font_dir: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let font_dir = font_dir.to_string();
    future_into_py_object(py, async move {
        tokio::task::spawn_blocking(move || register_font_directory_rs(&font_dir))
            .await
            .map_err(|err| PyValueError::new_err(format!("Task join error: {err}")))?
            .map_err(|err| {
                PyValueError::new_err(format!("Failed to register font directory: {err}"))
            })?;
        Python::with_gil(|py| Ok(py.None().into()))
    })
}

#[doc = async_variant_doc!("configure")]
#[pyfunction(name = "configure")]
#[pyo3(signature = (**kwargs))]
fn configure_asyncio<'py>(
    py: Python<'py>,
    kwargs: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let overrides = parse_config_overrides(kwargs)
        .map_err(|err| prefixed_py_error("Failed to configure converter", err))?;
    future_into_py_object(py, async move {
        configure_converter_with_config_overrides(overrides)
            .map_err(|err| prefixed_py_error("Failed to configure converter", err))?;
        Python::with_gil(|py| Ok(py.None().into()))
    })
}

#[doc = async_variant_doc!("get_config")]
#[pyfunction(name = "get_config")]
#[pyo3(signature = ())]
fn get_config_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    future_into_py_object(py, async move {
        let config = converter_config()
            .map_err(|err| prefixed_py_error("Failed to read converter config", err))?;
        Python::with_gil(|py| {
            pythonize(py, &converter_config_json(&config))
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("warm_up_workers")]
#[pyfunction(name = "warm_up_workers")]
#[pyo3(signature = ())]
fn warm_up_workers_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("warm_up_workers request failed", err))?;

    future_into_py_object(py, async move {
        tokio::task::spawn_blocking(move || converter.warm_up())
            .await
            .map_err(|err| prefixed_py_error("warm_up_workers request failed", err))?
            .map_err(|err| prefixed_py_error("warm_up_workers request failed", err))?;
        Python::with_gil(|py| Ok(py.None().into()))
    })
}

#[doc = async_variant_doc!("get_local_tz")]
#[pyfunction(name = "get_local_tz")]
#[pyo3(signature = ())]
fn get_local_tz_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    run_converter_future_async(
        py,
        |converter| async move { converter.get_local_tz().await },
        "get_local_tz request failed",
        |py, value| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        },
    )
}

#[doc = async_variant_doc!("get_worker_memory_usage")]
#[pyfunction(name = "get_worker_memory_usage")]
#[pyo3(signature = ())]
fn get_worker_memory_usage_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    run_converter_future_async(
        py,
        |converter| async move { converter.get_worker_memory_usage().await },
        "get_worker_memory_usage request failed",
        |py, stats| {
            let list = pyo3::types::PyList::empty(py);
            for s in &stats {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("worker_index", s.worker_index)?;
                dict.set_item("used_heap_size", s.used_heap_size)?;
                dict.set_item("total_heap_size", s.total_heap_size)?;
                dict.set_item("heap_size_limit", s.heap_size_limit)?;
                dict.set_item("external_memory", s.external_memory)?;
                list.append(dict)?;
            }
            Ok(list.into())
        },
    )
}

#[doc = async_variant_doc!("get_themes")]
#[pyfunction(name = "get_themes")]
#[pyo3(signature = ())]
fn get_themes_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    run_converter_future_async(
        py,
        |converter| async move { converter.get_themes().await },
        "get_themes request failed",
        |py, value| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        },
    )
}

#[doc = async_variant_doc!("get_format_locale")]
#[pyfunction(name = "get_format_locale")]
#[pyo3(signature = (name))]
fn get_format_locale_asyncio<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    let locale = match FORMATE_LOCALE_MAP.get(name) {
        None => {
            return Err(PyValueError::new_err(format!(
                "Invalid format locale name: {name}\nSee https://github.com/d3/d3-format/tree/main/locale for available names"
            )))
        }
        Some(locale) => parse_embedded_locale_json(locale, "format locale")?,
    };

    future_into_py_object(py, async move {
        Python::with_gil(|py| {
            pythonize(py, &locale)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("get_time_format_locale")]
#[pyfunction(name = "get_time_format_locale")]
#[pyo3(signature = (name))]
fn get_time_format_locale_asyncio<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    let locale = match TIME_FORMATE_LOCALE_MAP.get(name) {
        None => {
            return Err(PyValueError::new_err(format!(
                "Invalid time format locale name: {name}\nSee https://github.com/d3/d3-time-format/tree/main/locale for available names"
            )))
        }
        Some(locale) => parse_embedded_locale_json(locale, "time format locale")?,
    };

    future_into_py_object(py, async move {
        Python::with_gil(|py| {
            pythonize(py, &locale)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("javascript_bundle")]
#[pyfunction(name = "javascript_bundle")]
#[pyo3(signature = (snippet=None, vl_version=None))]
fn javascript_bundle_asyncio<'py>(
    py: Python<'py>,
    snippet: Option<String>,
    vl_version: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    if let Some(snippet) = snippet {
        run_converter_future_async(
            py,
            move |converter| async move { converter.bundle_vega_snippet(snippet, vl_version).await },
            "javascript_bundle request failed",
            |py, value| {
                pythonize(py, &value)
                    .map_err(|err| PyValueError::new_err(err.to_string()))
                    .map(|obj| obj.into())
            },
        )
    } else {
        run_converter_future_async(
            py,
            move |converter| async move { converter.get_vegaembed_bundle(vl_version).await },
            "javascript_bundle request failed",
            |py, value| {
                pythonize(py, &value)
                    .map_err(|err| PyValueError::new_err(err.to_string()))
                    .map(|obj| obj.into())
            },
        )
    }
}

#[doc = async_variant_doc!("get_vega_version")]
#[pyfunction(name = "get_vega_version")]
#[pyo3(signature = ())]
fn get_vega_version_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    let value = VEGA_VERSION.to_string();
    future_into_py_object(py, async move {
        Python::with_gil(|py| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("get_vega_themes_version")]
#[pyfunction(name = "get_vega_themes_version")]
#[pyo3(signature = ())]
fn get_vega_themes_version_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    let value = VEGA_THEMES_VERSION.to_string();
    future_into_py_object(py, async move {
        Python::with_gil(|py| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("get_vega_embed_version")]
#[pyfunction(name = "get_vega_embed_version")]
#[pyo3(signature = ())]
fn get_vega_embed_version_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    let value = VEGA_EMBED_VERSION.to_string();
    future_into_py_object(py, async move {
        Python::with_gil(|py| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("get_vegalite_versions")]
#[pyfunction(name = "get_vegalite_versions")]
#[pyo3(signature = ())]
fn get_vegalite_versions_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    let value: Vec<String> = VL_VERSIONS
        .iter()
        .map(|v| v.to_semver().to_string())
        .collect();
    future_into_py_object(py, async move {
        Python::with_gil(|py| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

fn add_asyncio_submodule(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Returns Err if already initialized (expected on module re-import).
    // We intentionally ignore this value to make initialization idempotent.
    let _ = pyo3_async_runtimes::tokio::init_with_runtime(&PYTHON_RUNTIME);

    let asyncio = PyModule::new(py, "asyncio")?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_vega_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_svg_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_scenegraph_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_png_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_jpeg_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_pdf_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_url_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_html_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_svg_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_scenegraph_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_png_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_jpeg_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_pdf_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_url_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_html_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_fonts_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_fonts_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(svg_to_png_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(svg_to_jpeg_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(svg_to_pdf_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(register_font_directory_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(configure_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_config_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(warm_up_workers_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_worker_memory_usage_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_local_tz_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_themes_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_format_locale_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_time_format_locale_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(javascript_bundle_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_vega_version_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_vega_themes_version_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_vega_embed_version_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_vegalite_versions_asyncio, &asyncio)?)?;

    m.add_submodule(&asyncio)?;
    py.import("sys")?
        .getattr("modules")?
        .set_item("vl_convert.asyncio", &asyncio)?;
    Ok(())
}

/// Convert Vega-Lite specifications to other formats
#[pymodule]
fn vl_convert(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();
    m.add_function(wrap_pyfunction!(vegalite_to_vega, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_svg, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_scenegraph, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_png, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_jpeg, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_pdf, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_url, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_html, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_svg, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_scenegraph, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_png, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_jpeg, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_pdf, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_url, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_html, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_fonts, m)?)?;
    m.add_function(wrap_pyfunction!(vega_fonts, m)?)?;
    m.add_function(wrap_pyfunction!(svg_to_png, m)?)?;
    m.add_function(wrap_pyfunction!(svg_to_jpeg, m)?)?;
    m.add_function(wrap_pyfunction!(svg_to_pdf, m)?)?;
    m.add_function(wrap_pyfunction!(register_font_directory, m)?)?;
    m.add_function(wrap_pyfunction!(configure, m)?)?;
    m.add_function(wrap_pyfunction!(get_config, m)?)?;
    m.add_function(wrap_pyfunction!(warm_up_workers, m)?)?;
    m.add_function(wrap_pyfunction!(get_worker_memory_usage, m)?)?;
    m.add_function(wrap_pyfunction!(get_local_tz, m)?)?;
    m.add_function(wrap_pyfunction!(get_themes, m)?)?;
    m.add_function(wrap_pyfunction!(get_format_locale, m)?)?;
    m.add_function(wrap_pyfunction!(get_time_format_locale, m)?)?;
    m.add_function(wrap_pyfunction!(javascript_bundle, m)?)?;
    m.add_function(wrap_pyfunction!(get_vega_version, m)?)?;
    m.add_function(wrap_pyfunction!(get_vega_themes_version, m)?)?;
    m.add_function(wrap_pyfunction!(get_vega_embed_version, m)?)?;
    m.add_function(wrap_pyfunction!(get_vegalite_versions, m)?)?;
    add_asyncio_submodule(py, m)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

// Utilities
fn warn_if_scale_not_one_for_pdf(scale: Option<f32>) -> PyResult<()> {
    if let Some(scale) = scale {
        if scale != 1.0 {
            Python::with_gil(|py| -> PyResult<()> {
                let warnings = py.import("warnings")?;
                warnings.call_method1(
                    "warn",
                    ("The scale argument is no longer supported for PDF export.",),
                )?;
                Ok(())
            })?;
        }
    }
    Ok(())
}

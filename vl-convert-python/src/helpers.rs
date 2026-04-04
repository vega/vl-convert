use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::depythonize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use vl_convert_rs::configure_font_cache as configure_font_cache_rs;
use vl_convert_rs::converter::{
    BaseUrlSetting, FormatLocale, GoogleFontRequest, MissingFontsPolicy, TimeFormatLocale,
    ValueOrString, VlcConfig, ACCESS_DENIED_MARKER,
};
use vl_convert_rs::serde_json;
use vl_convert_rs::VlConverter as VlConverterRs;
use vl_convert_rs::{FontStyle, VariantRequest};

use crate::{CONFIGURED_GOOGLE_FONTS, PYTHON_RUNTIME, VL_CONVERTER};

pub fn effective_google_fonts(
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

pub fn converter_read_handle() -> Result<Arc<VlConverterRs>, vl_convert_rs::anyhow::Error> {
    VL_CONVERTER
        .read()
        .map_err(|e| vl_convert_rs::anyhow::anyhow!("Failed to acquire converter read lock: {e}"))
        .map(|guard| guard.clone())
}

pub fn converter_config() -> Result<VlcConfig, vl_convert_rs::anyhow::Error> {
    VL_CONVERTER
        .read()
        .map_err(|e| vl_convert_rs::anyhow::anyhow!("Failed to acquire converter read lock: {e}"))
        .map(|guard| guard.config())
}

pub fn converter_config_json(config: &VlcConfig) -> serde_json::Value {
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
        "subset_fonts": config.subset_fonts,
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
pub struct ConverterConfigOverrides {
    pub num_workers: Option<usize>,
    pub base_url: Option<BaseUrlSetting>,
    pub allowed_base_urls: Option<Option<Vec<String>>>,
    pub google_fonts_cache_size_mb: Option<u64>,
    pub auto_google_fonts: Option<bool>,
    pub embed_local_fonts: Option<bool>,
    pub subset_fonts: Option<bool>,
    pub missing_fonts: Option<MissingFontsPolicy>,
    pub google_fonts: Option<Option<Vec<GoogleFontRequest>>>,
    pub max_v8_heap_size_mb: Option<usize>,
    pub max_v8_execution_time_secs: Option<u64>,
    pub gc_after_conversion: Option<bool>,
    pub vega_plugins: Option<Option<Vec<String>>>,
    pub plugin_import_domains: Option<Vec<String>>,
    pub allow_per_request_plugins: Option<bool>,
    pub per_request_plugin_import_domains: Option<Vec<String>>,
    pub default_theme: Option<Option<String>>,
    pub default_format_locale: Option<Option<FormatLocale>>,
    pub default_time_format_locale: Option<Option<TimeFormatLocale>>,
    pub themes: Option<Option<HashMap<String, serde_json::Value>>>,
}

pub fn parse_config_overrides(
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
            "subset_fonts" => {
                if !value.is_none() {
                    overrides.subset_fonts = Some(value.extract::<bool>().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid subset_fonts value for configure: {err}"
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

pub fn apply_config_overrides(
    config: &mut VlcConfig,
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
    if let Some(subset_fonts) = overrides.subset_fonts {
        config.subset_fonts = subset_fonts;
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

pub fn configure_converter_with_config_overrides(
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

pub fn run_converter_future<R, Fut, F>(make_future: F) -> Result<R, vl_convert_rs::anyhow::Error>
where
    F: FnOnce(Arc<VlConverterRs>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<R, vl_convert_rs::anyhow::Error>> + 'static,
    R: Send + 'static,
{
    let converter = converter_read_handle()?;
    Python::with_gil(|py| py.allow_threads(move || PYTHON_RUNTIME.block_on(make_future(converter))))
}

pub fn is_permission_denied_message(message: &str) -> bool {
    if message.contains(ACCESS_DENIED_MARKER) {
        return true;
    }

    let lowercase = message.to_ascii_lowercase();
    lowercase.contains("permission denied")
        || lowercase.contains("access denied")
        || lowercase.contains("requires read access")
        || lowercase.contains("requires net access")
}

pub fn prefixed_py_error(prefix: &'static str, err: impl std::fmt::Display) -> PyErr {
    let message = format!("{prefix}:\n{err}");
    if is_permission_denied_message(&message) {
        pyo3::exceptions::PyPermissionError::new_err(message)
    } else {
        PyValueError::new_err(message)
    }
}

pub fn future_into_py_object<'py, Fut>(py: Python<'py>, fut: Fut) -> PyResult<Bound<'py, PyAny>>
where
    Fut: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    pyo3_async_runtimes::tokio::future_into_py::<_, PyObject>(py, fut)
}

pub fn run_converter_future_async<'py, R, Fut, F, C>(
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
pub(crate) use async_variant_doc;

pub fn parse_json_spec(vl_spec: PyObject) -> PyResult<serde_json::Value> {
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

pub fn parse_optional_config(config: Option<PyObject>) -> PyResult<Option<serde_json::Value>> {
    config.map(parse_json_spec).transpose()
}

pub fn handle_show_warnings(show_warnings: Option<bool>) {
    if show_warnings == Some(true) {
        Python::with_gil(|py| {
            let _ = py.run(
                c"import warnings; warnings.warn(\
                  'show_warnings is deprecated. Warnings are now always forwarded via '\
                  'Python\\'s logging module. Configure with: import logging; '\
                  'logging.getLogger(\"vl_convert\").setLevel(logging.WARNING)', DeprecationWarning, stacklevel=2)",
                None,
                None,
            );
        });
    }
}

pub fn parse_spec_to_value_or_string(vl_spec: PyObject) -> PyResult<ValueOrString> {
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

pub fn parse_format_locale(v: PyObject) -> PyResult<FormatLocale> {
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

pub fn parse_option_format_locale(v: Option<PyObject>) -> PyResult<Option<FormatLocale>> {
    match v {
        None => Ok(None),
        Some(v) => Ok(Some(parse_format_locale(v)?)),
    }
}

pub fn parse_time_format_locale(v: PyObject) -> PyResult<TimeFormatLocale> {
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

pub fn parse_option_time_format_locale(v: Option<PyObject>) -> PyResult<Option<TimeFormatLocale>> {
    match v {
        None => Ok(None),
        Some(v) => Ok(Some(parse_time_format_locale(v)?)),
    }
}

pub fn parse_embedded_locale_json(raw: &str, kind: &str) -> PyResult<serde_json::Value> {
    serde_json::from_str(raw).map_err(|err| {
        PyValueError::new_err(format!("Failed to parse internal {kind} as JSON: {err}"))
    })
}

pub fn parse_variant_args(
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

pub fn parse_google_fonts_arg(
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

pub fn warn_if_scale_not_one_for_pdf(scale: Option<f32>) -> PyResult<()> {
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

pub fn load_config_inner(path: Option<String>) -> Result<(), vl_convert_rs::anyhow::Error> {
    let config = match path {
        Some(p) => VlcConfig::from_file(std::path::Path::new(&p))?,
        None => {
            let standard = vl_convert_rs::vlc_config_path();
            if !standard.exists() {
                VlcConfig::default()
            } else {
                VlcConfig::from_file(&standard)?
            }
        }
    };
    let new_converter = Arc::new(VlConverterRs::with_config(config)?);

    let mut gf_guard = CONFIGURED_GOOGLE_FONTS.write().map_err(|e| {
        vl_convert_rs::anyhow::anyhow!("Failed to acquire google_fonts write lock: {e}")
    })?;
    let mut guard = VL_CONVERTER.write().map_err(|e| {
        vl_convert_rs::anyhow::anyhow!("Failed to acquire converter write lock: {e}")
    })?;
    *gf_guard = None;
    *guard = new_converter;
    Ok(())
}

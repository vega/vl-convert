use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::{depythonize, pythonize};
use std::collections::HashMap;
use std::sync::Arc;
use vl_convert_rs::configure_font_cache as configure_font_cache_rs;
use vl_convert_rs::converter::{
    BaseUrlSetting, FormatLocale, GoogleFontRequest, MissingFontsPolicy, TimeFormatLocale,
    VlcConfig,
};
use vl_convert_rs::serde_json;
use vl_convert_rs::vlc_config_path;
use vl_convert_rs::VlConverter as VlConverterRs;

use crate::fonts::parse_google_fonts_arg;
use crate::utils::{
    async_variant_doc, future_into_py_object, parse_format_locale, parse_time_format_locale,
    prefixed_py_error,
};
use crate::{CONFIGURED_GOOGLE_FONTS, VL_CONVERTER};

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

pub fn load_config_inner(path: Option<String>) -> Result<(), vl_convert_rs::anyhow::Error> {
    let config = match path {
        Some(p) => VlcConfig::from_file(std::path::Path::new(&p))?,
        None => {
            let standard = vl_convert_rs::vlc_config_path();
            if !standard.exists() {
                crate::default_python_config()
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

/// Configure converter options for subsequent requests
#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub fn configure(kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    let overrides = parse_config_overrides(kwargs)
        .map_err(|err| prefixed_py_error("Failed to configure converter", err))?;
    configure_converter_with_config_overrides(overrides)
        .map_err(|err| prefixed_py_error("Failed to configure converter", err))
}

/// Return the platform-standard path for the vl-convert JSONC config file.
#[pyfunction(name = "get_config_path")]
#[pyo3(signature = ())]
pub fn get_config_path() -> String {
    vlc_config_path().to_string_lossy().into_owned()
}

/// Load converter configuration from a JSONC file, replacing the active config.
///
/// Unlike ``configure()``, which patches individual fields, ``load_config()``
/// resets all settings to their defaults and then applies the file. Call
/// ``configure()`` after ``load_config()`` to override specific fields in code.
///
/// Args:
///     path (str | None): Path to the JSONC config file. When omitted, loads
///         from the standard location returned by ``get_config_path()``.
///         If that file does not exist, resets to built-in defaults.
///
/// Raises:
///     ValueError: If the path is provided but the file cannot be read or parsed.
#[pyfunction(name = "load_config")]
#[pyo3(signature = (path=None))]
pub fn load_config(path: Option<String>) -> PyResult<()> {
    load_config_inner(path).map_err(|err| prefixed_py_error("Failed to load config", err))
}

/// Get the currently configured converter options.
#[pyfunction(name = "get_config")]
#[pyo3(signature = ())]
pub fn get_config() -> PyResult<PyObject> {
    let config = converter_config()
        .map_err(|err| prefixed_py_error("Failed to read converter config", err))?;
    Python::with_gil(|py| {
        pythonize(py, &converter_config_json(&config))
            .map_err(|err| PyValueError::new_err(err.to_string()))
            .map(|obj| obj.into())
    })
}

#[doc = async_variant_doc!("configure")]
#[pyfunction(name = "configure")]
#[pyo3(signature = (**kwargs))]
pub fn configure_asyncio<'py>(
    py: Python<'py>,
    kwargs: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let overrides = parse_config_overrides(kwargs)
        .map_err(|err| prefixed_py_error("Failed to configure converter", err))?;
    future_into_py_object(py, async move {
        tokio::task::spawn_blocking(move || configure_converter_with_config_overrides(overrides))
            .await
            .map_err(|err| prefixed_py_error("Failed to configure converter", err))?
            .map_err(|err| prefixed_py_error("Failed to configure converter", err))?;
        Python::with_gil(|py| Ok(py.None().into()))
    })
}

#[doc = async_variant_doc!("load_config")]
#[pyfunction(name = "load_config")]
#[pyo3(signature = (path=None))]
pub fn load_config_asyncio<'py>(
    py: Python<'py>,
    path: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py_object(py, async move {
        tokio::task::spawn_blocking(move || load_config_inner(path))
            .await
            .map_err(|err| prefixed_py_error("Failed to load config", err))?
            .map_err(|err| prefixed_py_error("Failed to load config", err))?;
        Python::with_gil(|py| Ok(py.None().into()))
    })
}

#[doc = async_variant_doc!("get_config")]
#[pyfunction(name = "get_config")]
#[pyo3(signature = ())]
pub fn get_config_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    future_into_py_object(py, async move {
        let config = tokio::task::spawn_blocking(converter_config)
            .await
            .map_err(|err| prefixed_py_error("Failed to read converter config", err))?
            .map_err(|err| prefixed_py_error("Failed to read converter config", err))?;
        Python::with_gil(|py| {
            pythonize(py, &converter_config_json(&config))
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

/// Eagerly start converter workers for the current converter configuration
#[pyfunction]
#[pyo3(signature = ())]
pub fn warm_up_workers() -> PyResult<()> {
    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("warm_up_workers request failed", err))?;

    Python::with_gil(|py| py.allow_threads(move || converter.warm_up()))
        .map_err(|err| prefixed_py_error("warm_up_workers request failed", err))?;
    Ok(())
}

#[doc = async_variant_doc!("warm_up_workers")]
#[pyfunction(name = "warm_up_workers")]
#[pyo3(signature = ())]
pub fn warm_up_workers_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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

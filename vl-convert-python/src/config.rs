use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::{depythonize, pythonize};
use std::collections::HashMap;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::PathBuf;
use std::sync::Arc;
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
use crate::VL_CONVERTER;

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
    let google_fonts_value: Vec<serde_json::Value> = config
        .google_fonts
        .iter()
        .map(|req| {
            let mut entry = serde_json::Map::new();
            entry.insert(
                "family".to_string(),
                serde_json::Value::String(req.family.clone()),
            );
            if let Some(variants) = req.variants.as_ref() {
                let variants_value: Vec<serde_json::Value> = variants
                    .iter()
                    .map(|v| {
                        serde_json::json!({
                            "weight": v.weight,
                            "style": v.style.as_str(),
                        })
                    })
                    .collect();
                entry.insert(
                    "variants".to_string(),
                    serde_json::Value::Array(variants_value),
                );
            }
            serde_json::Value::Object(entry)
        })
        .collect();
    let font_directories_value: Vec<serde_json::Value> = config
        .font_directories
        .iter()
        .map(|p| serde_json::Value::String(p.to_string_lossy().into_owned()))
        .collect();
    serde_json::json!({
        "num_workers": config.num_workers.get(),
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
        "google_fonts": google_fonts_value,
        "google_fonts_cache_dir": vl_convert_rs::google_fonts_cache_dir()
            .map(|p| p.to_string_lossy().into_owned()),
        "google_fonts_cache_size_mb": config.google_fonts_cache_size_mb.map(|n| n.get()),
        "max_v8_heap_size_mb": config.max_v8_heap_size_mb.map(|n| n.get()),
        "max_v8_execution_time_secs": config.max_v8_execution_time_secs.map(|n| n.get()),
        "gc_after_conversion": config.gc_after_conversion,
        "vega_plugins": config.vega_plugins,
        "plugin_import_domains": config.plugin_import_domains,
        "allow_per_request_plugins": config.allow_per_request_plugins,
        "max_ephemeral_workers": config.max_ephemeral_workers.map(|n| n.get()),
        "allow_google_fonts": config.allow_google_fonts,
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
        "font_directories": font_directories_value,
    })
}

/// Per-field overrides collected from `configure()` kwargs. A `Some(_)` entry
/// means "the caller passed this kwarg; apply this value"; `None` means "the
/// kwarg was absent; leave the field alone." Passing `None` as the Python
/// value for a kwarg uniformly resets that field to its library default.
#[derive(Default)]
pub struct ConverterConfigOverrides {
    pub num_workers: Option<NonZeroUsize>,
    pub base_url: Option<BaseUrlSetting>,
    pub allowed_base_urls: Option<Vec<String>>,
    pub google_fonts_cache_size_mb: Option<Option<NonZeroU64>>,
    pub auto_google_fonts: Option<bool>,
    pub embed_local_fonts: Option<bool>,
    pub subset_fonts: Option<bool>,
    pub missing_fonts: Option<MissingFontsPolicy>,
    pub google_fonts: Option<Vec<GoogleFontRequest>>,
    pub max_v8_heap_size_mb: Option<Option<NonZeroUsize>>,
    pub max_v8_execution_time_secs: Option<Option<NonZeroU64>>,
    pub gc_after_conversion: Option<bool>,
    pub vega_plugins: Option<Vec<String>>,
    pub plugin_import_domains: Option<Vec<String>>,
    pub allow_per_request_plugins: Option<bool>,
    pub max_ephemeral_workers: Option<Option<NonZeroUsize>>,
    pub allow_google_fonts: Option<bool>,
    pub per_request_plugin_import_domains: Option<Vec<String>>,
    pub default_theme: Option<Option<String>>,
    pub default_format_locale: Option<Option<FormatLocale>>,
    pub default_time_format_locale: Option<Option<TimeFormatLocale>>,
    pub themes: Option<HashMap<String, serde_json::Value>>,
    pub font_directories: Option<Vec<PathBuf>>,
}

/// Extract a positive integer, rejecting `0`. The caller is responsible for
/// converting to the appropriate `NonZero*` wrapper.
fn extract_positive_u64(
    key: &str,
    value: &Bound<'_, PyAny>,
) -> Result<NonZeroU64, vl_convert_rs::anyhow::Error> {
    let raw = value.extract::<u64>().map_err(|err| {
        vl_convert_rs::anyhow::anyhow!("Invalid {key} value for configure: {err}")
    })?;
    NonZeroU64::new(raw).ok_or_else(|| {
        vl_convert_rs::anyhow::anyhow!("Invalid {key} value for configure: must be >= 1, got 0")
    })
}

fn extract_positive_usize(
    key: &str,
    value: &Bound<'_, PyAny>,
) -> Result<NonZeroUsize, vl_convert_rs::anyhow::Error> {
    let raw = value.extract::<usize>().map_err(|err| {
        vl_convert_rs::anyhow::anyhow!("Invalid {key} value for configure: {err}")
    })?;
    NonZeroUsize::new(raw).ok_or_else(|| {
        vl_convert_rs::anyhow::anyhow!("Invalid {key} value for configure: must be >= 1, got 0")
    })
}

pub fn parse_config_overrides(
    kwargs: Option<&Bound<'_, PyDict>>,
) -> Result<ConverterConfigOverrides, vl_convert_rs::anyhow::Error> {
    let mut overrides = ConverterConfigOverrides::default();
    let Some(kwargs) = kwargs else {
        return Ok(overrides);
    };

    // Uniform `None` semantics: passing `None` for any kwarg resets that
    // field to its `VlcConfig::default()` value.
    let default = VlcConfig::default();

    for (key, value) in kwargs.iter() {
        let key_str: String = key.extract().map_err(|err| {
            vl_convert_rs::anyhow::anyhow!("configure keyword parsing failed: {err}")
        })?;
        match key_str.as_str() {
            "num_workers" => {
                overrides.num_workers = Some(if value.is_none() {
                    default.num_workers
                } else {
                    extract_positive_usize("num_workers", &value)?
                });
            }
            "base_url" => {
                if value.is_none() {
                    overrides.base_url = Some(default.base_url.clone());
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
                    overrides.allowed_base_urls = Some(default.allowed_base_urls.clone());
                } else {
                    overrides.allowed_base_urls =
                        Some(value.extract::<Vec<String>>().map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid allowed_base_urls value for configure: {err}"
                            )
                        })?);
                }
            }
            "google_fonts_cache_size_mb" => {
                if value.is_none() {
                    overrides.google_fonts_cache_size_mb = Some(default.google_fonts_cache_size_mb);
                } else {
                    let n = extract_positive_u64("google_fonts_cache_size_mb", &value)?;
                    overrides.google_fonts_cache_size_mb = Some(Some(n));
                }
            }
            "auto_google_fonts" => {
                overrides.auto_google_fonts = Some(if value.is_none() {
                    default.auto_google_fonts
                } else {
                    value.extract::<bool>().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid auto_google_fonts value for configure: {err}"
                        )
                    })?
                });
            }
            "embed_local_fonts" => {
                overrides.embed_local_fonts = Some(if value.is_none() {
                    default.embed_local_fonts
                } else {
                    value.extract::<bool>().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid embed_local_fonts value for configure: {err}"
                        )
                    })?
                });
            }
            "subset_fonts" => {
                overrides.subset_fonts = Some(if value.is_none() {
                    default.subset_fonts
                } else {
                    value.extract::<bool>().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid subset_fonts value for configure: {err}"
                        )
                    })?
                });
            }
            "missing_fonts" => {
                if value.is_none() {
                    overrides.missing_fonts = Some(default.missing_fonts);
                } else {
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
                    overrides.google_fonts = Some(default.google_fonts.clone());
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
                    overrides.google_fonts = Some(parsed.unwrap_or_default());
                }
            }
            "max_v8_heap_size_mb" => {
                if value.is_none() {
                    overrides.max_v8_heap_size_mb = Some(default.max_v8_heap_size_mb);
                } else {
                    let n = extract_positive_usize("max_v8_heap_size_mb", &value)?;
                    overrides.max_v8_heap_size_mb = Some(Some(n));
                }
            }
            "max_v8_execution_time_secs" => {
                if value.is_none() {
                    overrides.max_v8_execution_time_secs = Some(default.max_v8_execution_time_secs);
                } else {
                    let n = extract_positive_u64("max_v8_execution_time_secs", &value)?;
                    overrides.max_v8_execution_time_secs = Some(Some(n));
                }
            }
            "gc_after_conversion" => {
                overrides.gc_after_conversion = Some(if value.is_none() {
                    default.gc_after_conversion
                } else {
                    value.extract::<bool>().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid gc_after_conversion value for configure: {err}"
                        )
                    })?
                });
            }
            "vega_plugins" => {
                if value.is_none() {
                    overrides.vega_plugins = Some(default.vega_plugins.clone());
                } else {
                    overrides.vega_plugins =
                        Some(value.extract::<Vec<String>>().map_err(|err| {
                            vl_convert_rs::anyhow::anyhow!(
                                "Invalid vega_plugins value for configure: {err}"
                            )
                        })?);
                }
            }
            "plugin_import_domains" => {
                if value.is_none() {
                    overrides.plugin_import_domains = Some(default.plugin_import_domains.clone());
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
                overrides.allow_per_request_plugins = Some(if value.is_none() {
                    default.allow_per_request_plugins
                } else {
                    value.extract::<bool>().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid allow_per_request_plugins value for configure: {err}"
                        )
                    })?
                });
            }
            "max_ephemeral_workers" => {
                if value.is_none() {
                    overrides.max_ephemeral_workers = Some(default.max_ephemeral_workers);
                } else {
                    let n = extract_positive_usize("max_ephemeral_workers", &value)?;
                    overrides.max_ephemeral_workers = Some(Some(n));
                }
            }
            "allow_google_fonts" => {
                overrides.allow_google_fonts = Some(if value.is_none() {
                    default.allow_google_fonts
                } else {
                    value.extract::<bool>().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid allow_google_fonts value for configure: {err}"
                        )
                    })?
                });
            }
            "per_request_plugin_import_domains" => {
                if value.is_none() {
                    overrides.per_request_plugin_import_domains =
                        Some(default.per_request_plugin_import_domains.clone());
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
                    overrides.default_theme = Some(default.default_theme.clone());
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
                    overrides.default_format_locale = Some(default.default_format_locale.clone());
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
                    overrides.default_time_format_locale =
                        Some(default.default_time_format_locale.clone());
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
                    overrides.themes = Some(default.themes.clone());
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
                    overrides.themes = Some(themes_map);
                }
            }
            "font_directories" => {
                if value.is_none() {
                    overrides.font_directories = Some(default.font_directories.clone());
                } else {
                    let raw: Vec<String> = value.extract().map_err(|err| {
                        vl_convert_rs::anyhow::anyhow!(
                            "Invalid font_directories value for configure: {err}"
                        )
                    })?;
                    overrides.font_directories = Some(raw.into_iter().map(PathBuf::from).collect());
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
    if let Some(google_fonts_cache_size_mb) = overrides.google_fonts_cache_size_mb {
        config.google_fonts_cache_size_mb = google_fonts_cache_size_mb;
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
        config.google_fonts = google_fonts;
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
    if let Some(font_directories) = overrides.font_directories {
        config.font_directories = font_directories;
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
                VlcConfig::default()
            } else {
                VlcConfig::from_file(&standard)?
            }
        }
    };
    let new_converter = Arc::new(VlConverterRs::with_config(config)?);

    let mut guard = VL_CONVERTER.write().map_err(|e| {
        vl_convert_rs::anyhow::anyhow!("Failed to acquire converter write lock: {e}")
    })?;
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

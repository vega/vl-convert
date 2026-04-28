use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::pythonize;
use std::str::FromStr;
use vl_convert_rs::converter::{GoogleFontRequest, VgOpts, VlOpts};
use vl_convert_rs::module_loader::import_map::VlVersion;
use vl_convert_rs::{FontStyle, VariantRequest};

use crate::utils::{
    async_variant_doc, future_into_py_object, parse_json_spec, parse_option_format_locale,
    parse_option_time_format_locale, parse_optional_config, prefixed_py_error,
    run_converter_future, run_converter_future_async,
};

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
///     google_fonts (list): Google Fonts for this conversion
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     list[FontInfo]: Structured font metadata for each font used by the chart
#[pyfunction]
#[pyo3(signature = (vl_spec, vl_version=None, config=None, theme=None, auto_google_fonts=None, include_font_face=false, google_fonts=None, format_locale=None, time_format_locale=None))]
pub fn vegalite_fonts(
    py: Python<'_>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    auto_google_fonts: Option<bool>,
    include_font_face: bool,
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

        format_locale,
        time_format_locale,
        google_fonts,
        ..Default::default()
    };

    let result = run_converter_future(move |converter| async move {
        let config = converter.config();
        let auto_gf = auto_google_fonts.unwrap_or(config.auto_google_fonts);
        let embed_lf = config.embed_local_fonts;
        let subset_f = config.subset_fonts;
        converter
            .vegalite_fonts(
                vl_spec,
                vl_opts,
                auto_gf,
                embed_lf,
                include_font_face,
                subset_f,
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
///     google_fonts (list): Google Fonts for this conversion
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     list[FontInfo]: Structured font metadata for each font used by the chart
#[pyfunction]
#[pyo3(signature = (vg_spec, auto_google_fonts=None, include_font_face=false, google_fonts=None, format_locale=None, time_format_locale=None))]
pub fn vega_fonts(
    py: Python<'_>,
    vg_spec: PyObject,
    auto_google_fonts: Option<bool>,
    include_font_face: bool,
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
        google_fonts,
        ..Default::default()
    };

    let result = run_converter_future(move |converter| async move {
        let config = converter.config();
        let auto_gf = auto_google_fonts.unwrap_or(config.auto_google_fonts);
        let embed_lf = config.embed_local_fonts;
        let subset_f = config.subset_fonts;
        converter
            .vega_fonts(
                vg_spec,
                vg_opts,
                auto_gf,
                embed_lf,
                include_font_face,
                subset_f,
            )
            .await
    })
    .map_err(|err| prefixed_py_error("vega_fonts request failed", err))?;

    pythonize(py, &result)
        .map_err(|err| PyValueError::new_err(err.to_string()))
        .map(|obj| obj.into())
}

/// Register a directory of fonts for use in subsequent conversions
///
/// Args:
///     font_dir (str): Absolute path to a directory containing font files
///
/// Returns:
///     None
fn register_font_directory_inner(font_dir: &str) -> Result<(), vl_convert_rs::anyhow::Error> {
    vl_convert_rs::register_font_directory(font_dir)
}

#[pyfunction]
#[pyo3(signature = (font_dir))]
pub fn register_font_directory(font_dir: &str) -> PyResult<()> {
    register_font_directory_inner(font_dir)
        .map_err(|err| PyValueError::new_err(format!("Failed to register font directory: {}", err)))
}

/// Replace the registered font directories with the given list.
///
/// Unlike ``register_font_directory``, which only adds, this replaces
/// the full list — directories previously registered but absent from
/// ``font_dirs`` are dropped from the global registry, and the fontdb
/// no longer resolves their fonts on future conversions. Pass an empty
/// list to clear all registrations.
///
/// Args:
///     font_dirs (list[str]): Absolute paths to directories containing
///         font files
///
/// Returns:
///     None
fn set_font_directories_inner(font_dirs: Vec<String>) -> Result<(), vl_convert_rs::anyhow::Error> {
    let paths: Vec<std::path::PathBuf> = font_dirs
        .into_iter()
        .map(std::path::PathBuf::from)
        .collect();
    vl_convert_rs::set_font_directories(&paths)
}

#[pyfunction]
#[pyo3(signature = (font_dirs))]
pub fn set_font_directories(font_dirs: Vec<String>) -> PyResult<()> {
    set_font_directories_inner(font_dirs)
        .map_err(|err| PyValueError::new_err(format!("Failed to set font directories: {}", err)))
}

#[doc = async_variant_doc!("vegalite_fonts")]
#[pyfunction(name = "vegalite_fonts")]
#[pyo3(signature = (vl_spec, vl_version=None, config=None, theme=None, auto_google_fonts=None, include_font_face=false, google_fonts=None, format_locale=None, time_format_locale=None))]
pub fn vegalite_fonts_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    auto_google_fonts: Option<bool>,
    include_font_face: bool,
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

        format_locale,
        time_format_locale,
        google_fonts,
        ..Default::default()
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            let config = converter.config();
            let auto_gf = auto_google_fonts.unwrap_or(config.auto_google_fonts);
            let embed_lf = config.embed_local_fonts;
            let subset_f = config.subset_fonts;
            converter
                .vegalite_fonts(
                    vl_spec,
                    vl_opts,
                    auto_gf,
                    embed_lf,
                    include_font_face,
                    subset_f,
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
#[pyo3(signature = (vg_spec, auto_google_fonts=None, include_font_face=false, google_fonts=None, format_locale=None, time_format_locale=None))]
pub fn vega_fonts_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    auto_google_fonts: Option<bool>,
    include_font_face: bool,
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
        google_fonts,
        ..Default::default()
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            let config = converter.config();
            let auto_gf = auto_google_fonts.unwrap_or(config.auto_google_fonts);
            let embed_lf = config.embed_local_fonts;
            let subset_f = config.subset_fonts;
            converter
                .vega_fonts(
                    vg_spec,
                    vg_opts,
                    auto_gf,
                    embed_lf,
                    include_font_face,
                    subset_f,
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

#[doc = async_variant_doc!("register_font_directory")]
#[pyfunction(name = "register_font_directory")]
#[pyo3(signature = (font_dir))]
pub fn register_font_directory_asyncio<'py>(
    py: Python<'py>,
    font_dir: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let font_dir = font_dir.to_string();
    future_into_py_object(py, async move {
        tokio::task::spawn_blocking(move || register_font_directory_inner(&font_dir))
            .await
            .map_err(|err| PyValueError::new_err(format!("Task join error: {err}")))?
            .map_err(|err| {
                PyValueError::new_err(format!("Failed to register font directory: {err}"))
            })?;
        Python::with_gil(|py| Ok(py.None().into()))
    })
}

#[doc = async_variant_doc!("set_font_directories")]
#[pyfunction(name = "set_font_directories")]
#[pyo3(signature = (font_dirs))]
pub fn set_font_directories_asyncio<'py>(
    py: Python<'py>,
    font_dirs: Vec<String>,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py_object(py, async move {
        tokio::task::spawn_blocking(move || set_font_directories_inner(font_dirs))
            .await
            .map_err(|err| PyValueError::new_err(format!("Task join error: {err}")))?
            .map_err(|err| {
                PyValueError::new_err(format!("Failed to set font directories: {err}"))
            })?;
        Python::with_gil(|py| Ok(py.None().into()))
    })
}

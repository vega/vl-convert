#![allow(clippy::too_many_arguments)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use pythonize::{depythonize, pythonize};
use std::str::FromStr;
use std::sync::Mutex;
use vl_convert_rs::converter::{FormatLocale, Renderer, TimeFormatLocale, VgOpts, VlOpts};
use vl_convert_rs::html::bundle_vega_snippet;
use vl_convert_rs::module_loader::import_map::VlVersion;
use vl_convert_rs::module_loader::{FORMATE_LOCALE_MAP, TIME_FORMATE_LOCALE_MAP};
use vl_convert_rs::serde_json;
use vl_convert_rs::text::register_font_directory as register_font_directory_rs;
use vl_convert_rs::VlConverter as VlConverterRs;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref VL_CONVERTER: Mutex<VlConverterRs> = Mutex::new(VlConverterRs::new());
    static ref PYTHON_RUNTIME: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
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
#[pyo3(text_signature = "(vl_spec, vl_version, config, theme, show_warnings)")]
fn vegalite_to_vega(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
) -> PyResult<PyObject> {
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = config.and_then(|c| parse_json_spec(c).ok());

    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");
    let vega_spec = match PYTHON_RUNTIME.block_on(converter.vegalite_to_vega(
        vl_spec,
        VlOpts {
            vl_version,
            config,
            theme,
            show_warnings: show_warnings.unwrap_or(false),
            allowed_base_urls: None,
            format_locale: None,
            time_format_locale: None,
        },
    )) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega-Lite to Vega conversion failed:\n{}",
                err
            )))
        }
    };
    Python::with_gil(|py| -> PyResult<PyObject> {
        pythonize(py, &vega_spec).map_err(|err| PyValueError::new_err(err.to_string()))
    })
}

/// Convert a Vega spec to an SVG image string
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     str: SVG image string
#[pyfunction]
#[pyo3(text_signature = "(vg_spec, allowed_base_urls, format_locale, time_format_locale)")]
fn vega_to_svg(
    vg_spec: PyObject,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<String> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let svg = match PYTHON_RUNTIME.block_on(converter.vega_to_svg(
        vg_spec,
        VgOpts {
            allowed_base_urls,
            format_locale,
            time_format_locale,
        },
    )) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega to SVG conversion failed:\n{}",
                err
            )))
        }
    };
    Ok(svg)
}

/// Convert a Vega spec to a Vega Scenegraph
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     dict: scenegraph
#[pyfunction]
#[pyo3(text_signature = "(vg_spec, allowed_base_urls, format_locale, time_format_locale)")]
fn vega_to_scenegraph(
    vg_spec: PyObject,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<PyObject> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let sg = match PYTHON_RUNTIME.block_on(converter.vega_to_scenegraph(
        vg_spec,
        VgOpts {
            allowed_base_urls,
            format_locale,
            time_format_locale,
        },
    )) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega to Scenegraph conversion failed:\n{}",
                err
            )))
        }
    };
    Python::with_gil(|py| -> PyResult<PyObject> {
        pythonize(py, &sg).map_err(|err| PyValueError::new_err(err.to_string()))
    })
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
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     str: SVG image string
#[pyfunction]
#[pyo3(
    text_signature = "(vl_spec, vl_version, config, theme, show_warnings, allowed_base_urls, format_locale, time_format_locale)"
)]
fn vegalite_to_svg(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<String> {
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = config.and_then(|c| parse_json_spec(c).ok());
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let svg = match PYTHON_RUNTIME.block_on(converter.vegalite_to_svg(
        vl_spec,
        VlOpts {
            vl_version,
            config,
            theme,
            show_warnings: show_warnings.unwrap_or(false),
            allowed_base_urls,
            format_locale,
            time_format_locale,
        },
    )) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega-Lite to SVG conversion failed:\n{}",
                err
            )))
        }
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
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     str: SVG image string
#[pyfunction]
#[pyo3(
    text_signature = "(vl_spec, vl_version, config, theme, show_warnings, allowed_base_urls, format_locale, time_format_locale)"
)]
fn vegalite_to_scenegraph(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<PyObject> {
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = config.and_then(|c| parse_json_spec(c).ok());
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let sg = match PYTHON_RUNTIME.block_on(converter.vegalite_to_scenegraph(
        vl_spec,
        VlOpts {
            vl_version,
            config,
            theme,
            show_warnings: show_warnings.unwrap_or(false),
            allowed_base_urls,
            format_locale,
            time_format_locale,
        },
    )) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega-Lite to SVG conversion failed:\n{}",
                err
            )))
        }
    };
    Python::with_gil(|py| -> PyResult<PyObject> {
        pythonize(py, &sg).map_err(|err| PyValueError::new_err(err.to_string()))
    })
}

/// Convert a Vega spec to PNG image data.
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     scale (float): Image scale factor (default 1.0)
///     ppi (float): Pixels per inch (default 72)
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(
    text_signature = "(vg_spec, scale, ppi, allowed_base_urls, format_locale, time_format_locale)"
)]
fn vega_to_png(
    vg_spec: PyObject,
    scale: Option<f32>,
    ppi: Option<f32>,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<PyObject> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let png_data = match PYTHON_RUNTIME.block_on(converter.vega_to_png(
        vg_spec,
        VgOpts {
            allowed_base_urls,
            format_locale,
            time_format_locale,
        },
        scale,
        ppi,
    )) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega to PNG conversion failed:\n{}",
                err
            )))
        }
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyObject::from(PyBytes::new(py, png_data.as_slice()))
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
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(
    text_signature = "(vl_spec, vl_version, scale, ppi, config, theme, show_warnings, allowed_base_urls, format_locale, time_format_locale)"
)]
fn vegalite_to_png(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    ppi: Option<f32>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<PyObject> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = config.and_then(|c| parse_json_spec(c).ok());
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let png_data = match PYTHON_RUNTIME.block_on(converter.vegalite_to_png(
        vl_spec,
        VlOpts {
            vl_version,
            config,
            theme,
            show_warnings: show_warnings.unwrap_or(false),
            allowed_base_urls,
            format_locale,
            time_format_locale,
        },
        scale,
        ppi,
    )) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega-Lite to PNG conversion failed:\n{}",
                err
            )))
        }
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyObject::from(PyBytes::new(py, png_data.as_slice()))
    }))
}

/// Convert a Vega spec to JPEG image data.
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     scale (float): Image scale factor (default 1.0)
///     quality (int): JPEG Quality between 0 (worst) and 100 (best). Default 90
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: JPEG image data
#[pyfunction]
#[pyo3(
    text_signature = "(vg_spec, scale, quality, allowed_base_urls, format_locale, time_format_locale)"
)]
fn vega_to_jpeg(
    vg_spec: PyObject,
    scale: Option<f32>,
    quality: Option<u8>,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<PyObject> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let jpeg_data = match PYTHON_RUNTIME.block_on(converter.vega_to_jpeg(
        vg_spec,
        VgOpts {
            allowed_base_urls,
            format_locale,
            time_format_locale,
        },
        scale,
        quality,
    )) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega to JPEG conversion failed:\n{}",
                err
            )))
        }
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyObject::from(PyBytes::new(py, jpeg_data.as_slice()))
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
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: JPEG image data
#[pyfunction]
#[pyo3(
    text_signature = "(vl_spec, vl_version, scale, quality, config, theme, show_warnings, allowed_base_urls, format_locale, time_format_locale)"
)]
fn vegalite_to_jpeg(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    quality: Option<u8>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<PyObject> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = config.and_then(|c| parse_json_spec(c).ok());
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let jpeg_data = match PYTHON_RUNTIME.block_on(converter.vegalite_to_jpeg(
        vl_spec,
        VlOpts {
            vl_version,
            config,
            theme,
            show_warnings: show_warnings.unwrap_or(false),
            allowed_base_urls,
            format_locale,
            time_format_locale,
        },
        scale,
        quality,
    )) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega-Lite to JPEG conversion failed:\n{}",
                err
            )))
        }
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyObject::from(PyBytes::new(py, jpeg_data.as_slice()))
    }))
}

/// Convert a Vega spec to PDF format
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     scale (float): Image scale factor (default 1.0)
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: PDF file bytes
#[pyfunction]
#[pyo3(text_signature = "(vg_spec, scale, allowed_base_urls, format_locale, time_format_locale)")]
fn vega_to_pdf(
    vg_spec: PyObject,
    scale: Option<f32>,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<PyObject> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let pdf_bytes = match PYTHON_RUNTIME.block_on(converter.vega_to_pdf(
        vg_spec,
        VgOpts {
            allowed_base_urls,
            format_locale,
            time_format_locale,
        },
        scale,
    )) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega to PDF conversion failed:\n{}",
                err
            )))
        }
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
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: PDF image data
#[pyfunction]
#[pyo3(
    text_signature = "(vl_spec, vl_version, scale, config, theme, allowed_base_urls, format_locale, time_format_locale)"
)]
fn vegalite_to_pdf(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<PyObject> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = config.and_then(|c| parse_json_spec(c).ok());
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let pdf_data = match PYTHON_RUNTIME.block_on(converter.vegalite_to_pdf(
        vl_spec,
        VlOpts {
            vl_version,
            config,
            theme,
            show_warnings: show_warnings.unwrap_or(false),
            allowed_base_urls,
            format_locale,
            time_format_locale,
        },
        scale,
    )) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega-Lite to PDF conversion failed:\n{}",
                err
            )))
        }
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
#[pyo3(text_signature = "(vl_spec, fullscreen)")]
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
#[pyo3(text_signature = "(vg_spec, fullscreen)")]
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
    text_signature = "(vl_spec, vl_version, bundle, config, theme, format_locale, time_format_locale, renderer)"
)]
fn vegalite_to_html(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    bundle: Option<bool>,
    config: Option<PyObject>,
    theme: Option<String>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    renderer: Option<String>,
) -> PyResult<String> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = config.and_then(|c| parse_json_spec(c).ok());
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let renderer = renderer.unwrap_or_else(|| "svg".to_string());
    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    Ok(PYTHON_RUNTIME.block_on(converter.vegalite_to_html(
        vl_spec,
        VlOpts {
            vl_version,
            config,
            theme,
            show_warnings: false,
            allowed_base_urls: None,
            format_locale,
            time_format_locale,
        },
        bundle.unwrap_or(false),
        Renderer::from_str(&renderer)?,
    ))?)
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
#[pyo3(text_signature = "(vg_spec, bundle, format_locale, time_format_locale, renderer)")]
fn vega_to_html(
    vg_spec: PyObject,
    bundle: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    renderer: Option<String>,
) -> PyResult<String> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let renderer = renderer.unwrap_or_else(|| "svg".to_string());
    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");
    Ok(PYTHON_RUNTIME.block_on(converter.vega_to_html(
        vg_spec,
        VgOpts {
            allowed_base_urls: None,
            format_locale,
            time_format_locale,
        },
        bundle.unwrap_or(false),
        Renderer::from_str(&renderer)?,
    ))?)
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
#[pyo3(text_signature = "(svg, scale, ppi)")]
fn svg_to_png(svg: &str, scale: Option<f32>, ppi: Option<f32>) -> PyResult<PyObject> {
    let png_data = vl_convert_rs::converter::svg_to_png(svg, scale.unwrap_or(1.0), ppi)?;
    Ok(Python::with_gil(|py| -> PyObject {
        PyObject::from(PyBytes::new(py, png_data.as_slice()))
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
#[pyo3(text_signature = "(svg, scale, quality)")]
fn svg_to_jpeg(svg: &str, scale: Option<f32>, quality: Option<u8>) -> PyResult<PyObject> {
    let jpeg_data = vl_convert_rs::converter::svg_to_jpeg(svg, scale.unwrap_or(1.0), quality)?;
    Ok(Python::with_gil(|py| -> PyObject {
        PyObject::from(PyBytes::new(py, jpeg_data.as_slice()))
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
#[pyo3(text_signature = "(svg, scale)")]
fn svg_to_pdf(svg: &str, scale: Option<f32>) -> PyResult<PyObject> {
    let pdf_data = vl_convert_rs::converter::svg_to_pdf(svg, scale.unwrap_or(1.0))?;
    Ok(Python::with_gil(|py| -> PyObject {
        PyObject::from(PyBytes::new(py, pdf_data.as_slice()))
    }))
}

/// Helper function to parse an input Python string or dict as a serde_json::Value
fn parse_json_spec(vl_spec: PyObject) -> PyResult<serde_json::Value> {
    Python::with_gil(|py| -> PyResult<serde_json::Value> {
        if let Ok(vl_spec) = vl_spec.extract::<&str>(py) {
            match serde_json::from_str::<serde_json::Value>(vl_spec) {
                Ok(vl_spec) => Ok(vl_spec),
                Err(err) => Err(PyValueError::new_err(format!(
                    "Failed to parse vl_spec string as JSON: {}",
                    err
                ))),
            }
        } else if let Ok(vl_spec) = vl_spec.downcast::<PyDict>(py) {
            match depythonize(vl_spec) {
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

/// Helper function to parse an input Python string or dict as a FormatLocale
fn parse_format_locale(v: PyObject) -> PyResult<FormatLocale> {
    Python::with_gil(|py| -> PyResult<FormatLocale> {
        if let Ok(name) = v.extract::<&str>(py) {
            let format_locale = FormatLocale::Name(name.to_string());
            if format_locale.as_object().is_err() {
                Err(PyValueError::new_err(
                    format!("Invalid format_locale name: {name}\nSee https://github.com/d3/d3-format/tree/main/locale for available names")
                ))
            } else {
                Ok(format_locale)
            }
        } else if let Ok(obj) = v.downcast::<PyDict>(py) {
            match depythonize(obj) {
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
        if let Ok(name) = v.extract::<&str>(py) {
            let time_format_locale = TimeFormatLocale::Name(name.to_string());
            if time_format_locale.as_object().is_err() {
                Err(PyValueError::new_err(
                    format!("Invalid time_format_locale name: {name}\nSee https://github.com/d3/d3-time-format/tree/main/locale for available names")
                ))
            } else {
                Ok(time_format_locale)
            }
        } else if let Ok(obj) = v.downcast::<PyDict>(py) {
            match depythonize(obj) {
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

/// Register a directory of fonts for use in subsequent conversions
///
/// Args:
///     font_dir (str): Absolute path to a directory containing font files
///
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(text_signature = "(font_dir)")]
fn register_font_directory(font_dir: &str) -> PyResult<()> {
    register_font_directory_rs(font_dir).map_err(|err| {
        PyValueError::new_err(format!("Failed to register font directory: {}", err))
    })?;
    Ok(())
}

/// Get the named local timezone that Vega uses to perform timezone calculations
///
/// Returns:
///     str: Named local timezone (e.g. "America/New_York"),
///          or None if the local timezone cannot be determined
#[pyfunction]
#[pyo3(text_signature = "()")]
fn get_local_tz() -> PyResult<Option<String>> {
    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");
    let local_tz = match PYTHON_RUNTIME.block_on(converter.get_local_tz()) {
        Ok(local_tz) => local_tz,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "get_local_tz request failed:\n{}",
                err
            )))
        }
    };
    Ok(local_tz)
}

/// Get the config dict for each built-in theme
///
/// Returns:
///     dict: dict from theme name to config object
#[pyfunction]
#[pyo3(text_signature = "()")]
fn get_themes() -> PyResult<PyObject> {
    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");
    let themes = match PYTHON_RUNTIME.block_on(converter.get_themes()) {
        Ok(themes) => themes,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "get_themes request failed:\n{}",
                err
            )))
        }
    };
    Python::with_gil(|py| -> PyResult<PyObject> {
        pythonize(py, &themes).map_err(|err| PyValueError::new_err(err.to_string()))
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
#[pyo3(text_signature = "(name)")]
fn get_format_locale(name: &str) -> PyResult<PyObject> {
    match FORMATE_LOCALE_MAP.get(name) {
        None => {
            Err(PyValueError::new_err(format!(
                "Invalid format locale name: {name}\nSee https://github.com/d3/d3-format/tree/main/locale for available names"
            )))
        }
        Some(locale) => {
            let locale: serde_json::Value = serde_json::from_str(locale).expect(
                "Failed to parse internal format locale as JSON"
            );
            Python::with_gil(|py| -> PyResult<PyObject> {
                pythonize(py, &locale).map_err(|err| PyValueError::new_err(err.to_string()))
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
#[pyo3(text_signature = "(name)")]
fn get_time_format_locale(name: &str) -> PyResult<PyObject> {
    match TIME_FORMATE_LOCALE_MAP.get(name) {
        None => {
            Err(PyValueError::new_err(format!(
                "Invalid time format locale name: {name}\nSee https://github.com/d3/d3-time-format/tree/main/locale for available names"
            )))
        }
        Some(locale) => {
            let locale: serde_json::Value = serde_json::from_str(locale).expect(
                "Failed to parse internal time format locale as JSON"
            );
            Python::with_gil(|py| -> PyResult<PyObject> {
                pythonize(py, &locale).map_err(|err| PyValueError::new_err(err.to_string()))
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
#[pyo3(text_signature = "(snippet, vl_version)")]
fn javascript_bundle(snippet: Option<String>, vl_version: Option<&str>) -> PyResult<String> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    if let Some(snippet) = &snippet {
        Ok(PYTHON_RUNTIME.block_on(bundle_vega_snippet(snippet, vl_version))?)
    } else {
        let mut converter = VL_CONVERTER
            .lock()
            .expect("Failed to acquire lock on Vega-Lite converter");
        Ok(PYTHON_RUNTIME.block_on(converter.get_vegaembed_bundle(vl_version))?)
    }
}

/// Convert Vega-Lite specifications to other formats
#[pymodule]
fn vl_convert(_py: Python, m: &PyModule) -> PyResult<()> {
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
    m.add_function(wrap_pyfunction!(svg_to_png, m)?)?;
    m.add_function(wrap_pyfunction!(svg_to_jpeg, m)?)?;
    m.add_function(wrap_pyfunction!(svg_to_pdf, m)?)?;
    m.add_function(wrap_pyfunction!(register_font_directory, m)?)?;
    m.add_function(wrap_pyfunction!(get_local_tz, m)?)?;
    m.add_function(wrap_pyfunction!(get_themes, m)?)?;
    m.add_function(wrap_pyfunction!(get_format_locale, m)?)?;
    m.add_function(wrap_pyfunction!(get_time_format_locale, m)?)?;
    m.add_function(wrap_pyfunction!(javascript_bundle, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

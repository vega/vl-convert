#![allow(clippy::too_many_arguments)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::uninlined_format_args)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use pythonize::{depythonize, pythonize};
use std::borrow::Cow;
use std::future::Future;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use vl_convert_rs::converter::{
    FormatLocale, Renderer, TimeFormatLocale, ValueOrString, VgOpts, VlOpts,
};
use vl_convert_rs::html::bundle_vega_snippet;
use vl_convert_rs::module_loader::import_map::{
    VlVersion, VEGA_EMBED_VERSION, VEGA_THEMES_VERSION, VEGA_VERSION, VL_VERSIONS,
};
use vl_convert_rs::module_loader::{FORMATE_LOCALE_MAP, TIME_FORMATE_LOCALE_MAP};
use vl_convert_rs::serde_json;
use vl_convert_rs::text::register_font_directory as register_font_directory_rs;
use vl_convert_rs::VlConverter as VlConverterRs;

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
}

fn converter_read_handle() -> Result<Arc<VlConverterRs>, vl_convert_rs::anyhow::Error> {
    VL_CONVERTER
        .read()
        .map_err(|e| vl_convert_rs::anyhow::anyhow!("Failed to acquire converter read lock: {e}"))
        .map(|guard| guard.clone())
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
    let config = config.and_then(|c| parse_json_spec(c).ok());

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
        allowed_base_urls: None,
        format_locale: None,
        time_format_locale: None,
    };

    let vega_spec = match run_converter_future(move |converter| async move {
        converter.vegalite_to_vega(vl_spec, vl_opts).await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega-Lite to Vega conversion failed:\n{}",
                err
            )))
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
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     str: SVG image string
#[pyfunction]
#[pyo3(signature = (vg_spec, allowed_base_urls=None, format_locale=None, time_format_locale=None))]
fn vega_to_svg(
    vg_spec: PyObject,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<String> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vg_opts = VgOpts {
        allowed_base_urls,
        format_locale,
        time_format_locale,
    };

    let svg = match run_converter_future(move |converter| async move {
        converter.vega_to_svg(vg_spec, vg_opts).await
    }) {
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
///     format (str): Output format, either "dict" (default) or "msgpack"
/// Returns:
///     dict | bytes: scenegraph as dict (format="dict") or msgpack bytes (format="msgpack")
#[pyfunction]
#[pyo3(signature = (vg_spec, allowed_base_urls=None, format_locale=None, time_format_locale=None, format="dict"))]
fn vega_to_scenegraph(
    vg_spec: PyObject,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    format: &str,
) -> PyResult<PyObject> {
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vg_opts = VgOpts {
        allowed_base_urls,
        format_locale,
        time_format_locale,
    };

    match format {
        "dict" => {
            let vg_spec = parse_json_spec(vg_spec)?;
            let sg = run_converter_future(move |converter| async move {
                converter.vega_to_scenegraph(vg_spec, vg_opts).await
            })
            .map_err(|err| {
                PyValueError::new_err(format!("Vega to Scenegraph conversion failed:\n{err}"))
            })?;
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
            .map_err(|err| {
                PyValueError::new_err(format!("Vega to Scenegraph conversion failed:\n{err}"))
            })?;
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
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     str: SVG image string
#[pyfunction]
#[pyo3(
    signature = (vl_spec, vl_version=None, config=None, theme=None, show_warnings=None, allowed_base_urls=None, format_locale=None, time_format_locale=None)
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

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),
        allowed_base_urls,
        format_locale,
        time_format_locale,
    };

    let svg = match run_converter_future(move |converter| async move {
        converter.vegalite_to_svg(vl_spec, vl_opts).await
    }) {
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
///     format (str): Output format, either "dict" (default) or "msgpack"
/// Returns:
///     dict | bytes: scenegraph as dict (format="dict") or msgpack bytes (format="msgpack")
#[pyfunction]
#[pyo3(
    signature = (vl_spec, vl_version=None, config=None, theme=None, show_warnings=None, allowed_base_urls=None, format_locale=None, time_format_locale=None, format="dict")
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
    format: &str,
) -> PyResult<PyObject> {
    let config = config.and_then(|c| parse_json_spec(c).ok());
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
        allowed_base_urls,
        format_locale,
        time_format_locale,
    };

    match format {
        "dict" => {
            let vl_spec = parse_json_spec(vl_spec)?;
            let sg = run_converter_future(move |converter| async move {
                converter.vegalite_to_scenegraph(vl_spec, vl_opts).await
            })
            .map_err(|err| {
                PyValueError::new_err(format!("Vega-Lite to Scenegraph conversion failed:\n{err}"))
            })?;
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
            .map_err(|err| {
                PyValueError::new_err(format!("Vega-Lite to Scenegraph conversion failed:\n{err}"))
            })?;
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
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(
    signature = (vg_spec, scale=None, ppi=None, allowed_base_urls=None, format_locale=None, time_format_locale=None)
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

    let vg_opts = VgOpts {
        allowed_base_urls,
        format_locale,
        time_format_locale,
    };

    let png_data = match run_converter_future(move |converter| async move {
        converter.vega_to_png(vg_spec, vg_opts, scale, ppi).await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega to PNG conversion failed:\n{}",
                err
            )))
        }
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
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(
    signature = (vl_spec, vl_version=None, scale=None, ppi=None, config=None, theme=None, show_warnings=None, allowed_base_urls=None, format_locale=None, time_format_locale=None)
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

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),
        allowed_base_urls,
        format_locale,
        time_format_locale,
    };

    let png_data = match run_converter_future(move |converter| async move {
        converter
            .vegalite_to_png(vl_spec, vl_opts, scale, ppi)
            .await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega-Lite to PNG conversion failed:\n{}",
                err
            )))
        }
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
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: JPEG image data
#[pyfunction]
#[pyo3(
    signature = (vg_spec, scale=None, quality=None, allowed_base_urls=None, format_locale=None, time_format_locale=None)
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

    let vg_opts = VgOpts {
        allowed_base_urls,
        format_locale,
        time_format_locale,
    };

    let jpeg_data = match run_converter_future(move |converter| async move {
        converter
            .vega_to_jpeg(vg_spec, vg_opts, scale, quality)
            .await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega to JPEG conversion failed:\n{}",
                err
            )))
        }
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
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: JPEG image data
#[pyfunction]
#[pyo3(
    signature = (vl_spec, vl_version=None, scale=None, quality=None, config=None, theme=None, show_warnings=None, allowed_base_urls=None, format_locale=None, time_format_locale=None)
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

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: show_warnings.unwrap_or(false),
        allowed_base_urls,
        format_locale,
        time_format_locale,
    };

    let jpeg_data = match run_converter_future(move |converter| async move {
        converter
            .vegalite_to_jpeg(vl_spec, vl_opts, scale, quality)
            .await
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega-Lite to JPEG conversion failed:\n{}",
                err
            )))
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
///     allowed_base_urls (list of str): List of allowed base URLs for external
///                                      data requests. Default allows any base URL
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
/// Returns:
///     bytes: PDF file bytes
#[pyfunction]
#[pyo3(signature = (vg_spec, scale=None, allowed_base_urls=None, format_locale=None, time_format_locale=None))]
fn vega_to_pdf(
    vg_spec: PyObject,
    scale: Option<f32>,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<PyObject> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vg_opts = VgOpts {
        allowed_base_urls,
        format_locale,
        time_format_locale,
    };

    let pdf_bytes = match run_converter_future(move |converter| async move {
        converter.vega_to_pdf(vg_spec, vg_opts).await
    }) {
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
    signature = (vl_spec, vl_version=None, scale=None, config=None, theme=None, allowed_base_urls=None, format_locale=None, time_format_locale=None)
)]
fn vegalite_to_pdf(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    config: Option<PyObject>,
    theme: Option<String>,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
) -> PyResult<PyObject> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = config.and_then(|c| parse_json_spec(c).ok());
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: false,
        allowed_base_urls,
        format_locale,
        time_format_locale,
    };

    let pdf_data = match run_converter_future(move |converter| async move {
        converter.vegalite_to_pdf(vl_spec, vl_opts).await
    }) {
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
    signature = (vl_spec, vl_version=None, bundle=None, config=None, theme=None, format_locale=None, time_format_locale=None, renderer=None)
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
    let renderer = Renderer::from_str(&renderer)?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        show_warnings: false,
        allowed_base_urls: None,
        format_locale,
        time_format_locale,
    };

    Ok(run_converter_future(move |converter| async move {
        converter
            .vegalite_to_html(vl_spec, vl_opts, bundle.unwrap_or(false), renderer)
            .await
    })?)
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
#[pyo3(signature = (vg_spec, bundle=None, format_locale=None, time_format_locale=None, renderer=None))]
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
    let renderer = Renderer::from_str(&renderer)?;
    let vg_opts = VgOpts {
        allowed_base_urls: None,
        format_locale,
        time_format_locale,
    };
    Ok(run_converter_future(move |converter| async move {
        converter
            .vega_to_html(vg_spec, vg_opts, bundle.unwrap_or(false), renderer)
            .await
    })?)
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
    let png_data = vl_convert_rs::converter::svg_to_png(svg, scale.unwrap_or(1.0), ppi)?;
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
    let jpeg_data = vl_convert_rs::converter::svg_to_jpeg(svg, scale.unwrap_or(1.0), quality)?;
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
    let pdf_data = vl_convert_rs::converter::svg_to_pdf(svg)?; // Always pass 1.0 as scale
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

/// Set the number of parallel converter workers for subsequent requests
#[pyfunction]
#[pyo3(signature = (num_workers))]
fn set_num_workers(num_workers: usize) -> PyResult<()> {
    if num_workers < 1 {
        return Err(PyValueError::new_err("num_workers must be >= 1"));
    }

    let converter = VlConverterRs::with_num_workers(num_workers).map_err(|err| {
        PyValueError::new_err(format!(
            "Failed to set worker count to {num_workers}: {err}"
        ))
    })?;

    let mut guard = VL_CONVERTER.write().map_err(|e| {
        PyValueError::new_err(format!("Failed to acquire converter write lock: {e}"))
    })?;
    *guard = Arc::new(converter);
    Ok(())
}

/// Get the number of configured converter workers
#[pyfunction]
#[pyo3(signature = ())]
fn get_num_workers() -> PyResult<usize> {
    let guard = VL_CONVERTER.read().map_err(|e| {
        PyValueError::new_err(format!("Failed to acquire converter read lock: {e}"))
    })?;
    Ok(guard.num_workers())
}

/// Get the named local timezone that Vega uses to perform timezone calculations
///
/// Returns:
///     str: Named local timezone (e.g. "America/New_York"),
///          or None if the local timezone cannot be determined
#[pyfunction]
#[pyo3(signature = ())]
fn get_local_tz() -> PyResult<Option<String>> {
    let local_tz =
        match run_converter_future(|converter| async move { converter.get_local_tz().await }) {
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
#[pyo3(signature = ())]
fn get_themes() -> PyResult<PyObject> {
    let themes = match run_converter_future(|converter| async move { converter.get_themes().await })
    {
        Ok(themes) => themes,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "get_themes request failed:\n{}",
                err
            )))
        }
    };
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
            let locale: serde_json::Value = serde_json::from_str(locale).expect(
                "Failed to parse internal format locale as JSON"
            );
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
            let locale: serde_json::Value = serde_json::from_str(locale).expect(
                "Failed to parse internal time format locale as JSON"
            );
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
        Ok(Python::with_gil(|py| {
            py.allow_threads(|| PYTHON_RUNTIME.block_on(bundle_vega_snippet(&snippet, vl_version)))
        })?)
    } else {
        Ok(run_converter_future(move |converter| async move {
            converter.get_vegaembed_bundle(vl_version).await
        })?)
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

/// Convert Vega-Lite specifications to other formats
#[pymodule]
fn vl_convert(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
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
    m.add_function(wrap_pyfunction!(set_num_workers, m)?)?;
    m.add_function(wrap_pyfunction!(get_num_workers, m)?)?;
    m.add_function(wrap_pyfunction!(get_local_tz, m)?)?;
    m.add_function(wrap_pyfunction!(get_themes, m)?)?;
    m.add_function(wrap_pyfunction!(get_format_locale, m)?)?;
    m.add_function(wrap_pyfunction!(get_time_format_locale, m)?)?;
    m.add_function(wrap_pyfunction!(javascript_bundle, m)?)?;
    m.add_function(wrap_pyfunction!(get_vega_version, m)?)?;
    m.add_function(wrap_pyfunction!(get_vega_themes_version, m)?)?;
    m.add_function(wrap_pyfunction!(get_vega_embed_version, m)?)?;
    m.add_function(wrap_pyfunction!(get_vegalite_versions, m)?)?;
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

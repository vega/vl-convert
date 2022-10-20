use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use pythonize::{depythonize, pythonize};
use std::str::FromStr;
use std::sync::Mutex;
use vl_convert_rs::converter::TOKIO_RUNTIME;
use vl_convert_rs::module_loader::import_map::VlVersion;
use vl_convert_rs::serde_json;
use vl_convert_rs::text::register_font_directory as register_font_directory_rs;
use vl_convert_rs::VlConverter as VlConverterRs;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref VL_CONVERTER: Mutex<VlConverterRs> = Mutex::new(VlConverterRs::new());
}

/// Convert a Vega-Lite spec to a Vega spec using a particular
/// version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str): Vega-Lite library version string (e.g. 'v5.5')
///         (default to latest)
///     pretty (bool): If True, pretty-print resulting Vega JSON
///         specification (default False)
///
/// Returns:
///     str: Vega JSON specification string
#[pyfunction]
#[pyo3(text_signature = "(vl_spec, vl_version, pretty)")]
fn vegalite_to_vega(vl_spec: PyObject, vl_version: Option<&str>) -> PyResult<PyObject> {
    let vl_spec = parse_json_spec(vl_spec)?;

    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");
    let vega_spec = match TOKIO_RUNTIME.block_on(converter.vegalite_to_vega(vl_spec, vl_version)) {
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
///
/// Returns:
///     str: SVG image string
#[pyfunction]
#[pyo3(text_signature = "(vg_spec)")]
fn vega_to_svg(vg_spec: PyObject) -> PyResult<String> {
    let vg_spec = parse_json_spec(vg_spec)?;

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let svg = match TOKIO_RUNTIME.block_on(converter.vega_to_svg(vg_spec)) {
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

/// Convert a Vega-Lite spec to an SVG image string using a
/// particular version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str): Vega-Lite library version string (e.g. 'v5.5')
///         (default to latest)
///
/// Returns:
///     str: SVG image string
#[pyfunction]
#[pyo3(text_signature = "(vl_spec, vl_version)")]
fn vegalite_to_svg(vl_spec: PyObject, vl_version: Option<&str>) -> PyResult<String> {
    let vl_spec = parse_json_spec(vl_spec)?;

    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let svg = match TOKIO_RUNTIME.block_on(converter.vegalite_to_svg(vl_spec, vl_version)) {
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

/// Convert a Vega spec to PNG image data.
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     scale (float): Image scale factor (default 1.0)
///
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(text_signature = "(vg_spec, scale)")]
fn vega_to_png(vg_spec: PyObject, scale: Option<f32>) -> PyResult<PyObject> {
    let vg_spec = parse_json_spec(vg_spec)?;

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let png_data = match TOKIO_RUNTIME.block_on(converter.vega_to_png(vg_spec, scale)) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(PyValueError::new_err(format!(
                "Vega to SVG conversion failed:\n{}",
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
///     vl_version (str): Vega-Lite library version string (e.g. 'v5.5')
///         (default to latest)
///     scale (float): Image scale factor (default 1.0)
///
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(text_signature = "(vl_spec, vl_version, scale)")]
fn vegalite_to_png(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
) -> PyResult<PyObject> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;

    let mut converter = VL_CONVERTER
        .lock()
        .expect("Failed to acquire lock on Vega-Lite converter");

    let png_data =
        match TOKIO_RUNTIME.block_on(converter.vegalite_to_png(vl_spec, vl_version, scale)) {
            Ok(vega_spec) => vega_spec,
            Err(err) => {
                return Err(PyValueError::new_err(format!(
                    "Vega to SVG conversion failed:\n{}",
                    err
                )))
            }
        };

    Ok(Python::with_gil(|py| -> PyObject {
        PyObject::from(PyBytes::new(py, png_data.as_slice()))
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
        } else if let Ok(vl_spec) = vl_spec.cast_as::<PyDict>(py) {
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

/// Convert Vega-Lite specifications to other formats
#[pymodule]
fn vl_convert(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(vegalite_to_vega, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_svg, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_png, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_svg, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_png, m)?)?;
    m.add_function(wrap_pyfunction!(register_font_directory, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

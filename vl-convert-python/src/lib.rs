use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::str::FromStr;
use std::sync::Mutex;
use vl_convert_rs::converter::TOKIO_RUNTIME;
use vl_convert_rs::module_loader::import_map::VlVersion;
use vl_convert_rs::serde_json;
use vl_convert_rs::VlConverter as VlConverterRs;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref VL_CONVERTER: Mutex<VlConverterRs> = Mutex::new(VlConverterRs::new());
}

#[pyclass]
struct VlConverter;

#[pymethods]
impl VlConverter {
    #[new]
    fn new() -> Self {
        Self
    }

    fn vegalite_to_vega(
        &mut self,
        vl_spec: &str,
        vl_version: &str,
        pretty: Option<bool>,
    ) -> PyResult<String> {
        let vl_version = VlVersion::from_str(vl_version)?;
        let pretty = pretty.unwrap_or(false);
        let vl_spec = match serde_json::from_str::<serde_json::Value>(vl_spec) {
            Ok(vl_spec) => vl_spec,
            Err(err) => {
                return Err(PyValueError::new_err(format!(
                    "Failed to parse vl_spec as JSON: {}",
                    err
                )))
            }
        };
        let mut converter = VL_CONVERTER
            .lock()
            .expect("Failed to acquire lock on Vega-Lite converter");
        let vega_spec =
            match TOKIO_RUNTIME.block_on(converter.vegalite_to_vega(vl_spec, vl_version, pretty)) {
                Ok(vega_spec) => vega_spec,
                Err(err) => {
                    return Err(PyValueError::new_err(format!(
                        "Vega-Lite to Vega conversion failed:\n{}",
                        err
                    )))
                }
            };
        Ok(vega_spec)
    }

    fn vega_to_svg(&mut self, vg_spec: &str) -> PyResult<String> {
        let vg_spec = match serde_json::from_str::<serde_json::Value>(vg_spec) {
            Ok(vg_spec) => vg_spec,
            Err(err) => {
                return Err(PyValueError::new_err(format!(
                    "Failed to parse vg_spec as JSON: {}",
                    err
                )))
            }
        };

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

    fn vegalite_to_svg(
        &mut self,
        vl_spec: &str,
        vl_version: &str,
        scale: Option<f32>,
    ) -> PyResult<String> {
        let vl_version = VlVersion::from_str(vl_version)?;
        let vl_spec = match serde_json::from_str::<serde_json::Value>(vl_spec) {
            Ok(vl_spec) => vl_spec,
            Err(err) => {
                return Err(PyValueError::new_err(format!(
                    "Failed to parse vl_spec as JSON: {}",
                    err
                )))
            }
        };

        let mut converter = VL_CONVERTER
            .lock()
            .expect("Failed to acquire lock on Vega-Lite converter");

        let svg =
            match TOKIO_RUNTIME.block_on(converter.vegalite_to_svg(vl_spec, vl_version, scale)) {
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

    fn vega_to_png(&mut self, vg_spec: &str, scale: Option<f32>) -> PyResult<PyObject> {
        let vg_spec = match serde_json::from_str::<serde_json::Value>(vg_spec) {
            Ok(vg_spec) => vg_spec,
            Err(err) => {
                return Err(PyValueError::new_err(format!(
                    "Failed to parse vg_spec as JSON: {}",
                    err
                )))
            }
        };

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

    fn vegalite_to_png(
        &mut self,
        vl_spec: &str,
        vl_version: &str,
        scale: Option<f32>,
    ) -> PyResult<PyObject> {
        let vl_version = VlVersion::from_str(vl_version)?;
        let vl_spec = match serde_json::from_str::<serde_json::Value>(vl_spec) {
            Ok(vl_spec) => vl_spec,
            Err(err) => {
                return Err(PyValueError::new_err(format!(
                    "Failed to parse vg_spec as JSON: {}",
                    err
                )))
            }
        };

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
}

/// A Python module implemented in Rust.
#[pymodule]
fn vl_convert(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<VlConverter>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

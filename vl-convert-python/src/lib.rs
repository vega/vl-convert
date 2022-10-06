use futures::executor::block_on;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::str::FromStr;
use std::sync::Mutex;
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
        let vega_spec = match block_on(converter.vegalite_to_vega(vl_spec, vl_version, pretty)) {
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
}

/// A Python module implemented in Rust.
#[pymodule]
fn vl_convert(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<VlConverter>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

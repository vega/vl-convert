use std::str::FromStr;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use vl_convert_rs::{VlConverter as VlConverterRs};
use tokio::runtime;
use vl_convert_rs::module_loader::import_map::VlVersion;
use vl_convert_rs::serde_json;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref TOKIO_RUNTIME: runtime::Runtime = runtime::Builder::new_multi_thread()
    .build()
    .expect("Failed to initialize tokio runtime");
}

// TODO: make VlConverterRs sendable
#[pyclass(unsendable)]
struct VlConverter {
    converter: VlConverterRs,
}

#[pymethods]
impl VlConverter {
    #[new]
    fn new(vl_version: &str) -> PyResult<Self> {
        let vl_version = VlVersion::from_str(vl_version)?;
        let converter = TOKIO_RUNTIME.block_on(VlConverterRs::try_new(vl_version)).unwrap();
        Ok(Self {
            converter,
        })
    }

    fn vegalite_to_vega(&mut self, vl_spec: &str, pretty: Option<bool>) -> PyResult<String> {
        let pretty = pretty.unwrap_or(false);
        let vl_spec = match serde_json::from_str::<serde_json::Value>(vl_spec) {
            Ok(vl_spec) => vl_spec,
            Err(err) => {
                return Err(PyValueError::new_err(format!("Failed to parse vl_spec as JSON: {}", err.to_string())))
            },
        };

        let vega_spec = TOKIO_RUNTIME.block_on(self.converter.vegalite_to_vega(&vl_spec, pretty))?;
        Ok(vega_spec)
    }
}

/// A Python module implemented in Rust.
#[pymodule]
fn vl_convert(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<VlConverter>()?;
    Ok(())
}
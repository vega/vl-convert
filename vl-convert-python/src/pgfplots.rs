//! Bindings for the shared native PGFPlots compiler.
use pyo3::prelude::*;
use pyo3::types::PyString;
use vl_convert_rs::PgfplotsOpts;

use crate::utils::{
    parse_json_spec, prefixed_py_error, run_converter_future, run_converter_future_async,
};

/// Compile inline Vega-Lite data to editable PGFPlots plots and axes.
///
/// Unsupported features raise ValueError. Labels are escaped unless tex_labels
/// is enabled for trusted input. This function never invokes a TeX process.
#[pyfunction]
#[pyo3(signature = (vl_spec, *, standalone=false, tex_labels=false))]
pub fn vegalite_to_pgfplots(
    vl_spec: Py<PyAny>,
    standalone: bool,
    tex_labels: bool,
) -> PyResult<String> {
    let spec = parse_json_spec(vl_spec)?;
    let opts = PgfplotsOpts { standalone, tex_labels };
    run_converter_future(move |converter| async move {
        converter.vegalite_to_pgfplots(spec, opts).await.map(|o| o.tex)
    })
    .map_err(|err| prefixed_py_error("Native PGFPlots compilation failed", err))
}

/// Asynchronous variant of vegalite_to_pgfplots, using the existing worker pool.
#[pyfunction(name = "vegalite_to_pgfplots")]
#[pyo3(signature = (vl_spec, *, standalone=false, tex_labels=false))]
pub fn vegalite_to_pgfplots_asyncio<'py>(
    py: Python<'py>,
    vl_spec: Py<PyAny>,
    standalone: bool,
    tex_labels: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let spec = parse_json_spec(vl_spec)?;
    let opts = PgfplotsOpts { standalone, tex_labels };
    run_converter_future_async(
        py,
        move |converter| async move {
            converter.vegalite_to_pgfplots(spec, opts).await.map(|o| o.tex)
        },
        "Native PGFPlots compilation failed",
        |py, value| Ok(PyString::new(py, &value).into_any().unbind()),
    )
}

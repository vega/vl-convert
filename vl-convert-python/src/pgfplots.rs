//! Bindings for the shared native PGFPlots compiler.
use std::str::FromStr;

use pyo3::prelude::*;
use pyo3::types::PyString;
use vl_convert_rs::{PgfplotsOpts, VlVersion};

use crate::utils::{
    async_variant_doc, parse_spec_to_value_or_string, prefixed_py_error, run_converter_future,
    run_converter_future_async,
};

/// Compile static Vega-Lite dataflows to editable PGFPlots plots and axes.
///
/// Unsupported features raise ValueError. Labels are escaped unless tex_labels
/// is enabled for trusted input. This function never invokes a TeX process.
#[pyfunction]
#[pyo3(signature = (vl_spec, *, standalone=false, tex_labels=false, vl_version=None))]
pub fn vegalite_to_pgfplots(
    vl_spec: Py<PyAny>,
    standalone: bool,
    tex_labels: bool,
    vl_version: Option<&str>,
) -> PyResult<String> {
    let spec = parse_spec_to_value_or_string(vl_spec)?;
    let version = vl_version
        .map(VlVersion::from_str)
        .transpose()?
        .unwrap_or_default();
    let opts = PgfplotsOpts {
        standalone,
        tex_labels,
    };
    run_converter_future(move |converter| async move {
        converter
            .vegalite_to_pgfplots_with_version(spec, version, opts)
            .await
            .map(|o| o.tex)
    })
    .map_err(|err| prefixed_py_error("Native PGFPlots compilation failed", err))
}

#[doc = async_variant_doc!("vegalite_to_pgfplots")]
#[pyfunction(name = "vegalite_to_pgfplots")]
#[pyo3(signature = (vl_spec, *, standalone=false, tex_labels=false, vl_version=None))]
pub fn vegalite_to_pgfplots_asyncio<'py>(
    py: Python<'py>,
    vl_spec: Py<PyAny>,
    standalone: bool,
    tex_labels: bool,
    vl_version: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let spec = parse_spec_to_value_or_string(vl_spec)?;
    let version = vl_version
        .map(VlVersion::from_str)
        .transpose()?
        .unwrap_or_default();
    let opts = PgfplotsOpts {
        standalone,
        tex_labels,
    };
    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vegalite_to_pgfplots_with_version(spec, version, opts)
                .await
                .map(|o| o.tex)
        },
        "Native PGFPlots compilation failed",
        |py, value| Ok(PyString::new(py, &value).into_any().unbind()),
    )
}

//! Python bindings for the strict static PGFPlots exporter.
use std::str::FromStr;

use pyo3::prelude::*;
use vl_convert_rs::{PgfplotsOpts, VgOpts, VlOpts, VlVersion};

use crate::utils::{parse_json_spec, prefixed_py_error, run_converter_future};

/// Export the initial Vega-Lite chart state as PGFPlots source.
///
/// Text is outlined; unsupported graphics raise ValueError. This does not
/// preserve source data, interactions, or semantic PGFPlots axes.
#[pyfunction]
#[pyo3(signature = (vl_spec, *, vl_version=None, standalone=false))]
pub fn vegalite_to_pgfplots(
    vl_spec: Py<PyAny>,
    vl_version: Option<&str>,
    standalone: bool,
) -> PyResult<String> {
    let spec = parse_json_spec(vl_spec)?;
    let vl_version = vl_version.map(VlVersion::from_str).transpose()?.unwrap_or_default();
    run_converter_future(move |converter| async move {
        converter.vegalite_to_pgfplots(
            spec,
            VlOpts { vl_version, ..Default::default() },
            PgfplotsOpts { standalone },
        ).await.map(|out| out.tex)
    }).map_err(|err| prefixed_py_error("Vega-Lite to PGFPlots conversion failed", err))
}

/// Export the initial Vega chart state as PGFPlots source.
#[pyfunction]
#[pyo3(signature = (vg_spec, *, standalone=false))]
pub fn vega_to_pgfplots(vg_spec: Py<PyAny>, standalone: bool) -> PyResult<String> {
    let spec = parse_json_spec(vg_spec)?;
    run_converter_future(move |converter| async move {
        converter.vega_to_pgfplots(
            spec, VgOpts::default(), PgfplotsOpts { standalone },
        ).await.map(|out| out.tex)
    }).map_err(|err| prefixed_py_error("Vega to PGFPlots conversion failed", err))
}

/// Convert restricted SVG text to PGFPlots without evaluating Vega.
#[pyfunction]
#[pyo3(signature = (svg, *, standalone=false))]
pub fn svg_to_pgfplots(py: Python<'_>, svg: String, standalone: bool) -> PyResult<String> {
    py.detach(move || {
        vl_convert_rs::svg_to_pgfplots(&svg, PgfplotsOpts { standalone })
    }).map_err(|err| prefixed_py_error("SVG to PGFPlots conversion failed", err))
}

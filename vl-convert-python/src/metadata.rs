use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pythonize::pythonize;
use std::str::FromStr;
use vl_convert_rs::module_loader::import_map::{
    VlVersion, VEGA_EMBED_VERSION, VEGA_THEMES_VERSION, VEGA_VERSION, VL_VERSIONS,
};
use vl_convert_rs::module_loader::{FORMATE_LOCALE_MAP, TIME_FORMATE_LOCALE_MAP};

use crate::helpers::{
    async_variant_doc, converter_read_handle, future_into_py_object, parse_embedded_locale_json,
    prefixed_py_error, run_converter_future, run_converter_future_async,
};

/// Get the bundled version of Vega
///
/// Returns:
///     str: Vega version string (e.g. "5.30.0")
#[pyfunction]
#[pyo3(signature = ())]
pub fn get_vega_version() -> String {
    VEGA_VERSION.to_string()
}

/// Get the bundled version of Vega-Themes
///
/// Returns:
///     str: Vega-Themes version string (e.g. "2.14.0")
#[pyfunction]
#[pyo3(signature = ())]
pub fn get_vega_themes_version() -> String {
    VEGA_THEMES_VERSION.to_string()
}

/// Get the bundled version of Vega-Embed
///
/// Returns:
///     str: Vega-Embed version string (e.g. "6.26.0")
#[pyfunction]
#[pyo3(signature = ())]
pub fn get_vega_embed_version() -> String {
    VEGA_EMBED_VERSION.to_string()
}

/// Get the bundled versions of Vega-Lite
///
/// Returns:
///     list: Vega-Lite version strings (e.g. ["5.8", "5.9", ..., "5.21"])
#[pyfunction]
#[pyo3(signature = ())]
pub fn get_vegalite_versions() -> Vec<String> {
    VL_VERSIONS
        .iter()
        .map(|v| v.to_semver().to_string())
        .collect()
}

/// Eagerly start converter workers for the current converter configuration
#[pyfunction]
#[pyo3(signature = ())]
pub fn warm_up_workers() -> PyResult<()> {
    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("warm_up_workers request failed", err))?;

    Python::with_gil(|py| py.allow_threads(move || converter.warm_up()))
        .map_err(|err| prefixed_py_error("warm_up_workers request failed", err))?;
    Ok(())
}

/// Get the named local timezone that Vega uses to perform timezone calculations
///
/// Returns:
///     str: Named local timezone (e.g. "America/New_York"),
///          or None if the local timezone cannot be determined
#[pyfunction]
#[pyo3(signature = ())]
pub fn get_local_tz() -> PyResult<Option<String>> {
    run_converter_future(|converter| async move { converter.get_local_tz().await })
        .map_err(|err| prefixed_py_error("get_local_tz request failed", err))
}

/// Get V8 memory usage for each worker in the converter pool.
///
/// Returns:
///     list[dict]: List of dicts with keys ``worker_index``, ``used_heap_size``,
///         ``total_heap_size``, ``heap_size_limit``, and ``external_memory``
///         (all sizes in bytes).
#[pyfunction]
#[pyo3(signature = ())]
pub fn get_worker_memory_usage() -> PyResult<PyObject> {
    let stats =
        run_converter_future(|converter| async move { converter.get_worker_memory_usage().await })
            .map_err(|err| prefixed_py_error("get_worker_memory_usage request failed", err))?;

    Python::with_gil(|py| {
        let list = pyo3::types::PyList::empty(py);
        for s in &stats {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("worker_index", s.worker_index)?;
            dict.set_item("used_heap_size", s.used_heap_size)?;
            dict.set_item("total_heap_size", s.total_heap_size)?;
            dict.set_item("heap_size_limit", s.heap_size_limit)?;
            dict.set_item("external_memory", s.external_memory)?;
            list.append(dict)?;
        }
        Ok(list.into())
    })
}

/// Get the config dict for each built-in theme
///
/// Returns:
///     dict: dict from theme name to config object
#[pyfunction]
#[pyo3(signature = ())]
pub fn get_themes() -> PyResult<PyObject> {
    let themes = run_converter_future(|converter| async move { converter.get_themes().await })
        .map_err(|err| prefixed_py_error("get_themes request failed", err))?;
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
pub fn get_format_locale(name: &str) -> PyResult<PyObject> {
    match FORMATE_LOCALE_MAP.get(name) {
        None => {
            Err(PyValueError::new_err(format!(
                "Invalid format locale name: {name}\nSee https://github.com/d3/d3-format/tree/main/locale for available names"
            )))
        }
        Some(locale) => {
            let locale = parse_embedded_locale_json(locale, "format locale")?;
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
pub fn get_time_format_locale(name: &str) -> PyResult<PyObject> {
    match TIME_FORMATE_LOCALE_MAP.get(name) {
        None => {
            Err(PyValueError::new_err(format!(
                "Invalid time format locale name: {name}\nSee https://github.com/d3/d3-time-format/tree/main/locale for available names"
            )))
        }
        Some(locale) => {
            let locale = parse_embedded_locale_json(locale, "time format locale")?;
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
pub fn javascript_bundle(snippet: Option<String>, vl_version: Option<&str>) -> PyResult<String> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    if let Some(snippet) = snippet {
        run_converter_future(move |converter| async move {
            converter.bundle_vega_snippet(snippet, vl_version).await
        })
        .map_err(|err| prefixed_py_error("javascript_bundle request failed", err))
    } else {
        run_converter_future(move |converter| async move {
            converter.get_vegaembed_bundle(vl_version).await
        })
        .map_err(|err| prefixed_py_error("javascript_bundle request failed", err))
    }
}

#[doc = async_variant_doc!("warm_up_workers")]
#[pyfunction(name = "warm_up_workers")]
#[pyo3(signature = ())]
pub fn warm_up_workers_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("warm_up_workers request failed", err))?;

    future_into_py_object(py, async move {
        tokio::task::spawn_blocking(move || converter.warm_up())
            .await
            .map_err(|err| prefixed_py_error("warm_up_workers request failed", err))?
            .map_err(|err| prefixed_py_error("warm_up_workers request failed", err))?;
        Python::with_gil(|py| Ok(py.None().into()))
    })
}

#[doc = async_variant_doc!("get_local_tz")]
#[pyfunction(name = "get_local_tz")]
#[pyo3(signature = ())]
pub fn get_local_tz_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    run_converter_future_async(
        py,
        |converter| async move { converter.get_local_tz().await },
        "get_local_tz request failed",
        |py, value| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        },
    )
}

#[doc = async_variant_doc!("get_worker_memory_usage")]
#[pyfunction(name = "get_worker_memory_usage")]
#[pyo3(signature = ())]
pub fn get_worker_memory_usage_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    run_converter_future_async(
        py,
        |converter| async move { converter.get_worker_memory_usage().await },
        "get_worker_memory_usage request failed",
        |py, stats| {
            let list = pyo3::types::PyList::empty(py);
            for s in &stats {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("worker_index", s.worker_index)?;
                dict.set_item("used_heap_size", s.used_heap_size)?;
                dict.set_item("total_heap_size", s.total_heap_size)?;
                dict.set_item("heap_size_limit", s.heap_size_limit)?;
                dict.set_item("external_memory", s.external_memory)?;
                list.append(dict)?;
            }
            Ok(list.into())
        },
    )
}

#[doc = async_variant_doc!("get_themes")]
#[pyfunction(name = "get_themes")]
#[pyo3(signature = ())]
pub fn get_themes_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    run_converter_future_async(
        py,
        |converter| async move { converter.get_themes().await },
        "get_themes request failed",
        |py, value| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        },
    )
}

#[doc = async_variant_doc!("javascript_bundle")]
#[pyfunction(name = "javascript_bundle")]
#[pyo3(signature = (snippet=None, vl_version=None))]
pub fn javascript_bundle_asyncio<'py>(
    py: Python<'py>,
    snippet: Option<String>,
    vl_version: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    if let Some(snippet) = snippet {
        run_converter_future_async(
            py,
            move |converter| async move { converter.bundle_vega_snippet(snippet, vl_version).await },
            "javascript_bundle request failed",
            |py, value| {
                pythonize(py, &value)
                    .map_err(|err| PyValueError::new_err(err.to_string()))
                    .map(|obj| obj.into())
            },
        )
    } else {
        run_converter_future_async(
            py,
            move |converter| async move { converter.get_vegaembed_bundle(vl_version).await },
            "javascript_bundle request failed",
            |py, value| {
                pythonize(py, &value)
                    .map_err(|err| PyValueError::new_err(err.to_string()))
                    .map(|obj| obj.into())
            },
        )
    }
}

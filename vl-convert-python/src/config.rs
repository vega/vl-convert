use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::pythonize;
use vl_convert_rs::vlc_config_path;

use crate::helpers::{
    async_variant_doc, configure_converter_with_config_overrides, converter_config,
    converter_config_json, future_into_py_object, load_config_inner, parse_config_overrides,
    prefixed_py_error,
};

/// Configure converter options for subsequent requests
#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub fn configure(kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    let overrides = parse_config_overrides(kwargs)
        .map_err(|err| prefixed_py_error("Failed to configure converter", err))?;
    configure_converter_with_config_overrides(overrides)
        .map_err(|err| prefixed_py_error("Failed to configure converter", err))
}

/// Return the platform-standard path for the vl-convert JSONC config file.
#[pyfunction(name = "get_config_path")]
#[pyo3(signature = ())]
pub fn get_config_path() -> String {
    vlc_config_path().to_string_lossy().into_owned()
}

/// Load converter configuration from a JSONC file, replacing the active config.
///
/// Unlike ``configure()``, which patches individual fields, ``load_config()``
/// resets all settings to their defaults and then applies the file. Call
/// ``configure()`` after ``load_config()`` to override specific fields in code.
///
/// Args:
///     path (str | None): Path to the JSONC config file. When omitted, loads
///         from the standard location returned by ``get_config_path()``.
///         If that file does not exist, resets to built-in defaults.
///
/// Raises:
///     ValueError: If the path is provided but the file cannot be read or parsed.
#[pyfunction(name = "load_config")]
#[pyo3(signature = (path=None))]
pub fn load_config(path: Option<String>) -> PyResult<()> {
    load_config_inner(path).map_err(|err| prefixed_py_error("Failed to load config", err))
}

/// Get the currently configured converter options.
#[pyfunction(name = "get_config")]
#[pyo3(signature = ())]
pub fn get_config() -> PyResult<PyObject> {
    let config = converter_config()
        .map_err(|err| prefixed_py_error("Failed to read converter config", err))?;
    Python::with_gil(|py| {
        pythonize(py, &converter_config_json(&config))
            .map_err(|err| PyValueError::new_err(err.to_string()))
            .map(|obj| obj.into())
    })
}

#[doc = async_variant_doc!("configure")]
#[pyfunction(name = "configure")]
#[pyo3(signature = (**kwargs))]
pub fn configure_asyncio<'py>(
    py: Python<'py>,
    kwargs: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let overrides = parse_config_overrides(kwargs)
        .map_err(|err| prefixed_py_error("Failed to configure converter", err))?;
    future_into_py_object(py, async move {
        tokio::task::spawn_blocking(move || configure_converter_with_config_overrides(overrides))
            .await
            .map_err(|err| prefixed_py_error("Failed to configure converter", err))?
            .map_err(|err| prefixed_py_error("Failed to configure converter", err))?;
        Python::with_gil(|py| Ok(py.None().into()))
    })
}

#[doc = async_variant_doc!("load_config")]
#[pyfunction(name = "load_config")]
#[pyo3(signature = (path=None))]
pub fn load_config_asyncio<'py>(
    py: Python<'py>,
    path: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    future_into_py_object(py, async move {
        tokio::task::spawn_blocking(move || load_config_inner(path))
            .await
            .map_err(|err| prefixed_py_error("Failed to load config", err))?
            .map_err(|err| prefixed_py_error("Failed to load config", err))?;
        Python::with_gil(|py| Ok(py.None().into()))
    })
}

#[doc = async_variant_doc!("get_config")]
#[pyfunction(name = "get_config")]
#[pyo3(signature = ())]
pub fn get_config_asyncio<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    future_into_py_object(py, async move {
        let config = tokio::task::spawn_blocking(converter_config)
            .await
            .map_err(|err| prefixed_py_error("Failed to read converter config", err))?
            .map_err(|err| prefixed_py_error("Failed to read converter config", err))?;
        Python::with_gil(|py| {
            pythonize(py, &converter_config_json(&config))
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

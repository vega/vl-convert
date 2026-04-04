use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::depythonize;
use std::borrow::Cow;
use std::future::Future;
use std::sync::Arc;
use vl_convert_rs::converter::{
    FormatLocale, TimeFormatLocale, ValueOrString, ACCESS_DENIED_MARKER,
};
use vl_convert_rs::serde_json;
use vl_convert_rs::VlConverter as VlConverterRs;

use crate::config::converter_read_handle;
use crate::PYTHON_RUNTIME;

pub fn run_converter_future<R, Fut, F>(make_future: F) -> Result<R, vl_convert_rs::anyhow::Error>
where
    F: FnOnce(Arc<VlConverterRs>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<R, vl_convert_rs::anyhow::Error>> + 'static,
    R: Send + 'static,
{
    let converter = converter_read_handle()?;
    Python::with_gil(|py| py.allow_threads(move || PYTHON_RUNTIME.block_on(make_future(converter))))
}

pub fn is_permission_denied_message(message: &str) -> bool {
    if message.contains(ACCESS_DENIED_MARKER) {
        return true;
    }

    let lowercase = message.to_ascii_lowercase();
    lowercase.contains("permission denied")
        || lowercase.contains("access denied")
        || lowercase.contains("requires read access")
        || lowercase.contains("requires net access")
}

pub fn prefixed_py_error(prefix: &'static str, err: impl std::fmt::Display) -> PyErr {
    let message = format!("{prefix}:\n{err}");
    if is_permission_denied_message(&message) {
        pyo3::exceptions::PyPermissionError::new_err(message)
    } else {
        PyValueError::new_err(message)
    }
}

pub fn future_into_py_object<'py, Fut>(py: Python<'py>, fut: Fut) -> PyResult<Bound<'py, PyAny>>
where
    Fut: Future<Output = PyResult<PyObject>> + Send + 'static,
{
    pyo3_async_runtimes::tokio::future_into_py::<_, PyObject>(py, fut)
}

pub fn run_converter_future_async<'py, R, Fut, F, C>(
    py: Python<'py>,
    make_future: F,
    error_prefix: &'static str,
    convert: C,
) -> PyResult<Bound<'py, PyAny>>
where
    F: FnOnce(Arc<VlConverterRs>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<R, vl_convert_rs::anyhow::Error>> + Send + 'static,
    R: Send + 'static,
    C: FnOnce(Python<'_>, R) -> PyResult<PyObject> + Send + 'static,
{
    let converter = converter_read_handle().map_err(|err| prefixed_py_error(error_prefix, err))?;

    future_into_py_object(py, async move {
        let value = make_future(converter)
            .await
            .map_err(|err| prefixed_py_error(error_prefix, err))?;
        Python::with_gil(|py| convert(py, value))
    })
}

macro_rules! async_variant_doc {
    ($name:literal) => {
        concat!(
            "Async awaitable variant of `vl_convert.",
            $name,
            "`.\n\nSee `vl_convert.",
            $name,
            "` for full arguments and behavior."
        )
    };
}
pub(crate) use async_variant_doc;

pub fn parse_json_spec(vl_spec: PyObject) -> PyResult<serde_json::Value> {
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

pub fn parse_optional_config(config: Option<PyObject>) -> PyResult<Option<serde_json::Value>> {
    config.map(parse_json_spec).transpose()
}

pub fn handle_show_warnings(show_warnings: Option<bool>) {
    if show_warnings == Some(true) {
        Python::with_gil(|py| {
            let _ = py.run(
                c"import warnings; warnings.warn(\
                  'show_warnings is deprecated. Warnings are now always forwarded via '\
                  'Python\\'s logging module. Configure with: import logging; '\
                  'logging.getLogger(\"vl_convert\").setLevel(logging.WARNING)', DeprecationWarning, stacklevel=2)",
                None,
                None,
            );
        });
    }
}

pub fn parse_spec_to_value_or_string(vl_spec: PyObject) -> PyResult<ValueOrString> {
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

pub fn parse_format_locale(v: PyObject) -> PyResult<FormatLocale> {
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

pub fn parse_option_format_locale(v: Option<PyObject>) -> PyResult<Option<FormatLocale>> {
    match v {
        None => Ok(None),
        Some(v) => Ok(Some(parse_format_locale(v)?)),
    }
}

pub fn parse_time_format_locale(v: PyObject) -> PyResult<TimeFormatLocale> {
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

pub fn parse_option_time_format_locale(v: Option<PyObject>) -> PyResult<Option<TimeFormatLocale>> {
    match v {
        None => Ok(None),
        Some(v) => Ok(Some(parse_time_format_locale(v)?)),
    }
}

pub fn parse_embedded_locale_json(raw: &str, kind: &str) -> PyResult<serde_json::Value> {
    serde_json::from_str(raw).map_err(|err| {
        PyValueError::new_err(format!("Failed to parse internal {kind} as JSON: {err}"))
    })
}

pub fn warn_if_scale_not_one_for_pdf(scale: Option<f32>) -> PyResult<()> {
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

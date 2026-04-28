use crate::config::converter_read_handle;
use crate::fonts::parse_google_fonts_arg;
use crate::utils::{
    async_variant_doc, future_into_py_object, handle_show_warnings, parse_json_spec,
    parse_option_format_locale, parse_option_time_format_locale, parse_optional_config,
    parse_spec_to_value_or_string, prefixed_py_error, run_converter_future,
    run_converter_future_async, warn_if_scale_not_one_for_pdf,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pythonize::pythonize;
use std::str::FromStr;
use vl_convert_rs::converter::{
    HtmlOpts, JpegOpts, PdfOpts, PngOpts, Renderer, SvgOpts, UrlOpts, VgOpts, VlOpts,
};
use vl_convert_rs::module_loader::import_map::VlVersion;

/// Convert a Vega-Lite spec to a Vega spec using a particular
/// version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str | None): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     show_warnings (bool | None): Deprecated. Warnings are now forwarded
///         via Python's logging module. Use ``import logging;
///         logging.getLogger("vl_convert").setLevel(logging.WARNING)`` to see them.
/// Returns:
///     dict: Vega JSON specification dict
#[pyfunction]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    config=None,
    theme=None,
    show_warnings=None,
))]
pub fn vegalite_to_vega(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
) -> PyResult<PyObject> {
    handle_show_warnings(show_warnings);
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;

    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale: None,
        time_format_locale: None,
        google_fonts: None,
        ..Default::default()
    };

    let vega_spec = match run_converter_future(move |converter| async move {
        converter
            .vegalite_to_vega(vl_spec, vl_opts)
            .await
            .map(|o| o.spec)
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(prefixed_py_error(
                "Vega-Lite to Vega conversion failed",
                err,
            ))
        }
    };
    Python::with_gil(|py| -> PyResult<PyObject> {
        pythonize(py, &vega_spec)
            .map_err(|err| PyValueError::new_err(err.to_string()))
            .map(|obj| obj.into())
    })
}

/// Convert a Vega spec to an SVG image string
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     vega_plugin (str): Per-request Vega plugin (inline ESM string or URL)
///     bundle (bool): If True, embed fonts and images as self-contained data URIs
///     google_fonts (list): Google Fonts for this conversion
///     config (str | dict): Vega config object merged via vega.mergeConfig()
///     background (str): Override the spec's background color
///     width (float): Override the spec's width
///     height (float): Override the spec's height
/// Returns:
///     str: SVG image string
#[pyfunction]
#[pyo3(signature = (
    vg_spec,
    *,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    bundle=None,
    google_fonts=None,
    config=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vega_to_svg(
    vg_spec: PyObject,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    bundle: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<String> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let config = parse_optional_config(config)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;

    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        config,
        background,
        width,
        height,
    };

    let svg_opts = SvgOpts {
        bundle: bundle.unwrap_or(false),
    };

    let svg = match run_converter_future(move |converter| async move {
        converter
            .vega_to_svg(vg_spec, vg_opts, svg_opts)
            .await
            .map(|o| o.svg)
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega to SVG conversion failed", err)),
    };
    Ok(svg)
}

/// Convert a Vega spec to a Vega Scenegraph
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     format (str): Output format, either "dict" (default) or "msgpack"
///     vega_plugin (str): Per-request Vega plugin (inline ESM string or URL)
///     google_fonts (list): Google Fonts for this conversion
///     config (str | dict): Vega config object merged via vega.mergeConfig()
///     background (str): Override the spec's background color
///     width (float): Override the spec's width
///     height (float): Override the spec's height
/// Returns:
///     dict | bytes: scenegraph as dict (format="dict") or msgpack bytes (format="msgpack")
#[pyfunction]
#[pyo3(signature = (
    vg_spec,
    *,
    format_locale=None,
    time_format_locale=None,
    format="dict",
    vega_plugin=None,
    google_fonts=None,
    config=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vega_to_scenegraph(
    vg_spec: PyObject,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    format: &str,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<PyObject> {
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let config = parse_optional_config(config)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;

    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        config,
        background,
        width,
        height,
    };

    match format {
        "dict" => {
            let vg_spec = parse_json_spec(vg_spec)?;
            let sg = run_converter_future(move |converter| async move {
                converter
                    .vega_to_scenegraph(vg_spec, vg_opts)
                    .await
                    .map(|o| o.scenegraph)
            })
            .map_err(|err| prefixed_py_error("Vega to Scenegraph conversion failed", err))?;
            Python::with_gil(|py| -> PyResult<PyObject> {
                pythonize(py, &sg)
                    .map_err(|err| PyValueError::new_err(err.to_string()))
                    .map(|obj| obj.into())
            })
        }
        "msgpack" => {
            let vg_spec = parse_spec_to_value_or_string(vg_spec)?;
            let sg_bytes = run_converter_future(move |converter| async move {
                converter
                    .vega_to_scenegraph_msgpack(vg_spec, vg_opts)
                    .await
                    .map(|o| o.data)
            })
            .map_err(|err| prefixed_py_error("Vega to Scenegraph conversion failed", err))?;
            Ok(Python::with_gil(|py| -> PyObject {
                PyBytes::new(py, sg_bytes.as_slice()).into()
            }))
        }
        _ => Err(PyValueError::new_err(format!(
            "Invalid format '{format}'. Expected 'dict' or 'msgpack'"
        ))),
    }
}

/// Convert a Vega-Lite spec to an SVG image string using a
/// particular version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str | None): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     show_warnings (bool | None): Deprecated. Warnings are now forwarded
///         via Python's logging module. Use ``import logging;
///         logging.getLogger("vl_convert").setLevel(logging.WARNING)`` to see them.
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     vega_plugin (str): Per-request Vega plugin (inline ESM string or URL)
///     bundle (bool): If True, embed fonts and images as self-contained data URIs
///     google_fonts (list): Google Fonts for this conversion
///     background (str): Override the spec's background color
///     width (float): Override the spec's width
///     height (float): Override the spec's height
/// Returns:
///     str: SVG image string
#[pyfunction]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    config=None,
    theme=None,
    show_warnings=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    bundle=None,
    google_fonts=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vegalite_to_svg(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    bundle: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<String> {
    handle_show_warnings(show_warnings);
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;

    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        background,
        width,
        height,
    };

    let svg_opts = SvgOpts {
        bundle: bundle.unwrap_or(false),
    };

    let svg = match run_converter_future(move |converter| async move {
        converter
            .vegalite_to_svg(vl_spec, vl_opts, svg_opts)
            .await
            .map(|o| o.svg)
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega-Lite to SVG conversion failed", err)),
    };
    Ok(svg)
}

/// Convert a Vega-Lite spec to a Vega Scenegraph using a
/// particular version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str | None): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     show_warnings (bool | None): Deprecated. Warnings are now forwarded
///         via Python's logging module. Use ``import logging;
///         logging.getLogger("vl_convert").setLevel(logging.WARNING)`` to see them.
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     format (str): Output format, either "dict" (default) or "msgpack"
///     vega_plugin (str): Per-request Vega plugin (inline ESM string or URL)
///     google_fonts (list): Google Fonts for this conversion
///     background (str): Override the spec's background color
///     width (float): Override the spec's width
///     height (float): Override the spec's height
/// Returns:
///     dict | bytes: scenegraph as dict (format="dict") or msgpack bytes (format="msgpack")
#[pyfunction]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    config=None,
    theme=None,
    show_warnings=None,
    format_locale=None,
    time_format_locale=None,
    format="dict",
    vega_plugin=None,
    google_fonts=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vegalite_to_scenegraph(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    format: &str,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<PyObject> {
    handle_show_warnings(show_warnings);
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;

    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        background,
        width,
        height,
    };

    match format {
        "dict" => {
            let vl_spec = parse_json_spec(vl_spec)?;
            let sg = run_converter_future(move |converter| async move {
                converter
                    .vegalite_to_scenegraph(vl_spec, vl_opts)
                    .await
                    .map(|o| o.scenegraph)
            })
            .map_err(|err| prefixed_py_error("Vega-Lite to Scenegraph conversion failed", err))?;
            Python::with_gil(|py| -> PyResult<PyObject> {
                pythonize(py, &sg)
                    .map_err(|err| PyValueError::new_err(err.to_string()))
                    .map(|obj| obj.into())
            })
        }
        "msgpack" => {
            let vl_spec = parse_spec_to_value_or_string(vl_spec)?;
            let sg_bytes = run_converter_future(move |converter| async move {
                converter
                    .vegalite_to_scenegraph_msgpack(vl_spec, vl_opts)
                    .await
                    .map(|o| o.data)
            })
            .map_err(|err| prefixed_py_error("Vega-Lite to Scenegraph conversion failed", err))?;
            Ok(Python::with_gil(|py| -> PyObject {
                PyBytes::new(py, sg_bytes.as_slice()).into()
            }))
        }
        _ => Err(PyValueError::new_err(format!(
            "Invalid format '{format}'. Expected 'dict' or 'msgpack'"
        ))),
    }
}

/// Convert a Vega spec to PNG image data.
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     scale (float): Image scale factor (default 1.0)
///     ppi (float): Pixels per inch (default 72)
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     vega_plugin (str): Per-request Vega plugin (inline ESM string or URL)
///     google_fonts (list): Google Fonts for this conversion
///     config (str | dict): Vega config object merged via vega.mergeConfig()
///     background (str): Override the spec's background color
///     width (float): Override the spec's width
///     height (float): Override the spec's height
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(signature = (
    vg_spec,
    *,
    scale=None,
    ppi=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    google_fonts=None,
    config=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vega_to_png(
    vg_spec: PyObject,
    scale: Option<f32>,
    ppi: Option<f32>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<PyObject> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let config = parse_optional_config(config)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;

    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        config,
        background,
        width,
        height,
    };

    let png_opts = PngOpts { scale, ppi };
    let png_data = match run_converter_future(move |converter| async move {
        converter
            .vega_to_png(vg_spec, vg_opts, png_opts)
            .await
            .map(|o| o.data)
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega to PNG conversion failed", err)),
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, png_data.as_slice()).into()
    }))
}

/// Convert a Vega-Lite spec to PNG image data using a particular
/// version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     scale (float): Image scale factor (default 1.0)
///     ppi (float): Pixels per inch (default 72)
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     show_warnings (bool | None): Deprecated. Warnings are now forwarded
///         via Python's logging module. Use ``import logging;
///         logging.getLogger("vl_convert").setLevel(logging.WARNING)`` to see them.
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     vega_plugin (str): Per-request Vega plugin (inline ESM string or URL)
///     google_fonts (list): Google Fonts for this conversion
///     background (str): Override the spec's background color
///     width (float): Override the spec's width
///     height (float): Override the spec's height
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    scale=None,
    ppi=None,
    config=None,
    theme=None,
    show_warnings=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    google_fonts=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vegalite_to_png(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    ppi: Option<f32>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<PyObject> {
    handle_show_warnings(show_warnings);
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        background,
        width,
        height,
    };

    let png_opts = PngOpts { scale, ppi };
    let png_data = match run_converter_future(move |converter| async move {
        converter
            .vegalite_to_png(vl_spec, vl_opts, png_opts)
            .await
            .map(|o| o.data)
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega-Lite to PNG conversion failed", err)),
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, png_data.as_slice()).into()
    }))
}

/// Convert a Vega spec to JPEG image data.
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     scale (float): Image scale factor (default 1.0)
///     quality (int): JPEG Quality between 0 (worst) and 100 (best). Default 90
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     vega_plugin (str): Per-request Vega plugin (inline ESM string or URL)
///     google_fonts (list): Google Fonts for this conversion
///     config (str | dict): Vega config object merged via vega.mergeConfig()
///     background (str): Override the spec's background color
///     width (float): Override the spec's width
///     height (float): Override the spec's height
/// Returns:
///     bytes: JPEG image data
#[pyfunction]
#[pyo3(signature = (
    vg_spec,
    *,
    scale=None,
    quality=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    google_fonts=None,
    config=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vega_to_jpeg(
    vg_spec: PyObject,
    scale: Option<f32>,
    quality: Option<u8>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<PyObject> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let config = parse_optional_config(config)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;

    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        config,
        background,
        width,
        height,
    };

    let jpeg_opts = JpegOpts { scale, quality };
    let jpeg_data = match run_converter_future(move |converter| async move {
        converter
            .vega_to_jpeg(vg_spec, vg_opts, jpeg_opts)
            .await
            .map(|o| o.data)
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega to JPEG conversion failed", err)),
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, jpeg_data.as_slice()).into()
    }))
}

/// Convert a Vega-Lite spec to JPEG image data using a particular
/// version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     scale (float): Image scale factor (default 1.0)
///     quality (int): JPEG Quality between 0 (worst) and 100 (best). Default 90
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     show_warnings (bool | None): Deprecated. Warnings are now forwarded
///         via Python's logging module. Use ``import logging;
///         logging.getLogger("vl_convert").setLevel(logging.WARNING)`` to see them.
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     vega_plugin (str): Per-request Vega plugin (inline ESM string or URL)
///     google_fonts (list): Google Fonts for this conversion
///     background (str): Override the spec's background color
///     width (float): Override the spec's width
///     height (float): Override the spec's height
/// Returns:
///     bytes: JPEG image data
#[pyfunction]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    scale=None,
    quality=None,
    config=None,
    theme=None,
    show_warnings=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    google_fonts=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vegalite_to_jpeg(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    quality: Option<u8>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<PyObject> {
    handle_show_warnings(show_warnings);
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        background,
        width,
        height,
    };

    let jpeg_opts = JpegOpts { scale, quality };
    let jpeg_data = match run_converter_future(move |converter| async move {
        converter
            .vegalite_to_jpeg(vl_spec, vl_opts, jpeg_opts)
            .await
            .map(|o| o.data)
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => {
            return Err(prefixed_py_error(
                "Vega-Lite to JPEG conversion failed",
                err,
            ))
        }
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, jpeg_data.as_slice()).into()
    }))
}

/// Convert a Vega spec to PDF format
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     scale (float): Image scale factor (default 1.0)
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     vega_plugin (str): Per-request Vega plugin (inline ESM string or URL)
///     google_fonts (list): Google Fonts for this conversion
///     config (str | dict): Vega config object merged via vega.mergeConfig()
///     background (str): Override the spec's background color
///     width (float): Override the spec's width
///     height (float): Override the spec's height
/// Returns:
///     bytes: PDF file bytes
#[pyfunction]
#[pyo3(signature = (
    vg_spec,
    *,
    scale=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    google_fonts=None,
    config=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vega_to_pdf(
    vg_spec: PyObject,
    scale: Option<f32>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<PyObject> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let config = parse_optional_config(config)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;

    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        config,
        background,
        width,
        height,
    };

    let pdf_bytes = match run_converter_future(move |converter| async move {
        converter
            .vega_to_pdf(vg_spec, vg_opts, PdfOpts::default())
            .await
            .map(|o| o.data)
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega to PDF conversion failed", err)),
    };
    Ok(Python::with_gil(|py| -> PyObject {
        PyObject::from(PyBytes::new(py, pdf_bytes.as_slice()))
    }))
}

/// Convert a Vega-Lite spec to PDF image data using a particular
/// version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     scale (float): Image scale factor (default 1.0)
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     vega_plugin (str): Per-request Vega plugin (inline ESM string or URL)
///     google_fonts (list): Google Fonts for this conversion
///     background (str): Override the spec's background color
///     width (float): Override the spec's width
///     height (float): Override the spec's height
/// Returns:
///     bytes: PDF image data
#[pyfunction]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    scale=None,
    config=None,
    theme=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    google_fonts=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vegalite_to_pdf(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    config: Option<PyObject>,
    theme: Option<String>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<PyObject> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;

    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        background,
        width,
        height,
    };

    let pdf_data = match run_converter_future(move |converter| async move {
        converter
            .vegalite_to_pdf(vl_spec, vl_opts, PdfOpts::default())
            .await
            .map(|o| o.data)
    }) {
        Ok(vega_spec) => vega_spec,
        Err(err) => return Err(prefixed_py_error("Vega-Lite to PDF conversion failed", err)),
    };

    Ok(Python::with_gil(|py| -> PyObject {
        PyObject::from(PyBytes::new(py, pdf_data.as_slice()))
    }))
}

/// Convert a Vega-Lite spec to a URL that opens the chart in the Vega editor
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     fullscreen (bool): Whether to open the chart in full screen in the editor
/// Returns:
///     str: URL string
#[pyfunction]
#[pyo3(signature = (vl_spec, *, fullscreen=None))]
pub fn vegalite_to_url(vl_spec: PyObject, fullscreen: Option<bool>) -> PyResult<String> {
    let vl_spec = parse_json_spec(vl_spec)?;
    Ok(vl_convert_rs::converter::vegalite_to_url(
        &vl_spec,
        UrlOpts {
            fullscreen: fullscreen.unwrap_or(false),
        },
    )?)
}

/// Convert a Vega spec to a URL that opens the chart in the Vega editor
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     fullscreen (bool): Whether to open the chart in full screen in the editor
/// Returns:
///     str: URL string
#[pyfunction]
#[pyo3(signature = (vg_spec, *, fullscreen=None))]
pub fn vega_to_url(vg_spec: PyObject, fullscreen: Option<bool>) -> PyResult<String> {
    let vg_spec = parse_json_spec(vg_spec)?;
    Ok(vl_convert_rs::converter::vega_to_url(
        &vg_spec,
        UrlOpts {
            fullscreen: fullscreen.unwrap_or(false),
        },
    )?)
}

/// Convert a Vega-Lite spec to self-contained HTML document using a particular
/// version of the Vega-Lite JavaScript library.
///
/// Args:
///     vl_spec (str | dict): Vega-Lite JSON specification string or dict
///     vl_version (str): Vega-Lite library version string (e.g. 'v5.15')
///         (default to latest)
///     bundle (bool): If True, bundle all dependencies in HTML file.
///         If False (default), HTML file will load dependencies from only CDN
///     google_fonts (list): Google Fonts for this conversion
///     config (dict | None): Chart configuration object to apply during conversion
///     theme (str | None): Named theme (e.g. "dark") to apply during conversion
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     renderer (str): Vega renderer. One of 'svg' (default), 'canvas',
///         or 'hybrid' (where text is svg and other marks are canvas)
///     vega_plugin (str): Per-request Vega plugin (inline ESM string or URL)
///     background (str): Override the spec's background color
///     width (float): Override the spec's width
///     height (float): Override the spec's height
/// Returns:
///     string: HTML document
#[pyfunction]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    bundle=None,
    google_fonts=None,
    config=None,
    theme=None,
    format_locale=None,
    time_format_locale=None,
    renderer=None,
    vega_plugin=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vegalite_to_html(
    vl_spec: PyObject,
    vl_version: Option<&str>,
    bundle: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    theme: Option<String>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    renderer: Option<String>,
    vega_plugin: Option<String>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<String> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let renderer = renderer.unwrap_or_else(|| "svg".to_string());
    let renderer = Renderer::from_str(&renderer)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        background,
        width,
        height,
    };

    let html_opts = HtmlOpts {
        bundle: bundle.unwrap_or(false),
        renderer,
    };
    run_converter_future(move |converter| async move {
        converter
            .vegalite_to_html(vl_spec, vl_opts, html_opts)
            .await
            .map(|o| o.html)
    })
    .map_err(|err| prefixed_py_error("Vega-Lite to HTML conversion failed", err))
}

/// Convert a Vega spec to a self-contained HTML document
///
/// Args:
///     vg_spec (str | dict): Vega JSON specification string or dict
///     bundle (bool): If True, bundle all dependencies in HTML file.
///         If False (default), HTML file will load dependencies from only CDN
///     google_fonts (list): Google Fonts for this conversion
///     format_locale (str | dict): d3-format locale name or dictionary
///     time_format_locale (str | dict): d3-time-format locale name or dictionary
///     renderer (str): Vega renderer. One of 'svg' (default), 'canvas',
///         or 'hybrid' (where text is svg and other marks are canvas)
///     vega_plugin (str): Per-request Vega plugin (inline ESM string or URL)
///     config (str | dict): Vega config object merged via vega.mergeConfig()
///     background (str): Override the spec's background color
///     width (float): Override the spec's width
///     height (float): Override the spec's height
/// Returns:
///     string: HTML document
#[pyfunction]
#[pyo3(signature = (
    vg_spec,
    *,
    bundle=None,
    google_fonts=None,
    format_locale=None,
    time_format_locale=None,
    renderer=None,
    vega_plugin=None,
    config=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vega_to_html(
    vg_spec: PyObject,
    bundle: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    renderer: Option<String>,
    vega_plugin: Option<String>,
    config: Option<PyObject>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<String> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let renderer = renderer.unwrap_or_else(|| "svg".to_string());
    let renderer = Renderer::from_str(&renderer)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let config = parse_optional_config(config)?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        config,
        background,
        width,
        height,
    };
    let html_opts = HtmlOpts {
        bundle: bundle.unwrap_or(false),
        renderer,
    };
    run_converter_future(move |converter| async move {
        converter
            .vega_to_html(vg_spec, vg_opts, html_opts)
            .await
            .map(|o| o.html)
    })
    .map_err(|err| prefixed_py_error("Vega to HTML conversion failed", err))
}

/// Convert an SVG image string to PNG image data
///
/// Args:
///     svg (str): SVG image string
///     scale (float): Image scale factor (default 1.0)
///     ppi (float): Pixels per inch (default 72)
/// Returns:
///     bytes: PNG image data
#[pyfunction]
#[pyo3(signature = (svg, *, scale=None, ppi=None))]
pub fn svg_to_png(svg: &str, scale: Option<f32>, ppi: Option<f32>) -> PyResult<PyObject> {
    let svg = svg.to_string();
    let png_opts = PngOpts { scale, ppi };
    let png_data = run_converter_future(move |converter| async move {
        converter.svg_to_png(&svg, png_opts).await.map(|o| o.data)
    })
    .map_err(|err| prefixed_py_error("SVG to PNG conversion failed", err))?;
    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, png_data.as_slice()).into()
    }))
}

/// Convert an SVG image string to JPEG image data
///
/// Args:
///     svg (str): SVG image string
///     scale (float): Image scale factor (default 1.0)
///     quality (int): JPEG Quality between 0 (worst) and 100 (best). Default 90
/// Returns:
///     bytes: JPEG image data
#[pyfunction]
#[pyo3(signature = (svg, *, scale=None, quality=None))]
pub fn svg_to_jpeg(svg: &str, scale: Option<f32>, quality: Option<u8>) -> PyResult<PyObject> {
    let svg = svg.to_string();
    let jpeg_opts = JpegOpts { scale, quality };
    let jpeg_data = run_converter_future(move |converter| async move {
        converter.svg_to_jpeg(&svg, jpeg_opts).await.map(|o| o.data)
    })
    .map_err(|err| prefixed_py_error("SVG to JPEG conversion failed", err))?;
    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, jpeg_data.as_slice()).into()
    }))
}

/// Convert an SVG image string to PDF document data
///
/// Args:
///     svg (str): SVG image string
///     scale (float): Image scale factor (default 1.0)
/// Returns:
///     bytes: PDF document data
#[pyfunction]
#[pyo3(signature = (svg, *, scale=None))]
pub fn svg_to_pdf(svg: &str, scale: Option<f32>) -> PyResult<PyObject> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let svg = svg.to_string();
    let pdf_data = run_converter_future(move |converter| async move {
        converter
            .svg_to_pdf(&svg, PdfOpts::default())
            .await
            .map(|o| o.data)
    })
    .map_err(|err| prefixed_py_error("SVG to PDF conversion failed", err))?;
    Ok(Python::with_gil(|py| -> PyObject {
        PyBytes::new(py, pdf_data.as_slice()).into()
    }))
}

#[doc = async_variant_doc!("vegalite_to_vega")]
#[pyfunction(name = "vegalite_to_vega")]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    config=None,
    theme=None,
    show_warnings=None,
))]
pub fn vegalite_to_vega_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
) -> PyResult<Bound<'py, PyAny>> {
    handle_show_warnings(show_warnings);
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale: None,
        time_format_locale: None,
        google_fonts: None,
        ..Default::default()
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vegalite_to_vega(vl_spec, vl_opts)
                .await
                .map(|o| o.spec)
        },
        "Vega-Lite to Vega conversion failed",
        |py, value| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        },
    )
}

#[doc = async_variant_doc!("vega_to_svg")]
#[pyfunction(name = "vega_to_svg")]
#[pyo3(signature = (
    vg_spec,
    *,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    bundle=None,
    google_fonts=None,
    config=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vega_to_svg_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    bundle: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let config = parse_optional_config(config)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        config,
        background,
        width,
        height,
    };
    let svg_opts = SvgOpts {
        bundle: bundle.unwrap_or(false),
    };

    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("Vega to SVG conversion failed", err))?;

    let error_prefix = "Vega to SVG conversion failed";
    future_into_py_object(py, async move {
        let value = converter
            .vega_to_svg(vg_spec, vg_opts, svg_opts)
            .await
            .map(|o| o.svg)
            .map_err(|err| prefixed_py_error(error_prefix, err))?;
        Python::with_gil(|py| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("vega_to_scenegraph")]
#[pyfunction(name = "vega_to_scenegraph")]
#[pyo3(signature = (
    vg_spec,
    *,
    format_locale=None,
    time_format_locale=None,
    format="dict",
    vega_plugin=None,
    google_fonts=None,
    config=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vega_to_scenegraph_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    format: &str,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let config = parse_optional_config(config)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        config,
        background,
        width,
        height,
    };

    match format {
        "dict" => {
            let vg_spec = parse_json_spec(vg_spec)?;
            run_converter_future_async(
                py,
                move |converter| async move {
                    converter
                        .vega_to_scenegraph(vg_spec, vg_opts)
                        .await
                        .map(|o| o.scenegraph)
                },
                "Vega to Scenegraph conversion failed",
                |py, value| {
                    pythonize(py, &value)
                        .map_err(|err| PyValueError::new_err(err.to_string()))
                        .map(|obj| obj.into())
                },
            )
        }
        "msgpack" => {
            let vg_spec = parse_spec_to_value_or_string(vg_spec)?;
            run_converter_future_async(
                py,
                move |converter| async move {
                    converter
                        .vega_to_scenegraph_msgpack(vg_spec, vg_opts)
                        .await
                        .map(|o| o.data)
                },
                "Vega to Scenegraph conversion failed",
                |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
            )
        }
        _ => Err(PyValueError::new_err(format!(
            "Invalid format '{format}'. Expected 'dict' or 'msgpack'"
        ))),
    }
}

#[doc = async_variant_doc!("vegalite_to_svg")]
#[pyfunction(name = "vegalite_to_svg")]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    config=None,
    theme=None,
    show_warnings=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    bundle=None,
    google_fonts=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vegalite_to_svg_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    bundle: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    handle_show_warnings(show_warnings);
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        background,
        width,
        height,
    };
    let svg_opts = SvgOpts {
        bundle: bundle.unwrap_or(false),
    };

    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("Vega-Lite to SVG conversion failed", err))?;

    let error_prefix = "Vega-Lite to SVG conversion failed";
    future_into_py_object(py, async move {
        let value = converter
            .vegalite_to_svg(vl_spec, vl_opts, svg_opts)
            .await
            .map(|o| o.svg)
            .map_err(|err| prefixed_py_error(error_prefix, err))?;
        Python::with_gil(|py| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("vegalite_to_scenegraph")]
#[pyfunction(name = "vegalite_to_scenegraph")]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    config=None,
    theme=None,
    show_warnings=None,
    format_locale=None,
    time_format_locale=None,
    format="dict",
    vega_plugin=None,
    google_fonts=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vegalite_to_scenegraph_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    format: &str,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    handle_show_warnings(show_warnings);
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        background,
        width,
        height,
    };

    match format {
        "dict" => {
            let vl_spec = parse_json_spec(vl_spec)?;
            run_converter_future_async(
                py,
                move |converter| async move {
                    converter
                        .vegalite_to_scenegraph(vl_spec, vl_opts)
                        .await
                        .map(|o| o.scenegraph)
                },
                "Vega-Lite to Scenegraph conversion failed",
                |py, value| {
                    pythonize(py, &value)
                        .map_err(|err| PyValueError::new_err(err.to_string()))
                        .map(|obj| obj.into())
                },
            )
        }
        "msgpack" => {
            let vl_spec = parse_spec_to_value_or_string(vl_spec)?;
            run_converter_future_async(
                py,
                move |converter| async move {
                    converter
                        .vegalite_to_scenegraph_msgpack(vl_spec, vl_opts)
                        .await
                        .map(|o| o.data)
                },
                "Vega-Lite to Scenegraph conversion failed",
                |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
            )
        }
        _ => Err(PyValueError::new_err(format!(
            "Invalid format '{format}'. Expected 'dict' or 'msgpack'"
        ))),
    }
}

#[doc = async_variant_doc!("vega_to_png")]
#[pyfunction(name = "vega_to_png")]
#[pyo3(signature = (
    vg_spec,
    *,
    scale=None,
    ppi=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    google_fonts=None,
    config=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vega_to_png_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    scale: Option<f32>,
    ppi: Option<f32>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let config = parse_optional_config(config)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        config,
        background,
        width,
        height,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vega_to_png(vg_spec, vg_opts, PngOpts { scale, ppi })
                .await
                .map(|o| o.data)
        },
        "Vega to PNG conversion failed",
        |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("vegalite_to_png")]
#[pyfunction(name = "vegalite_to_png")]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    scale=None,
    ppi=None,
    config=None,
    theme=None,
    show_warnings=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    google_fonts=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vegalite_to_png_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    ppi: Option<f32>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    handle_show_warnings(show_warnings);
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        background,
        width,
        height,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vegalite_to_png(vl_spec, vl_opts, PngOpts { scale, ppi })
                .await
                .map(|o| o.data)
        },
        "Vega-Lite to PNG conversion failed",
        |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("vega_to_jpeg")]
#[pyfunction(name = "vega_to_jpeg")]
#[pyo3(signature = (
    vg_spec,
    *,
    scale=None,
    quality=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    google_fonts=None,
    config=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vega_to_jpeg_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    scale: Option<f32>,
    quality: Option<u8>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let config = parse_optional_config(config)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        config,
        background,
        width,
        height,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vega_to_jpeg(vg_spec, vg_opts, JpegOpts { scale, quality })
                .await
                .map(|o| o.data)
        },
        "Vega to JPEG conversion failed",
        |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("vegalite_to_jpeg")]
#[pyfunction(name = "vegalite_to_jpeg")]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    scale=None,
    quality=None,
    config=None,
    theme=None,
    show_warnings=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    google_fonts=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vegalite_to_jpeg_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    quality: Option<u8>,
    config: Option<PyObject>,
    theme: Option<String>,
    show_warnings: Option<bool>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    handle_show_warnings(show_warnings);
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        background,
        width,
        height,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vegalite_to_jpeg(vl_spec, vl_opts, JpegOpts { scale, quality })
                .await
                .map(|o| o.data)
        },
        "Vega-Lite to JPEG conversion failed",
        |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("vega_to_pdf")]
#[pyfunction(name = "vega_to_pdf")]
#[pyo3(signature = (
    vg_spec,
    *,
    scale=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    google_fonts=None,
    config=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vega_to_pdf_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    scale: Option<f32>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let config = parse_optional_config(config)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        config,
        background,
        width,
        height,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vega_to_pdf(vg_spec, vg_opts, PdfOpts::default())
                .await
                .map(|o| o.data)
        },
        "Vega to PDF conversion failed",
        |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("vegalite_to_pdf")]
#[pyfunction(name = "vegalite_to_pdf")]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    scale=None,
    config=None,
    theme=None,
    format_locale=None,
    time_format_locale=None,
    vega_plugin=None,
    google_fonts=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vegalite_to_pdf_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    scale: Option<f32>,
    config: Option<PyObject>,
    theme: Option<String>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    vega_plugin: Option<String>,
    google_fonts: Option<Vec<PyObject>>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        background,
        width,
        height,
    };

    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .vegalite_to_pdf(vl_spec, vl_opts, PdfOpts::default())
                .await
                .map(|o| o.data)
        },
        "Vega-Lite to PDF conversion failed",
        |py, value| Ok(PyBytes::new(py, value.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("vegalite_to_url")]
#[pyfunction(name = "vegalite_to_url")]
#[pyo3(signature = (vl_spec, *, fullscreen=None))]
pub fn vegalite_to_url_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    fullscreen: Option<bool>,
) -> PyResult<Bound<'py, PyAny>> {
    let vl_spec = parse_json_spec(vl_spec)?;
    let url = vl_convert_rs::converter::vegalite_to_url(
        &vl_spec,
        UrlOpts {
            fullscreen: fullscreen.unwrap_or(false),
        },
    )?;
    future_into_py_object(py, async move {
        Python::with_gil(|py| {
            pythonize(py, &url)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("vega_to_url")]
#[pyfunction(name = "vega_to_url")]
#[pyo3(signature = (vg_spec, *, fullscreen=None))]
pub fn vega_to_url_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    fullscreen: Option<bool>,
) -> PyResult<Bound<'py, PyAny>> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let url = vl_convert_rs::converter::vega_to_url(
        &vg_spec,
        UrlOpts {
            fullscreen: fullscreen.unwrap_or(false),
        },
    )?;
    future_into_py_object(py, async move {
        Python::with_gil(|py| {
            pythonize(py, &url)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("vegalite_to_html")]
#[pyfunction(name = "vegalite_to_html")]
#[pyo3(signature = (
    vl_spec,
    *,
    vl_version=None,
    bundle=None,
    google_fonts=None,
    config=None,
    theme=None,
    format_locale=None,
    time_format_locale=None,
    renderer=None,
    vega_plugin=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vegalite_to_html_asyncio<'py>(
    py: Python<'py>,
    vl_spec: PyObject,
    vl_version: Option<&str>,
    bundle: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    config: Option<PyObject>,
    theme: Option<String>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    renderer: Option<String>,
    vega_plugin: Option<String>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    let vl_version = if let Some(vl_version) = vl_version {
        VlVersion::from_str(vl_version)?
    } else {
        Default::default()
    };
    let vl_spec = parse_json_spec(vl_spec)?;
    let config = parse_optional_config(config)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let renderer = renderer.unwrap_or_else(|| "svg".to_string());
    let renderer = Renderer::from_str(&renderer)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let vl_opts = VlOpts {
        vl_version,
        config,
        theme,
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        background,
        width,
        height,
    };

    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("Vega-Lite to HTML conversion failed", err))?;

    let error_prefix = "Vega-Lite to HTML conversion failed";
    future_into_py_object(py, async move {
        let value = converter
            .vegalite_to_html(
                vl_spec,
                vl_opts,
                HtmlOpts {
                    bundle: bundle.unwrap_or(false),
                    renderer,
                },
            )
            .await
            .map(|o| o.html)
            .map_err(|err| prefixed_py_error(error_prefix, err))?;
        Python::with_gil(|py| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("vega_to_html")]
#[pyfunction(name = "vega_to_html")]
#[pyo3(signature = (
    vg_spec,
    *,
    bundle=None,
    google_fonts=None,
    format_locale=None,
    time_format_locale=None,
    renderer=None,
    vega_plugin=None,
    config=None,
    background=None,
    width=None,
    height=None,
))]
pub fn vega_to_html_asyncio<'py>(
    py: Python<'py>,
    vg_spec: PyObject,
    bundle: Option<bool>,
    google_fonts: Option<Vec<PyObject>>,
    format_locale: Option<PyObject>,
    time_format_locale: Option<PyObject>,
    renderer: Option<String>,
    vega_plugin: Option<String>,
    config: Option<PyObject>,
    background: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    let vg_spec = parse_json_spec(vg_spec)?;
    let format_locale = parse_option_format_locale(format_locale)?;
    let time_format_locale = parse_option_time_format_locale(time_format_locale)?;
    let renderer = renderer.unwrap_or_else(|| "svg".to_string());
    let renderer = Renderer::from_str(&renderer)?;
    let google_fonts = parse_google_fonts_arg(google_fonts)
        .map_err(|err| prefixed_py_error("Invalid google_fonts", err))?;
    let config = parse_optional_config(config)?;
    let vg_opts = VgOpts {
        format_locale,
        time_format_locale,
        google_fonts,
        vega_plugin,
        config,
        background,
        width,
        height,
    };

    let converter = converter_read_handle()
        .map_err(|err| prefixed_py_error("Vega to HTML conversion failed", err))?;

    let error_prefix = "Vega to HTML conversion failed";
    future_into_py_object(py, async move {
        let value = converter
            .vega_to_html(
                vg_spec,
                vg_opts,
                HtmlOpts {
                    bundle: bundle.unwrap_or(false),
                    renderer,
                },
            )
            .await
            .map(|o| o.html)
            .map_err(|err| prefixed_py_error(error_prefix, err))?;
        Python::with_gil(|py| {
            pythonize(py, &value)
                .map_err(|err| PyValueError::new_err(err.to_string()))
                .map(|obj| obj.into())
        })
    })
}

#[doc = async_variant_doc!("svg_to_png")]
#[pyfunction(name = "svg_to_png")]
#[pyo3(signature = (svg, *, scale=None, ppi=None))]
pub fn svg_to_png_asyncio<'py>(
    py: Python<'py>,
    svg: &str,
    scale: Option<f32>,
    ppi: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    let svg = svg.to_string();
    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .svg_to_png(&svg, PngOpts { scale, ppi })
                .await
                .map(|o| o.data)
        },
        "SVG to PNG conversion failed",
        |py, data| Ok(PyBytes::new(py, data.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("svg_to_jpeg")]
#[pyfunction(name = "svg_to_jpeg")]
#[pyo3(signature = (svg, *, scale=None, quality=None))]
pub fn svg_to_jpeg_asyncio<'py>(
    py: Python<'py>,
    svg: &str,
    scale: Option<f32>,
    quality: Option<u8>,
) -> PyResult<Bound<'py, PyAny>> {
    let svg = svg.to_string();
    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .svg_to_jpeg(&svg, JpegOpts { scale, quality })
                .await
                .map(|o| o.data)
        },
        "SVG to JPEG conversion failed",
        |py, data| Ok(PyBytes::new(py, data.as_slice()).into()),
    )
}

#[doc = async_variant_doc!("svg_to_pdf")]
#[pyfunction(name = "svg_to_pdf")]
#[pyo3(signature = (svg, *, scale=None))]
pub fn svg_to_pdf_asyncio<'py>(
    py: Python<'py>,
    svg: &str,
    scale: Option<f32>,
) -> PyResult<Bound<'py, PyAny>> {
    warn_if_scale_not_one_for_pdf(scale)?;
    let svg = svg.to_string();
    run_converter_future_async(
        py,
        move |converter| async move {
            converter
                .svg_to_pdf(&svg, PdfOpts::default())
                .await
                .map(|o| o.data)
        },
        "SVG to PDF conversion failed",
        |py, data| Ok(PyBytes::new(py, data.as_slice()).into()),
    )
}

#![allow(clippy::too_many_arguments)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::uninlined_format_args)]

#[macro_use]
extern crate lazy_static;

mod config;
mod conversions;
mod fonts;
mod metadata;
mod utils;

pub use config::*;
pub use conversions::*;
pub use fonts::*;
pub use metadata::*;

use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::sync::{Arc, RwLock};
use vl_convert_rs::converter::GoogleFontRequest;
use vl_convert_rs::VlConverter as VlConverterRs;

lazy_static! {
    static ref VL_CONVERTER: RwLock<Arc<VlConverterRs>> =
        RwLock::new(Arc::new(VlConverterRs::new()));
    static ref PYTHON_RUNTIME: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
    static ref CONFIGURED_GOOGLE_FONTS: RwLock<Option<Vec<GoogleFontRequest>>> = RwLock::new(None);
}

fn add_asyncio_submodule(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Returns Err if already initialized (expected on module re-import).
    // We intentionally ignore this value to make initialization idempotent.
    let _ = pyo3_async_runtimes::tokio::init_with_runtime(&PYTHON_RUNTIME);

    let asyncio = PyModule::new(py, "asyncio")?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_vega_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_svg_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_scenegraph_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_png_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_jpeg_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_pdf_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_url_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_to_html_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_svg_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_scenegraph_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_png_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_jpeg_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_pdf_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_url_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_to_html_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vegalite_fonts_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(vega_fonts_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(svg_to_png_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(svg_to_jpeg_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(svg_to_pdf_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(register_font_directory_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(configure_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(load_config_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_config_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(warm_up_workers_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_worker_memory_usage_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_local_tz_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_themes_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_format_locale, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_time_format_locale, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(javascript_bundle_asyncio, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_vega_version, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_vega_themes_version, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_vega_embed_version, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_vegalite_versions, &asyncio)?)?;
    asyncio.add_function(wrap_pyfunction!(get_config_path, &asyncio)?)?;

    m.add_submodule(&asyncio)?;
    py.import("sys")?
        .getattr("modules")?
        .set_item("vl_convert.asyncio", &asyncio)?;
    Ok(())
}

/// Convert Vega-Lite specifications to other formats
#[pymodule]
fn vl_convert(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();
    m.add_function(wrap_pyfunction!(vegalite_to_vega, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_svg, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_scenegraph, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_png, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_jpeg, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_pdf, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_url, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_to_html, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_svg, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_scenegraph, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_png, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_jpeg, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_pdf, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_url, m)?)?;
    m.add_function(wrap_pyfunction!(vega_to_html, m)?)?;
    m.add_function(wrap_pyfunction!(vegalite_fonts, m)?)?;
    m.add_function(wrap_pyfunction!(vega_fonts, m)?)?;
    m.add_function(wrap_pyfunction!(svg_to_png, m)?)?;
    m.add_function(wrap_pyfunction!(svg_to_jpeg, m)?)?;
    m.add_function(wrap_pyfunction!(svg_to_pdf, m)?)?;
    m.add_function(wrap_pyfunction!(register_font_directory, m)?)?;
    m.add_function(wrap_pyfunction!(configure, m)?)?;
    m.add_function(wrap_pyfunction!(load_config, m)?)?;
    m.add_function(wrap_pyfunction!(get_config_path, m)?)?;
    m.add_function(wrap_pyfunction!(get_config, m)?)?;
    m.add_function(wrap_pyfunction!(warm_up_workers, m)?)?;
    m.add_function(wrap_pyfunction!(get_worker_memory_usage, m)?)?;
    m.add_function(wrap_pyfunction!(get_local_tz, m)?)?;
    m.add_function(wrap_pyfunction!(get_themes, m)?)?;
    m.add_function(wrap_pyfunction!(get_format_locale, m)?)?;
    m.add_function(wrap_pyfunction!(get_time_format_locale, m)?)?;
    m.add_function(wrap_pyfunction!(javascript_bundle, m)?)?;
    m.add_function(wrap_pyfunction!(get_vega_version, m)?)?;
    m.add_function(wrap_pyfunction!(get_vega_themes_version, m)?)?;
    m.add_function(wrap_pyfunction!(get_vega_embed_version, m)?)?;
    m.add_function(wrap_pyfunction!(get_vegalite_versions, m)?)?;
    add_asyncio_submodule(py, m)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

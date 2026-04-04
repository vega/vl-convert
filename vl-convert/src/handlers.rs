use itertools::Itertools;
use vl_convert_rs::converter::{
    JpegOpts, PdfOpts, PngOpts, SvgOpts, VgOpts, VlConverter, VlOpts, VlcConfig,
};
use vl_convert_rs::{anyhow, anyhow::bail};

use crate::io_utils::{
    parse_as_json, parse_format_locale_option, parse_time_format_locale_option, parse_vl_version,
    read_config_json, read_input_string, write_output_binary, write_output_string,
};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn vl_2_vg(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    pretty: bool,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vl_version = parse_vl_version(vl_version)?;
    let vegalite_str = read_input_string(input)?;
    let vegalite_json = parse_as_json(&vegalite_str)?;
    let config = read_config_json(config)?;

    let converter = VlConverter::with_config(converter_config)?;

    let vega_output = match converter
        .vegalite_to_vega(
            vegalite_json,
            VlOpts {
                vl_version,
                theme,
                config,
                ..Default::default()
            },
        )
        .await
    {
        Ok(output) => output,
        Err(err) => {
            bail!("Vega-Lite to Vega conversion failed: {}", err);
        }
    };
    let vega_str_res = if pretty {
        serde_json::to_string_pretty(&vega_output.spec)
    } else {
        serde_json::to_string(&vega_output.spec)
    };
    match vega_str_res {
        Ok(vega_str) => {
            write_output_string(output, &vega_str)?;
        }
        Err(err) => {
            bail!("Failed to serialize Vega spec to JSON string: {err}")
        }
    }

    Ok(())
}

pub(crate) async fn vg_2_svg(
    input: Option<&str>,
    output: Option<&str>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    svg_opts: SvgOpts,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vega_str = read_input_string(input)?;
    let vg_spec = parse_as_json(&vega_str)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let svg_output = match converter
        .vega_to_svg(
            vg_spec,
            VgOpts {
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            svg_opts,
        )
        .await
    {
        Ok(output) => output,
        Err(err) => {
            bail!("Vega to SVG conversion failed: {}", err);
        }
    };

    write_output_string(output, &svg_output.svg)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn vg_2_png(
    input: Option<&str>,
    output: Option<&str>,
    scale: f32,
    ppi: f32,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vega_str = read_input_string(input)?;
    let vg_spec = parse_as_json(&vega_str)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let png_output = match converter
        .vega_to_png(
            vg_spec,
            VgOpts {
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            PngOpts {
                scale: Some(scale),
                ppi: Some(ppi),
            },
        )
        .await
    {
        Ok(output) => output,
        Err(err) => {
            bail!("Vega to PNG conversion failed: {}", err);
        }
    };

    write_output_binary(output, &png_output.data, "PNG")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn vg_2_jpeg(
    input: Option<&str>,
    output: Option<&str>,
    scale: f32,
    quality: u8,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vega_str = read_input_string(input)?;
    let vg_spec = parse_as_json(&vega_str)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let jpeg_output = match converter
        .vega_to_jpeg(
            vg_spec,
            VgOpts {
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            JpegOpts {
                scale: Some(scale),
                quality: Some(quality),
            },
        )
        .await
    {
        Ok(output) => output,
        Err(err) => {
            bail!("Vega to JPEG conversion failed: {}", err);
        }
    };

    write_output_binary(output, &jpeg_output.data, "JPEG")?;

    Ok(())
}

pub(crate) async fn vg_2_pdf(
    input: Option<&str>,
    output: Option<&str>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vega_str = read_input_string(input)?;
    let vg_spec = parse_as_json(&vega_str)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let pdf_output = match converter
        .vega_to_pdf(
            vg_spec,
            VgOpts {
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            PdfOpts::default(),
        )
        .await
    {
        Ok(output) => output,
        Err(err) => {
            bail!("Vega to PDF conversion failed: {}", err);
        }
    };

    write_output_binary(output, &pdf_output.data, "PDF")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn vl_2_svg(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    svg_opts: SvgOpts,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vl_version = parse_vl_version(vl_version)?;
    let vegalite_str = read_input_string(input)?;
    let vl_spec = parse_as_json(&vegalite_str)?;
    let config = read_config_json(config)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let svg_output = match converter
        .vegalite_to_svg(
            vl_spec,
            VlOpts {
                vl_version,
                config,
                theme,
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            svg_opts,
        )
        .await
    {
        Ok(output) => output,
        Err(err) => {
            bail!("Vega-Lite to SVG conversion failed: {}", err);
        }
    };

    write_output_string(output, &svg_output.svg)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn vl_2_png(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    scale: f32,
    ppi: f32,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vl_version = parse_vl_version(vl_version)?;
    let vegalite_str = read_input_string(input)?;
    let vl_spec = parse_as_json(&vegalite_str)?;
    let config = read_config_json(config)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let png_output = match converter
        .vegalite_to_png(
            vl_spec,
            VlOpts {
                vl_version,
                config,
                theme,
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            PngOpts {
                scale: Some(scale),
                ppi: Some(ppi),
            },
        )
        .await
    {
        Ok(output) => output,
        Err(err) => {
            bail!("Vega-Lite to PNG conversion failed: {}", err);
        }
    };

    write_output_binary(output, &png_output.data, "PNG")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn vl_2_jpeg(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    scale: f32,
    quality: u8,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vl_version = parse_vl_version(vl_version)?;
    let vegalite_str = read_input_string(input)?;
    let vl_spec = parse_as_json(&vegalite_str)?;
    let config = read_config_json(config)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let jpeg_output = match converter
        .vegalite_to_jpeg(
            vl_spec,
            VlOpts {
                vl_version,
                config,
                theme,
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            JpegOpts {
                scale: Some(scale),
                quality: Some(quality),
            },
        )
        .await
    {
        Ok(output) => output,
        Err(err) => {
            bail!("Vega-Lite to JPEG conversion failed: {}", err);
        }
    };

    write_output_binary(output, &jpeg_output.data, "JPEG")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn vl_2_pdf(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
    converter_config: VlcConfig,
) -> Result<(), anyhow::Error> {
    let vl_version = parse_vl_version(vl_version)?;
    let vegalite_str = read_input_string(input)?;
    let vl_spec = parse_as_json(&vegalite_str)?;
    let config = read_config_json(config)?;

    let format_locale = parse_format_locale_option(format_locale.as_deref())?;
    let time_format_locale = parse_time_format_locale_option(time_format_locale.as_deref())?;

    let converter = VlConverter::with_config(converter_config)?;

    let pdf_output = match converter
        .vegalite_to_pdf(
            vl_spec,
            VlOpts {
                vl_version,
                config,
                theme,
                format_locale,
                time_format_locale,
                ..Default::default()
            },
            PdfOpts::default(),
        )
        .await
    {
        Ok(output) => output,
        Err(err) => {
            bail!("Vega-Lite to PDF conversion failed: {}", err);
        }
    };

    write_output_binary(output, &pdf_output.data, "PDF")?;

    Ok(())
}

pub(crate) async fn list_themes(config: VlcConfig) -> Result<(), anyhow::Error> {
    let converter = VlConverter::with_config(config)?;

    if let serde_json::Value::Object(themes) = converter.get_themes().await? {
        for theme in themes.keys().sorted() {
            println!("{}", theme)
        }
    } else {
        bail!("Failed to load themes")
    }

    Ok(())
}

pub(crate) async fn cat_theme(theme: &str, config: VlcConfig) -> Result<(), anyhow::Error> {
    let converter = VlConverter::with_config(config)?;

    if let serde_json::Value::Object(themes) = converter.get_themes().await? {
        if let Some(theme_config) = themes.get(theme) {
            let theme_config_str = serde_json::to_string_pretty(theme_config).unwrap();
            println!("{}", theme_config_str);
        } else {
            bail!("No theme named '{}'", theme)
        }
    } else {
        bail!("Failed to load themes")
    }
    Ok(())
}

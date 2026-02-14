#![allow(clippy::uninlined_format_args)]
#![doc = include_str!("../README.md")]

use clap::{Parser, Subcommand};
use itertools::Itertools;
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;
use std::str::FromStr;
use vl_convert_rs::converter::{
    vega_to_url, vegalite_to_url, FormatLocale, Renderer, TimeFormatLocale, VgOpts, VlConverter,
    VlOpts,
};
use vl_convert_rs::module_loader::import_map::VlVersion;
use vl_convert_rs::text::register_font_directory;
use vl_convert_rs::{anyhow, anyhow::bail};

const DEFAULT_VL_VERSION: &str = "6.4";
const DEFAULT_CONFIG_PATH: &str = "~/.config/vl-convert/config.json";

#[derive(Debug, Parser)] // requires `derive` feature
#[command(version, name = "vl-convert")]
#[command(about = "vl-convert: A utility for converting Vega-Lite specifications", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Convert a Vega-Lite specification to a Vega specification
    Vl2vg {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output Vega file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(short, long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file. Defaults to ~/.config/vl-convert/config.json
        #[arg(short, long)]
        config: Option<String>,

        /// Pretty-print JSON in output file
        #[arg(short, long)]
        pretty: bool,

        /// Whether to show Vega-Lite compilation warnings
        #[arg(long)]
        show_warnings: bool,
    },

    /// Convert a Vega-Lite specification to an SVG image
    Vl2svg {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output SVG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file. Defaults to ~/.config/vl-convert/config.json
        #[arg(short, long)]
        config: Option<String>,

        /// Whether to show Vega-Lite compilation warnings
        #[arg(long)]
        show_warnings: bool,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// Allowed base URL for external data requests. Default allows any base URL
        #[arg(short, long)]
        allowed_base_url: Option<Vec<String>>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega-Lite specification to an PNG image
    Vl2png {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PNG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file. Defaults to ~/.config/vl-convert/config.json
        #[arg(short, long)]
        config: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// Pixels per inch
        #[arg(short, long, default_value = "72.0")]
        ppi: f32,

        /// Whether to show Vega-Lite compilation warnings
        #[arg(long)]
        show_warnings: bool,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// Allowed base URL for external data requests. Default allows any base URL
        #[arg(short, long)]
        allowed_base_url: Option<Vec<String>>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega-Lite specification to an JPEG image
    Vl2jpeg {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JPEG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file. Defaults to ~/.config/vl-convert/config.json
        #[arg(short, long)]
        config: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// JPEG Quality between 0 (worst) and 100 (best)
        #[arg(short, long, default_value = "90")]
        quality: u8,

        /// Whether to show Vega-Lite compilation warnings
        #[arg(short, long)]
        show_warnings: bool,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// Allowed base URL for external data requests. Default allows any base URL
        #[arg(short, long)]
        allowed_base_url: Option<Vec<String>>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega-Lite specification to a PDF image
    Vl2pdf {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PDF file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file. Defaults to ~/.config/vl-convert/config.json
        #[arg(short, long)]
        config: Option<String>,

        /// Whether to show Vega-Lite compilation warnings
        #[arg(long)]
        show_warnings: bool,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// Allowed base URL for external data requests. Default allows any base URL
        #[arg(short, long)]
        allowed_base_url: Option<Vec<String>>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega-Lite specification to a URL that opens the chart in the Vega editor
    Vl2url {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Open chart in fullscreen mode
        #[arg(long, default_value = "false")]
        fullscreen: bool,
    },

    /// Convert a Vega-Lite specification to an HTML file
    Vl2html {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output HTML file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file. Defaults to ~/.config/vl-convert/config.json
        #[arg(short, long)]
        config: Option<String>,

        /// Whether to bundle JavaScript dependencies in the HTML file
        /// instead of loading them from a CDN
        #[arg(short, long)]
        bundle: bool,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Vega renderer. One of 'svg' (default), 'canvas', or 'hybrid'
        #[arg(long)]
        renderer: Option<String>,
    },

    /// Convert a Vega specification to an SVG image
    Vg2svg {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output SVG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// Allowed base URL for external data requests. Default allows any base URL
        #[arg(short, long)]
        allowed_base_url: Option<Vec<String>>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega specification to an PNG image
    Vg2png {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PNG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// Pixels per inch
        #[arg(short, long, default_value = "72.0")]
        ppi: f32,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// Allowed base URL for external data requests. Default allows any base URL
        #[arg(short, long)]
        allowed_base_url: Option<Vec<String>>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega specification to an JPEG image
    Vg2jpeg {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JPEG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// JPEG Quality between 0 (worst) and 100 (best)
        #[arg(short, long, default_value = "90")]
        quality: u8,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// Allowed base URL for external data requests. Default allows any base URL
        #[arg(short, long)]
        allowed_base_url: Option<Vec<String>>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega specification to an PDF image
    Vg2pdf {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PDF file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// Allowed base URL for external data requests. Default allows any base URL
        #[arg(short, long)]
        allowed_base_url: Option<Vec<String>>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,
    },

    /// Convert a Vega specification to a URL that opens the chart in the Vega editor
    Vg2url {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Open chart in fullscreen mode
        #[arg(long, default_value = "false")]
        fullscreen: bool,
    },

    /// Convert a Vega specification to an HTML file
    Vg2html {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output HTML file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Whether to bundle JavaScript dependencies in the HTML file
        /// instead of loading them from a CDN
        #[arg(short, long)]
        bundle: bool,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Vega renderer. One of 'svg' (default), 'canvas', or 'hybrid'
        #[arg(long)]
        renderer: Option<String>,
    },

    /// Convert an SVG image to a PNG image
    Svg2png {
        /// Path to input SVG file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PNG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// Pixels per inch
        #[arg(short, long, default_value = "72.0")]
        ppi: f32,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,
    },

    /// Convert an SVG image to a JPEG image
    Svg2jpeg {
        /// Path to input SVG file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JPEG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// JPEG Quality between 0 (worst) and 100 (best)
        #[arg(short, long, default_value = "90")]
        quality: u8,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,
    },

    /// Convert an SVG image to a PDF image
    Svg2pdf {
        /// Path to input SVG file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PDF file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,
    },

    /// List available themes
    LsThemes,

    /// Print the config JSON for a theme
    #[command(arg_required_else_help = true)]
    CatTheme {
        /// Name of a theme
        theme: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args = Cli::parse();
    use crate::Commands::*;
    match args.command {
        Vl2vg {
            input: input_vegalite_file,
            output: output_vega_file,
            vl_version,
            theme,
            config,
            pretty,
            show_warnings,
        } => {
            vl_2_vg(
                input_vegalite_file.as_deref(),
                output_vega_file.as_deref(),
                &vl_version,
                theme,
                config,
                pretty,
                show_warnings,
            )
            .await?
        }
        Vl2svg {
            input,
            output,
            vl_version,
            theme,
            config,
            show_warnings,
            font_dir,
            allowed_base_url,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vl_2_svg(
                input.as_deref(),
                output.as_deref(),
                &vl_version,
                theme,
                config,
                show_warnings,
                allowed_base_url,
                format_locale,
                time_format_locale,
            )
            .await?
        }
        Vl2png {
            input,
            output,
            vl_version,
            theme,
            config,
            scale,
            ppi,
            show_warnings,
            font_dir,
            allowed_base_url,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vl_2_png(
                input.as_deref(),
                output.as_deref(),
                &vl_version,
                theme,
                config,
                scale,
                ppi,
                show_warnings,
                allowed_base_url,
                format_locale,
                time_format_locale,
            )
            .await?
        }
        Vl2jpeg {
            input,
            output,
            vl_version,
            theme,
            config,
            scale,
            quality,
            show_warnings,
            font_dir,
            allowed_base_url,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vl_2_jpeg(
                input.as_deref(),
                output.as_deref(),
                &vl_version,
                theme,
                config,
                scale,
                quality,
                show_warnings,
                allowed_base_url,
                format_locale,
                time_format_locale,
            )
            .await?
        }
        Vl2pdf {
            input,
            output,
            vl_version,
            theme,
            config,
            show_warnings,
            font_dir,
            allowed_base_url,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vl_2_pdf(
                input.as_deref(),
                output.as_deref(),
                &vl_version,
                theme,
                config,
                show_warnings,
                allowed_base_url,
                format_locale,
                time_format_locale,
            )
            .await?
        }
        Vl2url {
            input,
            output,
            fullscreen,
        } => {
            let vl_str = read_input_string(input.as_deref())?;
            let vl_spec = serde_json::from_str(&vl_str)?;
            let url = vegalite_to_url(&vl_spec, fullscreen)?;
            write_output_string(output.as_deref(), &url)?
        }
        Vl2html {
            input,
            output,
            vl_version,
            theme,
            config,
            bundle,
            format_locale,
            time_format_locale,
            renderer,
        } => {
            // Initialize converter
            let vl_str = read_input_string(input.as_deref())?;
            let vl_spec: serde_json::Value = serde_json::from_str(&vl_str)?;
            let config = read_config_json(config)?;
            let vl_version = parse_vl_version(&vl_version)?;
            let format_locale = match &format_locale {
                None => None,
                Some(p) => Some(format_locale_from_str(p)?),
            };

            let time_format_locale = match &time_format_locale {
                None => None,
                Some(p) => Some(time_format_locale_from_str(p)?),
            };
            let renderer = renderer.unwrap_or_else(|| "svg".to_string());

            let mut converter = VlConverter::new();
            let html = converter
                .vegalite_to_html(
                    vl_spec,
                    VlOpts {
                        config,
                        theme,
                        vl_version,
                        show_warnings: false,
                        allowed_base_urls: None,
                        format_locale,
                        time_format_locale,
                    },
                    bundle,
                    Renderer::from_str(&renderer)?,
                )
                .await?;
            write_output_string(output.as_deref(), &html)?;
        }
        Vg2svg {
            input,
            output,
            font_dir,
            allowed_base_url,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vg_2_svg(
                input.as_deref(),
                output.as_deref(),
                allowed_base_url,
                format_locale,
                time_format_locale,
            )
            .await?
        }
        Vg2png {
            input,
            output,
            scale,
            ppi,
            font_dir,
            allowed_base_url,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vg_2_png(
                input.as_deref(),
                output.as_deref(),
                scale,
                ppi,
                allowed_base_url,
                format_locale,
                time_format_locale,
            )
            .await?
        }
        Vg2jpeg {
            input,
            output,
            scale,
            quality,
            font_dir,
            allowed_base_url,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vg_2_jpeg(
                input.as_deref(),
                output.as_deref(),
                scale,
                quality,
                allowed_base_url,
                format_locale,
                time_format_locale,
            )
            .await?
        }
        Vg2pdf {
            input,
            output,
            font_dir,
            allowed_base_url,
            format_locale,
            time_format_locale,
        } => {
            register_font_dir(font_dir)?;
            vg_2_pdf(
                input.as_deref(),
                output.as_deref(),
                allowed_base_url,
                format_locale,
                time_format_locale,
            )
            .await?
        }
        Vg2url {
            input,
            output,
            fullscreen,
        } => {
            let vg_str = read_input_string(input.as_deref())?;
            let vg_spec = serde_json::from_str(&vg_str)?;
            let url = vega_to_url(&vg_spec, fullscreen)?;
            write_output_string(output.as_deref(), &url)?
        }
        Vg2html {
            input,
            output,
            bundle,
            format_locale,
            time_format_locale,
            renderer,
        } => {
            // Initialize converter
            let vg_str = read_input_string(input.as_deref())?;
            let vg_spec: serde_json::Value = serde_json::from_str(&vg_str)?;

            let format_locale = match &format_locale {
                None => None,
                Some(p) => Some(format_locale_from_str(p)?),
            };

            let time_format_locale = match &time_format_locale {
                None => None,
                Some(p) => Some(time_format_locale_from_str(p)?),
            };

            let renderer = renderer.unwrap_or_else(|| "svg".to_string());

            let mut converter = VlConverter::new();
            let html = converter
                .vega_to_html(
                    vg_spec,
                    VgOpts {
                        allowed_base_urls: None,
                        format_locale,
                        time_format_locale,
                    },
                    bundle,
                    Renderer::from_str(&renderer)?,
                )
                .await?;
            write_output_string(output.as_deref(), &html)?;
        }
        Svg2png {
            input,
            output,
            scale,
            ppi,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            let svg = read_input_string(input.as_deref())?;
            let png_data = vl_convert_rs::converter::svg_to_png(&svg, scale, Some(ppi))?;
            write_output_binary(output.as_deref(), &png_data, "PNG")?;
        }
        Svg2jpeg {
            input,
            output,
            scale,
            quality,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            let svg = read_input_string(input.as_deref())?;
            let jpeg_data = vl_convert_rs::converter::svg_to_jpeg(&svg, scale, Some(quality))?;
            write_output_binary(output.as_deref(), &jpeg_data, "JPEG")?;
        }
        Svg2pdf {
            input,
            output,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            let svg = read_input_string(input.as_deref())?;
            let pdf_data = vl_convert_rs::converter::svg_to_pdf(&svg)?;
            write_output_binary(output.as_deref(), &pdf_data, "PDF")?;
        }
        LsThemes => list_themes().await?,
        CatTheme { theme } => cat_theme(&theme).await?,
    }

    Ok(())
}

fn register_font_dir(dir: Option<String>) -> Result<(), anyhow::Error> {
    if let Some(dir) = dir {
        register_font_directory(&dir)?
    }
    Ok(())
}

fn parse_vl_version(vl_version: &str) -> Result<VlVersion, anyhow::Error> {
    if let Ok(vl_version) = VlVersion::from_str(vl_version) {
        Ok(vl_version)
    } else {
        bail!("Invalid or unsupported Vega-Lite version: {}", vl_version);
    }
}

fn read_input_string(input: Option<&str>) -> Result<String, anyhow::Error> {
    match input {
        Some(path) if path != "-" => std::fs::read_to_string(path)
            .map_err(|err| anyhow::anyhow!("Failed to read input file: {}\n{}", path, err)),
        _ => {
            // Check if stdin is an interactive terminal
            if io::stdin().is_terminal() {
                eprintln!("Reading from stdin... (Press Ctrl-D when done, or use -i <file>)");
            }

            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .map_err(|err| anyhow::anyhow!("Failed to read from stdin: {}", err))?;

            // Check for empty or whitespace-only input
            if buffer.trim().is_empty() {
                bail!("No input provided. Provide a specification via stdin or use -i <file>");
            }

            Ok(buffer)
        }
    }
}

/// Read a file that is always a filesystem path (never stdin).
///
/// This function is used for reading configuration files (locale, time format)
/// that should not come from stdin. For reading input specifications that may
/// come from stdin or a file, use `read_input_string()` instead.
fn read_file_string(path: &str) -> Result<String, anyhow::Error> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(err) => {
            bail!("Failed to read file: {}\n{}", path, err);
        }
    }
}

fn parse_as_json(input_str: &str) -> Result<serde_json::Value, anyhow::Error> {
    match serde_json::from_str::<serde_json::Value>(input_str) {
        Ok(input_json) => Ok(input_json),
        Err(err) => {
            bail!("Failed to parse input file as JSON: {}", err);
        }
    }
}

fn format_locale_from_str(s: &str) -> Result<FormatLocale, anyhow::Error> {
    if s.ends_with(".json") {
        let s = read_file_string(s)?;
        Ok(FormatLocale::Object(parse_as_json(&s)?))
    } else {
        Ok(FormatLocale::Name(s.to_string()))
    }
}

fn time_format_locale_from_str(s: &str) -> Result<TimeFormatLocale, anyhow::Error> {
    if s.ends_with(".json") {
        let s = read_file_string(s)?;
        Ok(TimeFormatLocale::Object(parse_as_json(&s)?))
    } else {
        Ok(TimeFormatLocale::Name(s.to_string()))
    }
}

fn write_output_string(output: Option<&str>, output_str: &str) -> Result<(), anyhow::Error> {
    match output {
        Some(path) if path != "-" => {
            // File output: write as-is without modification
            std::fs::write(path, output_str)
                .map_err(|err| anyhow::anyhow!("Failed to write output to {}\n{}", path, err))
        }
        _ => {
            // Stdout output: ensure trailing newline and handle BrokenPipe
            let stdout = io::stdout();
            let mut handle = stdout.lock();

            // Write the string
            if let Err(err) = handle.write_all(output_str.as_bytes()) {
                if err.kind() == io::ErrorKind::BrokenPipe {
                    std::process::exit(0);
                }
                return Err(anyhow::anyhow!("Failed to write to stdout: {}", err));
            }

            // Add trailing newline if not already present
            if !output_str.ends_with('\n') {
                if let Err(err) = handle.write_all(b"\n") {
                    if err.kind() == io::ErrorKind::BrokenPipe {
                        std::process::exit(0);
                    }
                    return Err(anyhow::anyhow!(
                        "Failed to write newline to stdout: {}",
                        err
                    ));
                }
            }

            // Flush
            if let Err(err) = handle.flush() {
                if err.kind() == io::ErrorKind::BrokenPipe {
                    std::process::exit(0);
                }
                return Err(anyhow::anyhow!("Failed to flush stdout: {}", err));
            }

            Ok(())
        }
    }
}

/// Write binary output data to a file or stdout with TTY safety guard.
///
/// # Behavior
/// - `output = Some(path)` where `path != "-"`: Write to file
/// - `output = Some("-")`: Force write to stdout (user override)
/// - `output = None`: Write to stdout only if not a TTY (safety guard)
///
/// # TTY Safety Guard
/// When `output = None` and stdout is a terminal, this function refuses to write
/// binary data to prevent terminal corruption. Users must either:
/// - Redirect to a file: `vl-convert vl2png -o output.png`
/// - Pipe to another command: `vl-convert vl2png | display`
/// - Force stdout: `vl-convert vl2png -o -`
///
/// # Testing Note
/// The TTY safety guard is tested manually because automated tests run with
/// piped stdout (not a TTY). To verify:
/// ```bash
/// # Should refuse (interactive terminal)
/// $ echo '{"$schema": "..."}' | vl-convert vl2png
///
/// # Should succeed (explicit override)
/// $ echo '{"$schema": "..."}' | vl-convert vl2png -o -
///
/// # Should succeed (piped)
/// $ echo '{"$schema": "..."}' | vl-convert vl2png | cat > test.png
/// ```
fn write_output_binary(
    output: Option<&str>,
    output_data: &[u8],
    format_name: &str,
) -> Result<(), anyhow::Error> {
    match output {
        Some(path) if path != "-" => std::fs::write(path, output_data)
            .map_err(|err| anyhow::anyhow!("Failed to write output to {}\n{}", path, err)),
        Some(_) => {
            // Explicit "-": write to stdout unconditionally (user override)
            write_stdout_bytes(output_data)
        }
        None => {
            // Implicit stdout: TTY safety guard
            if io::stdout().is_terminal() {
                bail!(
                    "Refusing to write binary {} data to terminal.\n\
                     Use -o <file> to write to a file, or pipe to another command.\n\
                     Use -o - to force output to stdout.",
                    format_name
                );
            }
            write_stdout_bytes(output_data)
        }
    }
}

/// Set stdout to binary mode on Windows to prevent newline translation.
///
/// On Windows, stdout defaults to "text mode" which translates `\n` (0x0A) to `\r\n` (0x0D 0x0A)
/// and treats `\x1A` (Ctrl-Z) as EOF. This corrupts binary data like PNG, JPEG, and PDF files.
///
/// This function uses the Windows C runtime `_setmode` function to switch stdout to binary mode.
/// On Unix systems (Linux, macOS), this is a no-op because stdout is always binary.
///
/// # References
/// - [Microsoft _setmode Documentation](https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/setmode)
///
/// # Safety
/// Uses unsafe FFI to call the Windows CRT function `_setmode`.
#[cfg(target_family = "windows")]
fn set_stdout_binary_mode() -> Result<(), anyhow::Error> {
    extern "C" {
        fn _setmode(fd: i32, mode: i32) -> i32;
    }
    const STDOUT_FILENO: i32 = 1;
    const O_BINARY: i32 = 0x8000;
    unsafe {
        let result = _setmode(STDOUT_FILENO, O_BINARY);
        if result == -1 {
            Err(anyhow::anyhow!("Failed to set binary mode on stdout"))
        } else {
            Ok(())
        }
    }
}

/// No-op on Unix systems where stdout is always binary.
#[cfg(not(target_family = "windows"))]
fn set_stdout_binary_mode() -> Result<(), anyhow::Error> {
    Ok(())
}

fn write_stdout_bytes(data: &[u8]) -> Result<(), anyhow::Error> {
    // Set stdout to binary mode on Windows before writing
    set_stdout_binary_mode()?;

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Write data, handling BrokenPipe as clean exit
    if let Err(err) = handle.write_all(data) {
        if err.kind() == io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(anyhow::anyhow!("Failed to write to stdout: {}", err));
    }

    // Flush, handling BrokenPipe as clean exit
    if let Err(err) = handle.flush() {
        if err.kind() == io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(anyhow::anyhow!("Failed to flush stdout: {}", err));
    }

    Ok(())
}

fn normalize_config_path(config: Option<String>) -> Option<String> {
    match config {
        Some(config) => Some(shellexpand::tilde(config.trim()).to_string()),
        None => {
            let default_path = shellexpand::tilde(DEFAULT_CONFIG_PATH).to_string();
            if Path::new(&default_path).exists() {
                Some(default_path)
            } else {
                None
            }
        }
    }
}

fn read_config_json(config: Option<String>) -> Result<Option<serde_json::Value>, anyhow::Error> {
    let config = normalize_config_path(config);
    match config {
        None => Ok(None),
        Some(config) => {
            let config_str = match std::fs::read_to_string(&config) {
                Ok(config_str) => config_str,
                Err(err) => {
                    bail!("Failed to read config file: {}\n{}", config, err);
                }
            };
            let config_json: serde_json::Value = serde_json::from_str(&config_str)?;
            Ok(Some(config_json))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_vg(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    pretty: bool,
    show_warnings: bool,
) -> Result<(), anyhow::Error> {
    // Parse version
    let vl_version = parse_vl_version(vl_version)?;

    // Read input file
    let vegalite_str = read_input_string(input)?;

    // Parse input as json
    let vegalite_json = parse_as_json(&vegalite_str)?;

    // Load config from file
    let config = read_config_json(config)?;

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let vega_json = match converter
        .vegalite_to_vega(
            vegalite_json,
            VlOpts {
                vl_version,
                theme,
                config,
                show_warnings,
                allowed_base_urls: None,
                format_locale: None,
                time_format_locale: None,
            },
        )
        .await
    {
        Ok(vega_str) => vega_str,
        Err(err) => {
            bail!("Vega-Lite to Vega conversion failed: {}", err);
        }
    };
    let vega_str_res = if pretty {
        serde_json::to_string_pretty(&vega_json)
    } else {
        serde_json::to_string(&vega_json)
    };
    match vega_str_res {
        Ok(vega_str) => {
            // Write result
            write_output_string(output, &vega_str)?;
        }
        Err(err) => {
            bail!("Failed to serialize Vega spec to JSON string: {err}")
        }
    }

    Ok(())
}

async fn vg_2_svg(
    input: Option<&str>,
    output: Option<&str>,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
) -> Result<(), anyhow::Error> {
    // Read input file
    let vega_str = read_input_string(input)?;

    // Parse input as json
    let vg_spec = parse_as_json(&vega_str)?;

    let format_locale = match &format_locale {
        None => None,
        Some(p) => Some(format_locale_from_str(p)?),
    };

    let time_format_locale = match &time_format_locale {
        None => None,
        Some(p) => Some(time_format_locale_from_str(p)?),
    };

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let svg = match converter
        .vega_to_svg(
            vg_spec,
            VgOpts {
                allowed_base_urls,
                format_locale,
                time_format_locale,
            },
        )
        .await
    {
        Ok(svg) => svg,
        Err(err) => {
            bail!("Vega to SVG conversion failed: {}", err);
        }
    };

    // Write result
    write_output_string(output, &svg)?;

    Ok(())
}

async fn vg_2_png(
    input: Option<&str>,
    output: Option<&str>,
    scale: f32,
    ppi: f32,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
) -> Result<(), anyhow::Error> {
    // Read input file
    let vega_str = read_input_string(input)?;

    // Parse input as json
    let vg_spec = parse_as_json(&vega_str)?;

    let format_locale = match &format_locale {
        None => None,
        Some(p) => Some(format_locale_from_str(p)?),
    };

    let time_format_locale = match &time_format_locale {
        None => None,
        Some(p) => Some(time_format_locale_from_str(p)?),
    };

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let png_data = match converter
        .vega_to_png(
            vg_spec,
            VgOpts {
                allowed_base_urls,
                format_locale,
                time_format_locale,
            },
            Some(scale),
            Some(ppi),
        )
        .await
    {
        Ok(png_data) => png_data,
        Err(err) => {
            bail!("Vega to PNG conversion failed: {}", err);
        }
    };

    // Write result
    write_output_binary(output, &png_data, "PNG")?;

    Ok(())
}

async fn vg_2_jpeg(
    input: Option<&str>,
    output: Option<&str>,
    scale: f32,
    quality: u8,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
) -> Result<(), anyhow::Error> {
    // Read input file
    let vega_str = read_input_string(input)?;

    // Parse input as json
    let vg_spec = parse_as_json(&vega_str)?;

    let format_locale = match &format_locale {
        None => None,
        Some(p) => Some(format_locale_from_str(p)?),
    };

    let time_format_locale = match &time_format_locale {
        None => None,
        Some(p) => Some(time_format_locale_from_str(p)?),
    };

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let jpeg_data = match converter
        .vega_to_jpeg(
            vg_spec,
            VgOpts {
                allowed_base_urls,
                format_locale,
                time_format_locale,
            },
            Some(scale),
            Some(quality),
        )
        .await
    {
        Ok(jpeg_data) => jpeg_data,
        Err(err) => {
            bail!("Vega to JPEG conversion failed: {}", err);
        }
    };

    // Write result
    write_output_binary(output, &jpeg_data, "JPEG")?;

    Ok(())
}

async fn vg_2_pdf(
    input: Option<&str>,
    output: Option<&str>,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
) -> Result<(), anyhow::Error> {
    // Read input file
    let vega_str = read_input_string(input)?;

    // Parse input as json
    let vg_spec = parse_as_json(&vega_str)?;

    let format_locale = match &format_locale {
        None => None,
        Some(p) => Some(format_locale_from_str(p)?),
    };

    let time_format_locale = match &time_format_locale {
        None => None,
        Some(p) => Some(time_format_locale_from_str(p)?),
    };

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let pdf_data = match converter
        .vega_to_pdf(
            vg_spec,
            VgOpts {
                allowed_base_urls,
                format_locale,
                time_format_locale,
            },
        )
        .await
    {
        Ok(pdf_data) => pdf_data,
        Err(err) => {
            bail!("Vega to PDF conversion failed: {}", err);
        }
    };

    // Write result
    write_output_binary(output, &pdf_data, "PDF")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_svg(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    show_warnings: bool,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
) -> Result<(), anyhow::Error> {
    // Parse version
    let vl_version = parse_vl_version(vl_version)?;

    // Read input file
    let vegalite_str = read_input_string(input)?;

    // Parse input as json
    let vl_spec = parse_as_json(&vegalite_str)?;

    // Load config from file
    let config = read_config_json(config)?;

    let format_locale = match &format_locale {
        None => None,
        Some(p) => Some(format_locale_from_str(p)?),
    };

    let time_format_locale = match &time_format_locale {
        None => None,
        Some(p) => Some(time_format_locale_from_str(p)?),
    };

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let svg = match converter
        .vegalite_to_svg(
            vl_spec,
            VlOpts {
                vl_version,
                config,
                theme,
                show_warnings,
                allowed_base_urls,
                format_locale,
                time_format_locale,
            },
        )
        .await
    {
        Ok(svg) => svg,
        Err(err) => {
            bail!("Vega-Lite to SVG conversion failed: {}", err);
        }
    };

    // Write result
    write_output_string(output, &svg)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_png(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    scale: f32,
    ppi: f32,
    show_warnings: bool,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
) -> Result<(), anyhow::Error> {
    // Parse version
    let vl_version = parse_vl_version(vl_version)?;

    // Read input file
    let vegalite_str = read_input_string(input)?;

    // Parse input as json
    let vl_spec = parse_as_json(&vegalite_str)?;

    // Load config from file
    let config = read_config_json(config)?;

    let format_locale = match &format_locale {
        None => None,
        Some(p) => Some(format_locale_from_str(p)?),
    };

    let time_format_locale = match &time_format_locale {
        None => None,
        Some(p) => Some(time_format_locale_from_str(p)?),
    };

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let png_data = match converter
        .vegalite_to_png(
            vl_spec,
            VlOpts {
                vl_version,
                config,
                theme,
                show_warnings,
                allowed_base_urls,
                format_locale,
                time_format_locale,
            },
            Some(scale),
            Some(ppi),
        )
        .await
    {
        Ok(png_data) => png_data,
        Err(err) => {
            bail!("Vega-Lite to PNG conversion failed: {}", err);
        }
    };

    // Write result
    write_output_binary(output, &png_data, "PNG")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_jpeg(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    scale: f32,
    quality: u8,
    show_warnings: bool,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
) -> Result<(), anyhow::Error> {
    // Parse version
    let vl_version = parse_vl_version(vl_version)?;

    // Read input file
    let vegalite_str = read_input_string(input)?;

    // Parse input as json
    let vl_spec = parse_as_json(&vegalite_str)?;

    // Load config from file
    let config = read_config_json(config)?;

    let format_locale = match &format_locale {
        None => None,
        Some(p) => Some(format_locale_from_str(p)?),
    };

    let time_format_locale = match &time_format_locale {
        None => None,
        Some(p) => Some(time_format_locale_from_str(p)?),
    };

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let jpeg_data = match converter
        .vegalite_to_jpeg(
            vl_spec,
            VlOpts {
                vl_version,
                config,
                theme,
                show_warnings,
                allowed_base_urls,
                format_locale,
                time_format_locale,
            },
            Some(scale),
            Some(quality),
        )
        .await
    {
        Ok(jpeg_data) => jpeg_data,
        Err(err) => {
            bail!("Vega-Lite to JPEG conversion failed: {}", err);
        }
    };

    // Write result
    write_output_binary(output, &jpeg_data, "JPEG")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_pdf(
    input: Option<&str>,
    output: Option<&str>,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    show_warnings: bool,
    allowed_base_urls: Option<Vec<String>>,
    format_locale: Option<String>,
    time_format_locale: Option<String>,
) -> Result<(), anyhow::Error> {
    // Parse version
    let vl_version = parse_vl_version(vl_version)?;

    // Read input file
    let vegalite_str = read_input_string(input)?;

    // Parse input as json
    let vl_spec = parse_as_json(&vegalite_str)?;

    // Load config from file
    let config = read_config_json(config)?;

    let format_locale = match &format_locale {
        None => None,
        Some(p) => Some(format_locale_from_str(p)?),
    };

    let time_format_locale = match &time_format_locale {
        None => None,
        Some(p) => Some(time_format_locale_from_str(p)?),
    };

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let pdf_data = match converter
        .vegalite_to_pdf(
            vl_spec,
            VlOpts {
                vl_version,
                config,
                theme,
                show_warnings,
                allowed_base_urls,
                format_locale,
                time_format_locale,
            },
        )
        .await
    {
        Ok(pdf_data) => pdf_data,
        Err(err) => {
            bail!("Vega-Lite to PDF conversion failed: {}", err);
        }
    };

    // Write result
    write_output_binary(output, &pdf_data, "PDF")?;

    Ok(())
}

async fn list_themes() -> Result<(), anyhow::Error> {
    // Initialize converter
    let mut converter = VlConverter::new();

    if let serde_json::Value::Object(themes) = converter.get_themes().await? {
        for theme in themes.keys().sorted() {
            println!("{}", theme)
        }
    } else {
        bail!("Failed to load themes")
    }

    Ok(())
}

async fn cat_theme(theme: &str) -> Result<(), anyhow::Error> {
    // Initialize converter
    let mut converter = VlConverter::new();

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

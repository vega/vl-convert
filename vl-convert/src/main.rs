#![doc = include_str!("../README.md")]

use clap::{arg, Parser, Subcommand};
use itertools::Itertools;
use std::path::Path;
use std::str::FromStr;
use vl_convert_rs::converter::{vega_to_url, vegalite_to_url, VgOpts, VlConverter, VlOpts};
use vl_convert_rs::module_loader::import_map::VlVersion;
use vl_convert_rs::text::register_font_directory;
use vl_convert_rs::{anyhow, anyhow::bail};

const DEFAULT_VL_VERSION: &str = "5.15";
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
    #[command(arg_required_else_help = true)]
    Vl2vg {
        /// Path to input Vega-Lite file
        #[arg(short, long)]
        input: String,

        /// Path to output Vega file to be created
        #[arg(short, long)]
        output: String,

        /// Vega-Lite Version. One of 4.17, 5.8, 5.9, 5.10, 5.11, 5.12, 5.13, 5.14, 5.15, 5.16
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
    #[command(arg_required_else_help = true)]
    Vl2svg {
        /// Path to input Vega-Lite file
        #[arg(short, long)]
        input: String,

        /// Path to output SVG file to be created
        #[arg(short, long)]
        output: String,

        /// Vega-Lite Version. One of 4.17, 5.8, 5.9, 5.10, 5.11, 5.12, 5.13, 5.14, 5.15, 5.16
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(short, long)]
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
    },

    /// Convert a Vega-Lite specification to an PNG image
    #[command(arg_required_else_help = true)]
    Vl2png {
        /// Path to input Vega-Lite file
        #[arg(short, long)]
        input: String,

        /// Path to output PNG file to be created
        #[arg(short, long)]
        output: String,

        /// Vega-Lite Version. One of 4.17, 5.8, 5.9, 5.10, 5.11, 5.12, 5.13, 5.14, 5.15, 5.16
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(short, long)]
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
    },

    /// Convert a Vega-Lite specification to an JPEG image
    #[command(arg_required_else_help = true)]
    Vl2jpeg {
        /// Path to input Vega-Lite file
        #[arg(short, long)]
        input: String,

        /// Path to output JPEG file to be created
        #[arg(short, long)]
        output: String,

        /// Vega-Lite Version. One of 4.17, 5.8, 5.9, 5.10, 5.11, 5.12, 5.13, 5.14, 5.15, 5.16
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(short, long)]
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
    },

    /// Convert a Vega-Lite specification to a PDF image
    #[command(arg_required_else_help = true)]
    Vl2pdf {
        /// Path to input Vega-Lite file
        #[arg(short, long)]
        input: String,

        /// Path to output PDF file to be created
        #[arg(short, long)]
        output: String,

        /// Vega-Lite Version. One of 4.17, 5.8, 5.9, 5.10, 5.11, 5.12, 5.13, 5.14, 5.15, 5.16
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(short, long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file. Defaults to ~/.config/vl-convert/config.json
        #[arg(short, long)]
        config: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// Whether to show Vega-Lite compilation warnings
        #[arg(long)]
        show_warnings: bool,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// Allowed base URL for external data requests. Default allows any base URL
        #[arg(short, long)]
        allowed_base_url: Option<Vec<String>>,
    },

    /// Convert a Vega-Lite specification to a URL that opens the chart in the Vega editor
    #[command(arg_required_else_help = true)]
    Vl2url {
        /// Path to input Vega-Lite file
        #[arg(short, long)]
        input: String,

        /// Open chart in fullscreen mode
        #[arg(long, default_value = "false")]
        fullscreen: bool,
    },

    /// Convert a Vega-Lite specification to an HTML file
    #[command(arg_required_else_help = true)]
    Vl2html {
        /// Path to input Vega-Lite file
        #[arg(short, long)]
        input: String,

        /// Path to output HTML file to be created
        #[arg(short, long)]
        output: String,

        /// Vega-Lite Version. One of 4.17, 5.8, 5.9, 5.10, 5.11, 5.12, 5.13, 5.14, 5.15, 5.16
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(short, long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file. Defaults to ~/.config/vl-convert/config.json
        #[arg(short, long)]
        config: Option<String>,

        /// Whether to bundle JavaScript dependencies in the HTML file
        /// instead of loading them from a CDN
        #[arg(short, long)]
        bundle: bool,
    },

    /// Convert a Vega specification to an SVG image
    #[command(arg_required_else_help = true)]
    Vg2svg {
        /// Path to input Vega file
        #[arg(short, long)]
        input: String,

        /// Path to output SVG file to be created
        #[arg(short, long)]
        output: String,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// Allowed base URL for external data requests. Default allows any base URL
        #[arg(short, long)]
        allowed_base_url: Option<Vec<String>>,
    },

    /// Convert a Vega specification to an PNG image
    #[command(arg_required_else_help = true)]
    Vg2png {
        /// Path to input Vega file
        #[arg(short, long)]
        input: String,

        /// Path to output PNG file to be created
        #[arg(short, long)]
        output: String,

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
    },

    /// Convert a Vega specification to an JPEG image
    #[command(arg_required_else_help = true)]
    Vg2jpeg {
        /// Path to input Vega file
        #[arg(short, long)]
        input: String,

        /// Path to output JPEG file to be created
        #[arg(short, long)]
        output: String,

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
    },

    /// Convert a Vega specification to an PDF image
    #[command(arg_required_else_help = true)]
    Vg2pdf {
        /// Path to input Vega file
        #[arg(short, long)]
        input: String,

        /// Path to output PDF file to be created
        #[arg(short, long)]
        output: String,

        /// Image scale factor
        #[arg(short, long, default_value = "1.0")]
        scale: f32,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,

        /// Allowed base URL for external data requests. Default allows any base URL
        #[arg(short, long)]
        allowed_base_url: Option<Vec<String>>,
    },

    /// Convert a Vega specification to a URL that opens the chart in the Vega editor
    #[command(arg_required_else_help = true)]
    Vg2url {
        /// Path to input Vega file
        #[arg(short, long)]
        input: String,

        /// Open chart in fullscreen mode
        #[arg(long, default_value = "false")]
        fullscreen: bool,
    },

    /// Convert a Vega specification to an HTML file
    #[command(arg_required_else_help = true)]
    Vg2html {
        /// Path to input Vega file
        #[arg(short, long)]
        input: String,

        /// Path to output HTML file to be created
        #[arg(short, long)]
        output: String,

        /// Whether to bundle JavaScript dependencies in the HTML file
        /// instead of loading them from a CDN
        #[arg(short, long)]
        bundle: bool,
    },

    /// Convert an SVG image to a PNG image
    #[command(arg_required_else_help = true)]
    Svg2png {
        /// Path to input SVG file
        #[arg(short, long)]
        input: String,

        /// Path to output PNG file to be created
        #[arg(short, long)]
        output: String,

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
    #[command(arg_required_else_help = true)]
    Svg2jpeg {
        /// Path to input SVG file
        #[arg(short, long)]
        input: String,

        /// Path to output JPEG file to be created
        #[arg(short, long)]
        output: String,

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
    #[command(arg_required_else_help = true)]
    Svg2pdf {
        /// Path to input SVG file
        #[arg(short, long)]
        input: String,

        /// Path to output PDF file to be created
        #[arg(short, long)]
        output: String,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

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
                &input_vegalite_file,
                &output_vega_file,
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
        } => {
            register_font_dir(font_dir)?;
            vl_2_svg(
                &input,
                &output,
                &vl_version,
                theme,
                config,
                show_warnings,
                allowed_base_url,
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
        } => {
            register_font_dir(font_dir)?;
            vl_2_png(
                &input,
                &output,
                &vl_version,
                theme,
                config,
                scale,
                ppi,
                show_warnings,
                allowed_base_url,
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
        } => {
            register_font_dir(font_dir)?;
            vl_2_jpeg(
                &input,
                &output,
                &vl_version,
                theme,
                config,
                scale,
                quality,
                show_warnings,
                allowed_base_url,
            )
            .await?
        }
        Vl2pdf {
            input,
            output,
            vl_version,
            theme,
            config,
            scale,
            show_warnings,
            font_dir,
            allowed_base_url,
        } => {
            register_font_dir(font_dir)?;
            vl_2_pdf(
                &input,
                &output,
                &vl_version,
                theme,
                config,
                scale,
                show_warnings,
                allowed_base_url,
            )
            .await?
        }
        Vl2url { input, fullscreen } => {
            let vl_str = read_input_string(&input)?;
            let vl_spec = serde_json::from_str(&vl_str)?;
            println!("{}", vegalite_to_url(&vl_spec, fullscreen)?)
        }
        Vl2html {
            input,
            output,
            vl_version,
            theme,
            config,
            bundle,
        } => {
            // Initialize converter
            let vl_str = read_input_string(&input)?;
            let vl_spec = serde_json::from_str(&vl_str)?;
            let config = read_config_json(config)?;
            let vl_version = parse_vl_version(&vl_version)?;

            let mut converter = VlConverter::new();
            let html = converter
                .vegalite_to_html(
                    vl_spec,
                    VlOpts {
                        config,
                        theme,
                        vl_version,
                        show_warnings: false,
                        allowed_base_urls: None
                    },
                    bundle,
                )
                .await?;
            write_output_string(&output, &html)?;
        }
        Vg2svg {
            input,
            output,
            font_dir,
            allowed_base_url,
        } => {
            register_font_dir(font_dir)?;
            vg_2_svg(&input, &output, allowed_base_url).await?
        }
        Vg2png {
            input,
            output,
            scale,
            ppi,
            font_dir,
            allowed_base_url,
        } => {
            register_font_dir(font_dir)?;
            vg_2_png(&input, &output, scale, ppi, allowed_base_url).await?
        }
        Vg2jpeg {
            input,
            output,
            scale,
            quality,
            font_dir,
            allowed_base_url,
        } => {
            register_font_dir(font_dir)?;
            vg_2_jpeg(&input, &output, scale, quality, allowed_base_url).await?
        }
        Vg2pdf {
            input,
            output,
            scale,
            font_dir,
            allowed_base_url,
        } => {
            register_font_dir(font_dir)?;
            vg_2_pdf(&input, &output, scale, allowed_base_url).await?
        }
        Vg2url { input, fullscreen } => {
            let vg_str = read_input_string(&input)?;
            let vg_spec = serde_json::from_str(&vg_str)?;
            println!("{}", vega_to_url(&vg_spec, fullscreen)?)
        }
        Vg2html {
            input,
            output,
            bundle,
        } => {
            // Initialize converter
            let vg_str = read_input_string(&input)?;
            let vg_spec = serde_json::from_str(&vg_str)?;

            let mut converter = VlConverter::new();
            let html = converter.vega_to_html(vg_spec, bundle).await?;
            write_output_string(&output, &html)?;
        }
        Svg2png {
            input,
            output,
            scale,
            ppi,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            let svg = read_input_string(&input)?;
            let png_data = vl_convert_rs::converter::svg_to_png(&svg, scale, Some(ppi))?;
            write_output_binary(&output, &png_data)?;
        }
        Svg2jpeg {
            input,
            output,
            scale,
            quality,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            let svg = read_input_string(&input)?;
            let jpeg_data = vl_convert_rs::converter::svg_to_jpeg(&svg, scale, Some(quality))?;
            write_output_binary(&output, &jpeg_data)?;
        }
        Svg2pdf {
            input,
            output,
            scale,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            let svg = read_input_string(&input)?;
            let pdf_data = vl_convert_rs::converter::svg_to_pdf(&svg, scale)?;
            write_output_binary(&output, &pdf_data)?;
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

fn read_input_string(input: &str) -> Result<String, anyhow::Error> {
    match std::fs::read_to_string(input) {
        Ok(input_str) => Ok(input_str),
        Err(err) => {
            bail!("Failed to read input file: {}\n{}", input, err);
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

fn write_output_string(output: &str, output_str: &str) -> Result<(), anyhow::Error> {
    match std::fs::write(output, output_str) {
        Ok(_) => Ok(()),
        Err(err) => {
            bail!("Failed to write converted output to {}\n{}", output, err);
        }
    }
}

fn write_output_binary(output: &str, output_data: &[u8]) -> Result<(), anyhow::Error> {
    match std::fs::write(output, output_data) {
        Ok(_) => Ok(()),
        Err(err) => {
            bail!("Failed to write converted output to {}\n{}", output, err);
        }
    }
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
    input: &str,
    output: &str,
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
                allowed_base_urls: None
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
            bail!(
                "Failed to serialize Vega spec to JSON string: {}",
                err.to_string()
            )
        }
    }

    Ok(())
}

async fn vg_2_svg(
    input: &str,
    output: &str,
    allowed_base_urls: Option<Vec<String>>,
) -> Result<(), anyhow::Error> {
    // Read input file
    let vega_str = read_input_string(input)?;

    // Parse input as json
    let vg_spec = parse_as_json(&vega_str)?;

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let svg = match converter
        .vega_to_svg(vg_spec, VgOpts { allowed_base_urls })
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
    input: &str,
    output: &str,
    scale: f32,
    ppi: f32,
    allowed_base_urls: Option<Vec<String>>,
) -> Result<(), anyhow::Error> {
    // Read input file
    let vega_str = read_input_string(input)?;

    // Parse input as json
    let vg_spec = parse_as_json(&vega_str)?;

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let png_data = match converter
        .vega_to_png(
            vg_spec,
            VgOpts { allowed_base_urls },
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
    write_output_binary(output, &png_data)?;

    Ok(())
}

async fn vg_2_jpeg(
    input: &str,
    output: &str,
    scale: f32,
    quality: u8,
    allowed_base_urls: Option<Vec<String>>,
) -> Result<(), anyhow::Error> {
    // Read input file
    let vega_str = read_input_string(input)?;

    // Parse input as json
    let vg_spec = parse_as_json(&vega_str)?;

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let jpeg_data = match converter
        .vega_to_jpeg(
            vg_spec,
            VgOpts { allowed_base_urls },
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
    write_output_binary(output, &jpeg_data)?;

    Ok(())
}

async fn vg_2_pdf(
    input: &str,
    output: &str,
    scale: f32,
    allowed_base_urls: Option<Vec<String>>,
) -> Result<(), anyhow::Error> {
    // Read input file
    let vega_str = read_input_string(input)?;

    // Parse input as json
    let vg_spec = parse_as_json(&vega_str)?;

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let pdf_data = match converter
        .vega_to_pdf(vg_spec, VgOpts { allowed_base_urls }, Some(scale))
        .await
    {
        Ok(pdf_data) => pdf_data,
        Err(err) => {
            bail!("Vega to PDF conversion failed: {}", err);
        }
    };

    // Write result
    write_output_binary(output, &pdf_data)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_svg(
    input: &str,
    output: &str,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    show_warnings: bool,
    allowed_base_urls: Option<Vec<String>>,
) -> Result<(), anyhow::Error> {
    // Parse version
    let vl_version = parse_vl_version(vl_version)?;

    // Read input file
    let vegalite_str = read_input_string(input)?;

    // Parse input as json
    let vl_spec = parse_as_json(&vegalite_str)?;

    // Load config from file
    let config = read_config_json(config)?;

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
    input: &str,
    output: &str,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    scale: f32,
    ppi: f32,
    show_warnings: bool,
    allowed_base_urls: Option<Vec<String>>,
) -> Result<(), anyhow::Error> {
    // Parse version
    let vl_version = parse_vl_version(vl_version)?;

    // Read input file
    let vegalite_str = read_input_string(input)?;

    // Parse input as json
    let vl_spec = parse_as_json(&vegalite_str)?;

    // Load config from file
    let config = read_config_json(config)?;

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
    write_output_binary(output, &png_data)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_jpeg(
    input: &str,
    output: &str,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    scale: f32,
    quality: u8,
    show_warnings: bool,
    allowed_base_urls: Option<Vec<String>>,
) -> Result<(), anyhow::Error> {
    // Parse version
    let vl_version = parse_vl_version(vl_version)?;

    // Read input file
    let vegalite_str = read_input_string(input)?;

    // Parse input as json
    let vl_spec = parse_as_json(&vegalite_str)?;

    // Load config from file
    let config = read_config_json(config)?;

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
    write_output_binary(output, &jpeg_data)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn vl_2_pdf(
    input: &str,
    output: &str,
    vl_version: &str,
    theme: Option<String>,
    config: Option<String>,
    scale: f32,
    show_warnings: bool,
    allowed_base_urls: Option<Vec<String>>,
) -> Result<(), anyhow::Error> {
    // Parse version
    let vl_version = parse_vl_version(vl_version)?;

    // Read input file
    let vegalite_str = read_input_string(input)?;

    // Parse input as json
    let vl_spec = parse_as_json(&vegalite_str)?;

    // Load config from file
    let config = read_config_json(config)?;

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
            },
            Some(scale),
        )
        .await
    {
        Ok(pdf_data) => pdf_data,
        Err(err) => {
            bail!("Vega-Lite to PDF conversion failed: {}", err);
        }
    };

    // Write result
    write_output_binary(output, &pdf_data)?;

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

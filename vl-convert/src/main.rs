#![doc = include_str!("../README.md")]

use clap::{arg, Parser, Subcommand};
use std::str::FromStr;
use vl_convert_rs::converter::VlConverter;
use vl_convert_rs::module_loader::import_map::VlVersion;
use vl_convert_rs::text::register_font_directory;
use vl_convert_rs::{anyhow, anyhow::bail};

const DEFAULT_VL_VERSION: &str = "5.5";

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

        /// Vega-Lite Version. One of 4.17, 5.0, 5.1, 5.2, 5.3, 5.4, 5.5
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Pretty-print JSON in output file
        #[arg(short, long)]
        pretty: bool,
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

        /// Vega-Lite Version. One of 4.17, 5.0, 5.1, 5.2, 5.3, 5.4, 5.5
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,
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

        /// Vega-Lite Version. One of 4.17, 5.0, 5.1, 5.2, 5.3, 5.4, 5.5
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Image scale factor
        #[arg(short, long, default_value = "1.0")]
        scale: f32,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,
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
        #[arg(short, long, default_value = "1.0")]
        scale: f32,

        /// Additional directory to search for fonts
        #[arg(long)]
        font_dir: Option<String>,
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
            pretty,
        } => vl_2_vg(&input_vegalite_file, &output_vega_file, &vl_version, pretty).await?,
        Vl2svg {
            input,
            output,
            vl_version,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            vl_2_svg(&input, &output, &vl_version).await?
        }
        Vl2png {
            input,
            output,
            vl_version,
            scale,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            vl_2_png(&input, &output, &vl_version, scale).await?
        }
        Vg2svg {
            input,
            output,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            vg_2_svg(&input, &output).await?
        }
        Vg2png {
            input,
            output,
            scale,
            font_dir,
        } => {
            register_font_dir(font_dir)?;
            vg_2_png(&input, &output, scale).await?
        }
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
    match std::fs::read_to_string(&input) {
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
    match std::fs::write(&output, output_str) {
        Ok(_) => Ok(()),
        Err(err) => {
            bail!("Failed to write converted output to {}\n{}", output, err);
        }
    }
}

fn write_output_binary(output: &str, output_data: &[u8]) -> Result<(), anyhow::Error> {
    match std::fs::write(&output, output_data) {
        Ok(_) => Ok(()),
        Err(err) => {
            bail!("Failed to write converted output to {}\n{}", output, err);
        }
    }
}

async fn vl_2_vg(
    input: &str,
    output: &str,
    vl_version: &str,
    pretty: bool,
) -> Result<(), anyhow::Error> {
    // Parse version
    let vl_version = parse_vl_version(vl_version)?;

    // Read input file
    let vegalite_str = read_input_string(input)?;

    // Parse input as json
    let vegalite_json = parse_as_json(&vegalite_str)?;

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let vega_json = match converter.vegalite_to_vega(vegalite_json, vl_version).await {
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

async fn vg_2_svg(input: &str, output: &str) -> Result<(), anyhow::Error> {
    // Read input file
    let vega_str = read_input_string(input)?;

    // Parse input as json
    let vg_spec = parse_as_json(&vega_str)?;

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let svg = match converter.vega_to_svg(vg_spec).await {
        Ok(svg) => svg,
        Err(err) => {
            bail!("Vega-Lite to Vega conversion failed: {}", err);
        }
    };

    // Write result
    write_output_string(output, &svg)?;

    Ok(())
}

async fn vg_2_png(input: &str, output: &str, scale: f32) -> Result<(), anyhow::Error> {
    // Read input file
    let vega_str = read_input_string(input)?;

    // Parse input as json
    let vg_spec = parse_as_json(&vega_str)?;

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let png_data = match converter.vega_to_png(vg_spec, Some(scale)).await {
        Ok(png_data) => png_data,
        Err(err) => {
            bail!("Vega-Lite to Vega conversion failed: {}", err);
        }
    };

    // Write result
    write_output_binary(output, &png_data)?;

    Ok(())
}

async fn vl_2_svg(input: &str, output: &str, vl_version: &str) -> Result<(), anyhow::Error> {
    // Parse version
    let vl_version = parse_vl_version(vl_version)?;

    // Read input file
    let vegalite_str = read_input_string(input)?;

    // Parse input as json
    let vl_spec = parse_as_json(&vegalite_str)?;

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let svg = match converter.vegalite_to_svg(vl_spec, vl_version).await {
        Ok(svg) => svg,
        Err(err) => {
            bail!("Vega-Lite to Vega conversion failed: {}", err);
        }
    };

    // Write result
    write_output_string(output, &svg)?;

    Ok(())
}

async fn vl_2_png(
    input: &str,
    output: &str,
    vl_version: &str,
    scale: f32,
) -> Result<(), anyhow::Error> {
    // Parse version
    let vl_version = parse_vl_version(vl_version)?;

    // Read input file
    let vegalite_str = read_input_string(input)?;

    // Parse input as json
    let vl_spec = parse_as_json(&vegalite_str)?;

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let png_data = match converter
        .vegalite_to_png(vl_spec, vl_version, Some(scale))
        .await
    {
        Ok(png_data) => png_data,
        Err(err) => {
            bail!("Vega-Lite to Vega conversion failed: {}", err);
        }
    };

    // Write result
    write_output_binary(output, &png_data)?;

    Ok(())
}

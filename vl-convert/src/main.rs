use clap::Parser;
use std::str::FromStr;
use vl_convert_rs::converter::VlConverter;
use vl_convert_rs::module_loader::import_map::VlVersion;

/// vl-convert: A utility for converting Vega-Lite specifications into Vega specification
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to input Vega-Lite file
    #[clap(short, long)]
    pub input_vegalite_file: String,

    /// Path to output Vega file to be created
    #[clap(short, long)]
    pub output_vega_file: String,

    /// Vega-Lite Version. One of 4.17, 5.0, 5.1, 5.2, 5.3, 5.4, 5.5
    #[clap(short, long, default_value = "5.5")]
    pub vl_version: String,

    /// Pretty-print JSON in output file
    #[clap(short, long)]
    pub pretty: bool,
}

#[tokio::main]
async fn main() {
    let args: Args = Args::parse();

    // Parse version
    let vl_version = if let Ok(vl_version) = VlVersion::from_str(&args.vl_version) {
        vl_version
    } else {
        println!(
            "Invalid or unsupported Vega-Lite version: {}",
            args.vl_version
        );
        return;
    };

    // Read input file
    let vegalite_str = match std::fs::read_to_string(&args.input_vegalite_file) {
        Ok(vegalite_str) => vegalite_str,
        Err(err) => {
            println!(
                "Failed to read input file: {}\n{}",
                args.input_vegalite_file, err
            );
            return;
        }
    };

    // Parse input as json
    let vegalite_json = match serde_json::from_str::<serde_json::Value>(&vegalite_str) {
        Ok(vegalite_json) => vegalite_json,
        Err(err) => {
            println!("Failed to parse input file as JSON: {}", err);
            return;
        }
    };

    // Initialize converter
    let mut converter = VlConverter::new();

    // Perform conversion
    let vega_str = match converter
        .vegalite_to_vega(vegalite_json, vl_version, args.pretty)
        .await
    {
        Ok(vega_str) => vega_str,
        Err(err) => {
            println!("Vega-Lite to Vega conversion failed: {}", err);
            return;
        }
    };

    // Write result
    match std::fs::write(&args.output_vega_file, vega_str) {
        Ok(_) => {}
        Err(err) => {
            println!(
                "Failed to write conversion output to {}\n{}",
                args.output_vega_file, err
            );
        }
    }
}

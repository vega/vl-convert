//! Experimental PGFPlots frontend. No TeX process is launched by this program.
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;

use clap::{Parser, ValueEnum};
use vl_convert_rs::{anyhow, svg_to_pgfplots, PgfplotsOpts, VgOpts, VlConverter, VlOpts, VlVersion};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum InputFormat { VegaLite, Vega, Svg }

#[derive(Parser)]
#[command(about = "Export static Vega geometry to PGFPlots (strict vector subset)")]
struct Args {
    /// Input file; omit or use '-' for stdin.
    #[arg(short, long)]
    input: Option<PathBuf>,
    /// Output file; omit or use '-' for stdout. Conversion completes before writing.
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "vega-lite")]
    input_format: InputFormat,
    /// Bundled Vega-Lite version, e.g. v6_4.
    #[arg(long)]
    vl_version: Option<String>,
    /// Include the standalone class and pgfplots package declarations.
    #[arg(long)]
    standalone: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut source = String::new();
    match &args.input {
        Some(path) if path.as_os_str() != "-" => source = std::fs::read_to_string(path)?,
        _ => { std::io::stdin().read_to_string(&mut source)?; }
    }
    let options = PgfplotsOpts { standalone: args.standalone };
    let tex = match args.input_format {
        InputFormat::Svg => {
            if args.vl_version.is_some() { anyhow::bail!("--vl-version only applies to Vega-Lite"); }
            svg_to_pgfplots(&source, options)?
        }
        format => {
            if matches!(format, InputFormat::Vega) && args.vl_version.is_some() {
                anyhow::bail!("--vl-version only applies to Vega-Lite");
            }
            let converter = VlConverter::new();
            let output = if matches!(format, InputFormat::Vega) {
                converter.vega_to_pgfplots(source, VgOpts::default(), options).await?
            } else {
                let vl_version = args.vl_version.as_deref()
                    .map(VlVersion::from_str).transpose()?.unwrap_or_default();
                converter.vegalite_to_pgfplots(source,
                    VlOpts { vl_version, ..Default::default() }, options).await?
            };
            for entry in output.logs { eprintln!("{}: {}", entry.level, entry.message); }
            output.tex
        }
    };
    match &args.output {
        Some(path) if path.as_os_str() != "-" => std::fs::write(path, tex)?,
        _ => std::io::stdout().lock().write_all(tex.as_bytes())?,
    }
    Ok(())
}

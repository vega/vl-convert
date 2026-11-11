//! Native PGFPlots frontend. This program does not invoke TeX.
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use vl_convert_rs::{anyhow, PgfplotsOpts, VlConverter, VlVersion, VlcConfig};

const MAX_INPUT_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Parser)]
#[command(about = "Compile inline Vega-Lite to native PGFPlots plots and axes")]
struct Args {
    /// Input JSON file; omit or use '-' for stdin.
    #[arg(short, long)]
    input: Option<PathBuf>,
    /// Output TeX file; omit or use '-' for stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Include document class and package declarations.
    #[arg(long)]
    standalone: bool,
    /// Treat labels as trusted TeX. Never use for untrusted specifications.
    #[arg(long)]
    tex_labels: bool,
    /// Select a bundled Vega-Lite version, for example v6_4.
    #[arg(long)]
    vl_version: Option<String>,
    /// Converter JSONC configuration, including resource and execution limits.
    #[arg(long)]
    vlc_config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let input: Box<dyn Read> = match &args.input {
        Some(path) if path.as_os_str() != "-" => Box::new(std::fs::File::open(path)?),
        _ => Box::new(std::io::stdin()),
    };
    let mut source = String::new();
    input
        .take(MAX_INPUT_BYTES + 1)
        .read_to_string(&mut source)?;
    if source.len() as u64 > MAX_INPUT_BYTES {
        anyhow::bail!("PGFPlots: input exceeds the 8 MiB limit");
    }
    let config = args
        .vlc_config
        .as_deref()
        .map(VlcConfig::from_file)
        .transpose()?
        .unwrap_or_default();
    let version = args
        .vl_version
        .as_deref()
        .map(VlVersion::from_str)
        .transpose()?
        .unwrap_or_default();
    let output = VlConverter::with_config(config)?
        .vegalite_to_pgfplots_with_version(
            source,
            version,
            PgfplotsOpts {
                standalone: args.standalone,
                tex_labels: args.tex_labels,
            },
        )
        .await?;
    for entry in &output.logs {
        eprintln!("{}: {}", entry.level, entry.message);
    }
    // Conversion errors cannot truncate an existing output file.
    match &args.output {
        Some(path) if path.as_os_str() != "-" => std::fs::write(path, output.tex)?,
        _ => std::io::stdout().lock().write_all(output.tex.as_bytes())?,
    }
    Ok(())
}

//! Native PGFPlots frontend. This program does not invoke TeX.
use std::io::{Read, Write};
use std::path::PathBuf;

use clap::Parser;
use vl_convert_rs::{anyhow, PgfplotsOpts, VlConverter};

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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let input: Box<dyn Read> = match &args.input {
        Some(path) if path.as_os_str() != "-" => Box::new(std::fs::File::open(path)?),
        _ => Box::new(std::io::stdin()),
    };
    let mut source = String::new();
    input.take(MAX_INPUT_BYTES + 1).read_to_string(&mut source)?;
    if source.len() as u64 > MAX_INPUT_BYTES {
        anyhow::bail!("PGFPlots: input exceeds the 8 MiB limit");
    }
    let output = VlConverter::new()
        .vegalite_to_pgfplots(source, PgfplotsOpts {
            standalone: args.standalone,
            tex_labels: args.tex_labels,
        })
        .await?;
    // Conversion errors cannot truncate an existing output file.
    match &args.output {
        Some(path) if path.as_os_str() != "-" => std::fs::write(path, output.tex)?,
        _ => std::io::stdout().lock().write_all(output.tex.as_bytes())?,
    }
    Ok(())
}

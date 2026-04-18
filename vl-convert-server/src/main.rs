mod settings;

use clap::Parser;
use settings::{resolve_settings, Cli};
use vl_convert_rs::anyhow;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    let resolved = resolve_settings(cli)?;

    vl_convert_server::init_tracing(&resolved.log_filter, resolved.serve_config.log_format);

    if let Some(ref dir) = resolved.font_dir {
        vl_convert_rs::text::register_font_directory(dir)?;
    }

    vl_convert_server::run(resolved.converter_config, resolved.serve_config).await
}

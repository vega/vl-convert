mod settings;

use clap::Parser;
use settings::{resolve_settings, Cli};
use std::time::Duration;
use vl_convert_rs::anyhow;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    let resolved = resolve_settings(cli)?;

    vl_convert_server::init_tracing(&resolved.log_filter, resolved.serve_config.log_format);

    if let Some(ref dir) = resolved.font_dir {
        vl_convert_rs::text::register_font_directory(dir)?;
    }

    let built = vl_convert_server::build_app(resolved.converter_config, &resolved.serve_config)?;

    let addr = if resolved.serve_config.host.contains(':') {
        format!(
            "[{}]:{}",
            resolved.serve_config.host, resolved.serve_config.port
        )
    } else {
        format!(
            "{}:{}",
            resolved.serve_config.host, resolved.serve_config.port
        )
    };
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let bound = listener.local_addr()?;

    if !bound.ip().is_loopback() && resolved.serve_config.api_key.is_none() {
        log::warn!(
            "Server binding to {bound} with no API key — accessible to any network client. \
             Set --api-key or VLC_API_KEY to restrict access."
        );
    }
    eprintln!("Listening on http://{bound}");
    log::info!("Listening on http://{bound}");

    let drain_secs = resolved.drain_timeout_secs;
    let (signal_tx, signal_rx) = tokio::sync::oneshot::channel::<()>();

    // Shutdown future passed to the library. Resolves on SIGTERM/ctrl-c
    // and notifies the drain watchdog below.
    let shutdown = async move {
        let ctrl_c = tokio::signal::ctrl_c();

        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to install SIGTERM handler");
            tokio::select! {
                _ = ctrl_c => log::info!("Received SIGINT, shutting down..."),
                _ = sigterm.recv() => log::info!("Received SIGTERM, shutting down..."),
            }
        }

        #[cfg(not(unix))]
        {
            ctrl_c.await.expect("failed to install Ctrl-C handler");
            log::info!("Received Ctrl-C, shutting down...");
        }

        let _ = signal_tx.send(());
    };

    // Drain watchdog. If graceful drain exceeds the deadline after a
    // signal fires, force-exit. Lives in the binary so the library
    // never calls `std::process::exit`.
    let watchdog = async move {
        if signal_rx.await.is_ok() {
            log::info!("Starting graceful drain ({drain_secs}s deadline)...");
            tokio::time::sleep(Duration::from_secs(drain_secs)).await;
            log::warn!("Drain timeout ({drain_secs}s) exceeded, forcing exit");
            std::process::exit(1);
        }
    };

    tokio::select! {
        result = vl_convert_server::serve(listener, built, shutdown) => result,
        _ = watchdog => unreachable!("watchdog exits the process before returning"),
    }
}

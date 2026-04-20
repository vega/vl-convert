mod settings;

use clap::Parser;
use settings::{resolve_settings, Cli};
use std::io::Write as _;
use std::time::Duration;
use vl_convert_rs::anyhow;
use vl_convert_server::{BoundListener, EndpointInfo, ListenAddr, ServeConfig};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    let resolved = resolve_settings(cli)?;

    vl_convert_server::init_tracing(&resolved.log_filter, resolved.serve_config.log_format);

    if let Some(ref dir) = resolved.font_dir {
        vl_convert_rs::text::register_font_directory(dir)?;
    }

    // Build the app (admin listener is bound inside build_app via the
    // shared `bind_listener` helper, applying the same probe-then-unlink
    // + socket-mode lifecycle).
    let built =
        vl_convert_server::build_app(resolved.converter_config, &resolved.serve_config).await?;

    // Bind the main listener via the library helper (identical UDS
    // lifecycle to admin: probe, unlink-if-stale, bind, chmod, register
    // cleanup guard).
    let listener: BoundListener = vl_convert_server::bind_listener(
        &resolved.serve_config.main,
        resolved.serve_config.socket_mode,
    )
    .await?;

    let endpoint = listener.endpoint_label();
    advise_listener_security(&listener, &resolved.serve_config);
    eprintln!("Listening on {endpoint}");
    log::info!("Listening on {endpoint}");

    // Emit one-shot readiness JSON on stdout after all listeners bound.
    // Must fire before any await that could cancel the process (stdin
    // watcher spawn, shutdown future setup) so parents blocked on
    // `read_line()` unblock promptly.
    emit_ready_json_if_enabled(resolved.ready_json, &listener, built.admin_endpoint_info())?;

    let drain_secs = resolved.drain_timeout_secs;
    let (signal_tx, signal_rx) = tokio::sync::oneshot::channel::<()>();

    // Decide whether to spawn the stdin-EOF watcher. Auto-enabled when
    // either listener is UDS (subprocess-style use case). Explicit
    // true/false from the user overrides auto-detection.
    let main_is_uds = matches!(resolved.serve_config.main, ListenAddr::Uds { .. });
    let admin_is_uds = matches!(resolved.serve_config.admin, Some(ListenAddr::Uds { .. }));
    let watcher_explicit = resolved.exit_on_parent_close;
    let watcher_enabled = match watcher_explicit {
        Some(v) => v,
        None => main_is_uds || admin_is_uds,
    };

    // Shared shutdown channel. Three producers can fire it: SIGINT,
    // SIGTERM (Unix), and the stdin-EOF watcher. One consumer: the
    // drain watchdog below.
    let (shutdown_trigger_tx, mut shutdown_trigger_rx) =
        tokio::sync::mpsc::channel::<&'static str>(4);

    // Install signal handlers *before* spawning any task that awaits
    // them. `tokio::signal::unix::signal(...)` registers the real
    // kernel handler on construction, not on first poll — deferring it
    // inside a spawned task creates a race where an early SIGTERM
    // takes the default disposition (terminate without cleanup).
    #[cfg(unix)]
    let mut sigterm_recv =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");

    {
        let tx = shutdown_trigger_tx.clone();
        tokio::spawn(async move {
            let ctrl_c = tokio::signal::ctrl_c();
            #[cfg(unix)]
            {
                tokio::select! {
                    _ = ctrl_c => { let _ = tx.send("SIGINT").await; }
                    _ = sigterm_recv.recv() => { let _ = tx.send("SIGTERM").await; }
                }
            }
            #[cfg(not(unix))]
            {
                let _ = ctrl_c.await;
                let _ = tx.send("Ctrl-C").await;
            }
        });
    }

    // Stdin-EOF watcher (conditional).
    if watcher_enabled {
        let tx = shutdown_trigger_tx.clone();
        let is_explicit = watcher_explicit == Some(true);
        tokio::spawn(async move {
            if let Some(reason) = watch_stdin_eof(is_explicit).await {
                let _ = tx.send(reason).await;
            }
        });
    }
    drop(shutdown_trigger_tx);

    // Aggregator: first trigger wins, fires the library's shutdown
    // future AND the drain watchdog's deadline.
    let shutdown = async move {
        if let Some(reason) = shutdown_trigger_rx.recv().await {
            log::info!("{reason} — initiating graceful shutdown");
        }
        let _ = signal_tx.send(());
    };

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

/// Log a security advisory at startup if the listener configuration
/// looks risky:
///
/// - TCP non-loopback + no API key → warn (the listener is reachable
///   to any network client).
/// - UDS `0600` + no API key → silent (the intended safe default;
///   filesystem permissions are the trust boundary).
/// - UDS with any group-permission bit set + no API key → warn
///   recommending an API key, since the socket grants access beyond
///   the owning uid.
/// - Any configuration with an API key set → silent.
fn advise_listener_security(main: &BoundListener, serve_config: &ServeConfig) {
    if serve_config.api_key.is_some() {
        return;
    }

    match main {
        BoundListener::Tcp(_) => {
            if !main.is_loopback() {
                let endpoint = main.endpoint_label();
                log::warn!(
                    "Server binding to {endpoint} with no API key — accessible to any \
                     network client. Set --api-key or VLC_API_KEY to restrict access."
                );
            }
        }
        #[cfg(unix)]
        BoundListener::Uds(..) => {
            // Owner-only (0o600 / 0o700): no warning. Any group-permission
            // bit set (0o040 read, 0o020 write, 0o010 exec): warn about
            // unintended access, since group-exec on a socket grants
            // connect-directory traversal.
            if serve_config.socket_mode & 0o070 != 0 {
                log::warn!(
                    "UDS socket mode {:o} grants group access and no API key is set. \
                     Consider tightening --socket-mode or setting --api-key.",
                    serve_config.socket_mode
                );
            }
        }
    }
}

/// Emit the readiness JSON signal on stdout. Exactly one line of
/// compact JSON terminated by `\n`, followed by an explicit flush so
/// parents blocked on `read_line()` unblock even through a
/// block-buffered pipe.
///
/// Emitted only when `--ready-json` / `VLC_READY_JSON` was set; when
/// disabled this function is a no-op so stdout stays silent for the
/// process lifetime.
///
/// # Schema
///
/// ```text
/// {
///   "ready":        true,
///   "version":      "<crate version>",
///   "pid":          <u32>,
///   "listen":       <endpoint-info>,
///   "admin_listen": <endpoint-info> | null
/// }
/// ```
///
/// `<endpoint-info>` is an internally-tagged object — the `transport`
/// field selects which other fields are present:
///
/// - **TCP**: `{"transport":"tcp","url":"http://<host>:<port>",`
///   `"host":"<ip>","port":<u16>}`
/// - **UDS** (Unix only):
///   `{"transport":"unix","url":"unix://<path>","path":"<path>"}`
///
/// `admin_listen` is `null` when `--admin-port` / `--admin-unix-socket`
/// were not set.
///
/// # Example (UDS main listener, no admin)
///
/// ```text
/// {"ready":true,"version":"2.0.0-rc1","pid":12345,"listen":{"transport":"unix","url":"unix:///tmp/vlc.sock","path":"/tmp/vlc.sock"},"admin_listen":null}
/// ```
///
/// # Single stdout writer
///
/// This is the only writer to stdout in the server binary. The
/// ast-grep rule `no-stray-stdout-writes` enforces the invariant at
/// lint time by flagging any `println!`/`print!` without the
/// `allow-ready-json-emitter` marker on the preceding line.
fn emit_ready_json_if_enabled(
    enabled: bool,
    main: &BoundListener,
    admin_endpoint: Option<EndpointInfo>,
) -> Result<(), anyhow::Error> {
    if !enabled {
        return Ok(());
    }
    let payload = serde_json::json!({
        "ready": true,
        "version": env!("CARGO_PKG_VERSION"),
        "pid": std::process::id(),
        "listen": main.endpoint_info(),
        "admin_listen": admin_endpoint,
    });
    let line = serde_json::to_string(&payload)
        .map_err(|e| anyhow::anyhow!("failed to serialize ready-JSON: {e}"))?;
    // `vl-convert-server`'s binary reserves stdout for this one-shot
    // readiness line. The ast-grep rule `no-stray-stdout-writes`
    // enforces the single-writer invariant by flagging any
    // `println!`/`print!` call in the crate without the marker below
    // on a preceding line. Do not add another stdout writer.
    //
    // allow-ready-json-emitter
    println!("{line}");
    // Explicit flush — stdout is block-buffered when piped to a parent
    // (the subprocess use case). Without this, a parent blocked on
    // read_line() could wait for the buffer to fill before ever
    // seeing the readiness signal.
    std::io::stdout()
        .flush()
        .map_err(|e| anyhow::anyhow!("failed to flush ready-JSON: {e}"))?;
    Ok(())
}

/// Watch inherited stdin for EOF. Returns `Some(reason)` when the
/// parent closes stdin (cleanly or by dying), signaling the caller to
/// trigger graceful shutdown. Returns `None` to silently disable the
/// watcher (e.g., first-read-is-EOF on an auto-enabled invocation with
/// `/dev/null`-redirected stdin).
///
/// `explicit = true` means the user set `--exit-on-parent-close` /
/// `VLC_EXIT_ON_PARENT_CLOSE=true`. In that case we honor their intent
/// even on a first-read-EOF. `explicit = false` means we auto-enabled
/// because a listener is UDS — there we degrade gracefully when stdin
/// is already gone (common under shell-backgrounded launches).
async fn watch_stdin_eof(explicit: bool) -> Option<&'static str> {
    use tokio::io::AsyncReadExt;
    let mut stdin = tokio::io::stdin();
    let mut buf = [0u8; 256];
    let mut first_read = true;
    loop {
        match stdin.read(&mut buf).await {
            Ok(0) if first_read && !explicit => {
                log::warn!(
                    "stdin already closed at startup; auto-enabled \
                     --exit-on-parent-close disabled. Pass \
                     --exit-on-parent-close=true to force."
                );
                return None;
            }
            Ok(0) => return Some("parent closed stdin"),
            Ok(_) => {
                // Discard bytes — we don't interpret stdin content, only EOF.
                first_read = false;
            }
            Err(e) => {
                log::debug!("stdin watcher read error: {e}; watcher disabled");
                return None;
            }
        }
    }
}

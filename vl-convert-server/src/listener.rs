//! Runtime listener abstractions — the post-bind counterpart to
//! [`crate::ListenAddr`]. The CLI/config layer works with `ListenAddr`
//! (a spec: "bind there"); once bound, we hold a [`BoundListener`] and
//! dispatch on its variant inside [`crate::serve`]. Keeping pre-bind
//! (`ListenAddr`) and post-bind (`BoundListener`) separate avoids
//! having to worry about pathname-vs-fd lifecycle at the config layer.
//!
//! Everything UDS-related is `#[cfg(unix)]`-gated end-to-end; on
//! Windows the `Uds` variant does not exist at all, so the `match`
//! arms reduce to one case (TCP) and clippy won't complain.

use std::future::Future;

#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};

/// Pre-bound listener ready to hand to [`crate::serve`].
///
/// The UDS variant carries a [`UdsCleanup`] guard so the socket file
/// is unlinked when the listener drops. Force-exit via the drain
/// watchdog bypasses `Drop`; [`bind_listener`]'s probe-then-unlink on
/// the next launch clears any stale file left behind.
pub enum BoundListener {
    Tcp(tokio::net::TcpListener),
    #[cfg(unix)]
    Uds(tokio::net::UnixListener, UdsCleanup),
}

impl BoundListener {
    /// Human-readable endpoint for log lines / readiness JSON.
    /// Matches [`crate::ListenAddr::Display`] so callers can round-trip
    /// the value into `--ready-json` without a second formatter.
    pub fn endpoint_label(&self) -> String {
        match self {
            Self::Tcp(l) => l
                .local_addr()
                .map(|a| {
                    if a.is_ipv6() {
                        format!("http://[{}]:{}", a.ip(), a.port())
                    } else {
                        format!("http://{}", a)
                    }
                })
                .unwrap_or_else(|_| "http://<unknown>".to_string()),
            #[cfg(unix)]
            Self::Uds(_, cleanup) => format!("unix://{}", cleanup.path.display()),
        }
    }

    /// Structured endpoint descriptor for readiness JSON. Gives wrappers
    /// direct `host`/`port` (TCP) or `path` (UDS) fields alongside the
    /// URL string, so they don't have to URL-parse `endpoint_label`.
    pub fn endpoint_info(&self) -> EndpointInfo {
        match self {
            Self::Tcp(l) => {
                let url = self.endpoint_label();
                match l.local_addr() {
                    Ok(a) => EndpointInfo::Tcp {
                        url,
                        host: a.ip().to_string(),
                        port: a.port(),
                    },
                    Err(_) => EndpointInfo::Tcp {
                        url,
                        host: String::new(),
                        port: 0,
                    },
                }
            }
            #[cfg(unix)]
            Self::Uds(_, cleanup) => EndpointInfo::Unix {
                url: self.endpoint_label(),
                path: cleanup.path.to_string_lossy().into_owned(),
            },
        }
    }

    /// True if the listener is only reachable from processes on the
    /// same host. TCP checks `local_addr().is_loopback()`; UDS is
    /// always local-only.
    pub fn is_loopback(&self) -> bool {
        match self {
            Self::Tcp(l) => l
                .local_addr()
                .map(|a| a.ip().is_loopback())
                .unwrap_or(false),
            #[cfg(unix)]
            Self::Uds(..) => true,
        }
    }
}

/// Structured form of [`BoundListener`] for wire emission (ready-JSON).
/// Serialized with an internal `transport` tag: TCP entries get
/// `{transport:"tcp", url, host, port}`, UDS entries get
/// `{transport:"unix", url, path}`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "transport", rename_all = "lowercase")]
pub enum EndpointInfo {
    Tcp {
        url: String,
        host: String,
        port: u16,
    },
    #[cfg(unix)]
    Unix {
        url: String,
        path: String,
    },
}

/// Drop guard that unlinks a pathname UDS file when the listener is
/// dropped. The `AtomicBool` gives a one-shot semantic so a manual
/// cleanup followed by `Drop` doesn't log spurious errors.
///
/// Force-exit (e.g., drain watchdog calling `std::process::exit`)
/// bypasses `Drop` entirely; the next launch's probe-then-unlink in
/// [`bind_listener`] handles any stale file left behind.
#[cfg(unix)]
pub struct UdsCleanup {
    pub path: PathBuf,
    active: AtomicBool,
}

#[cfg(unix)]
impl UdsCleanup {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            active: AtomicBool::new(true),
        }
    }
}

#[cfg(unix)]
impl Drop for UdsCleanup {
    fn drop(&mut self) {
        if self.active.swap(false, Ordering::SeqCst) {
            if let Err(e) = std::fs::remove_file(&self.path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("UDS cleanup failed for {:?}: {e}", self.path);
                }
            }
        }
    }
}

/// Connection metadata extracted once per accepted UDS connection.
/// Middleware and handlers pull this via
/// `axum::extract::Extension<axum::extract::ConnectInfo<UdsConnectInfo>>`
/// when the request originates from a UDS listener.
///
/// `peer_cred` is `Option<UCred>` (not `UCred`) because the syscall is
/// observability-only — filesystem permissions already enforce uid
/// access at bind time. A missing `UCred` degrades the tracing span
/// rather than dropping the request, which keeps the extractor safe
/// against sandboxed kernels or future tokio versions that may probe
/// fields unavailable in restricted environments.
#[cfg(unix)]
#[derive(Clone, Debug)]
pub(crate) struct UdsConnectInfo {
    pub peer_addr: std::sync::Arc<tokio::net::unix::SocketAddr>,
    pub peer_cred: Option<tokio::net::unix::UCred>,
}

#[cfg(unix)]
impl axum::extract::connect_info::Connected<&tokio::net::UnixStream> for UdsConnectInfo {
    fn connect_info(stream: &tokio::net::UnixStream) -> Self {
        let peer_addr = stream
            .peer_addr()
            .expect("UnixStream::peer_addr on a just-accepted socket cannot fail");
        let peer_cred = stream.peer_cred().ok();
        Self {
            peer_addr: std::sync::Arc::new(peer_addr),
            peer_cred,
        }
    }
}

/// Manual accept loop for UDS listeners. axum 0.7 has no generic
/// `Listener` trait (that landed in 0.8), so `axum::serve` can't take a
/// `UnixListener` directly. This helper mirrors the canonical
/// axum-v0.7.9 `unix-domain-socket` example, adapted to:
///
/// - Track in-flight connections in a [`tokio::task::JoinSet`] so
///   graceful shutdown drains properly (mirrors the TCP path in
///   [`crate::serve`]).
/// - Accept an arbitrary `shutdown` future, matching `axum::serve`'s
///   `.with_graceful_shutdown(..)` contract.
/// - Use `UdsConnectInfo` so downstream middleware/handlers can access
///   peer credentials when needed.
#[cfg(unix)]
pub(crate) async fn serve_uds(
    listener: tokio::net::UnixListener,
    router: axum::Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), std::io::Error> {
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use hyper_util::server::conn::auto::Builder;
    use tokio::task::JoinSet;
    use tokio_util::sync::CancellationToken;
    use tower::Service;

    let token = CancellationToken::new();
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        shutdown.await;
        shutdown_token.cancel();
    });

    let mut make_service = router.into_make_service_with_connect_info::<UdsConnectInfo>();
    let mut conn_tasks: JoinSet<()> = JoinSet::new();

    loop {
        tokio::select! {
            _ = token.cancelled() => break,
            accept = listener.accept() => {
                let (socket, _peer_addr) = accept?;
                let tower_svc = unwrap_infallible(make_service.call(&socket).await);
                let conn_token = token.clone();
                conn_tasks.spawn(async move {
                    let io = TokioIo::new(socket);
                    let hyper_svc = hyper::service::service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                        tower_svc.clone().call(req)
                    });
                    let builder = Builder::new(TokioExecutor::new());
                    let conn = builder.serve_connection_with_upgrades(io, hyper_svc);
                    tokio::pin!(conn);
                    tokio::select! {
                        res = conn.as_mut() => {
                            if let Err(e) = res {
                                log::debug!("UDS connection error: {e:#}");
                            }
                        }
                        _ = conn_token.cancelled() => {
                            conn.as_mut().graceful_shutdown();
                            let _ = conn.await;
                        }
                    }
                });
            }
        }
    }

    while conn_tasks.join_next().await.is_some() {}
    Ok(())
}

#[cfg(unix)]
fn unwrap_infallible<T>(r: Result<T, std::convert::Infallible>) -> T {
    match r {
        Ok(v) => v,
        Err(i) => match i {},
    }
}

/// Bind a listener according to a [`ListenAddr`] spec, applying the
/// full lifecycle contract:
///
/// - **TCP**: `tokio::net::TcpListener::bind(host:port)`. No cleanup
///   obligations; `BoundListener::Tcp` drops naturally.
/// - **UDS** (`#[cfg(unix)]` only): probe the path with a 100ms
///   `UnixStream::connect` timeout, unlink only stale ECONNREFUSED
///   sockets, bind, apply `PermissionsExt::set_mode(mode)` immediately
///   (no await between bind and chmod), register a [`UdsCleanup`]
///   guard for unlink-on-drop.
///
/// Used by both `main.rs` (main listener) and `lib::build_app` (admin
/// listener) so the lifecycle semantics are identical.
///
/// The probe→unlink→bind sequence is **not** atomic — a competing
/// process could win between unlink and bind, returning `EADDRINUSE`.
/// That's surfaced as the returned error; retrying would only widen
/// the race window. Force-exit via the drain watchdog skips the `Drop`
/// guard; the next launch's probe handles the stale file.
pub async fn bind_listener(
    spec: &crate::ListenAddr,
    #[cfg_attr(not(unix), allow(unused_variables))] mode: u32,
) -> Result<BoundListener, vl_convert_rs::anyhow::Error> {
    use vl_convert_rs::anyhow::anyhow;
    match spec {
        crate::ListenAddr::Tcp { host, port } => {
            let addr = if host.contains(':') {
                format!("[{host}]:{port}")
            } else {
                format!("{host}:{port}")
            };
            let l = tokio::net::TcpListener::bind(&addr)
                .await
                .map_err(|e| anyhow!("Failed to bind TCP {addr}: {e}"))?;
            Ok(BoundListener::Tcp(l))
        }
        #[cfg(unix)]
        crate::ListenAddr::Uds { path } => {
            probe_then_unlink(path).await?;
            let l = tokio::net::UnixListener::bind(path)
                .map_err(|e| anyhow!("Failed to bind UDS {}: {e}", path.display()))?;
            // Chmod synchronously after bind — no await between or the
            // umask-race window widens. `PermissionsExt::set_mode` is a
            // sync syscall.
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
                .map_err(|e| anyhow!("Failed to set mode on UDS {}: {e}", path.display()))?;
            let cleanup = UdsCleanup::new(path.clone());
            Ok(BoundListener::Uds(l, cleanup))
        }
    }
}

/// Probe the socket path to distinguish "live server" / "stale file" /
/// "no file." Called before `bind` on every UDS listener setup.
///
/// - `Ok(Ok(_))` within timeout → a live server answered; refuse to
///   stomp. Exit non-zero.
/// - `Ok(Err(ECONNREFUSED))` → the file exists but nothing's listening;
///   it's a stale leftover from a crashed previous run. Remove it so
///   `bind` can succeed.
/// - `Ok(Err(ENOENT))` → the path doesn't exist yet; straight to bind.
/// - Any other `Err` → surface as a bind failure (don't unlink —
///   preserves non-socket files at the path).
/// - Timeout → treat as "can't tell"; fail the bind rather than risk
///   clobbering a slow live peer.
#[cfg(unix)]
async fn probe_then_unlink(path: &std::path::Path) -> Result<(), vl_convert_rs::anyhow::Error> {
    use vl_convert_rs::anyhow::{anyhow, bail};

    let probe = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        tokio::net::UnixStream::connect(path),
    )
    .await;
    match probe {
        Ok(Ok(_)) => bail!(
            "UDS socket {} is in use by another process; refusing to stomp",
            path.display()
        ),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            // Stale socket file from a previous run. Safe to remove.
            std::fs::remove_file(path)
                .map_err(|e| anyhow!("Failed to remove stale UDS {}: {e}", path.display()))?;
            Ok(())
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            // Path doesn't exist yet. Bind will create it.
            Ok(())
        }
        Ok(Err(e)) => bail!("Unexpected error probing UDS path {}: {e}", path.display()),
        Err(_elapsed) => bail!(
            "Timed out probing UDS path {} for an existing listener",
            path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn uds_cleanup_unlinks_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.sock");
        std::fs::write(&path, b"stand-in").unwrap();
        assert!(path.exists());
        {
            let _guard = UdsCleanup::new(path.clone());
        }
        assert!(!path.exists(), "UdsCleanup::Drop should have unlinked");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn uds_cleanup_is_one_shot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.sock");
        std::fs::write(&path, b"stand-in").unwrap();
        let guard = UdsCleanup::new(path.clone());
        drop(guard);
        assert!(!path.exists());
        // Second drop would be a no-op — create a second path, write a
        // file, then construct+drop another guard to confirm the
        // Ordering::SeqCst swap works correctly in isolation.
        let path2 = dir.path().join("u.sock");
        std::fs::write(&path2, b"stand-in").unwrap();
        let guard2 = UdsCleanup::new(path2.clone());
        drop(guard2);
        assert!(!path2.exists());
    }
}

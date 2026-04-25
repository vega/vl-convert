//! Canonical internal form for "where does a listener bind."
//!
//! The CLI surface exposes two parallel flag families per listener —
//! TCP (`--host`/`--port`/`--admin-port`) and UDS
//! (`--unix-socket`/`--admin-unix-socket`). The resolve layer
//! synthesises one of these variants into a [`ListenAddr`] so the rest
//! of the server (bind helper, readiness emitter, tracing spans) sees
//! a single abstraction.
//!
//! [`Display`](std::fmt::Display) produces the URL wire-form that
//! `--ready-json` and tracing spans emit: `http://HOST:PORT` for TCP
//! and `unix:///ABS/PATH` for UDS.
//!
//! Lives at the library crate root — not under `settings/` — because
//! `ServeConfig` in `config.rs` holds a `ListenAddr` field and
//! `settings/` is a binary-only module tree unreachable from the
//! library. The settings layer re-imports via `crate::ListenAddr`.

use std::fmt;
#[cfg(unix)]
use std::path::PathBuf;

/// A pre-resolved listener binding target. TCP is cross-platform;
/// `Uds` is `#[cfg(unix)]`-gated end-to-end and does not exist at all
/// on Windows (so every match needs at most one arm there).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenAddr {
    Tcp {
        host: String,
        port: u16,
    },
    #[cfg(unix)]
    Uds {
        path: PathBuf,
    },
}

impl ListenAddr {
    /// Convenience constructor for the common loopback-TCP case used
    /// by the admin listener default and several tests.
    pub fn loopback_tcp(port: u16) -> Self {
        Self::Tcp {
            host: "127.0.0.1".to_string(),
            port,
        }
    }
}

impl fmt::Display for ListenAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // IPv6 literal → bracketed authority, per RFC 3986.
            Self::Tcp { host, port } if host.contains(':') => {
                write!(f, "http://[{host}]:{port}")
            }
            Self::Tcp { host, port } => write!(f, "http://{host}:{port}"),
            #[cfg(unix)]
            Self::Uds { path } => write!(f, "unix://{}", path.display()),
        }
    }
}

/// Rejection message emitted by `parse_socket_path_arg` on Windows.
/// Lives here so both `--unix-socket` and `--admin-unix-socket` use
/// the same wording (and so the subprocess e2e tests on the Windows
/// CI leg can assert on it).
#[cfg(windows)]
pub(crate) const WINDOWS_UDS_REJECTION: &str =
    "--unix-socket PATH listeners are not supported on Windows. \
     Use --port PORT instead.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_tcp_ipv4_emits_http_url() {
        let addr = ListenAddr::Tcp {
            host: "127.0.0.1".to_string(),
            port: 3000,
        };
        assert_eq!(addr.to_string(), "http://127.0.0.1:3000");
    }

    #[test]
    fn display_tcp_ipv6_emits_bracketed_authority() {
        let addr = ListenAddr::Tcp {
            host: "::1".to_string(),
            port: 8080,
        };
        assert_eq!(addr.to_string(), "http://[::1]:8080");
    }

    #[test]
    fn display_tcp_hostname_preserves_literal() {
        let addr = ListenAddr::Tcp {
            host: "my.host".to_string(),
            port: 80,
        };
        assert_eq!(addr.to_string(), "http://my.host:80");
    }

    #[cfg(unix)]
    #[test]
    fn display_uds_emits_unix_url() {
        let addr = ListenAddr::Uds {
            path: PathBuf::from("/tmp/vlc.sock"),
        };
        assert_eq!(addr.to_string(), "unix:///tmp/vlc.sock");
    }

    #[test]
    fn loopback_tcp_helper_is_127_0_0_1() {
        let addr = ListenAddr::loopback_tcp(9000);
        assert!(matches!(
            addr,
            ListenAddr::Tcp { ref host, port: 9000 } if host == "127.0.0.1"
        ));
    }
}

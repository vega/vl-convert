//! Internal form for listener bind targets.
//!
//! The CLI surface exposes two parallel flag families per listener:
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

    /// True when the bind target is local-only: UDS or a TCP IP literal
    /// that is loopback. Used by
    /// `validate_serve_config` to enforce the admin-without-key rule and
    /// by callers that need a transport-level trust-boundary check.
    ///
    /// The TCP arm parses `host` as an `IpAddr` and asks the OS view of
    /// loopback status. Hostnames (e.g. `localhost`) deliberately read as
    /// non-loopback because this type does not resolve DNS; callers
    /// should pass `127.0.0.1` or `::1` when loopback semantics matter.
    pub fn is_loopback_or_uds(&self) -> bool {
        match self {
            Self::Tcp { host, .. } => host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback()),
            #[cfg(unix)]
            Self::Uds { .. } => true,
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

    #[test]
    fn is_loopback_or_uds_tcp_table() {
        // Hostnames are not resolved by this type.
        for (host, expected) in [
            ("127.0.0.1", true),
            ("::1", true),
            ("0.0.0.0", false),
            ("localhost", false),
        ] {
            let addr = ListenAddr::Tcp {
                host: host.to_string(),
                port: 0,
            };
            assert_eq!(addr.is_loopback_or_uds(), expected, "host = {host:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn is_loopback_or_uds_treats_uds_as_local() {
        let addr = ListenAddr::Uds {
            path: PathBuf::from("/tmp/x.sock"),
        };
        assert!(addr.is_loopback_or_uds());
    }
}

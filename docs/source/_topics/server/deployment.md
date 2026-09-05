---
title: Server Deployment Profiles
path: deployment
section: Server
order: 440
interfaces: [server]
---

<!-- topic-body -->

# Deploying the Server

Choose a profile from the trust boundary around the service. The examples are
starting points. Tune worker counts and limits with representative charts.

Put a reverse proxy or platform load balancer in front of a TCP deployment to
provide TLS, connection controls, and application-specific identity. Keep the
admin listener on a management-only address or a Unix domain socket.

## Private Backend Service

Use a bearer token when known backend services call the converter. Supply
`VLC_API_KEY` through the deployment's secret manager before starting this
command:

```bash
vl-convert \
  --base-url disabled \
  --allowed-base-urls https://data.example.com/ \
  --max-v8-heap-size-mb 512 \
  --max-v8-execution-time-secs 10 \
  --log-format json \
  serve \
  --host 127.0.0.1 \
  --port 3000 \
  --workers 2 \
  --max-concurrent-requests 4 \
  --request-timeout-secs 15 \
  --max-body-size-mb 8 \
  --opaque-errors
```

Bind to a private network address instead of loopback when the reverse proxy
or caller runs on another host. Do not expose this listener without TLS at the
network edge.

## Intentionally Anonymous Browser Service

A browser cannot keep a shared API key secret. If a tool must accept anonymous
internet requests, use strict access and resource controls:

```bash
vl-convert \
  --base-url disabled \
  --allowed-base-urls none \
  --max-v8-heap-size-mb 512 \
  --max-v8-execution-time-secs 10 \
  --missing-fonts warn \
  --log-format json \
  serve \
  --host 0.0.0.0 \
  --port 3000 \
  --workers 2 \
  --max-concurrent-requests 4 \
  --request-timeout-secs 15 \
  --max-body-size-mb 4 \
  --per-ip-budget-ms 5000 \
  --global-budget-ms 30000 \
  --cors-origin https://editor.example.com \
  --opaque-errors
```

CORS only controls browser access to responses. Keep network-level rate
limits, abuse monitoring, and egress restrictions in front of the process.
Leave automatic Google Fonts and per-request plugins disabled unless the
product needs them and has tighter controls for their cost and risk.

If a trusted reverse proxy supplies client IP headers, add `--trust-proxy`
only after configuring the proxy to strip inbound forwarded headers and write
its own.

## Local Subprocess or Sidecar

A Unix domain socket avoids opening a TCP port and restricts access with
filesystem permissions:

```bash
vl-convert serve \
  --unix-socket /run/myapp/vl-convert.sock \
  --admin-unix-socket /run/myapp/vl-convert-admin.sock \
  --socket-mode 0600 \
  --ready-json
```

`--ready-json` writes one machine-readable line to standard output after the
listeners bind. With a Unix socket listener, the server also exits when the
parent process closes its standard input; `--exit-on-parent-close` turns that
behavior on or off explicitly. Per-IP budgets do not apply to Unix sockets,
which have no client IP, so use a global budget when a sidecar has several
callers.

## Health and Shutdown

Use `/healthz` for liveness and `/readyz` for readiness. Readiness runs a
cached converter check and reports `503` during live reconfiguration. `/infoz`
reports component versions, the local timezone, and the Google Fonts cache
location. All three routes skip authentication, so filter `/infoz` at the proxy
if those host details should not be public.

The server drains in-flight requests during shutdown. `--drain-timeout-secs`
bounds how long shutdown waits, and the separate `--reconfig-drain-timeout-secs`
bounds live configuration changes.

See {doc}`authentication`, {doc}`rate-limiting`, and {doc}`guides/security`
for the controls these profiles use.

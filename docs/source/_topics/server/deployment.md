---
title: Server Deployment Profiles
path: deployment
section: Server
order: 410
interfaces: [server]
---

<!-- topic-body -->

# Deploying the Server

Choose a profile from the trust boundary around the service. The examples are
starting points. Tune worker count and limits with representative charts.

Put a reverse proxy or platform load balancer in front of a TCP deployment to
provide TLS, connection controls, and application-specific identity. Keep the
admin listener on a management-only address or Unix domain socket.

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

Bind to a private network address instead of loopback when the reverse proxy or
caller runs on another host. Do not expose this HTTP listener without TLS at
the network edge.

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
product requires them and has tighter controls for their cost and risk.

If a trusted reverse proxy supplies client IP headers, add `--trust-proxy`
only after configuring the proxy to remove inbound forwarded headers and write
its own.

## Local Subprocess or Sidecar

A Unix domain socket avoids opening a TCP port and can restrict access with
filesystem permissions:

```bash
vl-convert serve \
  --unix-socket /run/myapp/vl-convert.sock \
  --admin-unix-socket /run/myapp/vl-convert-admin.sock \
  --socket-mode 0600 \
  --ready-json \
  --exit-on-parent-close
```

`--ready-json` writes one machine-readable line after the listeners bind.
`--exit-on-parent-close` lets a parent process stop the sidecar by closing its
standard input. Per-IP budgets do not apply to Unix sockets because they have
no client IP, so use a global budget when a sidecar has multiple callers.

## Health and Shutdown

Use `/healthz` for a basic liveness check and `/readyz` for readiness.
Readiness performs a cached converter check and reports `503` during live
reconfiguration. `/infoz` reports component versions, the local timezone, and
the Google Fonts cache location. All three routes are unauthenticated on the
main listener, so filter `/infoz` at the proxy if those host details should not
be public.

The server drains requests during normal shutdown. Set
`--drain-timeout-secs` to bound how long shutdown waits. A separate
`--reconfig-drain-timeout-secs` controls live configuration changes.

See {doc}`authentication`, {doc}`rate-limiting`, and
{doc}`guides/security` for the controls used by these profiles.

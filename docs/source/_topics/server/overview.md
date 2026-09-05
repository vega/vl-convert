---
title: Server Overview
path: overview
section: Server
order: 400
interfaces: [server]
---

<!-- topic-body -->

# Server Overview

`vl-convert serve` makes VlConvert available over HTTP. Use it when several
applications need one rendering service, when a non-Python and non-Rust
application needs conversion, or when rendering should run outside the caller's
process.

For a single script or an application that can embed the Python or Rust
library, those interfaces usually require less operational work.

## Start a Local Server

```bash
vl-convert serve --host 127.0.0.1 --port 3000
```

The main listener provides these endpoint groups:

| Path | Purpose |
| --- | --- |
| `/vegalite/*` | Compile and convert Vega-Lite input |
| `/vega/*` | Convert Vega input |
| `/svg/*` | Convert existing SVG |
| `/themes` | List and inspect themes |
| `/bundling/*` | Build browser JavaScript bundles |
| `/healthz`, `/readyz`, `/infoz` | Process and readiness information |

See {doc}`getting-started/quick-start` for a complete request and
{doc}`api-reference` for every endpoint.

The default server binds to loopback and is intended for local use. It does not
require authentication, enforce render-time budgets, cap concurrent requests,
or hide error details until those options are configured.

## Main and Admin Listeners

Conversion and health routes use the main listener. Optional runtime management
routes use a separate admin listener. Keeping them separate lets a deployment
expose rendering without exposing configuration, budget, font-cache, or worker
controls.

Do not publish the admin listener to the internet. Bind it to loopback, a
private management network, or a Unix domain socket, and configure a separate
admin API key.

## Before Production

For specifications that are not fully trusted:

- restrict data and image access
- keep caller-supplied plugins disabled
- set JavaScript heap and execution-time limits
- set request body, timeout, concurrency, and render-time limits
- put the service behind TLS and an authentication layer when access is not
  intentionally anonymous
- return opaque errors to clients and retain detailed structured logs
- enable proxy-derived client addresses only behind a proxy that rewrites those
  headers

CORS controls which browser origins can read responses. It is not
authentication and does not stop non-browser clients.

Use {doc}`deployment` for concrete deployment profiles,
{doc}`authentication` for listener credentials, and
{doc}`rate-limiting` for render-time budgets.

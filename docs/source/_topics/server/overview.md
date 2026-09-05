---
title: Server Overview
path: overview
section: Server
order: 400
interfaces: [server]
---

<!-- topic-body -->

# Server Overview

`vl-convert serve` exposes VlConvert over HTTP. Use it when several
applications share one rendering service, when the calling application is not
written in Python or Rust, or when rendering should run outside the caller's
process.

## Compare the Interfaces

All four interfaces run the same conversion engine and produce the same output.
They differ in where rendering runs and what you have to operate.

| Interface | Where rendering runs | Choose it when |
| --- | --- | --- |
| Python | Inside the Python process | The caller is Python, including Altair workflows |
| Rust | Inside the Rust process | The caller is a Rust application |
| CLI | A short-lived process per command | You convert files from shell scripts or build pipelines |
| Server | A long-running HTTP service | Callers use other languages, several services share one renderer, or rendering must be isolated from the caller |

The libraries add no network hop and nothing to deploy, but they load the
JavaScript runtime into the caller's process. The server keeps that runtime in
its own process, which you then have to secure, monitor, and scale. Prefer a
library when the caller can embed one.

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
| `/healthz`, `/readyz`, `/infoz` | Liveness, readiness, and version information |
| `/docs`, `/api-doc/openapi.json` | Interactive API documentation and the OpenAPI document |

See {doc}`getting-started/quick-start` for a complete request and
{doc}`api-reference` for every endpoint.

By default the server binds to loopback, allows 30 seconds per request, accepts
request bodies up to 50 MB, and accepts browser requests from loopback origins
only. It does not require authentication, enforce render-time budgets, cap
concurrent requests, or hide error details until you configure those options.

## Main and Admin Listeners

Conversion and health routes use the main listener. Optional runtime
management routes use a separate admin listener, so a deployment can expose
rendering without exposing configuration, budget, font-cache, or worker
controls.

Never publish the admin listener to the internet. Bind it to loopback, a
private management network, or a Unix domain socket, and give it its own API
key. See {doc}`admin-api`.

## Before Production

For specifications that are not fully trusted:

- restrict data and image access
- keep caller-supplied plugins disabled
- set JavaScript heap and execution-time limits
- set request body, timeout, concurrency, and render-time limits
- put the service behind TLS and an authentication layer unless access is
  meant to be anonymous
- return opaque errors to clients and keep detailed structured logs
- enable proxy-derived client addresses only behind a proxy that rewrites
  those headers

CORS controls which browser origins can read responses. It is not
authentication and does not stop non-browser clients.

{doc}`authentication` covers listener credentials, {doc}`rate-limiting` covers
render-time budgets, {doc}`logging` covers request logs, and {doc}`deployment`
combines them into complete profiles.

---
title: Server Overview
path: overview
section: Server
order: 400
interfaces: [server]
---

<!-- topic-body -->

# Server Overview

`vl-convert serve` runs the same conversion engine behind HTTP endpoints. Use
it when another service, browser app, or non-Rust runtime needs conversion
without embedding Rust bindings.

```bash
vl-convert serve --host 127.0.0.1 --port 3000
```

Public conversion routes live on the main listener. Admin routes use a
separate listener when enabled.

The server adds operational controls around the library:

- HTTP routes and generated OpenAPI references for conversion, font, theme, and
  bundling endpoints.
- A shared worker pool so callers do not each create their own V8 isolates.
- Request timeouts, body limits, concurrency limits, and per-IP/global budget
  windows.
- Optional API-key authentication, CORS, request IDs, and structured logs.
- An admin API for live config replacement, budget inspection/reset, worker
  diagnostics, and Google Fonts cache operations.
- Unix domain socket listeners, readiness JSON, graceful drain, and
  parent-close shutdown for subprocess sidecars.

See {doc}`/server/api-reference` for the generated public endpoint reference.

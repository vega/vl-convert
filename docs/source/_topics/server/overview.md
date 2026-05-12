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

See {doc}`/server/api-reference` for the generated public endpoint reference.

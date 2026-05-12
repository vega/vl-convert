---
title: Server Deployment Profiles
path: deployment
section: Server
order: 410
interfaces: [server]
---

<!-- topic-body -->

# Deploying the Server

Choose bind addresses, budgets, data access, and CORS for the deployment
shape.

## Private Backend Worker

```bash
vl-convert serve \
  --host 127.0.0.1 \
  --port 3000 \
  --api-key "$VLC_API_KEY" \
  --allowed-base-urls=net \
  --log-format=json
```

Use this profile when trusted backend code calls the server.

## Public Browser-Facing Converter

```bash
vl-convert serve \
  --host 0.0.0.0 \
  --port 3000 \
  --allowed-base-urls=none \
  --base-url=disabled \
  --per-ip-budget-ms 5000 \
  --global-budget-ms 30000 \
  --auto-google-fonts=true \
  --google-font-variant-threshold 16 \
  --google-font-cache-miss-penalty-ms 250 \
  --cors-origin=https://editor.example.com \
  --opaque-errors=true \
  --log-format=json
```

Use this profile when arbitrary browsers or internet clients call the server.

## Subprocess Sidecar

```bash
vl-convert serve \
  --unix-socket /run/myapp/vl-convert.sock \
  --admin-unix-socket /run/myapp/vl-convert-admin.sock \
  --socket-mode 0600 \
  --ready-json \
  --exit-on-parent-close=true
```

Use this profile when another runtime owns startup and shutdown.

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

## Production Hardening

For untrusted input, set resource and data-access controls explicitly. Local
defaults are convenient for development, but public deployments should not rely
on them.

- Bind only the listeners you need. Keep the admin listener on loopback, a
  private network, or a Unix domain socket; use `--admin-api-key` for shared
  environments.
- Set `--allowed-base-urls=none` unless charts need remote data. If they do,
  allow only the schemes, domains, or filesystem roots the application owns.
- Disable relative data loading with `--base-url=disabled` for user-supplied
  specs unless the deployment intentionally serves a known data root.
- Set `--max-v8-heap-size-mb` and `--max-v8-execution-time-secs` for public or
  multi-tenant workloads.
- Use `--per-ip-budget-ms`, `--global-budget-ms`, `--max-body-size-mb`, and
  `--opaque-errors=true` on public endpoints.
- Use `--log-format=json` in production so request IDs, budget fields, and
  Google Fonts fields remain queryable.
- Enable CORS only for browser clients that need direct access.
- If user input can trigger Google Fonts, set
  `--google-font-variant-threshold` and
  `--google-font-cache-miss-penalty-ms`.
- Enable `--trust-proxy` only behind a reverse proxy that strips untrusted
  forwarded headers and writes its own.

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
  --max-v8-heap-size-mb 1024 \
  --max-v8-execution-time-secs 10 \
  --per-ip-budget-ms 5000 \
  --global-budget-ms 30000 \
  --max-body-size-mb 4 \
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

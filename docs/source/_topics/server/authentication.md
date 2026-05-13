---
title: Server Authentication
path: authentication
section: Server
order: 420
interfaces: [server]
---

<!-- topic-body -->

# Authentication

The main listener and admin listener have independent bearer-token settings.
Use `--api-key` for public conversion routes and `--admin-api-key` for admin
routes.

## Main API Key

Set `--api-key` to require `Authorization: Bearer <key>` on conversion,
theme, font, and bundling routes.

```bash
vl-convert serve --api-key "$VLC_API_KEY"
```

```bash
curl -H "Authorization: Bearer $VLC_API_KEY" \
  http://localhost:3000/themes
```

Missing or incorrect credentials return `401 Unauthorized` with a
`WWW-Authenticate: Bearer` header. Unless `--opaque-errors=true` is enabled,
the response body is:

```json
{"error":"unauthorized"}
```

With opaque errors enabled, the response body is empty.

Health routes (`/healthz`, `/readyz`, and `/infoz`) stay unauthenticated so
load balancers and process supervisors can check the server without knowing the
API key. Do not use those routes to verify authentication.

## Admin API Key

Admin routes are served from the separate admin listener. The admin key does
not grant access to the main listener, and the main key does not grant access
to admin routes.

```bash
vl-convert serve \
  --admin-host 127.0.0.1 \
  --admin-port 3001 \
  --admin-api-key "$ADMIN_API_KEY"
```

```bash
curl -H "Authorization: Bearer $ADMIN_API_KEY" \
  http://127.0.0.1:3001/admin/diagnostics/workers
```

A non-loopback TCP admin listener requires `--admin-api-key` and fails startup
without one. Loopback and Unix domain socket admin listeners may rely on
listener placement or filesystem permissions, but a key is still useful as a
second guard in shared environments.

For subprocess sidecars, prefer an admin Unix domain socket with restrictive
permissions:

```bash
vl-convert serve \
  --unix-socket /run/myapp/vl-convert.sock \
  --admin-unix-socket /run/myapp/vl-convert-admin.sock \
  --socket-mode 0600 \
  --admin-api-key "$ADMIN_API_KEY"
```

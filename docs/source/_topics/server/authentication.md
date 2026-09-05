---
title: Server Authentication
path: authentication
section: Server
order: 420
interfaces: [server]
---

<!-- topic-body -->

# Authentication

VlConvert supports one bearer token for the main listener and a separate token
for the admin listener. This is suitable for service-to-service access. Put a
gateway in front of VlConvert when you need user accounts, several credentials,
permissions, or independent token rotation.

Bearer tokens are not encrypted. Use TLS at the reverse proxy or platform edge
whenever a token crosses a network.

## Protect the Main Listener

Set `VLC_API_KEY` in the server process environment, preferably through a
secret manager, or pass `--api-key`:

```bash
vl-convert serve --host 127.0.0.1 --port 3000
```

When `VLC_API_KEY` is set, conversion, theme, font, and bundling routes require
this header:

```text
Authorization: Bearer <key>
```

For example:

```bash
curl http://127.0.0.1:3000/themes \
  -H "Authorization: Bearer $VLC_API_KEY"
```

A missing or incorrect token returns `401 Unauthorized` and a
`WWW-Authenticate: Bearer` header. The normal JSON body is
`{"error":"unauthorized"}`. With `--opaque-errors`, the body is empty.

The main token grants access to every protected route on that listener. It does
not restrict which external resources a specification can load. Configure data,
font, plugin, and resource policies separately.

## Unauthenticated Health Routes

`/healthz`, `/readyz`, and `/infoz` remain unauthenticated so load balancers and
process supervisors can reach them. Do not use those routes to test whether
authentication is active.

`/infoz` includes component versions, the local timezone, and the Google Fonts
cache path. A public reverse proxy can expose only `/healthz` and `/readyz` if
those host details are not intended for clients.

## Protect the Admin Listener

The admin listener is optional and independent of the main listener. Set
`VLC_ADMIN_API_KEY` through the deployment's secret manager, then enable the
listener:

```bash
vl-convert serve \
  --admin-host 127.0.0.1 \
  --admin-port 3001
```

```bash
curl http://127.0.0.1:3001/admin/diagnostics/workers \
  -H "Authorization: Bearer $VLC_ADMIN_API_KEY"
```

The main token does not grant admin access, and the admin token does not grant
main-listener access.

A non-loopback TCP admin listener will not start without an admin API key.
Loopback and Unix domain socket listeners can run without one because listener
placement or filesystem permissions can be the primary boundary. A key remains
useful on shared hosts.

For a local sidecar, a restrictive Unix domain socket is often the simplest
boundary:

```bash
vl-convert serve \
  --unix-socket /run/myapp/vl-convert.sock \
  --admin-unix-socket /run/myapp/vl-convert-admin.sock \
  --socket-mode 0600
```

Do not expose the admin listener through the same public route as conversion
traffic. See {doc}`admin-api` for the operations it permits.

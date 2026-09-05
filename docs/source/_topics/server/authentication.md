---
title: Server Authentication
path: authentication
section: Server
order: 410
interfaces: [server]
---

<!-- topic-body -->

# Authentication

The server supports one bearer token for the main listener and a separate token
for the admin listener. This suits service-to-service access. Put a gateway in
front of the server when you need user accounts, several credentials,
permissions, or independent token rotation.

Bearer tokens travel in plain text. Use TLS at the reverse proxy or platform
edge whenever a token crosses a network.

## Protect the Main Listener

Supply the token through the `VLC_API_KEY` environment variable, preferably
from a secret manager, or through `--api-key`. With the variable set, start the
server normally:

```bash
vl-convert serve --host 127.0.0.1 --port 3000
```

Conversion, theme, font, bundling, and API documentation routes now require
this header:

```text
Authorization: Bearer <key>
```

For example:

```bash
curl http://127.0.0.1:3000/themes \
  -H "Authorization: Bearer $VLC_API_KEY"
```

A missing or incorrect token returns `401 Unauthorized` with a
`WWW-Authenticate: Bearer` header. The body is `{"error":"unauthorized"}`, or
empty when `--opaque-errors` is set.

The token grants access to every protected route on the listener. It does not
restrict which external resources a specification can load. Configure data,
font, plugin, and resource policies separately.

## Unauthenticated Health Routes

`/healthz`, `/readyz`, and `/infoz` never require a token, so load balancers
and process supervisors can reach them. Do not use them to test whether
authentication is active.

`/infoz` reports component versions, the local timezone, and the Google Fonts
cache path. If those host details should stay private, expose only `/healthz`
and `/readyz` through a public reverse proxy.

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

A TCP admin listener on a non-loopback address refuses to start without an
admin API key. Loopback and Unix domain socket listeners can run without one,
because listener placement or filesystem permissions can serve as the boundary.
A key is still worthwhile on shared hosts.

For a local sidecar, a restrictive Unix domain socket is often the simplest
boundary:

```bash
vl-convert serve \
  --unix-socket /run/myapp/vl-convert.sock \
  --admin-unix-socket /run/myapp/vl-convert-admin.sock \
  --socket-mode 0600
```

Never expose the admin listener through the same public route as conversion
traffic. See {doc}`admin-api` for the operations it permits.

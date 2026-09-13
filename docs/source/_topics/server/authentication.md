---
title: Server Authentication
path: authentication
section: Server
order: 410
interfaces: [server]
---

<!-- topic-body -->

# Authentication

The server supports one bearer token for the server API and a separate token for the admin API. This supports service-to-service authentication.

:::{warning}
Bearer tokens travel in plain text. Use TLS at the reverse proxy or platform edge whenever a token crosses a network.
:::

## API Authentication

Supply the token through the `VLC_API_KEY` environment variable or `--api-key`. With the variable set, start the server normally:

```console
$ vl-convert serve --port 3000
```

Conversion, theme, font, bundling, and API documentation routes now require this header:

```text
Authorization: Bearer <key>
```

For example:

```console
$ curl http://127.0.0.1:3000/themes \
>   -H "Authorization: Bearer $VLC_API_KEY"
```

A missing or incorrect token returns `401 Unauthorized` with a `WWW-Authenticate: Bearer` header. The body is `{"error":"unauthorized"}`, or empty when `--opaque-errors` is set.

The token grants access to every protected route in the server API. It does not restrict which external resources a specification can load. Configure data, font, plugin, and resource policies separately.

## Unauthenticated Health Routes

`/healthz`, `/readyz`, and `/infoz` never require a token, so load balancers and process supervisors can reach them.

`/infoz` reports component versions and the local timezone. If those host details should stay private, expose only `/healthz` and `/readyz` through a public reverse proxy.

## Admin API Authentication

The admin API is optional and independent of the server API. Set `VLC_ADMIN_API_KEY`, then enable the admin API:

```console
$ vl-convert serve \
>   --admin-host 127.0.0.1 \
>   --admin-port 3001
```

```console
$ curl http://127.0.0.1:3001/admin/diagnostics/workers \
>   -H "Authorization: Bearer $VLC_ADMIN_API_KEY"
```

The server API token does not grant admin API access, and the admin API token does not grant server API access.

The admin API requires a key when bound to a non-loopback TCP address. It can run without a key on loopback or a Unix domain socket, where network access or filesystem permissions can restrict who can connect.

See {doc}`deployment` for a Unix socket example and {doc}`admin-api` for the management operations.

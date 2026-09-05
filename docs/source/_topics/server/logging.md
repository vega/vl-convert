---
title: Server Logging
path: logging
section: Server
order: 440
interfaces: [server]
---

<!-- topic-body -->

# Server Logging

The server writes logs to standard error. Use text logs for local development
and JSON logs for deployed services:

```bash
vl-convert --log-format json --log-level info \
  serve --host 127.0.0.1 --port 3000
```

`--log-filter` accepts a `tracing-subscriber` directive and takes priority over
`--log-level`. Use it when one component needs more detail:

```bash
vl-convert --log-format json \
  --log-filter 'vl_convert=debug,tower_http=info' \
  serve --host 127.0.0.1 --port 3000
```

Debug logs can be high volume and may contain resource names or error details.
Return to an appropriate production level after investigation.

## Request Fields

A JSON response event contains standard request context:

| Field | Meaning |
| --- | --- |
| `http.method` | Request method |
| `http.url` | Request path and query |
| `http.version` | HTTP protocol version |
| `http.useragent` | Caller user agent |
| `http.request_id` | Request correlation identifier |
| `http.status_code` | Response status |
| `response_time_ms` | Request latency in milliseconds |
| `trace_id`, `span_id` | Incoming W3C trace context when supplied |

When render-time budgets are enabled, request events can also contain:

| Field | Meaning |
| --- | --- |
| `budget.outcome` | Final admission or settlement result, such as `accepted`, `rejected_per_ip`, or `refunded_on_drop` |
| `budget.charged_ms` | Settled charge, or the refunded provisional hold when `budget.outcome` is `refunded_on_drop` |
| `budget.elapsed_ms` | Measured processing time |
| `budget.font_cache_miss_penalty_ms` | Added Google Fonts charge |
| `budget.global_remaining_ms` | Shared capacity after the request |
| `budget.ip_remaining_ms` | Client capacity after the request |
| `budget.client_ip` | Address used for per-IP accounting |

Font work can add `google_font.css_cache_misses`,
`google_font.file_cache_misses`, `google_font.downloaded_bytes`, and
`google_font.resolved_variants`.

## Correlate a Request

Send `X-Request-Id` when a non-browser client already has a correlation ID:

```bash
curl http://127.0.0.1:3000/themes \
  -H 'X-Request-Id: render-01'
```

If the header is absent, the server creates an ID. The response includes the
same `X-Request-Id` so callers can attach it to their own logs. Browser clients
can read the generated response header when CORS is enabled.

Successful conversion responses also include `X-VLC-Logs`. Its value is a JSON
array containing up to 50 Vega diagnostic messages. Inspect it when a chart
renders but Vega reported warnings.

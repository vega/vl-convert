---
title: Server Logging
path: logging
section: Server
order: 430
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

Debug logs are high volume and can contain resource names or error details.
Return to a production level after investigating.

## Request Fields

Each JSON response event contains standard request context:

| Field | Meaning |
| --- | --- |
| `http.method` | Request method |
| `http.url` | Request path and query |
| `http.version` | HTTP protocol version |
| `http.useragent` | Caller user agent |
| `http.request_id` | Request correlation identifier |
| `http.status_code` | Response status |
| `response_time_ms` | Request latency in milliseconds |
| `trace_id`, `span_id` | Incoming W3C trace context, when supplied |

When render-time budgets are enabled, response events also contain:

| Field | Meaning |
| --- | --- |
| `budget.outcome` | `accepted`, `rejected_per_ip`, `rejected_global`, or `refunded_on_drop` |
| `budget.charged_ms` | Settled charge, or the refunded hold when the outcome is `refunded_on_drop` |
| `budget.elapsed_ms` | Measured processing time |
| `budget.font_cache_miss_penalty_ms` | Added Google Fonts charge |
| `budget.global_remaining_ms` | Shared capacity after the request |
| `budget.ip_remaining_ms` | Client capacity after the request |
| `budget.client_ip` | Address used for per-IP accounting |

Font work adds `google_font.css_cache_misses`,
`google_font.file_cache_misses`, `google_font.downloaded_bytes`, and
`google_font.resolved_variants`.

## Correlate a Request

Send `X-Request-Id` when a non-browser client already has a correlation ID:

```bash
curl http://127.0.0.1:3000/themes \
  -H 'X-Request-Id: render-01'
```

If the header is absent, the server generates an ID. Either way the response
carries the same `X-Request-Id`, so callers can attach it to their own logs.
Browser clients can read it when CORS allows their origin.

Successful conversion responses also carry `X-VLC-Logs`, a JSON array of up to
50 Vega diagnostic messages. Inspect it when a chart renders but Vega reported
warnings.

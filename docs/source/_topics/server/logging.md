---
title: Server Logging
path: logging
section: Server
order: 440
interfaces: [server]
---

<!-- topic-body -->

# Server Logging

Use JSON logs for container and production deployments.

```bash
vl-convert serve --log-format=json --log-level=info
```

Request logs include HTTP fields, request budget fields, and Google Fonts
fields when font work runs:

- `budget.outcome`
- `budget.elapsed_ms`
- `budget.font_cache_miss_penalty_ms`
- `google_font.css_cache_misses`
- `google_font.font_file_cache_misses`
- `google_font.downloaded_bytes`
- `google_font.resolved_variants`

Use `X-Request-Id` to correlate caller logs with server logs.

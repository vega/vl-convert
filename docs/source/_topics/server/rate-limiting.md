---
title: Rate Limiting and Budgets
path: rate-limiting
section: Server
order: 430
interfaces: [server]
---

<!-- topic-body -->

# Rate Limiting and Budgets

Budgets charge request processing time against per-IP and global pools.

```bash
vl-convert serve \
  --per-ip-budget-ms 5000 \
  --global-budget-ms 30000 \
  --budget-hold-ms 1000
```

When Google Fonts are enabled for user input, combine request budgets with a
variant threshold and a cache-miss penalty.

```bash
vl-convert serve \
  --auto-google-fonts=true \
  --google-font-variant-threshold 16 \
  --google-font-cache-miss-penalty-ms 250
```

Enable `--trust-proxy=true` only behind a proxy that overwrites forwarded
client IP headers.

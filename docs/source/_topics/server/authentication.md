---
title: Server Authentication
path: authentication
section: Server
order: 420
interfaces: [server]
---

<!-- topic-body -->

# Authentication

Set `--api-key` to require bearer-token authentication on conversion routes.

```bash
vl-convert serve --api-key "$VLC_API_KEY"
```

```bash
curl -H "Authorization: Bearer $VLC_API_KEY" \
  http://localhost:3000/readyz
```

Health routes stay unauthenticated. Admin routes use their own listener and
admin API key.

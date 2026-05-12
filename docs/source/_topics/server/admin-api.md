---
title: Admin API
path: admin-api
section: Server
order: 450
interfaces: [server]
---

<!-- topic-body -->

# Admin API

Enable the admin listener for runtime budget updates, config updates, font
cache controls, and worker diagnostics.

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

Keep the admin listener on loopback, a private network, or a Unix domain
socket.

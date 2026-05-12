---
title: Admin Server API
path: admin-api
section: API Reference
order: 910
interfaces: [server]
---

.. topic-body

Admin Server API
================

The admin server API is served on the separate admin listener. Enable it when
you need runtime budget updates, config updates, font cache controls, or worker
diagnostics.

.. code-block:: bash

   vl-convert serve \
     --admin-host 127.0.0.1 \
     --admin-port 3001 \
     --admin-api-key "$ADMIN_API_KEY"

.. code-block:: bash

   curl -H "Authorization: Bearer $ADMIN_API_KEY" \
     http://127.0.0.1:3001/admin/diagnostics/workers

Keep the admin listener on loopback, a private network, or a Unix domain
socket. TCP admin listeners on non-loopback addresses require
``--admin-api-key``; loopback and Unix domain socket listeners can also use it
as a redundant guard.

The reference below is generated from ``vl-convert serve --dump-openapi=admin``.

.. openapi:: ../_generated/openapi-admin.json
   :group:
   :examples:
   :format: markdown

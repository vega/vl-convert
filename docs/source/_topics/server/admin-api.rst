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

The optional admin listener manages a running server. It can inspect and update
render-time budgets, replace converter configuration, manage font directories
and cache size, and report worker memory use.

Do not expose this listener with public conversion traffic. Bind it to
loopback, a private management network, or a Unix domain socket. Configure a
separate admin bearer token on shared systems.

.. code-block:: bash

   vl-convert serve \
     --admin-host 127.0.0.1 \
     --admin-port 3001 \
     --admin-api-key "$ADMIN_API_KEY"

.. code-block:: bash

   curl http://127.0.0.1:3001/admin/diagnostics/workers \
     -H "Authorization: Bearer $ADMIN_API_KEY"

A non-loopback TCP admin listener requires ``--admin-api-key``. Loopback and
Unix domain socket listeners can run without a key, but filesystem or network
placement must then provide the access boundary. See
:doc:``/server/authentication``.

Live Configuration Changes
--------------------------

``GET /admin/config`` returns the active converter settings. ``PATCH
/admin/config`` changes selected fields. ``PUT /admin/config`` replaces the
complete converter configuration. ``DELETE /admin/config`` restores the
configuration that the server started with. The font-directory list and Google
Fonts cache size have separate endpoints because they apply to the whole
process.

A configuration change follows this sequence:

#. The server validates the proposed configuration.
#. The main listener stops admitting conversion requests.
#. In-flight requests receive time to finish.
#. The server starts replacement workers and checks that they are ready.
#. New requests begin using the replacement.

During this sequence, ``/readyz`` returns ``503`` and newly admitted conversion
requests receive ``503`` with ``Retry-After: 5``. If draining or warm-up fails,
the previous configuration remains active. Use
``--reconfig-drain-timeout-secs`` to bound the wait.

Sending values identical to the active configuration does not rebuild workers.

Patch and Replacement Bodies
----------------------------

A ``PATCH`` field has three possible states:

- Omitted means keep the current value.
- A JSON value means set the field.
- ``null`` clears a nullable field such as ``default_theme`` or
  ``max_v8_heap_size_mb``.

``null`` is invalid for required fields such as ``num_workers``, ``base_url``,
``allowed_base_urls``, and ``themes``.

``PUT`` is a full replacement. Required fields must be present and valid.
Nullable fields can be ``null``. The request uses the same field names as a
JSONC converter config file.

``base_url`` accepts ``true`` for the Vega datasets default, ``false`` to
reject relative data URLs, or a URL or filesystem path. ``allowed_base_urls``
is a list of Content Security Policy-style patterns. An empty list blocks
absolute data and image URLs.

Generated Endpoint Reference
----------------------------

The reference below is generated from
``vl-convert serve --dump-openapi=admin`` and lists all request and response
schemas.

.. openapi:: ../_generated/openapi-admin.json
   :group:
   :examples:
   :format: markdown

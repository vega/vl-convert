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

Admin Config Schema
-------------------

``GET /admin/config`` returns the active converter configuration. ``PATCH
/admin/config`` updates selected fields, and ``PUT /admin/config`` replaces the
converter configuration.

``PATCH`` uses three states:

- Omitted field: keep the current value.
- JSON value: set the field.
- ``null``: clear nullable fields such as ``default_theme`` or
  ``max_v8_heap_size_mb``. ``null`` is rejected for non-nullable fields such as
  ``num_workers``, ``base_url``, ``allowed_base_urls``, and ``themes``.

``PUT`` is a full replacement. Non-nullable fields must be present with valid
values. Nullable fields may be ``null``.

The config request bodies use the same field names as the JSONC config file.
``base_url`` accepts ``true`` for the Vega datasets default, ``false`` to
disable relative data loading, or a URL/path string. ``allowed_base_urls`` is a
list of CSP-style allowlist patterns; use ``[]`` to block data fetches. The
OpenAPI schema below lists ``ConfigPatch``, ``ConfigReplace``, and
``ConfigView`` for the full field set.

The reference below is generated from ``vl-convert serve --dump-openapi=admin``.

.. openapi:: ../_generated/openapi-admin.json
   :group:
   :examples:
   :format: markdown

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
render-time budgets, replace the converter configuration, manage font
directories and the Google Fonts cache size, and report worker memory use.

Never expose this listener alongside public conversion traffic. Bind it to
loopback, a private management network, or a Unix domain socket, and give it
its own bearer token on shared systems.

.. code-block:: bash

   vl-convert serve \
     --admin-host 127.0.0.1 \
     --admin-port 3001 \
     --admin-api-key "$ADMIN_API_KEY"

.. code-block:: bash

   curl http://127.0.0.1:3001/admin/diagnostics/workers \
     -H "Authorization: Bearer $ADMIN_API_KEY"

A TCP admin listener on a non-loopback address requires ``--admin-api-key``.
Loopback and Unix domain socket listeners can run without a key, in which case
network placement or filesystem permissions must provide the access boundary.
See :doc:`/server/authentication`.

A running server also serves this reference at ``/admin/api-doc/openapi.json``
and an interactive Swagger UI at ``/admin/docs``.

Live Configuration Changes
--------------------------

``GET /admin/config`` returns the active converter settings. ``PATCH
/admin/config`` changes selected fields. ``PUT /admin/config`` replaces the
complete converter configuration. ``DELETE /admin/config`` restores the
configuration the server started with. Font directories and the Google Fonts
cache size have their own endpoints because they apply to the whole process
rather than to one converter.

A configuration change follows this sequence:

#. The server validates the proposed configuration.
#. The main listener stops admitting conversion requests.
#. In-flight requests are given time to finish.
#. The server starts replacement workers.
#. New requests begin using the replacement.

While this runs, ``/readyz`` returns ``503`` and new conversion requests
receive ``503`` with ``Retry-After: 5``. If draining or worker start-up fails,
the previous configuration stays active. ``--reconfig-drain-timeout-secs``
bounds the wait.

Sending values identical to the active configuration does not rebuild workers.

Patch and Replacement Bodies
----------------------------

A ``PATCH`` field has three states:

- Omitted keeps the current value.
- A JSON value sets the field.
- ``null`` clears a nullable field such as ``default_theme`` or
  ``max_v8_heap_size_mb``.

``null`` is rejected for required fields such as ``num_workers``,
``base_url``, ``allowed_base_urls``, and ``themes``.

``PUT`` is a full replacement. Every required field must be present and valid,
and nullable fields may be ``null``. The field names match the JSONC converter
config file.

``base_url`` accepts ``true`` for the Vega datasets default, ``false`` to reject
relative data URLs, or a URL or filesystem path. ``allowed_base_urls`` is a
list of Content Security Policy-style patterns, and an empty list blocks every
absolute data and image URL.

Generated Endpoint Reference
----------------------------

The reference below is generated from
``vl-convert serve --dump-openapi=admin``.

.. openapi:: ../_generated/openapi-admin.json
   :group:
   :examples:
   :format: markdown

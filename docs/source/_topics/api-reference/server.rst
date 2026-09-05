---
title: Server API
path: api-reference
section: API Reference
order: 900
interfaces: [server]
---

.. topic-body

Server API
==========

The public reference below is generated from the version 2 executable with
``vl-convert serve --dump-openapi=public``. Start with
:doc:`/server/getting-started/quick-start` for a complete request.

Vega and Vega-Lite conversion endpoints accept JSON request bodies shaped like
``{"spec": <spec>, ...overrides}``. Overrides use the same names as the
Python/Rust options, such as ``scale``, ``ppi``, ``theme``,
``format_locale``, ``width``, and ``height``. Unknown request fields are
rejected.

SVG conversion endpoints use JSON request bodies with the markup in an ``svg``
string field. Binary results are returned directly in the response body. SVG,
HTML, URL, and JSON results use their corresponding text or JSON content type.
Vega diagnostic messages are available in the ``X-VLC-Logs`` response header.

The :doc:`/server/admin-api` uses a separate listener and credential, so it has
its own reference.

.. openapi:: ../_generated/openapi-public.json
   :group:
   :examples:
   :format: markdown

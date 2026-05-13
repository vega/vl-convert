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

The public server reference is generated from ``vl-convert serve
--dump-openapi=public``.

Vega and Vega-Lite conversion endpoints accept JSON request bodies shaped like
``{"spec": <spec>, ...overrides}``. Overrides use the same names as the
Python/Rust options, such as ``scale``, ``ppi``, ``theme``,
``format_locale``, ``width``, and ``height``. SVG conversion endpoints also use
JSON request bodies, with the SVG markup in an ``svg`` string field and
format-specific overrides such as ``scale`` and ``ppi``. Binary outputs are
returned directly in the response body; SVG, HTML, JSON, and URL outputs use
their natural text or JSON response types.

The :doc:`/server/admin-api` reference is generated from the admin OpenAPI
document because those endpoints use a separate listener and authentication
posture.

.. openapi:: ../_generated/openapi-public.json
   :group:
   :examples:
   :format: markdown

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

The :doc:`/server/admin-api` reference is generated from the admin OpenAPI
document because those endpoints use a separate listener and authentication
posture.

.. openapi:: ../_generated/openapi-public.json
   :group:
   :examples:
   :format: markdown

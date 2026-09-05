---
title: Render-Time Budgets
path: rate-limiting
section: Server
order: 430
interfaces: [server]
---

<!-- topic-body -->

# Render-Time Budgets

Render-time budgets limit processing time rather than request count. They are
useful because two requests can have very different conversion costs.

`--per-ip-budget-ms` sets the milliseconds available to one client IP during
each minute of sustained use. `--global-budget-ms` sets the shared capacity for
all clients. Both buckets start full and refill gradually once per second. A
value of `0` disables that budget dimension.

A request must have capacity in every enabled bucket. If either bucket is
exhausted, the server returns `429 Too Many Requests`.

## Configure Budgets

This example permits about five seconds of processing per minute for one IP and
thirty seconds per minute across the server:

```bash
vl-convert serve \
  --host 127.0.0.1 \
  --port 3000 \
  --per-ip-budget-ms 5000 \
  --global-budget-ms 30000 \
  --budget-hold-ms 1000
```

`budget_hold_ms` is a temporary reservation made when the request enters. When
the response is ready, VlConvert replaces the reservation with the measured
processing time. A larger hold prevents many costly requests from entering at
once but can reject a burst of short requests. A smaller hold allows more
concurrency but can temporarily overspend a bucket when requests run longer
than expected.

Start near the typical conversion time, then tune from request logs. The hold
must not exceed an enabled budget cap or that bucket cannot admit a request.

Budgets complement these separate controls:

- `--max-concurrent-requests` caps work admitted at the same time
- `--request-timeout-secs` bounds an HTTP request
- `--max-v8-execution-time-secs` bounds JavaScript execution
- `--max-body-size-mb` bounds request payload size

## Identify Clients Correctly

Without `--trust-proxy`, the TCP peer address identifies the client. Behind a
reverse proxy, that address is usually the proxy, so every caller would share
one per-IP bucket.

Enable `--trust-proxy` only when the proxy removes client-supplied forwarding
headers and writes trusted values. VlConvert then checks
`X-Envoy-External-Address`, `X-Forwarded-For`, and `X-Real-IP` before falling
back to the peer address.

Unix domain sockets have no client IP. Requests over them use the global budget
but skip the per-IP budget.

## Charge Google Fonts Work

When a specification can trigger automatic Google Fonts, add a charge for each
CSS or font-file cache miss:

```bash
vl-convert \
  --auto-google-fonts \
  --google-font-variant-threshold 16 \
  serve \
  --per-ip-budget-ms 5000 \
  --global-budget-ms 30000 \
  --google-font-cache-miss-penalty-ms 250
```

The final charge is measured processing time plus the font cache-miss penalty.
The variant threshold limits how many automatically discovered font variants
can be admitted.

## Observe and Update Budgets

JSON request logs include the outcome, charged time, remaining capacity, and
font penalty. See {doc}`logging` for field names.

An enabled admin listener can inspect current budget state with
`GET /admin/budget` and update caps or the reservation with
`POST /admin/budget`. Existing balances are clamped when a cap is lowered.
Protect this listener as described in {doc}`authentication`.

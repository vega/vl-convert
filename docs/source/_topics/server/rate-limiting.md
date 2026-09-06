---
title: Render-Time Budgets
path: rate-limiting
section: Server
order: 420
interfaces: [server]
---

<!-- topic-body -->

# Render-Time Budgets

Render-time budgets limit processing time rather than request count, because
two requests can have very different conversion costs.

`--per-ip-budget-ms` sets the milliseconds of processing available to one
client IP per minute. `--global-budget-ms` sets the shared capacity for all
clients. Each budget is a bucket that starts full and refills every second at
one sixtieth of its capacity. A value of `0` disables that budget.

A request needs capacity in every enabled bucket. If either is exhausted, the
server returns `429 Too Many Requests`.

## Configure Budgets

This example allows about five seconds of processing per minute for one IP and
thirty seconds per minute across the server:

```bash
vl-convert serve \
  --port 3000 \
  --per-ip-budget-ms 5000 \
  --global-budget-ms 30000 \
  --budget-hold-ms 1000
```

`--budget-hold-ms` is the reservation taken when a request is admitted. When
the response is ready, the reservation is replaced by the measured processing
time. A larger hold stops many costly requests from entering at once but can
reject a burst of cheap ones. A smaller hold allows more concurrency but can
briefly overspend a bucket when requests run longer than expected. The default
is 1000.

Start near the typical conversion time, then tune from the request logs. The
hold must not exceed an enabled budget, or that bucket can never admit a
request.

Budgets complement these separate controls:

- `--max-concurrent-requests` caps work admitted at the same time
- `--request-timeout-secs` bounds one HTTP request
- `--max-v8-execution-time-secs` bounds JavaScript execution
- `--max-body-size-mb` bounds request payload size

## Identify Clients Correctly

Without `--trust-proxy`, the TCP peer address identifies the client. Behind a
reverse proxy that address is the proxy itself, so every caller would share one
per-IP bucket.

Enable `--trust-proxy` only when the proxy strips client-supplied forwarding
headers and writes trusted values. The server then reads
`X-Envoy-External-Address`, `X-Forwarded-For`, and `X-Real-IP` before falling
back to the peer address.

Unix domain sockets have no client IP. Requests over them use the global budget
and skip the per-IP budget.

## Charge Google Fonts Work

When a specification can trigger automatic Google Fonts, add a charge for each
CSS or font-file cache miss:

```bash
vl-convert serve \
  --auto-google-fonts \
  --google-font-variant-threshold 16 \
  --per-ip-budget-ms 5000 \
  --global-budget-ms 30000 \
  --google-font-cache-miss-penalty-ms 250
```

The final charge is the measured processing time plus the penalty for each
cache miss. The variant threshold caps how many Google Font variants one
conversion can load.

## Observe and Update Budgets

JSON request logs record the outcome, charged time, remaining capacity, and
font penalty. See {doc}`logging` for the field names.

When the admin listener is enabled, `GET /admin/budget` reports the current
state and `POST /admin/budget` updates the caps or the hold. Lowering a cap
clamps existing balances to the new value. See {doc}`authentication` for
protecting that listener.

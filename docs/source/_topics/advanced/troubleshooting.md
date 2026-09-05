---
title: Troubleshooting
path: advanced/troubleshooting
section: Advanced
order: 490
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Troubleshooting

Start with the error message and Vega diagnostic logs. Then reduce the input to
the smallest specification that still fails. A small reproduction usually
reveals whether the problem is the specification, an external resource, a font,
or a converter limit.

::::{interface} python
Enable Python logging while reproducing the problem:

```python
import logging

logging.basicConfig(level=logging.INFO)
logging.getLogger("vl_convert").setLevel(logging.DEBUG)
```

Conversion failures raise an exception. Successful conversions can still log
Vega warnings.
::::

::::{interface} cli
The CLI writes diagnostics to standard error and returns a nonzero exit status
on failure. Use an explicit configuration while debugging:

```bash
vl-convert \
  --vlc-config disabled \
  --log-level debug \
  vl2png --input chart.vl.json --output chart.png
```

Adding settings back one at a time can identify a config-file or environment
override that caused the problem.
::::

::::{interface} rust
Preserve the error chain when reporting a failure, and inspect the `logs` field
on successful output:

```rust
match converter
    .vegalite_to_png(spec, Default::default(), Default::default())
    .await
{
    Ok(output) => {
        for entry in output.logs {
            eprintln!("{}: {}", entry.level, entry.message);
        }
    }
    Err(error) => eprintln!("{error:#}"),
}
```
::::

::::{interface} server
Use detailed errors only on a trusted development or staging listener:

```bash
vl-convert --log-format json --log-level debug \
  serve --host 127.0.0.1 --port 3000 --opaque-errors=false
```

Production services should normally use `--opaque-errors` and rely on
server-side logs. Use the response's `X-Request-Id` to correlate it with those
logs. A non-browser client can also supply its own `X-Request-Id`.
::::

## The Specification Fails or Looks Wrong

Confirm that the input type matches the API. A Vega-Lite specification belongs
in a `vegalite_*` function, `vl2*` command, or `/vegalite/*` endpoint. An
already compiled Vega specification belongs in the corresponding Vega
interface.

For Vega-Lite input, select the compiler version that matches the specification
when version-dependent behavior is involved. Compile the chart to Vega first
when you need to determine whether a problem occurs during compilation or
rendering.

Warnings about unknown properties often mean the specification contains a
misspelled field or a feature unsupported by the selected Vega-Lite version.

## Data or Images Cannot Load

An error that says an external URL is not allowed means the resolved resource
did not match `allowed_base_urls`. Check all of these values:

- the URL in the specification
- `base_url` when the specification uses a relative URL
- redirects made by the data or image host
- the exact allowed prefix, including scheme and path

Grant the narrowest required prefix. Do not use a wildcard to hide an allowlist
mistake in an untrusted workload. See {doc}`../guides/security`.

Also verify that the rendering process can resolve DNS, establish TLS, and
reach the host. An allowed URL can still fail because of normal network or
authentication errors.

## Text Uses the Wrong Font

A font can be installed on the browser machine but absent from the machine
running VlConvert. Set `missing_fonts` to `warn` or `error` and inspect the
fonts used by the evaluated chart.

See {doc}`font-introspection` to list required families and variants. Register
the missing directory or configure an explicit Google Font as described in
{doc}`../guides/fonts`.

If automatic Google Fonts is enabled, a lookup can fail because the family is
not in the catalog, the requested variant does not exist, or the network fetch
failed. The missing-font policy determines whether VlConvert falls back, warns,
or fails.

## A Conversion Times Out or Uses Too Much Memory

`max_v8_execution_time_secs` covers JavaScript compilation and evaluation.
`max_v8_heap_size_mb` covers the JavaScript heap of one worker. An error at
either boundary stops the current conversion.

First test whether the specification or data is unexpectedly large. Reduce
inline data, simplify expensive transforms, or lower the number of marks when
possible. Raise a limit only after measuring a representative workload.

Raster images and other native allocations are outside the JavaScript heap, so
a process can use more memory than `max_v8_heap_size_mb`. See
{doc}`memory-management`.

## A Plugin Does Not Load

Check that the module has a default export function and does not depend on
browser-only globals during static conversion. A local multi-file plugin should
be bundled into one ESM file.

For URL plugins, verify the entry URL and each imported domain. Startup plugins
use `plugin_import_domains`. Caller-supplied plugins use the separate
`per_request_plugin_import_domains` setting and must be enabled first.

See {doc}`plugin-loading` for examples and trust boundaries.

## Generated HTML Is Blank

Open the browser developer console. A page created with `bundle=false` must be
able to load its JavaScript dependencies from the CDN. Try `bundle=true` when
the page will be opened offline or behind a restrictive network.

A content security policy can also block scripts, data URLs, blob URLs, fonts,
or plugin imports used by the generated page. Adjust the policy for the
specific output or choose a static format when interaction is not required.

::::{interface} server
## Interpret Common HTTP Statuses

- `400 Bad Request` usually means a request field has the wrong type, an unknown
  field was supplied, or an option value is invalid.
- `401 Unauthorized` means the bearer token is absent or incorrect.
- `422 Unprocessable Entity` means the JSON request was accepted but conversion
  failed.
- `429 Too Many Requests` means a render-time budget is exhausted.
- `503 Service Unavailable` can occur while the server is draining or when a
  concurrency permit is unavailable.
- `504 Gateway Timeout` means the configured request timeout expired.

When opaque errors are enabled, the status and correlated server logs are the
diagnostic source.
::::

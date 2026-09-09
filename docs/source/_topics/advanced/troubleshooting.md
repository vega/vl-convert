---
title: Troubleshooting
path: advanced/troubleshooting
section: Advanced
order: 490
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Troubleshooting

Start with the error message and Vega's diagnostic messages, then reduce the
input to the smallest specification that still fails. A small reproduction
usually shows whether the problem is the specification, an external resource,
a font, or a converter limit. See {doc}`../guides/logging` for how to see the
diagnostic messages.

The snippets below add diagnostics to an existing failing conversion. In the
CLI example, save the reduced input as `chart.vl.json`. See
{doc}`../getting-started/quick-start` for complete conversion setup.

::::{interface} python
Enable logging while reproducing the problem:

```python
import logging

logging.basicConfig(level=logging.INFO)
logging.getLogger("vl_convert").setLevel(logging.DEBUG)
```

Conversion failures raise an exception. Successful conversions can still log
Vega warnings.
::::

::::{interface} cli
The CLI writes diagnostics to standard error and exits with a nonzero status on
failure. Rule out config files and environment variables while debugging:

```console
$ vl-convert \
>   --vlc-config disabled \
>   --log-level debug \
>   vl2png --input chart.vl.json --output chart.png
```

Add settings back one at a time to find the one that causes the problem.
::::

::::{interface} rust
Print the full error chain when a conversion fails, and inspect `logs` when it
succeeds:

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
Return detailed errors only on a trusted development or staging listener:

```console
$ vl-convert serve \
>   --log-format json --log-level debug \
>   --port 3000 --opaque-errors=false
```

Production services should use `--opaque-errors` and rely on server-side logs.
Use the response's `X-Request-Id` header to find the matching log entries. A
non-browser client can also send its own `X-Request-Id`.
::::

## The Specification Fails or Looks Wrong

Check that the input type matches the API. A Vega-Lite specification belongs in
a `vegalite_*` function, `vl2*` command, or `/vegalite/*` endpoint. A compiled
Vega specification belongs in the corresponding Vega interface.

For Vega-Lite input, select the compiler version that matches the specification
when version-dependent behavior is involved. Compile the chart to Vega first to
see whether a problem occurs during compilation or rendering.

Warnings about unknown properties usually mean a misspelled field or a feature
that the selected Vega-Lite version does not support.

## Data or Images Cannot Load

An error containing `VLC_ACCESS_DENIED` means the resolved resource did not
match `allowed_base_urls`. Check all of these:

- the URL in the specification
- `base_url`, when the specification uses a relative URL or a bare path
- the exact allowed prefix, including scheme and path
- the destination of any redirect the host returns, which is checked too
- for a local file, that the URL uses `file://` or a filesystem `base_url`

A bare absolute path such as `/data/cars.csv` is treated as relative to
`base_url`, so with the default base URL it becomes a CDN address. Grant the
narrowest prefix that works, and do not use a wildcard to hide an allowlist
mistake in an untrusted workload. See {doc}`../guides/data-loading`.

Also confirm that the rendering process can resolve DNS, complete TLS, and
reach the host. An allowed URL can still fail for ordinary network or
authentication reasons.

## Text Uses the Wrong Font

A font can be installed on the machine that shows the browser preview but
missing from the machine running VlConvert. Set `missing_fonts` to `warn` or
`error` to learn which first-choice fonts are unavailable. With
`embed_local_fonts` or `auto_google_fonts` enabled, {doc}`font-introspection`
lists the fonts that did resolve. Register the missing directory or configure
an explicit Google Font as described in {doc}`../guides/fonts`.

When automatic Google Fonts is enabled, a lookup can fail because the family is
not in the catalog, the requested variant does not exist, or the download
failed. The missing-font policy decides whether VlConvert falls back, warns, or
fails.

## A Conversion Times Out or Uses Too Much Memory

`max_v8_execution_time_secs` covers JavaScript compilation and evaluation, and
`max_v8_heap_size_mb` covers the JavaScript heap of one worker. Hitting either
limit fails the current conversion only.

First check whether the specification or its data is unexpectedly large.
Reduce inline data, simplify expensive transforms, or lower the number of
marks where possible. Raise a limit only after measuring a representative
workload.

Raster images and other native allocations live outside the JavaScript heap, so
a process can use more memory than `max_v8_heap_size_mb`. See
{doc}`memory-management`.

## A Plugin Does Not Load

Check that the module has a default export function and does not use
browser-only globals during static conversion. Bundle a multi-file plugin into
one ESM file.

For URL plugins, verify the entry URL and every imported domain. Startup
plugins use `plugin_import_domains`. Caller-supplied plugins must be enabled
first and use the separate `per_request_plugin_import_domains` setting. See
{doc}`plugin-loading`.

## Generated HTML Is Blank

Open the browser's developer console. A page created with `bundle=false` must
be able to load its JavaScript dependencies from the CDN. Use `bundle=true`
when the page will be opened offline or behind a restrictive network.

A content security policy can also block scripts, data URLs, blob URLs, fonts,
or plugin imports used by the page. Adjust the policy for the specific output,
or choose a static format when interaction is not required. See
{doc}`../guides/html-output`.

::::{interface} server
## Interpret Common HTTP Statuses

- `400 Bad Request` usually means a request field has the wrong type, an
  unknown field was supplied, or an option value is invalid.
- `401 Unauthorized` means the bearer token is missing or incorrect.
- `413 Content Too Large` means the request body exceeds `--max-body-size-mb`.
- `422 Unprocessable Entity` means the request was well formed but the
  conversion failed.
- `429 Too Many Requests` means a render-time budget is exhausted.
- `503 Service Unavailable` occurs while the server is reconfiguring or
  draining, or when the concurrency limit is reached.
- `504 Gateway Timeout` means the request timeout expired.

With opaque errors enabled, the status code and the correlated server logs are
the only diagnostic sources.
::::

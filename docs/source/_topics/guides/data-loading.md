---
title: Loading Data and Images
path: guides/data-loading
section: Guides
order: 225
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Loading Data and Images

A specification can carry its data inline or point at it with a URL. Inline
data in `data.values` or `datasets` never touches the network or the disk. This
page covers everything else: how VlConvert resolves a `data.url`, which URLs
and files it may read, and how the same rules apply to images.

Two converter settings control loading:

- `base_url` is the base for relative URLs. It defaults to the Vega datasets
  CDN, so the `data/cars.json` shorthand used by the Vega editor and the
  Vega-Lite examples works without configuration.
- `allowed_base_urls` lists the URLs and directories a specification may read.
  It defaults to any HTTP or HTTPS URL and no local files.

## How a Data URL Is Resolved

Vega resolves the `url` of a data source before VlConvert fetches it:

| URL in the specification | Result with default settings |
| --- | --- |
| `https://example.com/data.json` | Fetched with an HTTP GET request |
| `data/cars.json` | Joined to `base_url`: `https://cdn.jsdelivr.net/npm/vega-datasets@v2.9.0/data/cars.json` |
| `/srv/data/cars.csv` | Also joined to `base_url`, because Vega treats a bare path as relative |
| `file:///srv/data/cars.csv` | Read from disk when the directory is allowed |
| `data:text/csv,...` | Decoded inline, always allowed |

Two things follow. To read a local file, use a `file://` URL or set `base_url`
to a directory, because a bare absolute path becomes a CDN address. And every
resolved HTTP URL or file path is checked against `allowed_base_urls`. A
request that fails the check raises an error whose message contains
`VLC_ACCESS_DENIED`.

VlConvert makes GET requests only, to `http` and `https` URLs only. Each
request has a 10-second connection timeout and a 30-second overall limit that
includes up to ten redirects, each checked against the allowlist. The limit
applies to each attempt: image fetches for SVG-based output retry transient
server errors up to four times, so a failing image can take longer overall.
Vega parses the response as JSON, CSV, TSV, or TopoJSON according to the file
extension or the `format` property, as described in the
[Vega-Lite data documentation](https://vega.github.io/vega-lite/docs/data.html).

## Allowlist Patterns

Each entry in `allowed_base_urls` is one of these patterns:

| Pattern | Example | Matches |
| --- | --- | --- |
| Scheme | `https:` | Any URL with that scheme |
| URL prefix | `https://data.example.com/public/` | URLs that start with the prefix. A trailing `/` is added when missing |
| Wildcard host | `https://*.example.com/` | The host and its subdomains, optionally limited to a path prefix |
| Directory | `/srv/data/` or `file:///srv/data/` | Files under that directory, after resolving symlinks and `..` |
| Everything | `*` | Any URL or path, including the whole filesystem |

Prefix entries cannot contain credentials, a query string, or a fragment.
Directory entries must exist when the converter starts, and on Windows they can
use drive letters such as `C:\data\`. An empty list blocks every HTTP or HTTPS
URL and filesystem path. Inline `data:` URLs remain allowed.

::::{interface} cli server
`--allowed-base-urls` also accepts the shortcuts `none` for an empty list,
`net` for HTTP and HTTPS only, and `all` for `*`. Separate several patterns
with semicolons.
::::

## Load Local Files

Set `base_url` to the directory that holds the data and add the same directory
to `allowed_base_urls`. Relative URLs in the specification then resolve to
files under that directory, and `file://` URLs inside it work as well.

Create this layout, then run the examples from the `example` directory:

```text
example/
├── chart.vl.json
└── data/
    └── sales.csv
```

Save this specification:

:::{dropdown} chart.vl.json
:open:

```{literalinclude} /_examples/local-data.vl.json
:language: json
```
:::

Save this data as `data/sales.csv`:

:::{dropdown} data/sales.csv
:open:

```{literalinclude} /_examples/data/sales.csv
:language: text
```
:::

::::{interface} python
```python
from pathlib import Path

import vl_convert as vlc

example_dir = Path.cwd().resolve()
vlc.configure(
    base_url=str(example_dir),
    allowed_base_urls=[str(example_dir)],
)
spec = Path("chart.vl.json").read_text(encoding="utf-8")
png = vlc.vegalite_to_png(spec)
Path("chart.png").write_bytes(png)
```

`base_url=False` rejects relative URLs, and `base_url=True` restores the CDN
default.
::::

::::{interface} cli
```console
$ vl-convert \
>   --base-url "$PWD" \
>   --allowed-base-urls "$PWD" \
>   vl2png --input chart.vl.json --output chart.png
```

`--base-url disabled` rejects relative URLs. The equivalent environment
variables are `VLC_BASE_URL` and `VLC_ALLOWED_BASE_URLS`. In a JSONC config
file, a relative `base_url` path is resolved against the file's directory.
::::

::::{interface} rust
```rust
use vl_convert_rs::{BaseUrlSetting, VlcConfig, VlConverter};

let example_dir = std::env::current_dir()?;
let example_dir = example_dir.to_string_lossy().to_string();
let converter = VlConverter::with_config(VlcConfig {
    base_url: BaseUrlSetting::Custom(example_dir.clone()),
    allowed_base_urls: vec![example_dir],
    ..Default::default()
})?;
let spec = std::fs::read_to_string("chart.vl.json")?;
let output = converter
    .vegalite_to_png(spec, Default::default(), Default::default())
    .await?;
std::fs::write("chart.png", output.data)?;
```

`BaseUrlSetting::Disabled` rejects relative URLs. `with_config()` returns an
error if an allowlisted directory does not exist.
::::

::::{interface} server
```console
$ vl-convert serve \
>   --base-url "$PWD" \
>   --allowed-base-urls "$PWD" \
>   --port 3000
```

Save this complete request as `request.json`:

:::{dropdown} request.json
:open:

```{literalinclude} /_generated/requests/local-data-png.json
:language: json
```
:::

Send it from a second terminal whose current directory contains
`request.json`:

```console
$ curl http://127.0.0.1:3000/vegalite/png \
>   -H 'Content-Type: application/json' \
>   --data-binary @request.json \
>   --output chart.png
```

Requests cannot change the loading settings. When the admin listener is enabled,
`PATCH /admin/config` updates them without a restart. See
{doc}`/server/admin-api`.
::::

For production, replace `$PWD` or `Path.cwd()` with a stable absolute path such
as `/srv/charts`. Keep the allowlist limited to the directory the chart needs.

## Use the Vega Example Datasets

Vega and Vega-Lite examples reference their sample data with paths such as
`data/movies.json`. With the default settings these work as they do in the Vega
editor: the path is joined to `base_url`, which points at version 2.9.0 of the
`vega-datasets` package on the jsDelivr CDN, and that CDN is covered by the
default HTTPS allowlist. No configuration is needed.

The paths stop working when either setting changes:

- If you narrow `allowed_base_urls`, add the CDN prefix
  `https://cdn.jsdelivr.net/npm/vega-datasets@v2.9.0/` to the list.
- If you set `base_url` to a local directory, `data/movies.json` resolves under
  that directory instead. Either copy the datasets there or use absolute CDN
  URLs in the specification.
- If `base_url` is disabled, relative paths fail. Use absolute URLs.

To render the examples offline, download the `vega-datasets` package, for
example with `npm pack vega-datasets@2.9.0` or by cloning
[github.com/vega/vega-datasets](https://github.com/vega/vega-datasets), then
set `base_url` to the directory that contains its `data` folder and add that
directory to `allowed_base_urls` as shown above. To use a different dataset
release, set `base_url` to its CDN URL, such as
`https://cdn.jsdelivr.net/npm/vega-datasets@3/`.

## Restrict Remote Data

The default allows any HTTP or HTTPS host. For specifications you do not fully
control, replace it with the prefixes or wildcard hosts of the services the
application uses, and keep local directories out of the list unless they are
needed. {doc}`security` shows the resulting configuration for each interface
together with the other limits a public service needs.

## Images

Images follow the same rules as data. This covers Vega `image` marks, whose
`url` can be a URL or a path, and `<image>` elements in SVG input. `data:` URLs
are always allowed, HTTP images must match `allowed_base_urls`, and a local
image file must sit under an allowlisted directory. A relative image path in an
SVG input document resolves against a filesystem `base_url` and fails without
one. An SVG used as an image cannot pull in further images from files or
hosts. Only `data:` references inside it are honored. To keep such images,
inline them as `data:` URLs or flatten the SVG before conversion.

Which outputs load images follows the same pattern as data. PNG, JPEG, and PDF
output, SVG input conversions, and SVG output with `bundle` load them during
conversion. Plain SVG output keeps each image URL for the viewer to load. HTML
usually leaves image loading to the browser, but the font processing described
below evaluates the chart and can load its images during conversion.

Where images are loaded, a blocked image fails the conversion with a
`VLC_ACCESS_DENIED` error. An image that is allowed but cannot be fetched, for
example because the host returned an error, is logged as a warning and left
blank, except with `bundle`, which fails because it must inline the image.

## When Data Is Loaded

Not every output fetches data. Compiling Vega-Lite to Vega and creating a Vega
editor URL embed the specification as written. The data loads later under the
browser's or Vega editor's rules rather than VlConvert's. HTML generation also
defers data loading unless Google Font discovery, an explicit Google Font
request, or local font embedding makes VlConvert evaluate the chart to resolve
fonts. In that case, VlConvert can load the data while generating the file, and
the browser loads it again when the page opens. Rendered outputs, scenegraph
output, and font inspection evaluate the chart and load its data during
conversion.

## Troubleshooting

| Message | Cause | Fix |
| --- | --- | --- |
| `VLC_ACCESS_DENIED: External data url not allowed: https://cdn.jsdelivr.net/.../srv/data/x.csv` | A bare path was joined to the CDN base URL | Use a `file://` URL or a filesystem `base_url` |
| `VLC_ACCESS_DENIED: Filesystem access denied for path: /srv/data/x.csv` | The directory is not in `allowed_base_urls` | Add the directory to the allowlist |
| `VLC_ACCESS_DENIED: External data url not allowed: https://...` or `External image url not allowed` | The URL, or a redirect it returned, matches no allowlist entry | Add a prefix, wildcard host, or scheme entry |
| `Unsupported data URL target after Vega loader sanitize: about:invalid/...` | `base_url` is disabled and the specification uses a relative data URL | Use an absolute URL or set `base_url` |
| `Unsupported image URL about:invalid/...` | `base_url` is disabled and an image uses a relative URL | Use an absolute URL or set `base_url` |
| `HTTP request failed for '...': status 404` | The URL resolved and was allowed, but the host returned an error | Check the URL and the host |

See {doc}`../advanced/configuration` to keep these settings in a config file,
and {doc}`../advanced/troubleshooting` for other failures.

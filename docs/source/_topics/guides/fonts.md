---
title: Fonts and Google Fonts
path: guides/fonts
section: Guides
order: 240
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Fonts and Google Fonts

Fonts change both the look and the layout of a chart. Vega measures labels,
titles, and legends before positioning chart elements, so a substituted font
can shift wrapping and alignment compared with a browser preview.

VlConvert bundles Liberation Sans and loads the fonts installed on the host. You
can also register font directories or request families from Google Fonts. Pick
the approach that matches your deployment:

- Install or register the fonts when you control the rendering host.
- Request specific Google Font families when the host may not have them.
- Enable automatic Google Fonts only for trusted specifications on hosts that
  may use the network.

`missing_fonts` decides what happens when a specification's first-choice font is
unavailable. `fallback` substitutes silently and is the default, `warn` records
a warning, and `error` fails the conversion.

## Make Fonts Available

::::{interface} python
Register a local directory once per process. Use `configure()` for Google Fonts
and the missing-font policy:

```python
import vl_convert as vlc

vlc.register_font_directory("/opt/app/fonts")
vlc.configure(
    google_fonts=[{"family": "Inter", "variants": [(400, "normal")]}],
    missing_fonts="error",
)
```

Set `auto_google_fonts=True` to download a missing first-choice font
automatically. Individual Vega and Vega-Lite conversions also accept a
`google_fonts` argument.
::::

::::{interface} cli
Pass font directories and Google Font requests as global options before the
conversion command:

```bash
vl-convert \
  --font-dir /opt/app/fonts \
  --google-font 'Inter:400,700italic' \
  --missing-fonts error \
  vl2png --input chart.vl.json --output chart.png
```

`--auto-google-fonts` downloads missing first-choice fonts automatically. Each
command creates its own converter, so these options apply to one invocation
unless they also appear in a config file or environment variable.
::::

::::{interface} rust
Register font directories before creating converters. Put Google Fonts and the
missing-font policy in `VlcConfig`:

```rust
use vl_convert_rs::converter::MissingFontsPolicy;
use vl_convert_rs::{
    register_font_directory, GoogleFontRequest, VlcConfig, VlConverter,
};

register_font_directory("/opt/app/fonts")?;
let converter = VlConverter::with_config(VlcConfig {
    google_fonts: vec![GoogleFontRequest {
        family: "Inter".to_string(),
        variants: None,
    }],
    missing_fonts: MissingFontsPolicy::Error,
    ..Default::default()
})?;
```

Set `auto_google_fonts: true` to look up missing first-choice fonts
automatically.
::::

::::{interface} server
Configure fonts when the server starts. This example registers a local
directory, enables automatic Google Fonts, and rejects a conversion whose
first-choice font is still unavailable:

```bash
vl-convert \
  --font-dir /opt/app/fonts \
  --auto-google-fonts \
  --missing-fonts error \
  serve --host 127.0.0.1 --port 3000
```

Clients can pass `google_fonts` in a request body only when the server starts
with `--allow-google-fonts`. Do not enable this for untrusted callers without
render-time budgets. See {doc}`/server/rate-limiting`.
::::

## Limit Automatic Downloads

`google_font_variant_threshold` caps the number of Google Font variants one
conversion may load. Configured, per-conversion, and automatically discovered
families all count toward it. When the cap is reached, the conversion fails
rather than loading another family. This stops a specification with a long
font list from triggering unbounded downloads. One family can carry the total
past the cap when it has several variants.

Google Fonts downloads are controlled by these font options only.
`allowed_base_urls` governs data and image URLs and has no effect on fonts.

::::{interface} server
On a public server, combine the threshold with
`--google-font-cache-miss-penalty-ms`. The penalty charges extra request budget
for every font lookup that misses the on-disk cache, including lookups for
families that do not exist.

```bash
vl-convert \
  --auto-google-fonts \
  --google-font-variant-threshold 16 \
  serve \
  --per-ip-budget-ms 30000 \
  --google-font-cache-miss-penalty-ms 250
```
::::

## Google Fonts Cache

Downloaded Google Fonts are cached on disk and reused across conversions and
process restarts. The cache holds both the Google Fonts CSS responses and the
font files. Its directory is the platform cache directory plus
`vl-convert/google-fonts`, such as `~/.cache/vl-convert/google-fonts` on Linux
or `~/Library/Caches/vl-convert/google-fonts` on macOS. Set the
`VL_CONVERT_FONT_CACHE_DIR` environment variable to move it, or set it to
`none` to disable caching and download fonts on every conversion. Set the
variable before starting VlConvert.

Cached font files are evicted least-recently-used once they exceed a 512 MB
cap. The cap applies to the whole process and can be changed at runtime.

::::{interface} python
```python
print(vlc.google_fonts_cache_dir())
print(vlc.google_fonts_cache_size_mb())
vlc.set_google_fonts_cache_size_mb(128)
```

`set_google_fonts_cache_size_mb(None)` restores the default. Fonts over the new
cap are evicted immediately.
::::

::::{interface} cli
```bash
vl-convert --google-fonts-cache-size-mb 128 \
  vl2png --input chart.vl.json --output chart.png
```

A value of `0` selects the default cap.
::::

::::{interface} rust
```rust
use std::num::NonZeroU64;
use vl_convert_rs::{google_fonts_cache_dir, set_google_fonts_cache_size_mb};

println!("{:?}", google_fonts_cache_dir());
set_google_fonts_cache_size_mb(NonZeroU64::new(128))?;
```

`current_google_fonts_cache_size_mb()` reads the active cap, and passing `None`
restores the default.
::::

::::{interface} server
Pass `--google-fonts-cache-size-mb` before `serve` to set the cap at startup.
`/infoz` reports the cache directory. When the admin listener is enabled,
`GET` and `PUT /admin/config/fonts/cache_size` read and change the cap without
a restart:

```bash
curl -X PUT http://127.0.0.1:3001/admin/config/fonts/cache_size \
  -H "Authorization: Bearer $ADMIN_API_KEY" \
  -H 'Content-Type: application/json' \
  --data '{"max_size_mb": 128}'
```

`{"max_size_mb": null}` restores the default.
::::

## Embed Fonts in SVG and HTML

Set `embed_local_fonts` to include the fonts a chart uses as base64
`@font-face` rules in SVG and HTML output. VlConvert subsets embedded fonts by
default so the output contains only the glyphs the chart uses. Set
`subset_fonts` to `false` only when the text may change later. Passing `bundle`
to an SVG conversion also embeds its fonts and images so the file is
self-contained.

PNG and JPEG contain rendered pixels, and PDF output embeds the fonts it needs,
so consumers of those formats do not need the font files.

Embedding increases output size and redistributes the font file, so confirm
that the font license permits it.

See {doc}`../advanced/font-introspection` to list the fonts VlConvert resolves
for a chart, and
{doc}`../advanced/troubleshooting` when text renders in the wrong font.

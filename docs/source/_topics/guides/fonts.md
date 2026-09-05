---
title: Fonts and Google Fonts
path: guides/fonts
section: Guides
order: 240
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Fonts and Google Fonts

Fonts affect both the appearance and layout of a chart. Vega measures labels,
titles, and legends before it positions chart elements. If the rendering
machine substitutes a different font, text can wrap or align differently from
the browser preview.

VlConvert includes Liberation Sans and loads fonts installed on the host. You
can also register font directories or request fonts from Google Fonts. The
best approach depends on where the chart will run:

- Install or register application fonts when you control the rendering host.
- Request specific Google Font families when the host may not have them.
- Enable automatic Google Fonts only when specifications are trusted and
  network access is acceptable.

`missing_fonts` controls what happens when a specification's first-choice font
cannot be found. Use `fallback` to substitute silently, `warn` to record a
warning, or `error` to reject the conversion.

## Make Fonts Available

::::{interface} python
Register a local directory once for the Python process. Use `configure()` for
Google Fonts and missing-font behavior:

```python
import vl_convert as vlc

vlc.register_font_directory("/opt/app/fonts")
vlc.configure(
    google_fonts=[{"family": "Inter", "variants": [(400, "normal")]}],
    missing_fonts="error",
)
```

To download a missing first-choice font automatically, set
`auto_google_fonts=True`. You can also pass `google_fonts` to an individual
Vega or Vega-Lite conversion.
::::

::::{interface} cli
Pass local font directories and Google Font requests as global options before
the conversion subcommand:

```bash
vl-convert \
  --font-dir /opt/app/fonts \
  --google-font 'Inter:400,700italic' \
  --missing-fonts error \
  vl2png --input chart.vl.json --output chart.png
```

Use `--auto-google-fonts` to download missing first-choice fonts automatically.
Each command creates a new converter, so these options apply only to that
command invocation unless they also appear in a config file or environment
variable.
::::

::::{interface} rust
Register local font directories before creating converters. Put Google Fonts
and missing-font behavior in `VlcConfig`:

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

Set `auto_google_fonts: true` when the converter should look up missing
first-choice fonts automatically.
::::

::::{interface} server
Configure fonts when the server starts. The following example registers a
local directory, permits automatic Google Fonts, and rejects a conversion when
its first-choice font remains unavailable:

```bash
vl-convert \
  --font-dir /opt/app/fonts \
  --auto-google-fonts \
  --missing-fonts error \
  serve --host 127.0.0.1 --port 3000
```

Clients can request `google_fonts` or `auto_google_fonts` in a conversion body
only when the server starts with `--allow-google-fonts`. Do not enable this for
untrusted callers without render-time budgets. See
{doc}`/server/rate-limiting`.
::::

## Limit Automatic Downloads

`google_font_variant_threshold` limits how many font variants VlConvert will
admit while it examines automatically requested families. This prevents a
specification with a long font fallback list from causing an unbounded number
of downloads. A single admitted family can take the total past the threshold
when that family has several variants.

Explicit `google_fonts` requests are not automatic discovery. Request only the
families and variants that the application needs.

Google Fonts access is separate from `allowed_base_urls`. That allowlist
controls data and image URLs referenced by a specification. Font options
control Google Fonts downloads.

::::{interface} server
For a public server, combine `google_font_variant_threshold` with
`--google-font-cache-miss-penalty-ms`. The penalty charges extra request budget
when a font lookup misses the on-disk cache, including a lookup for a family
that does not exist.

```bash
vl-convert \
  --auto-google-fonts \
  --google-font-variant-threshold 16 \
  serve \
  --per-ip-budget-ms 30000 \
  --google-font-cache-miss-penalty-ms 250
```
::::

## Embed Fonts in SVG and HTML

Set `embed_local_fonts` to include available fonts as base64 `@font-face`
rules in SVG or HTML output. VlConvert subsets embedded fonts by default so the
output contains only glyphs used by the chart. Set `subset_fonts` to `false`
only when the output must support text that can change later.

PNG and JPEG contain rendered pixels, so their consumers do not need the font
files. PDF output is self-contained for normal viewing. Its consumer does not
need the rendering host's font files.

Embedding increases output size. It can also redistribute the font file, so
confirm that the font license permits embedding.

See {doc}`../advanced/font-introspection` to inspect the fonts that VlConvert
can find and the fonts used by a conversion.

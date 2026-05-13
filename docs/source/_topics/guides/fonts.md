---
title: Fonts and Google Fonts
path: guides/fonts
section: Guides
order: 240
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Fonts and Google Fonts

vl-convert starts with its built-in font baseline plus registered font
directories and system fonts. Configured Google Fonts are overlaid for the
conversion that needs them, then removed from the worker's font database. When
`auto_google_fonts` is enabled, vl-convert inspects the spec's first-choice font
families and downloads matching Google Fonts as needed.

If a first-choice family is not available locally and is not provided by Google
Fonts, `missing_fonts` controls whether vl-convert falls back silently, logs a
warning, or returns an error. `allowed_base_urls` controls data loading from
Vega specs; Google Fonts downloads are controlled by the font options here and
by the server budget controls.

::::{interface} python
`register_font_directory()` and `configure()` update the Python process
configuration used by later conversions.

```python
import vl_convert as vlc

vlc.register_font_directory("/opt/app/fonts")
vlc.configure(auto_google_fonts=True, google_font_variant_threshold=16)
```
::::

::::{interface} cli
Font directories and Google Fonts options apply to the command invocation that
receives the flags.

```bash
vl-convert --auto-google-fonts \
  --google-font-variant-threshold 16 \
  vl2png --font-dir /opt/app/fonts \
  --input chart.vl.json --output chart.png
```
::::


::::{interface} rust
Font behavior is part of `VlcConfig`; pass the config when constructing the
converter.

```rust
use vl_convert_rs::{GoogleFontRequest, VlcConfig, VlConverter};

let converter = VlConverter::with_config(VlcConfig {
    auto_google_fonts: true,
    google_font_variant_threshold: std::num::NonZeroU64::new(16),
    google_fonts: vec![GoogleFontRequest {
        family: "Inter".to_string(),
        variants: None,
    }],
    ..Default::default()
})?;
```
::::


::::{interface} server
Startup flags set server defaults for conversion requests. Runtime font
configuration can also be changed through the admin API when it is enabled.

```bash
vl-convert serve \
  --auto-google-fonts=true \
  --google-font-variant-threshold 16 \
  --google-font-cache-miss-penalty-ms 250
```

See {doc}`/server/rate-limiting` for public server controls.
::::

## Font Embedding

HTML and SVG output can embed local fonts with `embed_local_fonts`. Embedded
fonts are subset by default, so the output contains only the glyphs used by the
chart. PNG, JPEG, and PDF rendering use the registered font database during
rendering and do not need CSS `@font-face` output.

## Public Server Controls

For public endpoints that allow automatic Google Fonts, set both
`google_font_variant_threshold` and `google_font_cache_miss_penalty_ms`. The
variant threshold stops admitting additional font families after enough variants
have been resolved; the cache-miss penalty charges request budget for work that
misses the local Google Fonts cache, including missing-family probes.

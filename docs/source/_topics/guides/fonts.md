---
title: Fonts and Google Fonts
path: guides/fonts
section: Guides
order: 240
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Fonts and Google Fonts

VlConvert uses local system fonts by default and can fetch Google Fonts when
configured to do so.

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
    google_font_variant_threshold: Some(16),
    google_fonts: vec![GoogleFontRequest::family("Inter")],
    ..Default::default()
});
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

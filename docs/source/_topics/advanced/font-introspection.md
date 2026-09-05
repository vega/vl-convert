---
title: Font Introspection
path: advanced/font-introspection
section: Advanced
order: 460
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Inspect Fonts Used by a Chart

Font inspection evaluates a Vega or Vega-Lite specification and reports the
font families, weights, and styles used by the resulting chart. Use it to find
missing fonts, prepare web font requests, or confirm that a deployment contains
the same fonts as a development machine.

Each result identifies the family and whether VlConvert resolved it locally or
through Google Fonts. Google Font results can also include a stylesheet URL,
HTML link tag, and CSS import rule.

::::{interface} python
```python
import vl_convert as vlc

fonts = vlc.vegalite_fonts(spec)
for font in fonts:
    print(font["name"], font["source"], font["variants"])
```

Set `include_font_face=True` to include generated `@font-face` CSS for available
variants. This can make the returned data much larger.
::::

::::{interface} cli
```bash
vl-convert vl2fonts \
  --input chart.vl.json \
  --output fonts.json \
  --pretty
```

Use `vg2fonts` for Vega input. Add `--include-font-face` only when the output
needs embedded font CSS.
::::

::::{interface} rust
```rust
let fonts = converter
    .vegalite_fonts(
        spec,
        Default::default(),
        false, // Do not discover Google Fonts automatically
        false, // Do not embed local fonts
        false, // Do not include @font-face CSS
        true,  // Subset fonts if CSS is requested
    )
    .await?;

for font in fonts {
    println!("{}: {:?}", font.name, font.variants);
}
```
::::

::::{interface} server
Send a normal specification request to `POST /vegalite/fonts` or
`POST /vega/fonts`:

```json
{
  "spec": {"mark": "text", "data": {"values": []}},
  "include_font_face": false
}
```

The response is a JSON array of font records. Font downloads follow the
server's Google Fonts settings and request gates.
::::

Font inspection performs chart compilation and evaluation, so it can load data
and consume similar resources to a render. Apply the same access controls and
resource limits that you use for conversion endpoints. See {doc}`../guides/fonts`
for registration, fallback, and embedding options.

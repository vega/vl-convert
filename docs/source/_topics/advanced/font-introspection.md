---
title: Font Introspection
path: advanced/font-introspection
section: Advanced
order: 475
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Inspect Fonts Resolved for a Chart

Font inspection evaluates a Vega or Vega-Lite specification and reports the
fonts VlConvert can supply for it, with the weights and styles the chart uses.
Use it to prepare web font links for HTML output, generate `@font-face` CSS, or
confirm that a deployment resolves the same fonts as a development machine.

The result lists only fonts that VlConvert resolved, so it depends on the
converter's font settings:

- Fonts found in the Google Fonts catalog are listed when `auto_google_fonts`
  is enabled or the family was requested through `google_fonts`. These records
  include a stylesheet URL, an HTML link tag, and a CSS import rule.
- Fonts installed on the host or registered from a directory are listed only
  when `embed_local_fonts` is enabled.
- Fonts that cannot be resolved are never listed. Set `missing_fonts` to `warn`
  or `error` to have them reported instead.

With the default settings both switches are off and the result is an empty
list. The examples below enable `embed_local_fonts` and inspect the bundled
Liberation Sans family. Save this specification:

:::{dropdown} chart.vl.json

```{literalinclude} /_examples/font-introspection.vl.json
:language: json
```
:::

::::{interface} python
```python
import json
from pathlib import Path

import vl_convert as vlc

vlc.configure(embed_local_fonts=True)
spec = Path("chart.vl.json").read_text(encoding="utf-8")
fonts = vlc.vegalite_fonts(spec)
print(json.dumps(fonts, indent=2))
```

`vegalite_fonts()` reads `embed_local_fonts` from the converter configuration.
Its `auto_google_fonts` argument overrides the configured value for one call.
Set `include_font_face=True` to include generated `@font-face` CSS for each
variant, which can make the result much larger.
::::

::::{interface} cli
```bash
vl-convert --embed-local-fonts \
  vl2fonts --input chart.vl.json --output fonts.json --pretty
```

Add `--auto-google-fonts` to include matches from the Google Fonts catalog. Use
`vg2fonts` for Vega input, and `--include-font-face` only when the output needs
embedded font CSS.
::::

::::{interface} rust
The Rust method takes the font switches as arguments instead of reading them
from `VlcConfig`:

```rust
use vl_convert_rs::VlConverter;

let spec = std::fs::read_to_string("chart.vl.json")?;
let converter = VlConverter::new();
let fonts = converter
    .vegalite_fonts(
        spec,
        Default::default(),
        false, // auto_google_fonts: do not probe the Google Fonts catalog
        true,  // embed_local_fonts: list locally available fonts
        false, // include_font_face: omit @font-face CSS
        true,  // subset_fonts: subset the CSS when it is included
    )
    .await?;

for font in fonts {
    println!("{}: {:?}", font.name, font.variants);
}
```
::::

::::{interface} server
Start the server with `--embed-local-fonts`, `--auto-google-fonts`, or both,
then send a normal specification request to `POST /vegalite/fonts` or
`POST /vega/fonts`. For this local-font example, start the server with:

```bash
vl-convert --embed-local-fonts \
  serve --host 127.0.0.1 --port 3000
```

Save this complete body as `request.json`:

:::{dropdown} request.json

```{literalinclude} /_generated/requests/font-introspection.json
:language: json
```
:::

```bash
curl http://127.0.0.1:3000/vegalite/fonts \
  -H 'Content-Type: application/json' \
  --data-binary @request.json
```

The response is a JSON array of font records. Google Fonts lookups follow the
server's font settings.
::::

The result includes this local-font record. The `font_face` field is `null`
because the examples set `include_font_face` to false:

```json
{
  "name": "Liberation Sans",
  "source": {"type": "local"},
  "variants": [
    {"weight": "400", "style": "normal", "font_face": null},
    {"weight": "700", "style": "normal", "font_face": null}
  ],
  "url": null,
  "link_tag": null,
  "import_rule": null
}
```

Font inspection compiles and evaluates the chart, so it loads data and uses
resources much like a render. Apply the same access controls and resource
limits as for conversions. See {doc}`../guides/fonts` for registration,
fallback, and embedding options.

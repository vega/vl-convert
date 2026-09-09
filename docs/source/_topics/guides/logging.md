---
title: Logging and Warnings
path: guides/logging
section: Guides
order: 270
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Logging and Warnings

Vega and Vega-Lite report warnings while compiling or evaluating a chart. A
conversion can succeed even though Vega dropped an invalid property or
recovered from a data problem, so check these messages when a chart does not
look as expected.

The examples use a logarithmic scale whose inferred domain includes zero. Save
this Vega-Lite specification:

:::{dropdown} chart.vl.json
:open:

```{literalinclude} /_examples/warning-demo.vl.json
:language: json
```
:::

::::{interface} python
VlConvert forwards its messages to the `vl_convert` logger in Python's
`logging` module. Configure a handler before converting if the application does
not already configure logging:

```python
import logging
from pathlib import Path

import vl_convert as vlc

logging.basicConfig(
    level=logging.WARNING,
    format="%(levelname)s:%(name)s:%(message)s",
)
logging.getLogger("vl_convert").setLevel(logging.WARNING)

spec = Path("chart.vl.json").read_text(encoding="utf-8")
svg = vlc.vegalite_to_svg(spec)
Path("chart.svg").write_text(svg, encoding="utf-8")
```

The conversion succeeds and emits this warning:

```text
WARNING:vl_convert:Log scale domain includes zero: [0,200]
```

Use `INFO` or `DEBUG` temporarily while diagnosing a problem.
::::

::::{interface} cli
The CLI writes logs to standard error, so standard output stays available for
conversion results.

```console
$ vl-convert --log-level warn \
>   vl2svg --input chart.vl.json --output chart.svg
```

The command succeeds and writes a timestamped warning to standard error. The
relevant part is:

```text
...
WARN vl_convert: Log scale domain includes zero: [0,200]
```

`--log-filter` accepts a `tracing-subscriber` filter directive for finer
control and takes priority over `--log-level`:

```console
$ vl-convert --log-filter 'vl_convert=debug' \
>   vl2svg --input chart.vl.json --output chart.svg
```
::::

::::{interface} rust
Every output struct carries the messages Vega produced in `logs`:

```rust
use vl_convert_rs::{VlConverter, VlOpts};

let spec = std::fs::read_to_string("chart.vl.json")?;
let converter = VlConverter::new();
let output = converter
    .vegalite_to_svg(spec, VlOpts::default(), Default::default())
    .await?;

for entry in output.logs {
    eprintln!("{}: {}", entry.level, entry.message);
}
```

This prints:

```text
WARN: Log scale domain includes zero: [0,200]
```

The crate also emits these messages, plus its own operational messages, through
the `log` crate under the `vl_convert` target. If no logger is installed when
the first converter is created, the crate installs `env_logger`, so setting
`RUST_LOG=vl_convert=info` works without any code. To use a different logger,
initialize it before creating a converter.
::::

::::{interface} server
Use structured JSON logs in deployed services:

```console
$ vl-convert serve \
>   --log-format json --log-level info \
>   --port 3000
```

The server logs request identifiers, status, duration, and budget information
alongside conversion diagnostics. See {doc}`/server/logging` for the request
fields and proxy behavior.

Each conversion response also carries Vega diagnostics in the `X-VLC-Logs`
header. Save this body as `request.json`:

:::{dropdown} request.json
:open:

```{literalinclude} /_generated/requests/logging-svg.json
:language: json
```
:::

Use `--dump-header` to save the response headers:

```console
$ curl http://127.0.0.1:3000/vegalite/svg \
>   -H 'Content-Type: application/json' \
>   --data-binary @request.json \
>   --dump-header headers.txt \
>   --output chart.svg
```

`X-VLC-Logs` contains a JSON array. Its first entry is:

```text
WARN: Log scale domain includes zero: [0,200]
```
::::

See {doc}`../advanced/troubleshooting` for how to use these messages when a
conversion fails or renders incorrectly.

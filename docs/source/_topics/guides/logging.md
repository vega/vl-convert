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

::::{interface} python
VlConvert forwards its messages to the `vl_convert` logger in Python's
`logging` module. Configure a handler before converting if the application does
not already configure logging:

```python
import logging
import vl_convert as vlc

logging.basicConfig(level=logging.WARNING)
logging.getLogger("vl_convert").setLevel(logging.WARNING)

svg = vlc.vegalite_to_svg(spec)
```

Use `INFO` or `DEBUG` temporarily while diagnosing a problem.
::::

::::{interface} cli
The CLI writes logs to standard error, so standard output stays available for
conversion results.

```bash
vl-convert --log-level warn \
  vl2svg --input chart.vl.json --output chart.svg
```

`--log-filter` accepts a `tracing-subscriber` filter directive for finer
control and takes priority over `--log-level`:

```bash
vl-convert --log-filter 'vl_convert=debug' \
  vl2svg --input chart.vl.json --output chart.svg
```
::::

::::{interface} rust
Every output struct carries the messages Vega produced in `logs`:

```rust
for entry in output.logs {
    eprintln!("{}: {}", entry.level, entry.message);
}
```

The crate also emits these messages, plus its own operational messages, through
the `log` crate under the `vl_convert` target. If no logger is installed when
the first converter is created, the crate installs `env_logger`, so setting
`RUST_LOG=vl_convert=info` works without any code. To use a different logger,
initialize it before creating a converter.
::::

::::{interface} server
Use structured JSON logs in deployed services:

```bash
vl-convert --log-format json --log-level info \
  serve --host 127.0.0.1 --port 3000
```

The server logs request identifiers, status, duration, and budget information
alongside conversion diagnostics. See {doc}`/server/logging` for the request
fields and proxy behavior.
::::

See {doc}`../advanced/troubleshooting` for how to use these messages when a
conversion fails or renders incorrectly.

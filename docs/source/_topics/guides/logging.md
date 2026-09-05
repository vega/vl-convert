---
title: Logging
path: guides/logging
section: Guides
order: 300
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Logging and Warnings

Vega and Vega-Lite can report warnings while they compile or evaluate a chart.
Pay attention to these messages because a conversion can succeed while Vega
drops an invalid property or encounters a recoverable data problem.

::::{interface} python
VlConvert sends diagnostic messages to Python's `vl_convert` logger. Configure
a handler before converting if the application does not already configure
logging:

```python
import logging
import vl_convert as vlc

logging.basicConfig(level=logging.WARNING)
logging.getLogger("vl_convert").setLevel(logging.WARNING)

svg = vlc.vegalite_to_svg(spec)
```

Use `INFO` or `DEBUG` temporarily when diagnosing a problem. Avoid enabling
verbose logging globally in a library.
::::

::::{interface} cli
The CLI writes logs to standard error, leaving standard output available for
conversion results.

```bash
vl-convert --log-level warn \
  vl2svg --input chart.vl.json --output chart.svg
```

`--log-filter` accepts a `tracing-subscriber` filter for finer control and
takes priority over `--log-level`:

```bash
vl-convert --log-filter 'vl_convert=debug' \
  vl2svg --input chart.vl.json --output chart.svg
```
::::

::::{interface} rust
Each conversion output includes a `logs` collection with messages produced by
Vega:

```rust
for entry in output.logs {
    eprintln!("{}: {}", entry.level, entry.message);
}
```

The crate also uses Rust's `log` facade for operational messages. Applications
must install a compatible logger if they want to receive them. For example, add
`env_logger` and initialize it once at process startup:

```rust
env_logger::Builder::from_env(
    env_logger::Env::default().default_filter_or("vl_convert=warn"),
)
.init();
```
::::

::::{interface} server
Use structured JSON logs in deployed services:

```bash
vl-convert --log-format json --log-level info \
  serve --host 127.0.0.1 --port 3000
```

The server logs request identifiers, status, duration, and budget information
in addition to conversion diagnostics. See {doc}`/server/logging` for the
request fields and proxy behavior.
::::

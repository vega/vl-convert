---
title: Installation
path: getting-started/installation
section: Getting Started
order: 100
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Installation

Install the interface that matches the application boundary.

::::{interface} python
```bash
pip install vl-convert-python
```

Import the package as `vl_convert`.
::::

::::{interface} cli
```bash
cargo install vl-convert
```

Run `vl-convert --help` to list commands.
::::


::::{interface} rust
```toml
[dependencies]
vl-convert-rs = "2"
```

Use the crate through `vl_convert_rs`.
::::


::::{interface} server
```bash
cargo install vl-convert
vl-convert serve --host 127.0.0.1 --port 3000
```

The server exposes conversion endpoints plus `/healthz`, `/readyz`, and
`/infoz`.
::::

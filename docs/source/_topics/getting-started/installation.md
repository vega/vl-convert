---
title: Installation
path: getting-started/installation
section: Getting Started
order: 100
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Installation

::::{not-interface} server
Already using 1.x? See {doc}`upgrading` for what changed in version 2.
::::

::::{interface} python
Install the package from PyPI. Prebuilt wheels are published for Linux, macOS,
and Windows, and Python 3.7 or later is required.

```bash
python -m pip install vl-convert-python
```

The distribution is named `vl-convert-python`, and Python imports it as
`vl_convert`. Confirm that the installation works:

```bash
python -c "import vl_convert as vlc; print(vlc.__version__)"
```
::::

::::{interface} cli
Install a Rust toolchain, then build and install the `vl-convert` executable
from crates.io:

```bash
cargo install vl-convert --locked
```

Confirm that the executable is on your `PATH`:

```bash
vl-convert --version
```

`vl-convert --help` lists the conversion commands.
::::

::::{interface} rust
Add the crate to `Cargo.toml`:

:::{dropdown} Cargo.toml
:open:

```toml
[dependencies]
vl-convert-rs = "2"
```
:::

The crate is imported as `vl_convert_rs`. Conversion methods are asynchronous,
so your application needs an async runtime such as Tokio. The
{doc}`quick-start` shows a complete Tokio setup.
::::

::::{interface} server
The server is the `serve` subcommand of the `vl-convert` executable. Install a
Rust toolchain, then build and install the executable from crates.io:

```bash
cargo install vl-convert --locked
```

Confirm the installation, then start a local server:

```bash
vl-convert --version
vl-convert serve --port 3000
```

The server keeps running and listens on port 3000. Leave this terminal open
while you follow the {doc}`quick-start`.
::::

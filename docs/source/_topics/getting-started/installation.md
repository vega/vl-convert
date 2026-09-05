---
title: Installation
path: getting-started/installation
section: Getting Started
order: 100
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Installation

Install the package or executable for the interface you plan to use.

::::{interface} python
VlConvert supports Python 3.7 and later. Install the prebuilt package from
PyPI:

```bash
python -m pip install vl-convert-python
```

The distribution is named `vl-convert-python`, and Python imports it as
`vl_convert`. Confirm that the installation works:

```bash
python -c "import vl_convert as vlc; print(len(vlc.get_themes()), 'themes available')"
```
::::

::::{interface} cli
Install a Rust toolchain with Cargo, then install the `vl-convert` executable
from crates.io:

```bash
cargo install vl-convert --locked
```

Confirm that the executable is on your `PATH`:

```bash
vl-convert --version
```

Run `vl-convert --help` to list the conversion commands.
::::

::::{interface} rust
Add the library to your application's `Cargo.toml`:

```toml
[dependencies]
vl-convert-rs = "2"
```

The Rust import name is `vl_convert_rs`. Conversion methods are asynchronous,
so applications also need an async executor such as Tokio.
::::

::::{interface} server
The server is a subcommand of the `vl-convert` executable. Install a Rust
toolchain with Cargo, then install the executable from crates.io:

```bash
cargo install vl-convert --locked
```

Confirm that the installation works, then start a local server:

```bash
vl-convert --version
vl-convert serve --host 127.0.0.1 --port 3000
```

The command continues running and listens for HTTP requests on port 3000. Keep
this terminal open while you follow the quick start.
::::

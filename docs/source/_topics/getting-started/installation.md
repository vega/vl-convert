---
title: Installation
path: getting-started/installation
section: Getting Started
order: 100
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Installation

Version 2 is currently published as a release candidate. Package managers do
not select prereleases by default, so the commands below opt in to version 2
prereleases.

::::{not-interface} server
Already using 1.x? See {doc}`upgrading` for what changed in version 2.
::::

::::{interface} python
Install the package from PyPI. Prebuilt wheels are published for Linux, macOS,
and Windows, and Python 3.10 or later is required.

```console
$ python -m pip install --pre --upgrade "vl-convert-python>=2.0.0rc1,<3"
```

The distribution is named `vl-convert-python`, and Python imports it as
`vl_convert`. Confirm that the installation works:

```console
$ python -c "import vl_convert as vlc; print(vlc.__version__)"
```
::::

::::{interface} cli
Install a Rust toolchain, then build and install the `vl-convert` executable
from crates.io:

```console
$ cargo install vl-convert --version '^2.0.0-rc1' --locked
```

Confirm that the executable is on your `PATH`:

```console
$ vl-convert --version
```

`vl-convert --help` lists the conversion commands.
::::

::::{interface} rust
Add the crate to `Cargo.toml`:

:::{dropdown} Cargo.toml
:open:

```toml
[dependencies]
vl-convert-rs = "2.0.0-rc1"
```
:::

The crate is imported as `vl_convert_rs`. Conversion methods are asynchronous,
so your application needs an async runtime such as Tokio. The
{doc}`quick-start` shows a complete Tokio setup.
::::

::::{interface} server
The server is the `serve` subcommand of the `vl-convert` executable. Install a
Rust toolchain, then build and install the executable from crates.io:

```console
$ cargo install vl-convert --version '^2.0.0-rc1' --locked
```

Confirm the installation, then start a local server:

```console
$ vl-convert --version
$ vl-convert serve --port 3000
```

The server keeps running and listens on port 3000. Leave this terminal open
while you follow the {doc}`quick-start`.
::::

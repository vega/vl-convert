---
title: Installation
path: getting-started/installation
section: Getting Started
order: 100
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Installation

The commands below install VlConvert {vlc_version}, the release covered by these docs.

::::{not-interface} server
Already using 1.x? See {doc}`upgrading` for what changed in version 2.
::::

::::{interface} python
Install [uv](https://docs.astral.sh/uv/getting-started/installation/), then add the package to a Python project. `uv add` records the dependency in `pyproject.toml`, updates `uv.lock`, and installs it in the project's environment. Prebuilt wheels are published for Linux, macOS, and Windows, and Python 3.10 or later is required.

```console
$ uv add "vl-convert-python=={vlc_python_version}"
```

The PyPI package is named `vl-convert-python`, and Python imports it as `vl_convert`. Confirm that the installation works:

```console
$ uv run python -c "import vl_convert as vlc; print(vlc.__version__)"
```

Example output (the installed version may differ):

```{program-output} python -c "import vl_convert as vlc; print(vlc.__version__)"
```
::::

::::{interface} server
The server is the `serve` subcommand of the `vl-convert` CLI executable.
::::

::::{interface} cli server
## Prebuilt Executable

Download the ZIP archive for your platform and `SHA256SUMS` from the [VlConvert {vlc_version} release](https://github.com/vega/vl-convert/releases/tag/v{vlc_version}).

| Platform | Archive |
| --- | --- |
| Linux x86-64 | `vl-convert_linux-64.zip` |
| Linux ARM64 | `vl-convert_linux-aarch64.zip` |
| macOS Intel | `vl-convert_osx-64.zip` |
| macOS Apple Silicon | `vl-convert_osx-arm64.zip` |
| Windows x86-64 | `vl-convert_win-64.zip` |

Calculate the archive's SHA-256 checksum with the command for your platform. Replace `<ARCHIVE>` with the downloaded filename:

| Platform | Command |
| --- | --- |
| Linux | `sha256sum <ARCHIVE>` |
| macOS | `shasum -a 256 <ARCHIVE>` |
| Windows PowerShell | `Get-FileHash <ARCHIVE> -Algorithm SHA256` |

Compare the result with the matching filename in `SHA256SUMS`. Do not use the archive if the checksums differ.

Extract the archive into a new directory, then add its `bin` subdirectory to your `PATH`. Confirm the installation:

```console
$ vl-convert --version
```

## Build from Source

To build the executable yourself, install a Rust toolchain and run:

```console
$ cargo install vl-convert --version '={vlc_version}' --locked
```
::::

::::{interface} cli
`vl-convert --help` lists the conversion commands.
::::

::::{interface} rust
Add these dependencies to `Cargo.toml`:

:::{dropdown} Cargo.toml
:open:

```toml
[dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
vl-convert-rs = "={vlc_version}"
```
:::

The crate is imported as `vl_convert_rs`. Conversion methods are asynchronous and require a Tokio runtime for full functionality. The {doc}`quick-start` shows a complete Tokio setup.
::::

::::{interface} server
Follow the {doc}`quick-start` to start the server and send a conversion request.
::::

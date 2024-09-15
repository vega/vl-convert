# Devlopment
The vl-convert project consists of both Rust and Python components. The project uses [Pixi](https://pixi.sh/latest/) to manage the development environment. Pixi handles the installation of all the development dependencies including Python and Rust themselves. If you don't have Pixi installed, follow the instructions at https://pixi.sh/

# Running Rust tests
Once pixi is installed, you can run the various test suites using Pixi commands.

```bash
pixi run test-rs  # Core Rust tests
pixi run test-cli  # Tests for the CLI interface
```

# Running Python tests
First build the Python library in development mode so that it is present in the pixi environment
```bash
pixi run dev-py
```

Then run the Python tests

```bash
pixi run test-py
```

# Debug Logging
To enable logging, set the RUST_LOG environment variable to info, warn, or error
```
RUST_LOG=info
```

# Updating Licenses
vl-convert uses the `cargo bundle-licenses` to bundle the licenses of its Rust dependencies for inclusion in Python packages. When a Rust dependency is changed, rebuild the license files with

```bash
pixi run bundle-licenses
```

If the generated license files are out of date, 

# Build wheels

You can build the Python wheel for your architecture with the `build-py` Pixi task

```bash
pixi run build-py
```

# Vendor JavaScript Dependencies
vl-convert embeds vendored copies of all the JavaScript libraries it uses. The `vendor` Pixi task performs this 
download

```bash
pixi run vendor
```

For more information on the vendoring process, see [vl-convert-vendor/README.md](vl-convert-vendor/README.md). 

# Release process
Releases of VlConvert crates are handled using [cargo-workspaces](https://github.com/pksunkara/cargo-workspaces), which can be installed with:

```bash
pixi shell
cargo install cargo-workspaces
```

## Tagging and publish to crates.io
Check out the main branch, then tag and publish a new version of the `vl-convert` and `vl-convert-rs` crates with:

(replacing `0.1.0` with the desired version)

```bash
pixi shell
cargo ws publish --all --force "vl-convert*" custom 0.1.0
```

## Publish Python packages to PyPI
The `cargo ws publish ...` command above will push a commit to the `main` branch. This push to `main` will trigger CI, including the "Publish to PyPI" job. This job must be approved manually in the GitHub interface. After it is approved it will run and publish the Python packages to PyPI.

## Create GitHub Release
Create a new GitHub release using the `v0.1.0` tag.



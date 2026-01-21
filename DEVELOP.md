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

# Linux Wheel Builds and V8

Building Python wheels for Linux requires special handling of V8 due to position-independent code (PIC) requirements.

## The Problem

When building `vl-convert-python` as a shared library (`.so`) for Linux, we encounter linker errors like:

```
relocation R_X86_64_TPOFF32 against symbol `_ZN2v88internal18g_current_isolate_E'
can not be used when making a shared object
```

This error occurs because:

1. **Python wheels are shared libraries**: The compiled `.so` file gets loaded via `dlopen()` at runtime
2. **V8 uses thread-local storage (TLS)**: V8 uses the "local-exec" TLS model for performance, which assumes variables are in the main executable
3. **TLS model incompatibility**: The `R_X86_64_TPOFF32` relocation type used by local-exec TLS cannot be used in shared objects because the offset isn't known at link time

This issue is tracked in [denoland/rusty_v8#1706](https://github.com/denoland/rusty_v8/issues/1706).

## The Solution

We build V8 from source with these GN arguments:

```
GN_ARGS="v8_monolithic=true v8_monolithic_for_shared_library=true is_component_build=false v8_enable_temporal_support=false enable_rust=false treat_warnings_as_errors=false symbol_level=0"
```

### GN Arguments Explained

| Argument | Value | Purpose |
|----------|-------|---------|
| `v8_monolithic` | `true` | Build all V8 functionality into a single static library |
| `v8_monolithic_for_shared_library` | `true` | Builds V8 with position-independent code and compatible TLS model for linking into shared libraries |
| `is_component_build` | `false` | Build static libraries, not shared libraries (we link statically into our .so) |
| `v8_enable_temporal_support` | `false` | Disable TC39 Temporal API. When enabled, V8 requires linking `temporal_rs` Rust library which needs `known-target-triples.txt` from Chromium. See [chromium build/rust](https://chromium.googlesource.com/chromium/src/build/config/+/main/rust.gni) |
| `enable_rust` | `false` | Disable V8's internal Rust components. Required because building V8's Rust code outside Chromium requires `known-target-triples.txt` which isn't available in standalone builds |
| `treat_warnings_as_errors` | `false` | Allow build to succeed despite compiler warnings |
| `symbol_level` | `0` | Minimal debug symbols to reduce binary size and build time |

### Why Not Use Pre-built rusty_v8 Binaries?

The [rusty_v8](https://github.com/denoland/rusty_v8) project publishes pre-built static libraries, but these are built with:
- `enable_rust = true`
- `v8_enable_temporal_support = true`

These settings cause the TLS/PIC issues described above when linking into a Python wheel. Building from source with our custom flags resolves this.

## Pre-built V8 Workflow

To avoid 1+ hour V8 builds on every CI run, we maintain pre-built V8 binaries:

1. **`build-v8.yml`**: Manually triggered workflow that builds V8 from source and uploads `librusty_v8-{platform}.a` to GitHub Releases (tagged `v8-{version}`)

2. **`Release.yml`**: Downloads pre-built V8 from releases if available, falls back to from-source build otherwise

The V8 version is determined from `Cargo.lock` (the `v8` crate version).

### Triggering a V8 Build

When updating the Deno/V8 version:

1. Update dependencies in `Cargo.toml`
2. Run `cargo update` to update `Cargo.lock`
3. Merge to `main` - the workflow runs automatically and checks if a pre-built V8 exists
4. If no pre-built exists for the new version, V8 builds from source (~1 hour) and uploads to releases

To trigger manually (e.g., to force a rebuild):

**Via CLI:**
```bash
gh workflow run build-v8.yml
gh workflow run build-v8.yml -f force_rebuild=true  # rebuild even if release exists
```

**Via Web UI:**
1. Go to Actions → "Build V8" workflow
2. Click "Run workflow" dropdown
3. Optionally check "Force rebuild even if release exists"
4. Click "Run workflow"

## References

- [rusty_v8#1706](https://github.com/denoland/rusty_v8/issues/1706) - Original fPIC relocation issue
- [V8 Build Documentation](https://v8.dev/docs/build-gn) - Official V8 GN build docs
- [v8-users mailing list](https://www.mail-archive.com/v8-users@googlegroups.com/msg14918.html) - Discussion of R_X86_64_TPOFF32 errors
- [rusty_v8 README](https://github.com/denoland/rusty_v8/blob/main/README.md) - Building from source with `V8_FROM_SOURCE`

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



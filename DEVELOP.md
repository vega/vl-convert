# Development
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
```sh
RUST_LOG=info
```

# Updating Licenses
vl-convert uses the `cargo bundle-licenses` to bundle the licenses of its Rust dependencies for inclusion in Python packages. When a Rust dependency is changed, rebuild the license files with

```bash
pixi run bundle-licenses
```

# Build wheels

You can build the Python wheel for your architecture with the `build-py` Pixi task

```bash
pixi run build-py
```

# Linux Wheel Builds and V8

Linux Python wheels link V8 into a shared library and therefore require a position-independent V8 archive. The upstream `rusty_v8` release archives support this use case by enabling `v8_monolithic_for_shared_library`. See [denoland/rusty_v8#2008](https://github.com/denoland/rusty_v8/pull/2008) for the implementation.

CI and release builds use the archive downloaded by the `v8` crate. No custom V8 source build or vl-convert-hosted archive is required. The original linker problem is documented in [denoland/rusty_v8#1706](https://github.com/denoland/rusty_v8/issues/1706).

# Vendor JavaScript Dependencies
vl-convert embeds vendored copies of all the JavaScript libraries it uses. The `vendor` Pixi task performs this 
download

```bash
pixi run vendor
```

For more information on the vendoring process, see [vl-convert-vendor/README.md](vl-convert-vendor/README.md). 

# Release process

## Versions

The Rust crates and `vl-convert-python` share one version. The private `vl-convert-vendor` crate remains at `0.0.0`.

## Prepare a release

Choose the next unused Rust SemVer without the `v` prefix. From a clean worktree, optionally validate the release inputs without changing local or remote state:

```sh
pixi run prepare-release <NEXT_VERSION> --dry-run
```

Create the release branch and draft pull request:

```sh
pixi run prepare-release <NEXT_VERSION>
```

The command creates and pushes `release/v<NEXT_VERSION>` from the latest `origin/main`, updates the workspace version and exact internal requirements, refreshes the lockfile, and opens a draft pull request. Merge the pull request after its checks pass, then wait for the merged commit's checks on `main` to pass.

## Publish a release

Create and publish a GitHub Release with a new `v<NEXT_VERSION>` tag that targets the merged version commit on `main`. Mark the GitHub Release as a prerelease when the version has a prerelease component such as `-rc3`.

Publishing the GitHub Release starts the `Release` workflow. The workflow builds and verifies the artifacts before it requests approval through the protected `release` environment. It then publishes the Rust crates and Python distributions and attaches the CLI archives, wheels, source distribution, and checksums.

WARNING: Registry uploads are irreversible. After an upload starts, do not move, delete, or reuse the tag. Prepare a new version when the source or workflow must change.

## Retry or test the workflow

For a transient failure, rerun the failed jobs on the same workflow run. The retry skips exact crate versions and Python files that are already published.

Use the manual `workflow_dispatch` trigger to test builds without publishing. The `linux_wheels_only` input limits the run to the two Linux wheel jobs.

## One-time repository setup

Configure a GitHub environment named `release` with a required reviewer and restrict it to `v*` tags. Add a `v*` tag ruleset that prevents unauthorized updates and deletions.

Configure trusted publishing with the `vega/vl-convert` repository, `.github/workflows/Release.yml` workflow, and `release` environment for:

- The `vl-convert-python` project on PyPI.
- The `vl-convert`, `vl-convert-canvas2d`, `vl-convert-canvas2d-deno`, `vl-convert-google-fonts`, `vl-convert-rs`, and `vl-convert-server` crates on crates.io.

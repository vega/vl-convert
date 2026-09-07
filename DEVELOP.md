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

The Rust crates and `vl-convert-python` use one release version. The private `vl-convert-vendor` crate stays at `0.0.0`.

## One-time repository setup

The release workflow expects a GitHub environment named `release`. Configure the environment with the required reviewer and restrict it to tags that match `v*`. Add a `v*` tag ruleset that prevents unauthorized tag updates and deletions.

Configure PyPI trusted publishing for the `vl-convert-python` project. Use the `vega/vl-convert` repository, `.github/workflows/Release.yml` workflow, and `release` environment.

Configure the same workflow and environment as the trusted publisher for `vl-convert`, `vl-convert-canvas2d`, `vl-convert-canvas2d-deno`, `vl-convert-google-fonts`, `vl-convert-rs`, and `vl-convert-server` on crates.io.

## Prepare the release pull request

Define `<NEXT_VERSION>` as the next unused Rust SemVer without the `v` prefix. For example, use `2.0.0-rc3` for a release candidate.

Run the dry run from a clean worktree:

```sh
pixi run prepare-release <NEXT_VERSION> -- --dry-run
```

The dry run validates the workspace, version, Git remote, and release branch and tag names. It does not change local or remote state.

Create the release branch and draft pull request:

```sh
pixi run prepare-release <NEXT_VERSION>
```

The command creates `release/v<NEXT_VERSION>` from the latest `origin/main`, updates `Cargo.toml` and `Cargo.lock`, commits the version change, pushes the branch, and opens a draft pull request. Review the version and exact internal crate requirements before you mark the pull request ready. Merge the pull request only after its required checks pass. Then wait for the merged commit's checks on `main` to pass.

## Publish the GitHub Release

Create a GitHub Release with a new `v<NEXT_VERSION>` tag. Target the merged version commit on `main`. Write the release title and notes in the GitHub interface. Select **Set as a pre-release** when `<NEXT_VERSION>` has a prerelease component such as `-rc3`.

WARNING: Publishing the GitHub Release starts irreversible uploads to crates.io and PyPI. The GitHub Release is visible before those uploads finish. After any registry upload starts, do not move, delete, or reuse the tag. Use a new release candidate or patch version for a source or workflow correction.

Publish the GitHub Release to start the `Release` workflow. The workflow builds and verifies all native artifacts before the protected `release` environment permits registry publication. It then publishes six Rust crates and the Python distributions, uploads the artifacts to the existing GitHub Release, and verifies all three destinations. It does not change the Release title or notes. If validation rejects the prerelease setting, correct the GitHub Release and rerun the failed workflow before you approve registry publication.

The GitHub Release must contain these assets:

- Five CLI ZIP archives for Linux x86_64, Linux ARM64, Windows x86_64, macOS x86_64, and macOS ARM64.
- Five Python wheels for the same platforms.
- One Python source distribution.
- `SHA256SUMS` for the eleven package artifacts.

Verify the published versions on [crates.io](https://crates.io/crates/vl-convert), [PyPI](https://pypi.org/project/vl-convert-python/), and the GitHub Release.

If a transient publication or upload step fails, use **Re-run failed jobs** on the same workflow run and immutable tag. A rerun skips exact crate versions and Python files that the failed attempt already published. Prepare a new version when the source or workflow must change.

Use the manual `workflow_dispatch` trigger to test release builds without publishing. The `linux_wheels_only` input limits that build-only run to the two Linux wheel jobs. The x86_64 job also builds the source distribution. Manual runs never publish packages or change a GitHub Release.

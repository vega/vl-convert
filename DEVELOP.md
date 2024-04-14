## Update rust_thirdparty.yaml and copy license files to Python directory

```
$ cargo bundle-licenses --format yaml --output thirdparty_rust.yaml && cp thirdparty_*.* vl-convert-python/ && cp thirdparty_*.* vl-convert-rs/ && cp thirdparty_*.* vl-convert/

```

## Release process
Releases of VlConvert crates are handled using [cargo-workspaces](https://github.com/pksunkara/cargo-workspaces), which can be installed with:

```
$ cargo install cargo-workspaces
```

## Tagging and publish to crates.io
Check out the main branch, then tag and publish a new version of the `vl-convert` and `vl-convert-rs` crates with:

(replacing `0.1.0` with the desired version)
```
$ cargo ws publish --all --force "vl-convert*" custom 0.1.0
```

## Publish Python packages to PyPI
The `cargo ws publish ...` command above will push a commit to the `main` branch. This push to `main` will trigger CI, including the "Publish to PyPI" job. This job must be approved manually in the GitHub interface. After it is approved it will run and publish the Python packages to PyPI.

## Build Apple Silicon packages
Cross compiling vl-convert packages from macOS x86 to macOS arm64 is not currently working in GitHub Actions, so for the time being the Apple Silicon packages must be built locally from an Apple Silicon machine.

### Build CLI
Build the Apple Silicon CLI application with:
```
cargo build -p vl-convert --release
zip -j target/release/vl-convert_osx-arm64.zip target/release/vl-convert
```

This will produce `target/release/vl-convert_osx-arm64.zip`, which should be uploaded to the GitHub Release below

### Build Python wheels
Build the Python wheels with:
```
rm -rf target/wheels
maturin build -m vl-convert-python/Cargo.toml --release --strip 
```

This will produce a wheel file in `target/wheels`, which should be uploaded to the GitHub Release below. These wheels must also be uploaded to PyPI with:

```
twine upload target/wheels/*.whl
```

## Create GitHub Release
Create a new GitHub release using the `v0.1.0` tag.

## Upgrading Deno
Updating the CI build to use QEMU seems to have fixed the issue. Leaving this comment for posterity.

 > The Deno dependencies currently correspond to Deno 1.30.3. When updating to later versions, we've run into errors for packages cross compiled to Linux aarch64. See https://github.com/vega/vl-convert/issues/52. Be sure to test this scenario when updating Deno in the future.

## Debug Logging
To enable logging, set the RUST_LOG environment variable to info, warn, or error
```
RUST_LOG=info
```

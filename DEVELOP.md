## Release process
Releases of vl-convert are handled using [cargo-workspaces](https://github.com/pksunkara/cargo-workspaces), which can be installed with:

```
$ cargo install cargo-workspaces
```

## Tagging and publish to crates.io
Check out the main branch, then tag and publish a new version of the `vl-convert` and `vl-convert-rs` crates with:

(replacing `0.1.0-rc3` with the desired version)
```
$ cargo ws publish --all --force "vl-convert*" custom 0.1.0-rc3
```

## Publish Python packages to PyPI
The `cargo ws publish ...` command above will push a commit to the `main` branch. This push to `main` will trigger CI, including the "Publish to PyPI" job. This job must be approved manually in the GitHub interface. After it is approved it will run and publish Python packages to PyPI.

## Create GitHub Release
Create a new GitHub release using the `v0.1.3-rc1` tag.
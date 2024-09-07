# Overview
`vl-convert-vendor` is a helper crate that downloads multiple versions of Vega-Lite, and their dependencies, using [Deno vendor](https://deno.land/manual@v1.26.0/tools/vendor). It also generates the `vl-convert-rs/src/module_loader/import_map.rs` file which inlines the source code of all the downloaded dependencies using the `include_str!` macro.

This crate only needs to be run when a new Vega-Lite version is to be added.

# Run

```bash
pixi run vendor
```

# Adding a new version of Vega-Lite
vl-convert inlines the source code of supported versions of Vega-Lite so that no internet connection is required at runtime. As a consequence, vl-convert must be updated each time a new version of Vega-Lite is released. Here are the steps to add support for a new version of Vega-Lite (called version X.Y.Z in this example)

1. Identify the Skypack CDN URL for the new version by opening https://cdn.skypack.dev/vega-lite@X.Y.Z in a web browser. Copy the *minified* URL displayed in the header comment of this page. This URL will start with https://cdn.skypack.dev/pin/vega-lite@vX.Y.Z-.
2. Update the `VL_PATHS` const variable at the top of `vl-convert-vendor/src/main.rs` to include a new tuple of the form `("X.Y", "https://cdn.skypack.dev/pin/vega-lite@vX.Y.Z-...")`. Note that only the major and minor version are included in the first element of the tuple.
3. Run the `vl-convert-vendor` binary from the `vl-convert-vendor` directory using `cargo run`. This will download the new version of Vega-Lite, and it's dependencies, using `deno vendor`. It will also generate a new version of `vl-convert-rs/src/module_loader/import_map.rs` that includes the new version.
4. Update the value of `DEFAULT_VL_VERSION` in `vl-convert/src/main.rs` to `X.Y`. Update the CLI argument documentation strings to include `X.Y`. 
5. Commit updated versions of `vl-convert-vendor/src/main.rs`, `vl-convert-rs/src/module_loader/import_map.rs`, and the files added under `vl-convert-rs/vendor`. 

# Failing codegen-clean CI task
The `codegen-clean` CI job checks that running `vl-convert-vendor` does not result in changes to the vendored files. If the files hosted on skypack change, it may be necessary to clear deno's local cache and rerun `vl-convert-vendor` locally. Find the `DENO_DIR` by running `deno info`:

```
% deno info
DENO_DIR location: /Users/jonmmease/Library/Caches/deno
Remote modules cache: /Users/jonmmease/Library/Caches/deno/deps
npm modules cache: /Users/jonmmease/Library/Caches/deno/npm
Emitted modules cache: /Users/jonmmease/Library/Caches/deno/gen
Language server registries cache: /Users/jonmmease/Library/Caches/deno/registries
Origin storage: /Users/jonmmease/Library/Caches/deno/location_data
```

The cache of downloaded files is stored in `$DENO_DIR/deps`. This directory is safe to delete, and will cause deno to re-download all of the JavaScript files from skypack.

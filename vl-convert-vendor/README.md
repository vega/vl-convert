# Overview
`vl-convert-vendor` is a helper crate that downloads multiple versions of Vega-Lite, and their dependencies, using [Deno vendor](https://deno.land/manual@v1.26.0/tools/vendor). It also generates the `vl-convert-rs/src/module_loader/import_map.rs` file which inlines the source code of all the downloaded dependencies using the `include_str!` macro.

This crate only needs to be run when a new Vega-Lite version is to be added or Vega is updated.

# Run

```bash
pixi run vendor
```

# Adding a new version of Vega-Lite
vl-convert inlines the source code of supported versions of Vega-Lite so that no internet connection is required at runtime. As a consequence, vl-convert must be updated each time a new version of Vega-Lite is released. Here are the steps to add support for a new version of Vega-Lite (called version X.Y.Z in this example):

1. Update the `VL_PATHS` const variable at the top of `vl-convert-vendor/src/main.rs` to include a new tuple of the form `("X.Y", "/npm/vega-lite@X.Y.Z/+esm")`. Note that only the major and minor version are included in the first element of the tuple.
2. Run `pixi run vendor` to download the new version and regenerate `import_map.rs`.
3. Update the `VlVersion` test values in:
   - `vl-convert-rs/tests/test_specs.rs` (multiple test functions)
   - `vl-convert/tests/test_cli.rs` (multiple test functions)
   - `vl-convert-python/tests/test_specs.py` (multiple test functions)
4. Update CLI help text in:
   - `vl-convert/src/main.rs` - Add the new version to help text version lists (6 occurrences of "One of 5.8, 5.14, ...")
   - `vl-convert/README.md` - Update help text examples showing version lists
5. Run tests: `pixi run test-rs`. The new version tests will fail because expected output files don't exist yet.
6. Create expected test outputs (see "Updating test snapshots" below).
7. Optionally update `DEFAULT_VL_VERSION` in `vl-convert/src/main.rs` if this should become the new default.
8. Run all tests to verify: `pixi run test-rs && pixi run test-cli`
9. Commit updated files.

# Updating Vega version
To update the Vega version:

1. Update `VEGA_PATH` in `vl-convert-vendor/src/main.rs`: `/npm/vega@X.Y.Z/+esm`
2. Run `pixi run vendor`
3. Run tests to verify compatibility

# Removing old Vega-Lite versions
When removing old versions (e.g., versions not used by any Altair release):

1. Remove the entry from `VL_PATHS` in `vl-convert-vendor/src/main.rs`
2. Run `pixi run vendor`
3. Remove the corresponding `VlVersion::vX_Y` entries from test files:
   - `vl-convert-rs/tests/test_specs.rs`
   - `vl-convert/tests/test_cli.rs`
   - `vl-convert-python/tests/test_specs.py`
4. Update CLI help text in:
   - `vl-convert/src/main.rs` - Remove the version from help text version lists (6 occurrences)
   - `vl-convert/README.md` - Update help text examples
5. Remove the expected test output directory: `rm -rf vl-convert-rs/tests/vl-specs/expected/vX_Y`
6. Run tests to verify: `pixi run test-rs && pixi run test-cli`

# Updating test snapshots
When tests fail due to missing or changed expected outputs:

**For new Vega-Lite versions:**
1. Run `pixi run test-rs` - tests for the new version will fail and write outputs to `vl-convert-rs/tests/vl-specs/failed/vX_Y/`
2. Create the expected directory and copy the failed outputs:
   ```bash
   mkdir -p vl-convert-rs/tests/vl-specs/expected/vX_Y
   cp vl-convert-rs/tests/vl-specs/failed/vX_Y/*.json vl-convert-rs/tests/vl-specs/expected/vX_Y/
   ```
3. Re-run tests to verify: `pixi run test-rs`

**For image comparison failures:**
If PNG/SVG tests fail, the failed outputs are written to `vl-convert-rs/tests/vl-specs/failed/`. Review the differences and update the expected files if the changes are intentional.

# Failing codegen-clean CI task
The `codegen-clean` CI job checks that running `vl-convert-vendor` does not result in changes to the vendored files. If the files hosted on jsdelivr change, it may be necessary to clear deno's local cache and rerun `vl-convert-vendor` locally. Find the `DENO_DIR` by running `deno info`:

```
% deno info
DENO_DIR location: /Users/jonmmease/Library/Caches/deno
Remote modules cache: /Users/jonmmease/Library/Caches/deno/deps
npm modules cache: /Users/jonmmease/Library/Caches/deno/npm
Emitted modules cache: /Users/jonmmease/Library/Caches/deno/gen
Language server registries cache: /Users/jonmmease/Library/Caches/deno/registries
Origin storage: /Users/jonmmease/Library/Caches/deno/location_data
```

The cache of downloaded files is stored in `$DENO_DIR/deps`. This directory is safe to delete, and will cause deno to re-download all of the JavaScript files.

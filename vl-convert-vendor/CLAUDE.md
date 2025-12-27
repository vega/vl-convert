# vl-convert-vendor

Development utility for vendoring JavaScript dependencies. Not published to crates.io.

## Documentation

@README.md

## Key Files

- `src/main.rs` - Vendoring logic, `VL_PATHS` and `VEGA_PATH` constants
- `patched/` - Patches applied to vendored files

## Files to Update When Changing Versions

When adding/removing Vega-Lite versions, update:
1. `vl-convert-vendor/src/main.rs` - `VL_PATHS` array
2. `vl-convert-rs/tests/test_specs.rs` - `VlVersion` test values (multiple places)
3. `vl-convert/tests/test_cli.rs` - version test values (multiple places)
4. `vl-convert-python/tests/test_specs.py` - version test values (multiple places)
5. `vl-convert-rs/tests/vl-specs/expected/` - add/remove version directories

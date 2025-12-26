# vl-convert-vendor

Development utility for vendoring JavaScript dependencies. Not published to crates.io.

## Purpose

Downloads Vega, Vega-Lite, and dependencies from CDN, deduplicates, patches, and generates `import_map.rs`.

## When to Run

```bash
pixi run vendor
```

Run when:
1. Adding new Vega-Lite version
2. Updating dependency versions
3. Applying patches to vendored JS
4. CI codegen-clean job fails

## Adding a New Vega-Lite Version

1. Open `https://cdn.skypack.dev/vega-lite@X.Y.Z`
2. Copy pinned URL from header comment
3. Add to `VL_PATHS` in `src/main.rs`
4. Run `pixi run vendor`
5. Update `DEFAULT_VL_VERSION` in `vl-convert/src/main.rs`

## Troubleshooting

If vendoring produces different output:
```bash
rm -rf ~/.cache/deno/deps  # Clear Deno cache
pixi run vendor
```

## Key Files

- `src/main.rs` - Vendoring logic
- `patched/` - Patches applied to vendored files

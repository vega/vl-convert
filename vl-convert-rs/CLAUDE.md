# vl-convert-rs

Core Rust library embedding Deno v8 runtime to execute Vega/Vega-Lite JavaScript.

## Key Files

- `src/converter.rs` - Main VlConverter, Deno integration (~1800 lines)
- `src/text.rs` - Font handling, text width measurement
- `src/module_loader/mod.rs` - JavaScript module resolution
- `src/module_loader/import_map.rs` - GENERATED: vendored JS (do not edit)

## Architecture Patterns

### Thread Model
- Deno runtime in dedicated worker thread
- Commands sent via MPSC channel
- All operations are async internally, sync externally

### JSON Argument Indirection
Large specs stored in `JSON_ARGS` HashMap, retrieved via V8 callback to avoid serialization issues.

## Common Pitfalls

1. **import_map.rs is generated** - Run `pixi run vendor` to regenerate, never edit manually
2. **Lock poisoning** - Mutexes can panic; `panic::catch_unwind()` used for rendering
3. **Module init order** - Vega must initialize before Vega-Lite compilation functions
4. **Font database** - Loads system fonts at startup (can be slow)

## Testing

```bash
pixi run test-rs   # Single-threaded required
```

Tests in `tests/test_specs.rs` compare output against expected files.

## Debugging

Enable logs via env var: `RUST_LOG=info` or `RUST_LOG=vl_convert_rs=debug`

## Performance Notes

- Cold start is expensive (Deno + fonts)
- Reuse VlConverter instance when possible
- Text measurement is the bottleneck for complex charts

## Supported Vega-Lite Versions

5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4 (defined in VlVersion enum)

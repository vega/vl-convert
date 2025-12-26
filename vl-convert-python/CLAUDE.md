# vl-convert-python

Python bindings via PyO3 and maturin.

## Key Files

- `src/lib.rs` - PyO3 bindings (28 exported functions)
- `vl_convert.pyi` - Type stubs (must stay in sync with lib.rs)
- `pyproject.toml` - Build config (maturin backend)

## Development

```bash
pixi run dev-py    # Build in development mode
pixi run test-py   # Run Python tests
pixi run fmt-py    # Format with Ruff
```

## Type Stub Maintenance

When adding/modifying functions in `lib.rs`:
1. Update the `#[pyfunction]` in lib.rs
2. Update `vl_convert.pyi` with matching signature
3. Use NumPy docstring convention

Type aliases in .pyi: FormatLocaleName, TimeFormatLocaleName, VegaThemes, Renderer, VlSpec

## Key Patterns

### Global State
```rust
lazy_static! {
    static ref VL_CONVERTER: Mutex<VlConverterRs>;
    static ref PYTHON_RUNTIME: tokio::runtime::Runtime;
}
```

### Type Conversion
- Specs accept both `str` and `dict` (via `parse_json_spec()`)
- Binary outputs returned as `PyBytes`
- Always acquire GIL with `Python::with_gil()`

## Testing

Tests in `tests/` use pytest with parameterization. Image comparison uses SSIM (threshold: 0.994).

Platform notes: PDF/font tests may skip on Windows.

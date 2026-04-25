# Changelog — vl-convert-python

## Unreleased (major version)

### Breaking changes

**Secure-by-default configuration.** The Python binding now inherits
`VlcConfig::default()` from `vl-convert-rs` directly. The previous
`default_python_config()`, which overrode `allowed_base_urls` to
`["http:", "https:"]`, has been removed. Fresh Python processes block all
network data access by default.

- `allowed_base_urls` default: `[]` (was implicitly `["http:", "https:"]`).
- `max_v8_heap_size_mb` default: `512` (was `None`/no cap).
- `max_ephemeral_workers` default: `2` (was `None`/unlimited).

**Migration — network data access:**

```python
# Pre-2.0 implicit behavior was permissive. To restore it:
vlc.configure(allowed_base_urls=["http:", "https:"])

# For stricter policies, use CSP-style patterns:
vlc.configure(allowed_base_urls=["https://cdn.mycompany.com/"])
```

**Uniform `None = reset-to-library-default`.** Every keyword argument of
`configure()` now treats `None` uniformly: the field is reset to its
`VlcConfig::default()` value. In prior releases most kwargs interpreted
`None` as "keep the current value." To leave a field untouched, omit the
keyword entirely.

- `configure(allowed_base_urls=None)` → resets to `[]` (was "no-op").
- `configure(max_v8_heap_size_mb=None)` → resets to `512` (was "no-op").
- `configure(max_ephemeral_workers=None)` → resets to `2` (was "no-op").
- `configure(default_theme=None)` → resets to `None` (unchanged; already matched default).
- `configure(google_fonts=None)` → resets to `[]` (was "no-op").
- `configure(themes=None)` → resets to `{}` (was "no-op").
- All other scalar/bool/list/dict kwargs follow the same rule.

**`configure(google_fonts=[...])` now has replace semantics.** Each call
replaces the full `VlcConfig.google_fonts` list; there is no append path.
The previous per-module `CONFIGURED_GOOGLE_FONTS` register (which appended
to per-request fonts) has been removed — the core library now merges
`config.google_fonts` with per-request fonts internally.

```python
vlc.configure(google_fonts=[{"family": "Roboto"}])
vlc.configure(google_fonts=[{"family": "Lato"}])
# Result: config.google_fonts == [{"family": "Lato"}]  # Not both.
```

**Positive-int validation for sized kwargs.** These kwargs now reject `0`
with `ValueError`; pass `None` to reset to the library default:

- `num_workers` — must be `>= 1` (backed by `NonZeroUsize`).
- `max_v8_heap_size_mb` — must be `>= 1` if set; `None` → `512` (library default).
- `max_v8_execution_time_secs` — must be `>= 1` if set; `None` → no cap (library default).
- `max_ephemeral_workers` — must be `>= 1` if set; `None` → `2` (library default).
- `google_fonts_cache_size_mb` — must be `>= 1` if set; `None` → library default.

Pre-2.0 code that relied on `0` as a "no-limit" sentinel must migrate to
either `None` (library default) or an explicit positive cap.

### New configure() keyword arguments

- `max_ephemeral_workers: int | None` — cap on concurrent ephemeral V8
  isolates for per-request plugins. `None` resets to the library default
  (`2`).
- `allow_google_fonts: bool | None` — whether to accept per-request
  `google_fonts` / `auto_google_fonts` overrides on conversion calls.
  `None` resets to the library default (`False`).
- `font_directories: list[str] | None` — font directories to register
  with the process-global font database. **Replacement semantics** —
  directories absent from this list are deregistered from the fontdb.
  `None` resets to the library default (empty list).

### New fields returned by get_config()

- `google_fonts: list[GoogleFontSpec]` — configured Google Fonts as a list
  of `{"family", "variants"?}` dicts.
- `google_fonts_cache_size_mb: int | None` — capacity of the Google Fonts
  on-disk LRU cache.
- `font_directories: list[str]` — currently registered font directories.
- `max_ephemeral_workers: int | None`, `allow_google_fonts: bool`,
  `max_v8_heap_size_mb: int | None`, `max_v8_execution_time_secs: int | None`
  — previously present but now reflect the new `Option<NonZero*>` typing
  (integer or `None`, no `0` sentinel).

### Internal changes

- Removed `default_python_config()` helper and the `CONFIGURED_GOOGLE_FONTS`
  process-global register; `VlConverter::new()` (which calls
  `VlcConfig::default()`) is used directly.
- Per-call conversion functions (`vega_to_svg`, `vegalite_to_png`, …) no
  longer merge `CONFIGURED_GOOGLE_FONTS` into `VlOpts.google_fonts` / 
  `VgOpts.google_fonts`. The merge with `config.google_fonts` now happens
  inside the core converter via `resolve_google_fonts`.

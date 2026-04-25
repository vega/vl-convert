# Changelog — vl-convert-python

## Unreleased (major version)

### Defaults

The Python binding's global converter inherits `VlcConfig::default()`
from `vl-convert-rs`:

- `allowed_base_urls = ["http:", "https:"]` — any HTTP/HTTPS URL is
  allowed; no filesystem access. Narrow with an explicit prefix list,
  block all with `[]`, or allow everything (including filesystem)
  with `["*"]`.
- `max_v8_heap_size_mb = 512` — per-worker V8 heap cap.
- `max_v8_execution_time_secs = None` — no execution-time cap.
- `max_ephemeral_workers = 2` — concurrent ephemeral V8 isolates.

```python
vlc.configure(allowed_base_urls=["https://cdn.mycompany.com/"])
vlc.configure(allowed_base_urls=[])  # block all network data
```

### `configure(field=None)` — uniform reset semantics

Passing `None` for any keyword resets that field to
`VlcConfig::default()`. To leave a field untouched, omit it.

| Kwarg | Reset value |
|---|---|
| `allowed_base_urls` | `["http:", "https:"]` |
| `max_v8_heap_size_mb` | `512` |
| `max_v8_execution_time_secs` | `None` (no cap) |
| `max_ephemeral_workers` | `2` |
| `google_fonts_cache_size_mb` | library default |
| `default_theme`, `default_format_locale`, `default_time_format_locale` | `None` |
| `google_fonts`, `themes`, `font_directories`, `vega_plugins` | empty |

### `configure(google_fonts=[...])` — replace semantics

Each call replaces `VlcConfig.google_fonts`; there is no append path.
Per-request `google_fonts=` on conversion calls is merged with
`config.google_fonts` inside the core converter.

### Positive-int validation

`num_workers`, `max_v8_heap_size_mb`, `max_v8_execution_time_secs`,
`max_ephemeral_workers`, `google_fonts_cache_size_mb` reject `0` with
`ValueError`. Pass `None` to reset.

### `configure()` keyword arguments

- `font_directories: list[str] | None` — replace the process-global
  font directory registry. Directories absent from the list are
  deregistered. `None` resets.
- `max_ephemeral_workers: int | None`
- `allow_google_fonts: bool | None`
- All previously documented kwargs.

### `get_config()` keys

`google_fonts`, `google_fonts_cache_size_mb`, `font_directories`,
plus `Option<NonZero*>`-typed `max_v8_*` / `max_ephemeral_workers`
returning `int | None`.

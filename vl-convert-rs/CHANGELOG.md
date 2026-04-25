# Changelog — vl-convert-rs

## Unreleased (major version)

### Breaking changes

**`VlcConfig::default()` profile.** The library default now sets:

- `allowed_base_urls = vec!["http:".to_string(), "https:".to_string()]`
  — any HTTP/HTTPS URL is allowed; no filesystem access. Pass an
  explicit prefix list (e.g. `vec!["https://cdn.mycompany.com/".into()]`)
  to narrow access; pass `Vec::new()` to block all network data;
  pass `vec!["*".into()]` to allow everything (including filesystem
  reads).
- `max_v8_heap_size_mb = Some(NonZeroUsize::new(512))` — 512 MB cap per
  worker. `None` removes the cap.
- `max_ephemeral_workers = Some(NonZeroUsize::new(2))` — harmless when
  per-request plugins are disabled.

**Sentinel-zero integer fields become `Option<NonZero*>`.** Fields that used `0` as a "no limit / use default" marker are now typed with explicit `Option<NonZero*>`:

- `max_v8_heap_size_mb: usize` → `Option<NonZeroUsize>`
- `max_v8_execution_time_secs: u64` → `Option<NonZeroU64>`
- `max_ephemeral_workers: usize` → `Option<NonZeroUsize>`
- `google_fonts_cache_size_mb` (new) is `Option<NonZeroU64>`

`None` = no limit / library default; `Some(n)` = explicit cap. JSON deserialization rejects `0` at parse time.

**`num_workers: usize` becomes `NonZeroUsize`.** The runtime `num_workers < 1` validation in `normalize_converter_config` is dropped in favor of a type-level guarantee. JSON `{"num_workers": 0}` is a deserialize error (400 at the HTTP layer), not a post-normalize 422.

**`Option<Vec<T>>` / `Option<HashMap<_,_>>` wrappers dropped where empty = unset:**

- `allowed_base_urls: Option<Vec<String>>` → `Vec<String>`
- `google_fonts: Option<Vec<GoogleFontRequest>>` → `Vec<GoogleFontRequest>`
- `vega_plugins: Option<Vec<String>>` → `Vec<String>`
- `themes: Option<HashMap<String, Value>>` → `HashMap<String, Value>`

Empty container is the natural "unset" state. `None` and `Some(vec![])` are no longer distinguishable at the type level.

### New fields

- `VlcConfig.google_fonts_cache_size_mb: Option<NonZeroU64>` — capacity (MB) of the on-disk Google Fonts LRU cache. `None` = library default. Backed by the process-global `GOOGLE_FONTS_CLIENT` LRU via `configure_font_cache`. Hot-applyable via `apply_hot_font_cache`.

### New public APIs in `vl_convert_rs::text`

- `register_font_directory(path: &str)` — append a font directory to the process-global font store and refresh the fontdb. Idempotent.
- `set_font_directories(paths: &[PathBuf])` — replace the global font-directory list; rebuilds the fontdb and bumps `FONT_CONFIG_VERSION`.
- `apply_hot_font_cache(size: Option<NonZeroU64>)` — set the Google Fonts LRU capacity. **`None` actively resets the LRU to the library default** (not a no-op like the older `configure_font_cache(None)`).
- `current_font_directories() -> Vec<PathBuf>` — read the current global font-directory list.
- `current_cache_size() -> Option<NonZeroU64>` — read the current global Google-Fonts LRU capacity.

All five APIs are safe to call concurrently with live `VlConverter`s — workers pick up changes on their next work item via `FONT_CONFIG_VERSION` refresh.

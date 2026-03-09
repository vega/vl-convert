# vl-convert-fontsource

Download fonts from the [Fontsource](https://fontsource.org/) API (which includes [Google Fonts](https://fonts.google.com/) and other open-source font collections), merge per-Unicode-subset TTF files into single font files that work with native font renderers, and store in a size bounded disk cache.

> This crate was built for [vl-convert](https://github.com/vega/vl-convert), but is designed for general use and is published individually.

## The problem

Fontsource distributes fonts as per-Unicode-subset TTF files (`latin.ttf`, `latin-ext.ttf`, `cyrillic.ttf`, etc.). Browsers handle this transparently via CSS `unicode-range`, but native font libraries like [fontdb](https://github.com/RazrFalcon/fontdb), [cosmic-text](https://github.com/pop-os/cosmic-text), and FreeType have no equivalent mechanism. Since the subset files share identical family/style metadata, loading them all produces duplicate face entries with no way to route characters to the correct subset. Loading only `latin.ttf` means mixed-script text like `"Hello ąść"` renders with missing glyphs for characters outside the Latin subset.

## What this crate does

For each requested (weight, style) variant, this crate:

1. **Downloads** all Unicode-subset TTF files from the Fontsource CDN
2. **Merges** them into a single TTF via cmap-based GID remapping (including composite glyph rewiring and GSUB/GPOS/GDEF layout table remapping)
3. **Caches** the merged result on disk with format-versioned LRU eviction

The merge uses Google's [fontations](https://github.com/googlefonts/fontations) crates (`read-fonts`, `write-fonts`, `skrifa`) for font parsing and assembly.

## Quick start

```rust
use vl_convert_fontsource::{FontsourceClient, FontStyle, VariantRequest};

// Create a client with default settings (OS cache dir, 512 MB limit)
let client = FontsourceClient::default();

// Load Roboto 400 normal — downloads, merges subsets, caches result
let batch = client.load_blocking("Roboto", Some(&[
    VariantRequest { weight: 400, style: FontStyle::Normal },
]))?;

// One merged TTF per requested (weight, style) variant
for ttf_data in batch.font_data() {
    // ttf_data: &Arc<Vec<u8>>
    // Register with your font renderer, write to disk, etc.
}

// load/load_blocking return FontsourceError on failure:
// network errors, font not found on Fontsource, invalid font name, cache I/O errors
```

### Async

```rust
let batch = client.load("Roboto", Some(&[
    VariantRequest { weight: 400, style: FontStyle::Normal },
])).await?;
```

### Load all available variants

Pass `None` to download every (weight, style) combination that has TTF URLs:

```rust
let batch = client.load_blocking("Playfair Display", None)?;
// batch.loaded_variants lists what was loaded
```

## Configuration

```rust
use vl_convert_fontsource::{ClientConfig, FontsourceClient};

let config = ClientConfig {
    cache_dir: Some("/tmp/my-font-cache".into()),
    max_cache_bytes: 256 * 1024 * 1024,  // 256 MB
    max_parallel_downloads: 4,
    ..ClientConfig::default()
};
let client = FontsourceClient::new(config)?;
```

| Field | Default | Description |
|-------|---------|-------------|
| `cache_dir` | OS cache dir / `vl-convert/fontsource` | `None` disables persistent caching |
| `max_cache_bytes` | 512 MB | `0` disables eviction |
| `max_parallel_downloads` | 8 | Concurrent subset downloads per variant |
| `request_timeout_secs` | 30 | Per-request HTTP timeout |
| `max_retries` | 3 | Retries for transient HTTP errors (5xx, 429) |

### Environment variables

| Variable | Effect |
|----------|--------|
| `VL_CONVERT_FONT_CACHE_DIR` | Override cache directory, or `"none"` to disable caching |
| `VL_CONVERT_FONTSOURCE_API_URL` | Override the Fontsource metadata API base URL |

The `VL_CONVERT_` prefix reflects this crate's origin in the vl-convert project. These would be renamed if the crate is published independently.

## fontdb integration

Enable the `fontdb` feature to register loaded fonts directly with a [`fontdb::Database`](https://docs.rs/fontdb):

```toml
[dependencies]
vl-convert-fontsource = { version = "2", features = ["fontdb"] }
```

```rust
use vl_convert_fontsource::FontsourceDatabaseExt;

let mut db = fontdb::Database::new();
let batch = client.load_blocking("Roboto", None)?;
let registration = db.register_fontsource_batch(batch);

// registration.face_ids() — all registered fontdb::IDs
// registration.per_source_ids() — IDs grouped by source TTF

// To remove later:
db.unregister_fontsource_batch(registration);
```

## Metadata preservation

The full Fontsource API response is available through the `types` module, including per-subset URLs for all formats (TTF, woff2, woff). This enables downstream use cases like generating HTML with standard Fontsource CSS/link references that use `unicode-range` for browser rendering, while using the merged TTFs for server-side rendering.

The metadata types are in `vl_convert_fontsource::types` — see `FontsourceFont`, `VariantMap`, and `FontsourceFileUrls`.

## Caching

Merged fonts are cached on disk at `<cache_dir>/fonts/` with filenames derived from the font ID, weight, style, and Fontsource `lastModified` timestamp. API metadata is cached separately at `<cache_dir>/metadata/`.

The cache uses:
- **Format versioning** — a `format-version` file triggers a full wipe when the merge format changes
- **LRU eviction** — when total size exceeds `max_cache_bytes`, least-recently-used files are evicted (current load is exempt)
- **Cross-process safety** — file-level locking via `fs4` prevents corruption from concurrent processes

## License

BSD-3-Clause

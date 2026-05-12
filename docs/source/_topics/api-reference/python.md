---
title: Python API Reference
path: api-reference
section: API Reference
order: 900
interfaces: [python]
---

<!-- topic-body -->

# Python API

The Python reference is generated from `vl-convert-python/vl_convert.pyi`.

## Vega-Lite Conversions

```{autodoc2-object} vl_convert.vegalite_to_png
```

```{autodoc2-object} vl_convert.vegalite_to_jpeg
```

```{autodoc2-object} vl_convert.vegalite_to_pdf
```

```{autodoc2-object} vl_convert.vegalite_to_svg
```

```{autodoc2-object} vl_convert.vegalite_to_html
```

```{autodoc2-object} vl_convert.vegalite_to_scenegraph
```

```{autodoc2-object} vl_convert.vegalite_to_vega
```

```{autodoc2-object} vl_convert.vegalite_to_url
```

## Vega Conversions

```{autodoc2-object} vl_convert.vega_to_png
```

```{autodoc2-object} vl_convert.vega_to_jpeg
```

```{autodoc2-object} vl_convert.vega_to_pdf
```

```{autodoc2-object} vl_convert.vega_to_svg
```

```{autodoc2-object} vl_convert.vega_to_html
```

```{autodoc2-object} vl_convert.vega_to_scenegraph
```

```{autodoc2-object} vl_convert.vega_to_url
```

## SVG Conversions

```{autodoc2-object} vl_convert.svg_to_png
```

```{autodoc2-object} vl_convert.svg_to_jpeg
```

```{autodoc2-object} vl_convert.svg_to_pdf
```

## JavaScript Bundling

```{autodoc2-object} vl_convert.javascript_bundle
```

## Fonts

```{autodoc2-object} vl_convert.register_font_directory
```

```{autodoc2-object} vl_convert.set_font_directories
```

```{autodoc2-object} vl_convert.current_font_directories
```

```{autodoc2-object} vl_convert.google_fonts_cache_dir
```

```{autodoc2-object} vl_convert.google_fonts_cache_size_mb
```

```{autodoc2-object} vl_convert.set_google_fonts_cache_size_mb
```

```{autodoc2-object} vl_convert.vegalite_fonts
```

```{autodoc2-object} vl_convert.vega_fonts
```

## Configuration

```{autodoc2-object} vl_convert.configure
```

```{autodoc2-object} vl_convert.load_config
```

```{autodoc2-object} vl_convert.get_config_path
```

```{autodoc2-object} vl_convert.get_config
```

## Worker Diagnostics

```{autodoc2-object} vl_convert.warm_up_workers
```

```{autodoc2-object} vl_convert.get_worker_memory_usage
```

## Locales and Themes

```{autodoc2-object} vl_convert.get_format_locale
```

```{autodoc2-object} vl_convert.get_time_format_locale
```

```{autodoc2-object} vl_convert.get_themes
```

```{autodoc2-object} vl_convert.get_local_tz
```

## Bundled JavaScript Versions

```{autodoc2-object} vl_convert.get_vega_version
```

```{autodoc2-object} vl_convert.get_vega_themes_version
```

```{autodoc2-object} vl_convert.get_vega_embed_version
```

```{autodoc2-object} vl_convert.get_vegalite_versions
```

## Async API

Import `vl_convert.asyncio` for awaitable versions of the conversion,
configuration, font, and diagnostics functions above. The async functions use
the same parameter and return shapes as their sync counterparts, except that
operations which touch worker state or perform conversions are `async`.

Functions that only return static metadata, such as `get_vega_version()` and
`get_config_path()`, are synchronous re-exports in the async namespace.

## Types

These names appear in public signatures and return values.

### `VlSpec`

Vega and Vega-Lite specifications may be passed as a JSON string or as a Python
dictionary.

```python
VlSpec = str | dict[str, Any]
```

### `Renderer`

HTML rendering accepts `"svg"`, `"canvas"`, or `"hybrid"`.

### `FormatLocale` and `TimeFormatLocale`

Locale parameters accept a built-in locale name or a locale dictionary.

```python
FormatLocale = FormatLocaleName | dict[str, Any]
TimeFormatLocale = TimeFormatLocaleName | dict[str, Any]
```

### `GoogleFontSpec`

Google Fonts may be passed as a family-name string or as a dictionary with a
family and optional variants.

```python
{
    "family": "Inter",
    "variants": [(400, "normal"), (700, "italic")],
}
```

### `FontInfo`

`FontInfo` is the dictionary returned by `vegalite_fonts()` and `vega_fonts()`.
It contains the font name, source, variants, and optional CSS helpers for
Google-hosted fonts.

### `ConverterConfig`

`ConverterConfig` is the dictionary returned by `get_config()`.

### `WorkerMemoryUsage`

`WorkerMemoryUsage` is the dictionary returned for each worker by
`get_worker_memory_usage()`. Sizes are reported in bytes.

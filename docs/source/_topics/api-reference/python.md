---
title: Python API Reference
path: api-reference
section: API Reference
order: 900
interfaces: [python]
---

<!-- topic-body -->

# Python API Reference

These entries are generated from the version 2 type stubs. Vega and Vega-Lite specifications may be passed as JSON strings or dictionaries. PNG, JPEG, and PDF functions return `bytes`. SVG, HTML, and URL functions return `str`. Scenegraph functions return a `dict` by default, or MessagePack `bytes` with `format="msgpack"`.

Start with {doc}`getting-started/quick-start` if you are choosing a conversion function for the first time.

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

`vl_convert.asyncio` provides awaitable versions of the conversion, configuration, font inspection, and worker functions above, with the same parameters and return types. These functions stay synchronous in that namespace: the version getters, `get_config_path()`, `get_format_locale()`, `get_time_format_locale()`, `current_font_directories()`, `google_fonts_cache_dir()`, `google_fonts_cache_size_mb()`, and `set_google_fonts_cache_size_mb()`. See {doc}`advanced/python-async`.

## Types

These names appear in public signatures and return values.

### `VlSpec`

A Vega or Vega-Lite specification as a JSON string or a dictionary.

```python
VlSpec = str | dict[str, Any]
```

### `Renderer`

The HTML browser renderer: `"svg"`, `"canvas"`, or `"hybrid"`.

### `FormatLocale` and `TimeFormatLocale`

A built-in locale name or a locale dictionary.

```python
FormatLocale = FormatLocaleName | dict[str, Any]
TimeFormatLocale = TimeFormatLocaleName | dict[str, Any]
```

### `GoogleFontSpec`

A Google Font request as a family-name string, or a dictionary with a family and optional variants:

```python
{
    "family": "Inter",
    "variants": [(400, "normal"), (700, "italic")],
}
```

### `FontInfo`

The dictionary returned for each font by `vegalite_fonts()` and `vega_fonts()`. It contains the font name, source, variants, and optional CSS helpers for Google-hosted fonts.

### `ConverterConfig`

The dictionary returned by `get_config()`.

### `WorkerMemoryUsage`

The dictionary returned for each worker by `get_worker_memory_usage()`. Sizes are in bytes.

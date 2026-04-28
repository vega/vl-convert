from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import sys
    from typing import Any, Literal, TypedDict

    if sys.version_info >= (3, 10):
        from typing import TypeAlias
    else:
        from typing_extensions import TypeAlias

    FormatLocaleName: TypeAlias = Literal[
        "ar-001",
        "ar-AE",
        "ar-BH",
        "ar-DJ",
        "ar-DZ",
        "ar-EG",
        "ar-EH",
        "ar-ER",
        "ar-IL",
        "ar-IQ",
        "ar-JO",
        "ar-KM",
        "ar-KW",
        "ar-LB",
        "ar-LY",
        "ar-MA",
        "ar-MR",
        "ar-OM",
        "ar-PS",
        "ar-QA",
        "ar-SA",
        "ar-SD",
        "ar-SO",
        "ar-SS",
        "ar-SY",
        "ar-TD",
        "ar-TN",
        "ar-YE",
        "ca-ES",
        "cs-CZ",
        "da-DK",
        "de-CH",
        "de-DE",
        "en-CA",
        "en-GB",
        "en-IE",
        "en-IN",
        "en-US",
        "es-BO",
        "es-ES",
        "es-MX",
        "fi-FI",
        "fr-CA",
        "fr-FR",
        "he-IL",
        "hu-HU",
        "it-IT",
        "ja-JP",
        "ko-KR",
        "mk-MK",
        "nl-NL",
        "pl-PL",
        "pt-BR",
        "pt-PT",
        "ru-RU",
        "sl-SI",
        "sv-SE",
        "uk-UA",
        "zh-CN",
    ]
    TimeFormatLocaleName: TypeAlias = Literal[
        "ar-EG",
        "ar-SY",
        "ca-ES",
        "cs-CZ",
        "da-DK",
        "de-CH",
        "de-DE",
        "en-CA",
        "en-GB",
        "en-US",
        "es-ES",
        "es-MX",
        "fa-IR",
        "fi-FI",
        "fr-CA",
        "fr-FR",
        "he-IL",
        "hr-HR",
        "hu-HU",
        "it-IT",
        "ja-JP",
        "ko-KR",
        "mk-MK",
        "nb-NO",
        "nl-BE",
        "nl-NL",
        "pl-PL",
        "pt-BR",
        "ru-RU",
        "sv-SE",
        "tr-TR",
        "uk-UA",
        "vi-VN",
        "zh-CN",
        "zh-TW",
    ]
    VegaThemes: TypeAlias = Literal[
        "carbong10",
        "carbong100",
        "carbong90",
        "carbonwhite",
        "dark",
        "excel",
        "fivethirtyeight",
        "ggplot2",
        "googlecharts",
        "latimes",
        "powerbi",
        "quartz",
        "urbaninstitute",
        "vox",
    ]
    Renderer: TypeAlias = Literal["canvas", "hybrid", "svg"]
    FormatLocale: TypeAlias = FormatLocaleName | dict[str, Any]
    TimeFormatLocale: TypeAlias = TimeFormatLocaleName | dict[str, Any]
    VlSpec: TypeAlias = str | dict[str, Any]

    class GoogleFontSource(TypedDict):
        type: Literal["google"]
        font_id: str

    class LocalFontSource(TypedDict):
        type: Literal["local"]

    FontSource: TypeAlias = GoogleFontSource | LocalFontSource

    class GoogleFontSpec(TypedDict, total=False):
        family: str  # required
        variants: list[tuple[int, str]]  # optional: [(400, "normal"), (700, "italic")]

    class FontVariant(TypedDict):
        weight: str
        style: str
        font_face: str | None

    class FontInfo(TypedDict):
        name: str
        source: FontSource
        variants: list[FontVariant]
        url: str | None
        link_tag: str | None
        import_rule: str | None

    class ConverterConfig(TypedDict):
        num_workers: int
        base_url: str | bool
        allowed_base_urls: list[str]
        auto_google_fonts: bool
        embed_local_fonts: bool
        subset_fonts: bool
        missing_fonts: Literal["fallback", "warn", "error"]
        google_fonts: list[GoogleFontSpec]
        google_fonts_cache_dir: str | None
        google_fonts_cache_size_mb: int | None
        max_v8_heap_size_mb: int | None
        max_v8_execution_time_secs: int | None
        gc_after_conversion: bool
        vega_plugins: list[str]
        plugin_import_domains: list[str]
        allow_per_request_plugins: bool
        max_ephemeral_workers: int | None
        allow_google_fonts: bool
        per_request_plugin_import_domains: list[str]
        default_theme: str | None
        default_format_locale: str | dict[str, Any] | None
        default_time_format_locale: str | dict[str, Any] | None
        themes: dict[str, dict[str, Any]]
        font_directories: list[str]

__all__ = [
    "asyncio",
    "configure",
    "load_config",
    "get_config_path",
    "get_format_locale",
    "get_config",
    "get_local_tz",
    "get_themes",
    "get_time_format_locale",
    "javascript_bundle",
    "register_font_directory",
    "set_font_directories",
    "warm_up_workers",
    "get_worker_memory_usage",
    "svg_to_jpeg",
    "svg_to_pdf",
    "svg_to_png",
    "vega_fonts",
    "vega_to_html",
    "vega_to_jpeg",
    "vega_to_pdf",
    "vega_to_png",
    "vega_to_scenegraph",
    "vega_to_svg",
    "vega_to_url",
    "vegalite_fonts",
    "vegalite_to_html",
    "vegalite_to_jpeg",
    "vegalite_to_pdf",
    "vegalite_to_png",
    "vegalite_to_scenegraph",
    "vegalite_to_svg",
    "vegalite_to_url",
    "vegalite_to_vega",
    "get_vega_version",
    "get_vega_themes_version",
    "get_vega_embed_version",
    "get_vegalite_versions",
]

def get_format_locale(name: FormatLocaleName) -> dict[str, Any]:
    """
    Get the d3-format locale dict for a named locale.

    See https://github.com/d3/d3-format/tree/main/locale for available names

    Parameters
    ----------
    name
        d3-format locale name (e.g. 'it-IT')

    Returns
    -------
    d3-format locale dict
    """
    ...

def get_local_tz() -> str | None:
    """
    Get the named local timezone that Vega uses to perform timezone calculations.

    Returns
    -------
    Named local timezone (e.g. "America/New_York"), or None if the local timezone
    cannot be determined.
    """
    ...

def get_themes() -> dict[VegaThemes, dict[str, Any]]:
    """
    Get the config dict for each built-in theme.

    Returns
    -------
    dict from theme name to config object.
    """
    ...

def get_time_format_locale(name: TimeFormatLocaleName) -> dict[str, Any]:
    """
    Get the d3-time-format locale dict for a named locale.

    See https://github.com/d3/d3-time-format/tree/main/locale for available names

    Parameters
    ----------
    name
        d3-time-format locale name (e.g. 'it-IT')

    Returns
    -------
    d3-time-format locale dict.
    """
    ...

def javascript_bundle(snippet: str | None = None, vl_version: str | None = None) -> str:
    """
    Create a JavaScript bundle containing the Vega Embed, Vega-Lite, and Vega libraries.

    Optionally, a JavaScript snippet may be provided that references Vega Embed
    as `vegaEmbed`, Vega-Lite as `vegaLite`, Vega and `vega`, and the lodash.debounce
    function as `lodashDebounce`.

    The resulting string will include these JavaScript libraries and all of their
    dependencies.
    This bundle result is suitable for inclusion in an HTML <script> tag with
    no external dependencies required.
    The default snippet assigns `vegaEmbed`, `vegaLite`, and `vega` to the global
    window object, making them available globally to other script tags.

    Parameters
    ----------
    snippet
        An ES6 JavaScript snippet which includes no imports
    vl_version
        Vega-Lite library version string (e.g. 'v5.15') (default to latest)

    Returns
    -------
    Bundled snippet with all dependencies.
    """
    ...

def register_font_directory(font_dir: str) -> None:
    """
    Register a directory of fonts for use in subsequent conversions.

    Parameters
    ----------
    font_dir
        Absolute path to a directory containing font files

    Returns
    -------
    None
    """
    ...

def set_font_directories(font_dirs: list[str]) -> None:
    """
    Replace the registered font directories with the given list.

    Unlike ``register_font_directory``, which only adds, this replaces
    the full list. Directories previously registered but absent from
    ``font_dirs`` are dropped from the global registry, and the fontdb
    no longer resolves their fonts on future conversions. Pass an empty
    list to clear all registrations.

    Parameters
    ----------
    font_dirs
        Absolute paths to directories containing font files

    Returns
    -------
    None
    """
    ...

def configure(
    *,
    num_workers: int | None = None,
    base_url: str | bool | None = None,
    allowed_base_urls: list[str] | None = None,
    google_fonts_cache_size_mb: int | None = None,
    auto_google_fonts: bool | None = None,
    embed_local_fonts: bool | None = None,
    subset_fonts: bool | None = None,
    missing_fonts: Literal["fallback", "warn", "error"] | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    max_v8_heap_size_mb: int | None = None,
    max_v8_execution_time_secs: int | None = None,
    gc_after_conversion: bool | None = None,
    vega_plugins: list[str] | None = None,
    plugin_import_domains: list[str] | None = None,
    allow_per_request_plugins: bool | None = None,
    max_ephemeral_workers: int | None = None,
    allow_google_fonts: bool | None = None,
    per_request_plugin_import_domains: list[str] | None = None,
    default_theme: str | None = None,
    default_format_locale: str | dict[str, Any] | None = None,
    default_time_format_locale: str | dict[str, Any] | None = None,
    themes: dict[str, dict[str, Any]] | None = None,
) -> None:
    """
    Configure converter worker/access settings used by subsequent conversions.

    Parameters
    ----------
    num_workers
        Worker count (must be >= 1). ``None`` resets to the library default (1).
        Passing ``0`` raises ``ValueError``.
    base_url
        Base URL for resolving relative data paths in Vega specs.
        ``None`` or ``True`` resets to the default (vega-datasets CDN).
        ``False`` disables relative path resolution.
        A string sets a custom base URL or filesystem path.
    allowed_base_urls
        CSP-style allowlist for data access (HTTP URLs, filesystem paths).
        Examples: ``"https:"`` (scheme), ``"https://example.com/"`` (prefix),
        ``"/data/"`` (filesystem), ``"*"`` (everything). ``None`` resets to
        the library default (``["http:", "https:"]``); ``[]`` blocks all
        network data.
    google_fonts_cache_size_mb
        Maximum Google Fonts on-disk LRU cache size in megabytes. Must be >= 1
        if provided. ``None`` resets to the library default. Passing ``0``
        raises ``ValueError``.
    auto_google_fonts
        Automatically download missing fonts from Google Fonts.
        ``None`` resets to the library default (``False``).
    embed_local_fonts
        Embed locally available fonts as base64-encoded data URIs in SVG and HTML
        output. Does not apply to PDF/PNG/JPEG (which always embed fonts via fontdb).
        ``None`` resets to the library default (``False``).
    subset_fonts
        Subset fonts to only the characters used in the chart. Applies to SVG
        and HTML output. ``None`` resets to the library default (``True``).
    missing_fonts
        Missing-font behavior: ``"fallback"`` (silent), ``"warn"``, or ``"error"``.
        ``None`` resets to the library default (``"fallback"``).
    google_fonts
        Google Fonts to register for all subsequent conversions. Each entry is
        a family-name string or a dict with ``"family"`` (required) and
        optionally ``"variants"`` (list of ``(weight, style)`` tuples). Fonts
        are downloaded and registered on each conversion call.

        **Replace semantics.** Each call to ``configure(google_fonts=[...])``
        **replaces** the full list on the config; it does not append to the
        previously configured fonts. ``None`` (or ``[]``) resets to the
        library default (empty list).
    max_v8_heap_size_mb
        Maximum V8 heap size per worker in megabytes. Must be >= 1 if provided.
        ``None`` resets to the library default (no cap). Passing ``0`` raises
        ``ValueError``.
    max_v8_execution_time_secs
        Maximum V8 execution time in seconds. Must be >= 1 if provided. When
        exceeded, V8 execution is terminated and an error is returned.
        ``None`` resets to the library default (no cap). Passing ``0`` raises
        ``ValueError``.
    gc_after_conversion
        Whether to run V8 garbage collection after each conversion to release
        memory back to the OS. ``None`` resets to the library default (``False``).
    vega_plugins
        List of Vega plugins to load. Each entry is a file path (``.js``/``.mjs``),
        URL (``https://...``), or inline ESM string. Plugins must be single-entry
        ESM modules with a default export function accepting a ``vega`` object.
        Multi-file plugins should be pre-bundled with esbuild or Rollup.
        URL plugins auto-allow their domain for imports. ``None`` (or ``[]``)
        resets to the library default (empty list).
    plugin_import_domains
        Domain patterns allowed for HTTP imports inside config-level plugins.
        Use ``["*"]`` for any domain, or ``["esm.sh", "*.jsdelivr.net"]``.
        ``None`` (or ``[]``) resets to the library default (empty list;
        HTTP imports disabled).
    allow_per_request_plugins
        Whether to accept per-request plugins via the ``vega_plugin`` parameter
        on conversion functions. ``None`` resets to the library default (``False``).
    max_ephemeral_workers
        Maximum concurrent ephemeral V8 isolates for per-request plugins. Must
        be >= 1 if provided. ``None`` resets to the library default (2).
        Passing ``0`` raises ``ValueError``.
    allow_google_fonts
        Whether to accept per-request ``google_fonts`` / ``auto_google_fonts``
        overrides on conversion calls. ``None`` resets to the library default
        (``False``).
    per_request_plugin_import_domains
        Domain patterns allowed for HTTP imports inside per-request plugins.
        Separate from ``plugin_import_domains``. ``None`` (or ``[]``) resets
        to the library default (empty list; HTTP imports disabled).
    default_theme
        Default named theme (e.g. ``"dark"``) applied to all Vega-Lite conversions.
        Per-request ``theme`` overrides this if set. ``None`` resets to the
        library default (no theme).
    default_format_locale
        Default d3-format locale name (e.g. ``"fr-FR"``) applied to all conversions.
        Per-request ``format_locale`` overrides this if set. ``None`` resets
        to the library default (no locale).
    default_time_format_locale
        Default d3-time-format locale name (e.g. ``"fr-FR"``) applied to all conversions.
        Per-request ``time_format_locale`` overrides this if set. ``None``
        resets to the library default (no locale).
    themes
        Custom named themes mapping names to Vega config objects.
        Registered alongside built-in vega-themes. Custom themes take
        priority over built-in themes if names collide. ``None`` (or ``{}``)
        resets to the library default (empty map).
    """
    ...

def load_config(path: str | None = None) -> None:
    """
    Load converter configuration from a JSONC file, replacing the active config.

    Unlike ``configure()``, which patches individual fields, ``load_config()``
    resets all settings to their defaults and then applies the file. Call
    ``configure()`` after ``load_config()`` to override specific fields in code.

    Parameters
    ----------
    path
        Path to the JSONC config file. When omitted, loads from the standard
        location returned by ``get_config_path()``. If that file does not
        exist, resets to built-in defaults.

    Raises
    ------
    ValueError
        If ``path`` is provided but the file cannot be read or parsed.
    """
    ...

def get_config_path() -> str:
    """
    Return the platform-standard path for the vl-convert JSONC config file.

    This is the path ``load_config()`` reads when called without arguments,
    and the same path printed by ``vl-convert config-path`` on the CLI.
    The file may not exist.

    Returns
    -------
    str
        Absolute path to the standard JSONC config file.
    """
    ...

def get_config() -> ConverterConfig:
    """
    Get the active converter worker/access configuration.

    Returns
    -------
    Converter configuration dictionary.
    """
    ...

def warm_up_workers() -> None:
    """
    Eagerly start converter workers for the current converter configuration.

    This can be used to avoid first-conversion startup latency by pre-initializing
    worker runtimes before submitting conversion requests.
    """
    ...

class WorkerMemoryUsage(TypedDict):
    worker_index: int
    used_heap_size: int
    total_heap_size: int
    heap_size_limit: int
    external_memory: int

def get_worker_memory_usage() -> list[WorkerMemoryUsage]:
    """
    Get V8 memory usage for each worker in the converter pool.

    Returns
    -------
    list[WorkerMemoryUsage]
        List of dicts with ``worker_index``, ``used_heap_size``,
        ``total_heap_size``, ``heap_size_limit``, and ``external_memory``
        (all sizes in bytes).
    """
    ...

def svg_to_jpeg(
    svg: str, *, scale: float | None = None, quality: int | None = None
) -> bytes:
    """
    Convert an SVG image string to JPEG image data.

    Parameters
    ----------
    svg
        SVG image string
    scale
        Image scale factor (default 1.0)
    quality
        JPEG Quality between 0 (worst) and 100 (best). Default 90

    Returns
    -------
    JPEG image data.
    """
    ...

def svg_to_pdf(svg: str, *, scale: float | None = None) -> bytes:
    """
    Convert an SVG image string to PDF document data.

    Parameters
    ----------
    svg
        SVG image string
    scale
        Image scale factor (default 1.0)

    Returns
    -------
    PDF document data.
    """
    ...

def svg_to_png(svg: str, *, scale: float | None = None, ppi: float | None = None) -> bytes:
    """
    Convert an SVG image string to PNG image data.

    Parameters
    ----------
    svg
        SVG image string
    scale
        Image scale factor (default 1.0)
    ppi
        Pixels per inch (default 72)

    Returns
    -------
    PNG image data.
    """
    ...

def vega_to_html(
    vg_spec: VlSpec,
    *,
    bundle: bool | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
    renderer: Renderer | None = None,
    vega_plugin: str | None = None,
    config: str | dict[str, Any] | None = None,
    background: str | None = None,
    width: float | None = None,
    height: float | None = None,
) -> str:
    """
    Convert a Vega spec to an HTML document, optionally bundling dependencies.

    Parameters
    ----------
    vg_spec
        Vega JSON specification string or dict
    bundle
        If True, bundle all dependencies in HTML file.
        If False (default), HTML file will load dependencies from only CDN
    google_fonts
        Google Fonts to use for this conversion. Each entry is a family name
        string or a dict with ``"family"`` and optional ``"variants"``.
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    renderer
        Vega renderer. One of 'svg' (default), 'canvas',
        or 'hybrid' (where text is svg and other marks are canvas)
    vega_plugin
        Per-request Vega plugin (inline ESM string or URL).
        Requires ``allow_per_request_plugins=True`` in ``configure()``.
    config
        Vega config object merged via ``vega.mergeConfig(spec.config, config)``.
    background
        Override the spec's background color.
    width
        Override the spec's width.
    height
        Override the spec's height.
    Returns
    -------
    HTML document.
    """
    ...

def vega_to_jpeg(
    vg_spec: VlSpec,
    *,
    scale: float | None = None,
    quality: int | None = None,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
    vega_plugin: str | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    config: str | dict[str, Any] | None = None,
    background: str | None = None,
    width: float | None = None,
    height: float | None = None,
) -> bytes:
    """
    Convert a Vega spec to JPEG image data.

    Parameters
    ----------
    vg_spec
        Vega JSON specification string or dict
    scale
        Image scale factor (default 1.0)
    quality
        JPEG Quality between 0 (worst) and 100 (best). Default 90
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    vega_plugin
        Per-request Vega plugin (inline ESM string or URL).
        Requires ``allow_per_request_plugins=True`` in ``configure()``.
    google_fonts
        Google Fonts to register for this conversion.
    config
        Vega config object merged via ``vega.mergeConfig(spec.config, config)``.
    background
        Override the spec's background color.
    width
        Override the spec's width.
    height
        Override the spec's height.
    Returns
    -------
    JPEG image data.
    """
    ...

def vega_to_pdf(
    vg_spec: VlSpec,
    *,
    scale: float | None = None,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
    vega_plugin: str | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    config: str | dict[str, Any] | None = None,
    background: str | None = None,
    width: float | None = None,
    height: float | None = None,
) -> bytes:
    """
    Convert a Vega spec to PDF format.

    Parameters
    ----------
    vg_spec
        Vega JSON specification string or dict
    scale
        Image scale factor (default 1.0)
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    vega_plugin
        Per-request Vega plugin (inline ESM string or URL).
        Requires ``allow_per_request_plugins=True`` in ``configure()``.
    google_fonts
        Google Fonts to register for this conversion.
    config
        Vega config object merged via ``vega.mergeConfig(spec.config, config)``.
    background
        Override the spec's background color.
    width
        Override the spec's width.
    height
        Override the spec's height.
    Returns
    -------
    PDF file bytes.
    """
    ...

def vega_to_png(
    vg_spec: VlSpec,
    *,
    scale: float | None = None,
    ppi: float | None = None,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
    vega_plugin: str | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    config: str | dict[str, Any] | None = None,
    background: str | None = None,
    width: float | None = None,
    height: float | None = None,
) -> bytes:
    """
    Convert a Vega spec to PNG image data.

    Parameters
    ----------
    vg_spec
        Vega JSON specification string or dict
    scale
        Image scale factor (default 1.0)
    ppi
        Pixels per inch (default 72)
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    vega_plugin
        Per-request Vega plugin (inline ESM string or URL).
        Requires ``allow_per_request_plugins=True`` in ``configure()``.
    google_fonts
        Google Fonts to register for this conversion.
    config
        Vega config object merged via ``vega.mergeConfig(spec.config, config)``.
    background
        Override the spec's background color.
    width
        Override the spec's width.
    height
        Override the spec's height.
    Returns
    -------
    PNG image data.
    """
    ...

def vega_to_scenegraph(
    vg_spec: VlSpec,
    *,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
    format: Literal["dict", "msgpack"] = "dict",
    vega_plugin: str | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    config: str | dict[str, Any] | None = None,
    background: str | None = None,
    width: float | None = None,
    height: float | None = None,
) -> dict[str, Any] | bytes:
    """
    Convert a Vega spec to a Vega Scenegraph.

    Parameters
    ----------
    vg_spec
        Vega JSON specification string or dict
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    format
        Output format: "dict" returns a Python dictionary (default),
        "msgpack" returns raw MessagePack bytes
    vega_plugin
        Per-request Vega plugin (inline ESM string or URL).
        Requires ``allow_per_request_plugins=True`` in ``configure()``.
    google_fonts
        Google Fonts to register for this conversion.
    config
        Vega config object merged via ``vega.mergeConfig(spec.config, config)``.
    background
        Override the spec's background color.
    width
        Override the spec's width.
    height
        Override the spec's height.
    Returns
    -------
    scenegraph as dict (format="dict") or msgpack bytes (format="msgpack")
    """
    ...

def vega_to_svg(
    vg_spec: VlSpec,
    *,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
    vega_plugin: str | None = None,
    bundle: bool | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    config: str | dict[str, Any] | None = None,
    background: str | None = None,
    width: float | None = None,
    height: float | None = None,
) -> str:
    """
    Convert a Vega spec to an SVG image string.

    Parameters
    ----------
    vg_spec
        Vega JSON specification string or dict
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    vega_plugin
        Per-request Vega plugin (inline ESM string or URL).
        Requires ``allow_per_request_plugins=True`` in ``configure()``.
    bundle
        If True, embed fonts and images as self-contained data URIs.
        If False (default), use ``@import`` references for Google Fonts.
    google_fonts
        Google Fonts to register for this conversion. Each entry is
        a family-name string or a dict with ``"family"`` (required) and
        optionally ``"variants"``.
    config
        Vega config object merged via ``vega.mergeConfig(spec.config, config)``.
    background
        Override the spec's background color.
    width
        Override the spec's width.
    height
        Override the spec's height.
    Returns
    -------
    SVG image string.
    """
    ...

def vega_to_url(vg_spec: VlSpec, *, fullscreen: bool | None = None) -> str:
    """
    Convert a Vega spec to a URL that opens the chart in the Vega editor.

    Parameters
    ----------
    vg_spec
        Vega JSON specification string or dict
    fullscreen
        Whether to open the chart in full screen in the editor

    Returns
    -------
    URL string.
    """
    ...

def vegalite_fonts(
    vl_spec: VlSpec,
    vl_version: str | None = None,
    config: dict[str, Any] | None = None,
    theme: VegaThemes | None = None,
    auto_google_fonts: bool | None = None,
    include_font_face: bool = False,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
) -> list[FontInfo]:
    """
    Return structured font metadata for a rendered Vega-Lite spec.

    Parameters
    ----------
    vl_spec
        Vega-Lite JSON specification string or dict
    vl_version
        Vega-Lite library version string (e.g. 'v5.15')
        (default to latest)
    config
        Chart configuration object to apply during conversion
    theme
        Named theme (e.g. "dark") to apply during conversion
    auto_google_fonts
        Override auto-download from Google Fonts
        (default: use converter config)
    include_font_face
        Whether to run the font subsetting pipeline and populate
        the ``font_face`` field on each variant (default False)
    google_fonts
        Google Fonts to use for this conversion. Each entry is a family name
        string or a dict with ``"family"`` and optional ``"variants"``.
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary

    Returns
    -------
    Structured font metadata for each font used by the chart.
    """
    ...

def vega_fonts(
    vg_spec: VlSpec,
    auto_google_fonts: bool | None = None,
    include_font_face: bool = False,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
) -> list[FontInfo]:
    """
    Return structured font metadata for a rendered Vega spec.

    Parameters
    ----------
    vg_spec
        Vega JSON specification string or dict
    auto_google_fonts
        Override auto-download from Google Fonts
        (default: use converter config)
    include_font_face
        Whether to run the font subsetting pipeline and populate
        the ``font_face`` field on each variant (default False)
    google_fonts
        Google Fonts to use for this conversion. Each entry is a family name
        string or a dict with ``"family"`` and optional ``"variants"``.
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary

    Returns
    -------
    Structured font metadata for each font used by the chart.
    """
    ...

def vegalite_to_html(
    vl_spec: VlSpec,
    *,
    vl_version: str | None = None,
    bundle: bool | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    config: dict[str, Any] | None = None,
    theme: VegaThemes | None = None,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
    renderer: Renderer | None = None,
    vega_plugin: str | None = None,
    background: str | None = None,
    width: float | None = None,
    height: float | None = None,
) -> str:
    """
    Convert a Vega-Lite spec to an HTML document, optionally bundling dependencies.

    Parameters
    ----------
    vl_spec
        Vega-Lite JSON specification string or dict
    vl_version
        Vega-Lite library version string (e.g. 'v5.15')
        (default to latest)
    bundle
        If True, bundle all dependencies in HTML file.
        If False (default), HTML file will load dependencies from only CDN
    google_fonts
        Google Fonts to use for this conversion. Each entry is a family name
        string or a dict with ``"family"`` and optional ``"variants"``.
    config
        Chart configuration object to apply during conversion
    theme
        Named theme (e.g. "dark") to apply during conversion
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    renderer
        Vega renderer. One of 'svg' (default), 'canvas',
        or 'hybrid' (where text is svg and other marks are canvas)
    vega_plugin
        Per-request Vega plugin (inline ESM string or URL).
        Requires ``allow_per_request_plugins=True`` in ``configure()``.
    background
        Override the spec's background color.
    width
        Override the spec's width.
    height
        Override the spec's height.
    Returns
    -------
    HTML document.
    """
    ...

def vegalite_to_jpeg(
    vl_spec: VlSpec,
    *,
    vl_version: str | None = None,
    scale: float | None = None,
    quality: int | None = None,
    config: dict[str, Any] | None = None,
    theme: VegaThemes | None = None,
    show_warnings: bool | None = None,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
    vega_plugin: str | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    background: str | None = None,
    width: float | None = None,
    height: float | None = None,
) -> bytes:
    """
    Convert a Vega-Lite spec to JPEG image data using a particular version of the Vega-Lite JavaScript library.

    Parameters
    ----------
    vl_spec
        Vega-Lite JSON specification string or dict
    vl_version
        Vega-Lite library version string (e.g. 'v5.15')
        (default to latest)
    scale
        Image scale factor (default 1.0)
    quality
        JPEG Quality between 0 (worst) and 100 (best). Default 90
    config
        Chart configuration object to apply during conversion
    theme
        Named theme (e.g. "dark") to apply during conversion
    show_warnings
        Whether to print Vega-Lite compilation warnings (default false)
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    vega_plugin
        Per-request Vega plugin (inline ESM string or URL).
        Requires ``allow_per_request_plugins=True`` in ``configure()``.
    google_fonts
        Google Fonts to register for this conversion.
    background
        Override the spec's background color.
    width
        Override the spec's width.
    height
        Override the spec's height.
    Returns
    -------
    JPEG image data.
    """
    ...

def vegalite_to_pdf(
    vl_spec: VlSpec,
    *,
    vl_version: str | None = None,
    scale: float | None = None,
    config: dict[str, Any] | None = None,
    theme: VegaThemes | None = None,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
    vega_plugin: str | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    background: str | None = None,
    width: float | None = None,
    height: float | None = None,
) -> bytes:
    """
    Convert a Vega-Lite spec to PDF image data using a particular version of the Vega-Lite JavaScript library.

    Parameters
    ----------
    vl_spec
        Vega-Lite JSON specification string or dict
    vl_version
        Vega-Lite library version string (e.g. 'v5.15')
        (default to latest)
    scale
        Image scale factor (default 1.0)
    config
        Chart configuration object to apply during conversion
    theme
        Named theme (e.g. "dark") to apply during conversion
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    vega_plugin
        Per-request Vega plugin (inline ESM string or URL).
        Requires ``allow_per_request_plugins=True`` in ``configure()``.
    google_fonts
        Google Fonts to register for this conversion.
    background
        Override the spec's background color.
    width
        Override the spec's width.
    height
        Override the spec's height.
    Returns
    -------
    PDF image data.
    """
    ...

def vegalite_to_png(
    vl_spec: VlSpec,
    *,
    vl_version: str | None = None,
    scale: float | None = None,
    ppi: float | None = None,
    config: dict[str, Any] | None = None,
    theme: VegaThemes | None = None,
    show_warnings: bool | None = None,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
    vega_plugin: str | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    background: str | None = None,
    width: float | None = None,
    height: float | None = None,
) -> bytes:
    """
    Convert a Vega-Lite spec to PNG image data using a particular version of the Vega-Lite JavaScript library.

    Parameters
    ----------
    vl_spec
        Vega-Lite JSON specification string or dict
    vl_version
        Vega-Lite library version string (e.g. 'v5.15')
        (default to latest)
    scale
        Image scale factor (default 1.0)
    ppi
        Pixels per inch (default 72)
    config
        Chart configuration object to apply during conversion
    theme
        Named theme (e.g. "dark") to apply during conversion
    show_warnings
        Whether to print Vega-Lite compilation warnings (default false)
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    vega_plugin
        Per-request Vega plugin (inline ESM string or URL).
        Requires ``allow_per_request_plugins=True`` in ``configure()``.
    google_fonts
        Google Fonts to register for this conversion.
    background
        Override the spec's background color.
    width
        Override the spec's width.
    height
        Override the spec's height.
    Returns
    -------
    PNG image data.
    """
    ...

def vegalite_to_scenegraph(
    vl_spec: VlSpec,
    *,
    vl_version: str | None = None,
    config: dict[str, Any] | None = None,
    theme: VegaThemes | None = None,
    show_warnings: bool | None = None,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
    format: Literal["dict", "msgpack"] = "dict",
    vega_plugin: str | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    background: str | None = None,
    width: float | None = None,
    height: float | None = None,
) -> dict[str, Any] | bytes:
    """
    Convert a Vega-Lite spec to a Vega Scenegraph using a particular version of the Vega-Lite JavaScript library.

    Parameters
    ----------
    vl_spec
        Vega-Lite JSON specification string or dict
    vl_version
        Vega-Lite library version string (e.g. 'v5.15')
        (default to latest)
    config
        Chart configuration object to apply during conversion
    theme
        Named theme (e.g. "dark") to apply during conversion
    show_warnings
        Whether to print Vega-Lite compilation warnings (default false)
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    format
        Output format: "dict" returns a Python dictionary (default),
        "msgpack" returns raw MessagePack bytes
    vega_plugin
        Per-request Vega plugin (inline ESM string or URL).
        Requires ``allow_per_request_plugins=True`` in ``configure()``.
    google_fonts
        Google Fonts to register for this conversion.
    background
        Override the spec's background color.
    width
        Override the spec's width.
    height
        Override the spec's height.
    Returns
    -------
    scenegraph as dict (format="dict") or msgpack bytes (format="msgpack")
    """
    ...

def vegalite_to_svg(
    vl_spec: VlSpec,
    *,
    vl_version: str | None = None,
    config: dict[str, Any] | None = None,
    theme: VegaThemes | None = None,
    show_warnings: bool | None = None,
    format_locale: FormatLocale | None = None,
    time_format_locale: TimeFormatLocale | None = None,
    vega_plugin: str | None = None,
    bundle: bool | None = None,
    google_fonts: list[str | GoogleFontSpec] | None = None,
    background: str | None = None,
    width: float | None = None,
    height: float | None = None,
) -> str:
    """
    Convert a Vega-Lite spec to an SVG image string using a particular version of the Vega-Lite JavaScript library.

    Parameters
    ----------
    vl_spec
        Vega-Lite JSON specification string or dict
    vl_version
        Vega-Lite library version string (e.g. 'v5.15')
        (default to latest)
    config
        Chart configuration object to apply during conversion
    theme
        Named theme (e.g. "dark") to apply during conversion
    show_warnings
        Whether to print Vega-Lite compilation warnings (default false)
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    vega_plugin
        Per-request Vega plugin (inline ESM string or URL).
        Requires ``allow_per_request_plugins=True`` in ``configure()``.
    bundle
        If True, embed fonts and images as self-contained data URIs.
        If False (default), use ``@import`` references for Google Fonts.
    google_fonts
        Google Fonts to register for this conversion.
    background
        Override the spec's background color.
    width
        Override the spec's width.
    height
        Override the spec's height.
    Returns
    -------
    SVG image string.
    """
    ...

def vegalite_to_url(vl_spec: VlSpec, *, fullscreen: bool | None = None) -> str:
    """
    Convert a Vega-Lite spec to a URL that opens the chart in the Vega editor.

    Parameters
    ----------
    vl_spec
        Vega-Lite JSON specification string or dict
    fullscreen
        Whether to open the chart in full screen in the editor

    Returns
    -------
    URL string.
    """
    ...

def vegalite_to_vega(
    vl_spec: VlSpec,
    *,
    vl_version: str | None = None,
    config: dict[str, Any] | None = None,
    theme: VegaThemes | None = None,
    show_warnings: bool | None = None,
) -> dict[str, Any]:
    """
    Convert a Vega-Lite spec to a Vega spec using a particular version of the Vega-Lite JavaScript library.

    Parameters
    ----------
    vl_spec
        Vega-Lite JSON specification string or dict
    vl_version
        Vega-Lite library version string (e.g. 'v5.15')
        (default to latest)
    config
        Chart configuration object to apply during conversion
    theme
        Named theme (e.g. "dark") to apply during conversion
    show_warnings
        Whether to print Vega-Lite compilation warnings (default false)

    Returns
    -------
    Vega JSON specification dict.
    """
    ...

def get_vega_version() -> str:
    """
    Get the bundled version of Vega

    Returns
    -------
    Vega version string (e.g. "5.30.0")
    """
    ...

def get_vega_themes_version() -> str:
    """
    Get the bundled version of Vega-Themes

    Returns
    -------
    Vega-Themes version string (e.g. "2.14.0")
    """
    ...

def get_vega_embed_version() -> str:
    """
    Get the bundled version of Vega-Embed

    Returns
    -------
    Vega-Embed version string (e.g. "6.26.0")
    """
    ...

def get_vegalite_versions() -> list[str]:
    """
    Get the bundled versions of Vega-Lite

    Returns
    -------
    Vega-Lite version strings (e.g. ["5.8", "5.9", ..., "5.21"])
    """
    ...

if TYPE_CHECKING:
    class _AsyncioModule:
        def get_format_locale(self, name: FormatLocaleName) -> dict[str, Any]:
            """See :func:`vl_convert.get_format_locale` for full documentation."""
            ...
        async def get_local_tz(self) -> str | None:
            """Async version of ``get_local_tz``. See sync function for full documentation."""
            ...
        async def get_themes(self) -> dict[VegaThemes, dict[str, Any]]:
            """Async version of ``get_themes``. See sync function for full documentation."""
            ...
        def get_time_format_locale(self, name: TimeFormatLocaleName) -> dict[str, Any]:
            """See :func:`vl_convert.get_time_format_locale` for full documentation."""
            ...
        async def javascript_bundle(
            self, snippet: str | None = None, vl_version: str | None = None
        ) -> str:
            """Async version of ``javascript_bundle``. See sync function for full documentation."""
            ...
        async def register_font_directory(self, font_dir: str) -> None:
            """Async version of ``register_font_directory``. See sync function for full documentation."""
            ...
        async def set_font_directories(self, font_dirs: list[str]) -> None:
            """Async version of ``set_font_directories``. See sync function for full documentation."""
            ...
        async def configure(
            self,
            *,
            num_workers: int | None = None,
            base_url: str | bool | None = None,
            allowed_base_urls: list[str] | None = None,
            google_fonts_cache_size_mb: int | None = None,
            auto_google_fonts: bool | None = None,
            embed_local_fonts: bool | None = None,
            subset_fonts: bool | None = None,
            missing_fonts: Literal["fallback", "warn", "error"] | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            max_v8_heap_size_mb: int | None = None,
            max_v8_execution_time_secs: int | None = None,
            gc_after_conversion: bool | None = None,
            vega_plugins: list[str] | None = None,
            plugin_import_domains: list[str] | None = None,
            allow_per_request_plugins: bool | None = None,
            max_ephemeral_workers: int | None = None,
            allow_google_fonts: bool | None = None,
            per_request_plugin_import_domains: list[str] | None = None,
            default_theme: str | None = None,
            default_format_locale: str | dict[str, Any] | None = None,
            default_time_format_locale: str | dict[str, Any] | None = None,
            themes: dict[str, dict[str, Any]] | None = None,
        ) -> None:
            """Async version of ``configure``. See sync function for full documentation."""
            ...
        async def load_config(self, path: str | None = None) -> None:
            """Async version of ``load_config``. See sync function for full documentation."""
            ...
        async def get_config(self) -> ConverterConfig:
            """Async version of ``get_config``. See sync function for full documentation."""
            ...
        async def warm_up_workers(self) -> None:
            """Async version of ``warm_up_workers``. See sync function for full documentation."""
            ...
        async def get_worker_memory_usage(self) -> list[WorkerMemoryUsage]:
            """Async version of ``get_worker_memory_usage``. See sync function for full documentation."""
            ...
        async def svg_to_jpeg(
            self, svg: str, scale: float | None = None, quality: int | None = None
        ) -> bytes:
            """Async version of ``svg_to_jpeg``. See sync function for full documentation."""
            ...
        async def svg_to_pdf(self, svg: str, *, scale: float | None = None) -> bytes:
            """Async version of ``svg_to_pdf``. See sync function for full documentation."""
            ...
        async def svg_to_png(
            self, svg: str, scale: float | None = None, ppi: float | None = None
        ) -> bytes:
            """Async version of ``svg_to_png``. See sync function for full documentation."""
            ...
        async def vega_to_html(
            self,
            vg_spec: VlSpec,
            *,
            bundle: bool | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
            renderer: Renderer | None = None,
            vega_plugin: str | None = None,
            config: str | dict[str, Any] | None = None,
            background: str | None = None,
            width: float | None = None,
            height: float | None = None,
        ) -> str:
            """Async version of ``vega_to_html``. See sync function for full documentation."""
            ...
        async def vega_to_jpeg(
            self,
            vg_spec: VlSpec,
            *,
            scale: float | None = None,
            quality: int | None = None,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
            vega_plugin: str | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            config: str | dict[str, Any] | None = None,
            background: str | None = None,
            width: float | None = None,
            height: float | None = None,
        ) -> bytes:
            """Async version of ``vega_to_jpeg``. See sync function for full documentation."""
            ...
        async def vega_to_pdf(
            self,
            vg_spec: VlSpec,
            *,
            scale: float | None = None,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
            vega_plugin: str | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            config: str | dict[str, Any] | None = None,
            background: str | None = None,
            width: float | None = None,
            height: float | None = None,
        ) -> bytes:
            """Async version of ``vega_to_pdf``. See sync function for full documentation."""
            ...
        async def vega_to_png(
            self,
            vg_spec: VlSpec,
            *,
            scale: float | None = None,
            ppi: float | None = None,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
            vega_plugin: str | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            config: str | dict[str, Any] | None = None,
            background: str | None = None,
            width: float | None = None,
            height: float | None = None,
        ) -> bytes:
            """Async version of ``vega_to_png``. See sync function for full documentation."""
            ...
        async def vega_to_scenegraph(
            self,
            vg_spec: VlSpec,
            *,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
            format: Literal["dict", "msgpack"] = "dict",
            vega_plugin: str | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            config: str | dict[str, Any] | None = None,
            background: str | None = None,
            width: float | None = None,
            height: float | None = None,
        ) -> dict[str, Any] | bytes:
            """Async version of ``vega_to_scenegraph``. See sync function for full documentation."""
            ...
        async def vega_to_svg(
            self,
            vg_spec: VlSpec,
            *,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
            vega_plugin: str | None = None,
            bundle: bool | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            config: str | dict[str, Any] | None = None,
            background: str | None = None,
            width: float | None = None,
            height: float | None = None,
        ) -> str:
            """Async version of ``vega_to_svg``. See sync function for full documentation."""
            ...
        async def vega_to_url(
            self, vg_spec: VlSpec, fullscreen: bool | None = None
        ) -> str:
            """Async version of ``vega_to_url``. See sync function for full documentation."""
            ...
        async def vegalite_fonts(
            self,
            vl_spec: VlSpec,
            vl_version: str | None = None,
            config: dict[str, Any] | None = None,
            theme: VegaThemes | None = None,
            auto_google_fonts: bool | None = None,
            include_font_face: bool = False,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
        ) -> list[FontInfo]:
            """Async version of ``vegalite_fonts``. See sync function for full documentation."""
            ...
        async def vega_fonts(
            self,
            vg_spec: VlSpec,
            auto_google_fonts: bool | None = None,
            include_font_face: bool = False,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
        ) -> list[FontInfo]:
            """Async version of ``vega_fonts``. See sync function for full documentation."""
            ...
        async def vegalite_to_html(
            self,
            vl_spec: VlSpec,
            *,
            vl_version: str | None = None,
            bundle: bool | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            config: dict[str, Any] | None = None,
            theme: VegaThemes | None = None,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
            renderer: Renderer | None = None,
            vega_plugin: str | None = None,
            background: str | None = None,
            width: float | None = None,
            height: float | None = None,
        ) -> str:
            """Async version of ``vegalite_to_html``. See sync function for full documentation."""
            ...
        async def vegalite_to_jpeg(
            self,
            vl_spec: VlSpec,
            *,
            vl_version: str | None = None,
            scale: float | None = None,
            quality: int | None = None,
            config: dict[str, Any] | None = None,
            theme: VegaThemes | None = None,
            show_warnings: bool | None = None,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
            vega_plugin: str | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            background: str | None = None,
            width: float | None = None,
            height: float | None = None,
        ) -> bytes:
            """Async version of ``vegalite_to_jpeg``. See sync function for full documentation."""
            ...
        async def vegalite_to_pdf(
            self,
            vl_spec: VlSpec,
            *,
            vl_version: str | None = None,
            scale: float | None = None,
            config: dict[str, Any] | None = None,
            theme: VegaThemes | None = None,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
            vega_plugin: str | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            background: str | None = None,
            width: float | None = None,
            height: float | None = None,
        ) -> bytes:
            """Async version of ``vegalite_to_pdf``. See sync function for full documentation."""
            ...
        async def vegalite_to_png(
            self,
            vl_spec: VlSpec,
            *,
            vl_version: str | None = None,
            scale: float | None = None,
            ppi: float | None = None,
            config: dict[str, Any] | None = None,
            theme: VegaThemes | None = None,
            show_warnings: bool | None = None,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
            vega_plugin: str | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            background: str | None = None,
            width: float | None = None,
            height: float | None = None,
        ) -> bytes:
            """Async version of ``vegalite_to_png``. See sync function for full documentation."""
            ...
        async def vegalite_to_scenegraph(
            self,
            vl_spec: VlSpec,
            *,
            vl_version: str | None = None,
            config: dict[str, Any] | None = None,
            theme: VegaThemes | None = None,
            show_warnings: bool | None = None,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
            format: Literal["dict", "msgpack"] = "dict",
            vega_plugin: str | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            background: str | None = None,
            width: float | None = None,
            height: float | None = None,
        ) -> dict[str, Any] | bytes:
            """Async version of ``vegalite_to_scenegraph``. See sync function for full documentation."""
            ...
        async def vegalite_to_svg(
            self,
            vl_spec: VlSpec,
            *,
            vl_version: str | None = None,
            config: dict[str, Any] | None = None,
            theme: VegaThemes | None = None,
            show_warnings: bool | None = None,
            format_locale: FormatLocale | None = None,
            time_format_locale: TimeFormatLocale | None = None,
            vega_plugin: str | None = None,
            bundle: bool | None = None,
            google_fonts: list[str | GoogleFontSpec] | None = None,
            background: str | None = None,
            width: float | None = None,
            height: float | None = None,
        ) -> str:
            """Async version of ``vegalite_to_svg``. See sync function for full documentation."""
            ...
        async def vegalite_to_url(
            self, vl_spec: VlSpec, fullscreen: bool | None = None
        ) -> str:
            """Async version of ``vegalite_to_url``. See sync function for full documentation."""
            ...
        async def vegalite_to_vega(
            self,
            vl_spec: VlSpec,
            *,
            vl_version: str | None = None,
            config: dict[str, Any] | None = None,
            theme: VegaThemes | None = None,
            show_warnings: bool | None = None,
        ) -> dict[str, Any]:
            """Async version of ``vegalite_to_vega``. See sync function for full documentation."""
            ...
        def get_vega_version(self) -> str:
            """See :func:`vl_convert.get_vega_version` for full documentation."""
            ...
        def get_vega_themes_version(self) -> str:
            """See :func:`vl_convert.get_vega_themes_version` for full documentation."""
            ...
        def get_vega_embed_version(self) -> str:
            """See :func:`vl_convert.get_vega_embed_version` for full documentation."""
            ...
        def get_vegalite_versions(self) -> list[str]:
            """See :func:`vl_convert.get_vegalite_versions` for full documentation."""
            ...
        def get_config_path(self) -> str:
            """See :func:`vl_convert.get_config_path` for full documentation."""
            ...

    asyncio: _AsyncioModule

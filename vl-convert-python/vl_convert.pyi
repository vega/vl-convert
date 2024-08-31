import sys
from typing import TYPE_CHECKING, Any, Dict, List, Union, Optional

if sys.version_info >= (3, 10):
    from typing import Literal, TypeAlias
elif sys.version_info >= (3, 9):
    from typing import Literal
    from typing_extensions import TypeAlias
else:
    from typing_extensions import Literal, TypeAlias

if TYPE_CHECKING:
    from typing import Any, Dict, List, Union
    from typing_extensions import Literal, TypeAlias

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
    FormatLocale: TypeAlias = Union[FormatLocaleName, Dict[str, Any]]
    TimeFormatLocale: TypeAlias = Union[TimeFormatLocaleName, Dict[str, Any]]
    VlSpec: TypeAlias = Union[str, Dict[str, Any]]

__all__ = [
    "get_format_locale",
    "get_local_tz",
    "get_themes",
    "get_time_format_locale",
    "javascript_bundle",
    "register_font_directory",
    "svg_to_jpeg",
    "svg_to_pdf",
    "svg_to_png",
    "vega_to_html",
    "vega_to_jpeg",
    "vega_to_pdf",
    "vega_to_png",
    "vega_to_scenegraph",
    "vega_to_svg",
    "vega_to_url",
    "vegalite_to_html",
    "vegalite_to_jpeg",
    "vegalite_to_pdf",
    "vegalite_to_png",
    "vegalite_to_scenegraph",
    "vegalite_to_svg",
    "vegalite_to_url",
    "vegalite_to_vega",
]

def get_format_locale(name: FormatLocaleName) -> Dict[str, Any]:
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

def get_local_tz() -> Optional[str]:
    """
    Get the named local timezone that Vega uses to perform timezone calculations.

    Returns
    -------
    Named local timezone (e.g. "America/New_York"), or None if the local timezone
    cannot be determined.
    """
    ...

def get_themes() -> Dict[VegaThemes, Dict[str, Any]]:
    """
    Get the config dict for each built-in theme.

    Returns
    -------
    dict from theme name to config object.
    """
    ...

def get_time_format_locale(name: TimeFormatLocaleName) -> Dict[str, Any]:
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

def javascript_bundle(
    snippet: Optional[str] = None, vl_version: Optional[str] = None
) -> str:
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

def svg_to_jpeg(
    svg: str, scale: Optional[float] = None, quality: Optional[int] = None
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

def svg_to_pdf(svg: str, scale: Optional[float] = None) -> bytes:
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

def svg_to_png(
    svg: str, scale: Optional[float] = None, ppi: Optional[float] = None
) -> bytes:
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
    bundle: Optional[bool] = None,
    format_locale: Optional[FormatLocale] = None,
    time_format_locale: Optional[TimeFormatLocale] = None,
    renderer: Optional[Renderer] = None,
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
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary
    renderer
        Vega renderer. One of 'svg' (default), 'canvas',
        or 'hybrid' (where text is svg and other marks are canvas)

    Returns
    -------
    HTML document.
    """
    ...

def vega_to_jpeg(
    vg_spec: VlSpec,
    scale: Optional[float] = None,
    quality: Optional[int] = None,
    allowed_base_urls: Optional[List[str]] = None,
    format_locale: Optional[FormatLocale] = None,
    time_format_locale: Optional[TimeFormatLocale] = None,
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
    allowed_base_urls
        List of allowed base URLs for external data requests.
        Default allows any base URL
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary

    Returns
    -------
    JPEG image data.
    """
    ...

def vega_to_pdf(
    vg_spec: VlSpec,
    scale: Optional[float] = None,
    allowed_base_urls: Optional[List[str]] = None,
    format_locale: Optional[FormatLocale] = None,
    time_format_locale: Optional[TimeFormatLocale] = None,
) -> bytes:
    """
    Convert a Vega spec to PDF format.

    Parameters
    ----------
    vg_spec
        Vega JSON specification string or dict
    scale
        Image scale factor (default 1.0)
    allowed_base_urls
        List of allowed base URLs for external data requests.
        Default allows any base URL
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary

    Returns
    -------
    PDF file bytes.
    """
    ...

def vega_to_png(
    vg_spec: VlSpec,
    scale: Optional[float] = None,
    ppi: Optional[float] = None,
    allowed_base_urls: Optional[List[str]] = None,
    format_locale: Optional[FormatLocale] = None,
    time_format_locale: Optional[TimeFormatLocale] = None,
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
    allowed_base_urls
        List of allowed base URLs for external data requests.
        Default allows any base URL
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary

    Returns
    -------
    PNG image data.
    """
    ...

def vega_to_scenegraph(
    vg_spec: VlSpec,
    allowed_base_urls: Optional[List[str]] = None,
    format_locale: Optional[FormatLocale] = None,
    time_format_locale: Optional[TimeFormatLocale] = None,
) -> Dict[str, Any]:
    """
    Convert a Vega spec to a Vega Scenegraph.

    Parameters
    ----------
    vg_spec
        Vega JSON specification string or dict
    allowed_base_urls
        List of allowed base URLs for external data requests.
        Default allows any base URL
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary

    Returns
    -------
    scenegraph dictionary
    """
    ...

def vega_to_svg(
    vg_spec: VlSpec,
    allowed_base_urls: Optional[List[str]] = None,
    format_locale: Optional[FormatLocale] = None,
    time_format_locale: Optional[TimeFormatLocale] = None,
) -> str:
    """
    Convert a Vega spec to an SVG image string.

    Parameters
    ----------
    vg_spec
        Vega JSON specification string or dict
    allowed_base_urls
        List of allowed base URLs for external data requests.
        Default allows any base URL
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary

    Returns
    -------
    SVG image string.
    """
    ...

def vega_to_url(vg_spec: VlSpec, fullscreen: Optional[bool] = None) -> str:
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

def vegalite_to_html(
    vl_spec: VlSpec,
    vl_version: Optional[str] = None,
    bundle: Optional[bool] = None,
    config: Optional[Dict[str, Any]] = None,
    theme: Optional[VegaThemes] = None,
    format_locale: Optional[FormatLocale] = None,
    time_format_locale: Optional[TimeFormatLocale] = None,
    renderer: Optional[Renderer] = None,
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
        If True, bundle all dependencies in HTML file
        If False (default), HTML file will load dependencies from only CDN
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

    Returns
    -------
    HTML document.
    """
    ...

def vegalite_to_jpeg(
    vl_spec: VlSpec,
    vl_version: Optional[str] = None,
    scale: Optional[float] = None,
    quality: Optional[int] = None,
    config: Optional[Dict[str, Any]] = None,
    theme: Optional[VegaThemes] = None,
    show_warnings: Optional[bool] = None,
    allowed_base_urls: Optional[List[str]] = None,
    format_locale: Optional[FormatLocale] = None,
    time_format_locale: Optional[TimeFormatLocale] = None,
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
    allowed_base_urls
        List of allowed base URLs for external data requests.
        Default allows any base URL
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary

    Returns
    -------
    JPEG image data.
    """
    ...

def vegalite_to_pdf(
    vl_spec: VlSpec,
    vl_version: Optional[str] = None,
    scale: Optional[float] = None,
    config: Optional[Dict[str, Any]] = None,
    theme: Optional[VegaThemes] = None,
    allowed_base_urls: Optional[List[str]] = None,
    format_locale: Optional[FormatLocale] = None,
    time_format_locale: Optional[TimeFormatLocale] = None,
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
    allowed_base_urls
        List of allowed base URLs for external data requests.
        Default allows any base URL
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary

    Returns
    -------
    PDF image data.
    """
    ...

def vegalite_to_png(
    vl_spec: VlSpec,
    vl_version: Optional[str] = None,
    scale: Optional[float] = None,
    ppi: Optional[float] = None,
    config: Optional[Dict[str, Any]] = None,
    theme: Optional[VegaThemes] = None,
    show_warnings: Optional[bool] = None,
    allowed_base_urls: Optional[List[str]] = None,
    format_locale: Optional[FormatLocale] = None,
    time_format_locale: Optional[TimeFormatLocale] = None,
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
    allowed_base_urls
        List of allowed base URLs for external data requests.
        Default allows any base URL
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary

    Returns
    -------
    PNG image data.
    """
    ...

def vegalite_to_scenegraph(
    vl_spec: VlSpec,
    vl_version: Optional[str] = None,
    config: Optional[Dict[str, Any]] = None,
    theme: Optional[VegaThemes] = None,
    show_warnings: Optional[bool] = None,
    allowed_base_urls: Optional[List[str]] = None,
    format_locale: Optional[FormatLocale] = None,
    time_format_locale: Optional[TimeFormatLocale] = None,
) -> Dict[str, Any]:
    """
    Convert a Vega-Lite spec to a Vega Scenegraph using a particular version of the Vega-Lite JavaScript library.

    Parameters
    ----------d
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
    allowed_base_urls
        List of allowed base URLs for external data requests.
        Default allows any base URL
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary

    Returns
    -------
    scenegraph dictionary
    """
    ...

def vegalite_to_svg(
    vl_spec: VlSpec,
    vl_version: Optional[str] = None,
    config: Optional[Dict[str, Any]] = None,
    theme: Optional[VegaThemes] = None,
    show_warnings: Optional[bool] = None,
    allowed_base_urls: Optional[List[str]] = None,
    format_locale: Optional[FormatLocale] = None,
    time_format_locale: Optional[TimeFormatLocale] = None,
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
    allowed_base_urls
        List of allowed base URLs for external data requests.
        Default allows any base URL
    format_locale
        d3-format locale name or dictionary
    time_format_locale
        d3-time-format locale name or dictionary

    Returns
    -------
    SVG image string.
    """
    ...

def vegalite_to_url(vl_spec: VlSpec, fullscreen: Optional[bool] = None) -> str:
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
    vl_version: Optional[str] = None,
    config: Optional[Dict[str, Any]] = None,
    theme: Optional[VegaThemes] = None,
    show_warnings: Optional[bool] = None,
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

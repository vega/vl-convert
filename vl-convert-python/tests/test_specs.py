import json
from pathlib import Path
import vl_convert as vlc
import pytest
from io import BytesIO
from skimage.io import imread
from skimage.metrics import structural_similarity as ssim
import os
import math
import ctypes
import sys
import pypdfium2.raw as pdfium_c
from tempfile import NamedTemporaryFile
import PIL.Image

tests_dir = Path(__file__).parent
root_dir = tests_dir.parent.parent
specs_dir = root_dir / "vl-convert-rs" / "tests" / "vl-specs"
fonts_dir = root_dir / "vl-convert-rs" / "tests" / "fonts"
locale_dir = root_dir / "vl-convert-rs" / "tests" / "locale"

BACKGROUND_COLOR = "#abc"


def setup_module(module):
    vlc.register_font_directory(str(fonts_dir))


def load_vl_spec(name):
    spec_path = specs_dir / f"{name}.vl.json"
    with open(spec_path, "rt") as f:
        spec_str = f.read()
    return spec_str


def load_locales(format_name, time_format_name):
    format_path = locale_dir / "format" / f"{format_name}.json"
    with open(format_path, "rt") as f:
        format_str = f.read()

    time_format_path = locale_dir / "time-format" / f"{time_format_name}.json"
    with open(time_format_path, "rt") as f:
        time_format_str = f.read()

    return (json.loads(format_str), json.loads(time_format_str))


def load_expected_vg_spec(name, vl_version):
    filename = f"{name}.vg.json"
    spec_path = specs_dir / "expected" / vl_version / filename
    if spec_path.exists():
        with spec_path.open("rt", encoding="utf8") as f:
            return json.load(f)
    else:
        return None


def load_expected_svg(name, vl_version):
    filename = f"{name}.svg"
    spec_path = specs_dir / "expected" / vl_version / filename
    with spec_path.open("rt", encoding="utf8") as f:
        return f.read()


def load_expected_png(name, vl_version, theme=None):
    filename = f"{name}-{theme}.png" if theme else f"{name}.png"
    spec_path = specs_dir / "expected" / vl_version / filename
    with spec_path.open("rb") as f:
        return f.read()


@pytest.mark.parametrize("name", ["circle_binned", "seattle-weather", "stacked_bar_h"])
@pytest.mark.parametrize(
    "vl_version",
    [
        "v5_8",
        "v5_9",
        "v5_10",
        "v5_11",
        "v5_12",
        "v5_13",
        "v5_14",
        "v5_15",
        "v5_16",
    ],
)
@pytest.mark.parametrize("as_dict", [False, True])
def test_vega(name, vl_version, as_dict):
    vl_spec = load_vl_spec(name)

    if as_dict:
        vl_spec = json.loads(vl_spec)

    expected_vg_spec = load_expected_vg_spec(name, vl_version)

    if expected_vg_spec is None:
        with pytest.raises(ValueError):
            vlc.vegalite_to_vega(vl_spec, vl_version=vl_version)
    else:
        vg_spec = vlc.vegalite_to_vega(vl_spec, vl_version=vl_version)
        assert expected_vg_spec == vg_spec


@pytest.mark.parametrize("name", ["circle_binned"])
@pytest.mark.parametrize(
    "vl_version",
    [
        "5.8",
        "5.9",
        "5.10",
        "5.11",
        "5.12",
        "5.13",
        "5.14",
        "5.15",
        "5.16",
    ],
)
def test_vegalite_to_html_no_bundle(name, vl_version):
    vl_spec = load_vl_spec(name)
    html = vlc.vegalite_to_html(vl_spec, vl_version=vl_version, bundle=False)
    assert html.startswith("<!DOCTYPE html>")
    assert f"cdn.jsdelivr.net/npm/vega-lite@{vl_version}" in html
    assert "cdn.jsdelivr.net/npm/vega@5" in html
    assert "cdn.jsdelivr.net/npm/vega-embed@6" in html

    # Check themes
    html = vlc.vegalite_to_html(
        vl_spec, vl_version=vl_version, bundle=False, theme="fivethirtyeight"
    )
    assert '"theme":"fivethirtyeight"' in html
    assert '"theme":"dark"' not in html

    html = vlc.vegalite_to_html(
        vl_spec, vl_version=vl_version, bundle=False, theme="dark"
    )
    assert '"theme":"dark"' in html


@pytest.mark.parametrize("name", ["circle_binned"])
@pytest.mark.parametrize(
    "vl_version",
    [
        "5.8",
        "5.9",
        "5.10",
        "5.11",
        "5.12",
        "5.13",
        "5.14",
        "5.15",
        "5.16",
    ],
)
def test_vegalite_to_html_bundle(name, vl_version):
    vl_spec = load_vl_spec(name)
    html = vlc.vegalite_to_html(vl_spec, vl_version=vl_version, bundle=True)
    assert html.startswith("<!DOCTYPE html>")
    assert vl_version in html
    assert "Jeffrey Heer" in html

    # Make sure themes aren't cached
    html = vlc.vegalite_to_html(
        vl_spec, vl_version=vl_version, bundle=True, theme="fivethirtyeight"
    )
    assert '"theme":"fivethirtyeight"' in html
    assert '"theme":"dark"' not in html

    html = vlc.vegalite_to_html(
        vl_spec, vl_version=vl_version, bundle=True, theme="dark"
    )
    assert '"theme":"dark"' in html


@pytest.mark.parametrize("name", ["circle_binned", "stacked_bar_h"])
@pytest.mark.parametrize("as_dict", [False, True])
def test_svg(name, as_dict):
    vl_version = "v5_8"
    vl_spec = load_vl_spec(name)

    if as_dict:
        vl_spec = json.loads(vl_spec)

    expected_svg = load_expected_svg(name, vl_version)

    # Convert to vega first
    vg_spec = vlc.vegalite_to_vega(vl_spec, vl_version=vl_version)
    svg = vlc.vega_to_svg(vg_spec)
    check_svg(svg, expected_svg)

    # Convert directly to image
    svg = vlc.vegalite_to_svg(vl_spec, vl_version=vl_version)
    check_svg(svg, expected_svg)


@pytest.mark.parametrize(
    "name,scale",
    [
        ("circle_binned", 1.0),
        ("stacked_bar_h", 2.0),
        ("remote_images", 1.0),
        ("maptile_background", 1.0),
        ("no_text_in_font_metrics", 1.0),
        ("lookup_urls", 1.0),
    ],
)
@pytest.mark.parametrize("as_dict", [False])
def test_png(name, scale, as_dict):
    vl_version = "v5_8"
    vl_spec = load_vl_spec(name)

    if as_dict:
        vl_spec = json.loads(vl_spec)

    expected_png = load_expected_png(name, vl_version)

    # Convert to vega first
    vg_spec = vlc.vegalite_to_vega(vl_spec, vl_version=vl_version)
    png = vlc.vega_to_png(vg_spec, scale=scale)
    check_png(png, expected_png)

    # Convert directly to image
    png = vlc.vegalite_to_png(vl_spec, vl_version=vl_version, scale=scale)
    check_png(png, expected_png)


@pytest.mark.parametrize(
    "name,scale,theme", [("circle_binned", 1.0, "dark"), ("stacked_bar_h", 2.0, "vox")]
)
def test_png_theme_config(name, scale, theme):
    vl_version = "v5_8"
    vl_spec = json.loads(load_vl_spec(name))

    expected_png = load_expected_png(name, vl_version, theme)

    # Convert directly to image
    config = dict(background=BACKGROUND_COLOR)
    png = vlc.vegalite_to_png(
        vl_spec,
        vl_version=vl_version,
        scale=scale,
        theme=theme,
        config=config,
    )
    check_png(png, expected_png)


@pytest.mark.parametrize(
    "name,scale",
    [
        ("circle_binned", 1.0),
        ("stacked_bar_h", 2.0),
        ("remote_images", 1.0),
        ("maptile_background", 1.0),
        ("no_text_in_font_metrics", 1.0),
        ("lookup_urls", 1.0),
    ],
)
@pytest.mark.parametrize("as_dict", [False])
def test_jpeg(name, scale, as_dict):
    vl_version = "v5_8"
    vl_spec = load_vl_spec(name)

    if as_dict:
        vl_spec = json.loads(vl_spec)

    # Convert to vega first
    jpeg_prefix = b"\xff\xd8\xff\xe0\x00\x10JFIF"
    vg_spec = vlc.vegalite_to_vega(vl_spec, vl_version=vl_version)
    jpeg = vlc.vega_to_jpeg(vg_spec, scale=scale)
    assert jpeg[:10] == jpeg_prefix

    # Convert directly to image
    jpeg = vlc.vegalite_to_jpeg(vl_spec, vl_version=vl_version, scale=scale)
    assert jpeg[:10] == jpeg_prefix


@pytest.mark.skipif(
    sys.platform.startswith("win"), reason="PDF tests not supported on windows"
)
@pytest.mark.parametrize(
    "name,scale,tol",
    [
        ("circle_binned", 1.0, 0.97),
        ("stacked_bar_h", 2.0, 0.98),
        ("remote_images", 1.0, 0.98),
        ("maptile_background", 1.0, 0.97),
        ("no_text_in_font_metrics", 1.0, 0.94),
        ("lookup_urls", 1.0, 0.99),
    ],
)
@pytest.mark.parametrize("as_dict", [False])
def test_pdf(name, scale, tol, as_dict):
    vl_version = "v5_8"
    vl_spec = load_vl_spec(name)

    if as_dict:
        vl_spec = json.loads(vl_spec)

    expected_png = load_expected_png(name, vl_version)

    # Convert to vega first
    vg_spec = vlc.vegalite_to_vega(vl_spec, vl_version=vl_version)
    pdf = vlc.vega_to_pdf(vg_spec, scale=scale)
    png = pdf_to_png(pdf)
    # Lower tolerance because pdfium does its own text rendering, which won't be pixel identical to resvg
    check_png(png, expected_png, tol=tol)

    # Convert directly to image
    pdf = vlc.vegalite_to_pdf(vl_spec, vl_version=vl_version, scale=scale)
    png = pdf_to_png(pdf)
    check_png(png, expected_png, tol=tol)


@pytest.mark.skipif(sys.platform.startswith("win"), reason="Font mismatch on windows")
def test_locale():
    vl_version = "v5_8"
    name = "stocks_locale"
    format_locale_name = "it-IT"
    time_format_locale_name = "it-IT"

    # Test locale by dict
    vl_spec = json.loads(load_vl_spec(name))
    format_locale, time_format_locale = load_locales(
        format_locale_name, time_format_locale_name
    )
    png = vlc.vegalite_to_png(
        vl_spec,
        vl_version=vl_version,
        scale=2,
        format_locale=format_locale,
        time_format_locale=time_format_locale,
    )

    expected_png = load_expected_png(name, vl_version)
    check_png(png, expected_png)

    # Test locale by name
    png = vlc.vegalite_to_png(
        vl_spec,
        vl_version=vl_version,
        scale=2,
        format_locale=format_locale_name,
        time_format_locale=time_format_locale_name,
    )

    expected_png = load_expected_png(name, vl_version)
    check_png(png, expected_png)


def test_gh_78():
    vl_version = "v5_8"
    name = "lookup_urls"
    vl_spec = json.loads(load_vl_spec(name))

    png = None
    for i in range(30):
        png = vlc.vegalite_to_png(vl_spec, vl_version=vl_version)

    expected_png = load_expected_png(name, vl_version)
    check_png(png, expected_png)


def check_png(png, expected_png, tol=0.994):
    png_img = imread(BytesIO(png))
    expected_png_img = imread(BytesIO(expected_png))
    similarity_value = ssim(png_img, expected_png_img, channel_axis=2)
    if similarity_value < tol:
        pytest.fail(f"png mismatch with similarity: {similarity_value}")


def check_svg(svg, expected_svg):
    if svg != expected_svg:
        pytest.fail(f"svg image mismatch")


def pdf_to_png(pdf_bytes):
    """
    Helper that uses pdfium to convert PDF to PNG

    Adapted from pdfium2 README
    """
    with NamedTemporaryFile() as nf:
        nf.write(pdf_bytes)
        filepath = os.path.abspath(nf.name)
        pdf = pdfium_c.FPDF_LoadDocument((filepath + "\x00").encode("utf-8"), None)
        page_count = pdfium_c.FPDF_GetPageCount(pdf)
        assert page_count >= 1

        # Load the first page and get its dimensions
        page = pdfium_c.FPDF_LoadPage(pdf, 0)
        width = math.ceil(pdfium_c.FPDF_GetPageWidthF(page))
        height = math.ceil(pdfium_c.FPDF_GetPageHeightF(page))

        use_alpha = False
        bitmap = pdfium_c.FPDFBitmap_Create(width, height, int(use_alpha))

        # Fill the whole bitmap with a white background
        # The color is given as a 32-bit integer in ARGB format (8 bits per channel)
        pdfium_c.FPDFBitmap_FillRect(bitmap, 0, 0, width, height, 0xFFFFFFFF)

        # Store common rendering arguments
        render_args = (
            bitmap,  # the bitmap
            page,  # the page
            # positions and sizes are to be given in pixels and may exceed the bitmap
            0,  # left start position
            0,  # top start position
            width,  # horizontal size
            height,  # vertical size
            0,  # rotation (as constant, not in degrees!)
            pdfium_c.FPDF_LCD_TEXT
            | pdfium_c.FPDF_ANNOT,  # rendering flags, combined with binary or
        )

        # Render the page
        pdfium_c.FPDF_RenderPageBitmap(*render_args)

        # Get a pointer to the first item of the buffer
        first_item = pdfium_c.FPDFBitmap_GetBuffer(bitmap)
        # Re-interpret the pointer to encompass the whole buffer
        buffer = ctypes.cast(
            first_item, ctypes.POINTER(ctypes.c_ubyte * (width * height * 4))
        )

        # Create a PIL image from the buffer contents
        img = PIL.Image.frombuffer(
            "RGBA", (width, height), buffer.contents, "raw", "BGRA", 0, 1
        )
        # Save it as file
        buffer = BytesIO()
        img.save(buffer, format="png")

        # Free resources
        pdfium_c.FPDFBitmap_Destroy(bitmap)
        pdfium_c.FPDF_ClosePage(page)
        pdfium_c.FPDF_CloseDocument(pdf)
    return buffer.getvalue()

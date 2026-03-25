"""Browser-based visual regression tests for SVG font bundling.

Renders SVG-exported charts (with embedded or referenced fonts) in headless
Chromium via Playwright and compares screenshots to baselines using SSIM.

Baselines are generated on Linux. Non-Linux platforms use a looser SSIM
threshold to tolerate font rendering differences.
"""

import http.server
import io
import socketserver
import sys
import tempfile
import threading
from pathlib import Path

import pytest

pytest.importorskip("playwright")

import numpy as np
from PIL import Image
from playwright.sync_api import sync_playwright
from skimage.metrics import structural_similarity as ssim

import vl_convert as vlc

SSIM_THRESHOLD = 0.98 if sys.platform == "linux" else 0.95

tests_dir = Path(__file__).parent
root_dir = tests_dir.parent.parent
specs_dir = root_dir / "vl-convert-rs" / "tests" / "vl-specs"
fonts_dir = root_dir / "vl-convert-rs" / "tests" / "fonts"
baselines_dir = tests_dir / "svg-baselines"
failures_dir = tests_dir / "svg-failures"


def load_spec(name: str) -> str:
    return (specs_dir / f"{name}.vl.json").read_text()


def load_spec_inline(name: str) -> str:
    import json
    import urllib.request

    spec = json.loads(load_spec(name))
    if "data" in spec and "url" in spec["data"]:
        url = spec["data"]["url"]
        with urllib.request.urlopen(url) as resp:
            spec["data"] = {"values": json.loads(resp.read())}
    return json.dumps(spec)


def wrap_svg_in_html(svg: str) -> str:
    """Wrap an SVG string in a minimal HTML page for Chromium rendering."""
    return f"""<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="margin:0;padding:0;background:white">
<div id="chart">{svg}</div>
</body>
</html>"""


def compare_screenshot(actual_bytes: bytes, baseline_name: str, update: bool) -> None:
    baseline_path = baselines_dir / baseline_name

    if update or not baseline_path.exists():
        baselines_dir.mkdir(exist_ok=True)
        baseline_path.write_bytes(actual_bytes)
        if not update:
            pytest.skip(f"Baseline created: {baseline_name}")
        return

    actual = np.array(Image.open(io.BytesIO(actual_bytes)).convert("RGB"))
    expected = np.array(Image.open(baseline_path).convert("RGB"))

    if actual.shape != expected.shape:
        h_diff = abs(actual.shape[0] - expected.shape[0])
        w_diff = abs(actual.shape[1] - expected.shape[1])
        if h_diff <= 5 and w_diff <= 5:
            h = max(actual.shape[0], expected.shape[0])
            w = max(actual.shape[1], expected.shape[1])
            for arr_name in ("actual", "expected"):
                arr = actual if arr_name == "actual" else expected
                if arr.shape[0] < h or arr.shape[1] < w:
                    padded = np.full((h, w, 3), 255, dtype=np.uint8)
                    padded[: arr.shape[0], : arr.shape[1]] = arr
                    if arr_name == "actual":
                        actual = padded
                    else:
                        expected = padded
        else:
            failures_dir.mkdir(exist_ok=True)
            (failures_dir / baseline_name).write_bytes(actual_bytes)
            (failures_dir / f"expected_{baseline_name}").write_bytes(
                baseline_path.read_bytes()
            )
            pytest.fail(
                f"Dimension mismatch for {baseline_name}: "
                f"actual {actual.shape} != expected {expected.shape} "
                f"(exceeds 5px tolerance)."
            )

    similarity = ssim(expected, actual, channel_axis=2)
    if similarity < SSIM_THRESHOLD:
        failures_dir.mkdir(exist_ok=True)
        (failures_dir / baseline_name).write_bytes(actual_bytes)
        (failures_dir / f"expected_{baseline_name}").write_bytes(
            baseline_path.read_bytes()
        )
        pytest.fail(
            f"SSIM {similarity:.4f} < {SSIM_THRESHOLD} for {baseline_name}"
        )


@pytest.fixture(scope="module")
def browser():
    with sync_playwright() as p:
        try:
            b = p.chromium.launch()
        except Exception as e:
            pytest.skip(f"Playwright browser not available: {e}")
        yield b
        b.close()


@pytest.fixture
def page(browser):
    p = browser.new_page(viewport={"width": 800, "height": 600})
    yield p
    p.close()


@pytest.fixture(scope="module")
def svg_server():
    """Local HTTP server for serving SVG files (needed for @import tests)."""
    tmp = tempfile.TemporaryDirectory()
    serve_dir = Path(tmp.name)

    class _Handler(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(serve_dir), **kwargs)

        def log_message(self, *args):
            pass

    server = socketserver.TCPServer(("localhost", 0), _Handler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    yield {"port": port, "serve_dir": serve_dir}

    server.shutdown()
    thread.join()
    tmp.cleanup()


def render_svg_inline(page, svg: str, baseline_name: str, *, block_network: bool = False) -> bytes:
    """Render an SVG wrapped in HTML, writing the HTML to a temp file."""
    if block_network:
        page.route("http://**/*", lambda route: route.abort())
        page.route("https://**/*", lambda route: route.abort())

    html = wrap_svg_in_html(svg)
    html_dir = baselines_dir / "html"
    html_dir.mkdir(parents=True, exist_ok=True)
    html_path = html_dir / baseline_name.replace(".png", ".html")
    html_path.write_text(html)

    svg_dir = baselines_dir / "svg"
    svg_dir.mkdir(parents=True, exist_ok=True)
    svg_path = svg_dir / baseline_name.replace(".png", ".svg")
    svg_path.write_text(svg)

    page.goto(f"file://{html_path}", wait_until="networkidle")
    page.wait_for_function(
        """() => {
            const svg = document.querySelector('svg');
            return svg && svg.querySelectorAll('path, rect, circle, line, text').length > 0;
        }""",
        timeout=15000,
    )
    chart = page.locator("#chart")
    return chart.screenshot()


def render_svg_served(page, svg: str, svg_server, filename: str) -> bytes:
    """Render SVG via HTTP server (needed for @import to work in Chrome)."""
    serve_dir = svg_server["serve_dir"]
    port = svg_server["port"]

    html = wrap_svg_in_html(svg)
    (serve_dir / filename).write_text(html)

    # Also save for inspection
    html_dir = baselines_dir / "html"
    html_dir.mkdir(parents=True, exist_ok=True)
    (html_dir / filename).write_text(html)

    svg_dir = baselines_dir / "svg"
    svg_dir.mkdir(parents=True, exist_ok=True)
    svg_path = svg_dir / filename.replace(".html", ".svg")
    svg_path.write_text(svg)

    page.goto(f"http://localhost:{port}/{filename}", wait_until="networkidle")
    page.wait_for_function(
        """() => {
            const svg = document.querySelector('svg');
            return svg && svg.querySelectorAll('path, rect, circle, line, text').length > 0;
        }""",
        timeout=15000,
    )
    chart = page.locator("#chart")
    return chart.screenshot()


# --- Google Fonts: bundled (self-contained, network blocked) ---

def test_svg_google_fonts_bundle(page, update_baselines):
    vlc.configure(auto_google_fonts=True)
    svg = vlc.vegalite_to_svg(load_spec_inline("google_fonts"), bundle=True)

    assert "<defs><style>" in svg
    assert "@font-face" in svg
    assert "data:font/woff2;base64," in svg

    screenshot = render_svg_inline(page, svg, "svg_google_fonts_bundle.png", block_network=True)
    compare_screenshot(screenshot, "svg_google_fonts_bundle.png", update_baselines)


# --- Google Fonts: CDN references (@import, needs HTTP server) ---

def test_svg_google_fonts_cdn(page, svg_server, update_baselines):
    vlc.configure(auto_google_fonts=True)
    svg = vlc.vegalite_to_svg(load_spec_inline("google_fonts"), bundle=False)

    assert "<defs><style>" in svg
    assert "@import" in svg
    assert "fonts.googleapis.com" in svg

    screenshot = render_svg_served(
        page, svg, svg_server, "svg_google_fonts_cdn.html"
    )
    compare_screenshot(screenshot, "svg_google_fonts_cdn.png", update_baselines)


# --- Pacifico (distinctive Google Font): bundled ---

def test_svg_pacifico_bundle(page, update_baselines):
    vlc.configure(auto_google_fonts=True)
    svg = vlc.vegalite_to_svg(load_spec("pacifico_title"), bundle=True)

    assert "<defs><style>" in svg
    assert "data:font/woff2;base64," in svg

    screenshot = render_svg_inline(page, svg, "svg_pacifico_bundle.png", block_network=True)
    compare_screenshot(screenshot, "svg_pacifico_bundle.png", update_baselines)


# --- Local font: bundled with embed_local_fonts ---

def test_svg_local_font_bundle(page, update_baselines):
    vlc.register_font_directory(str(fonts_dir / "Caveat" / "static"))
    vlc.configure(auto_google_fonts=False, embed_local_fonts=True)
    svg = vlc.vegalite_to_svg(load_spec("local_font"), bundle=True)

    assert "<defs><style>" in svg
    assert "@font-face" in svg
    assert "data:font/woff2;base64," in svg

    screenshot = render_svg_inline(page, svg, "svg_local_font_bundle.png", block_network=True)
    compare_screenshot(screenshot, "svg_local_font_bundle.png", update_baselines)


# --- No Google Fonts, no embed_local: SVG unchanged ---

def test_svg_no_fonts_unchanged(page, update_baselines):
    vlc.configure(auto_google_fonts=False, embed_local_fonts=False)
    svg = vlc.vegalite_to_svg(load_spec("pacifico_title"), bundle=False)

    # pacifico_title uses Pacifico which is a Google Font,
    # but with auto_google_fonts=False it won't be detected
    assert "<defs><style>" not in svg
    assert "@font-face" not in svg


# --- Remote images: bundled (inlined as data URIs, network blocked) ---

def test_svg_remote_images_bundle(page, update_baselines):
    vlc.configure(auto_google_fonts=True)
    svg = vlc.vegalite_to_svg(load_spec("remote_images"), bundle=True)

    # All image hrefs should be replaced with data URIs
    # (URLs may still appear in aria-label text, so check href attributes specifically)
    import re
    image_hrefs = re.findall(r'<image[^>]*href="([^"]+)"', svg)
    for href in image_hrefs:
        assert href.startswith("data:image/"), f"Image href not inlined: {href[:80]}"
    assert len(image_hrefs) == 3

    screenshot = render_svg_inline(page, svg, "svg_remote_images_bundle.png", block_network=True)
    compare_screenshot(screenshot, "svg_remote_images_bundle.png", update_baselines)


# --- Remote images: not bundled (original URLs preserved) ---

def test_svg_remote_images_no_bundle(page, svg_server, update_baselines):
    vlc.configure(auto_google_fonts=True)
    svg = vlc.vegalite_to_svg(load_spec("remote_images"), bundle=False)

    # External image hrefs should be preserved (not inlined)
    import re
    image_hrefs = re.findall(r'<image[^>]*href="([^"]+)"', svg)
    for href in image_hrefs:
        assert not href.startswith("data:"), f"Image href should not be inlined: {href[:80]}"
    assert len(image_hrefs) == 3

    screenshot = render_svg_served(
        page, svg, svg_server, "svg_remote_images_no_bundle.html"
    )
    compare_screenshot(screenshot, "svg_remote_images_no_bundle.png", update_baselines)

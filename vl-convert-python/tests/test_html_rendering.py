"""Browser-based visual regression tests for HTML export.

Renders HTML-exported charts in headless Chromium via Playwright and compares
screenshots to baselines using SSIM. Run with `pixi run test-html`.

Baselines are generated on Linux. Non-Linux platforms use a looser SSIM
threshold to tolerate font rendering differences.
"""

import io
import os
import sys
import tempfile
from pathlib import Path

import pytest
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
baselines_dir = tests_dir / "html-baselines"
failures_dir = tests_dir / "html-failures"


def load_spec(name: str) -> str:
    return (specs_dir / f"{name}.vl.json").read_text()


def load_spec_inline(name: str) -> str:
    """Load a VL spec and replace remote data URLs with inline data."""
    import json
    import urllib.request

    spec = json.loads(load_spec(name))
    if "data" in spec and "url" in spec["data"]:
        url = spec["data"]["url"]
        with urllib.request.urlopen(url) as resp:
            spec["data"] = {"values": json.loads(resp.read())}
    return json.dumps(spec)


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


def render_html(page, html: str, *, block_network: bool = False) -> bytes:
    if block_network:
        page.route("http://**/*", lambda route: route.abort())
        page.route("https://**/*", lambda route: route.abort())

    with tempfile.NamedTemporaryFile(
        suffix=".html", delete=False, mode="w"
    ) as f:
        f.write(html)
        tmp_path = f.name

    try:
        page.goto(f"file://{tmp_path}", wait_until="networkidle")
        page.wait_for_function(
            """() => {
                const svg = document.querySelector('svg');
                const canvas = document.querySelector('canvas');
                return (svg && svg.querySelectorAll('path, rect, circle, line, text').length > 0)
                    || (canvas && canvas.width > 0);
            }""",
            timeout=15000,
        )
        # Screenshot just the chart container, not the full page
        chart = page.locator("#vega-chart")
        return chart.screenshot()
    finally:
        os.unlink(tmp_path)


@pytest.fixture(scope="module")
def browser():
    with sync_playwright() as p:
        b = p.chromium.launch()
        yield b
        b.close()


@pytest.fixture
def page(browser):
    p = browser.new_page(viewport={"width": 800, "height": 600})
    yield p
    p.close()


# --- Basic chart tests (replace Puppeteer CI validation) ---


def test_circle_binned_bundle(page, update_baselines):
    html = vlc.vegalite_to_html(load_spec_inline("circle_binned"), bundle=True)
    screenshot = render_html(page, html, block_network=True)
    compare_screenshot(screenshot, "circle_binned_bundle.png", update_baselines)


def test_circle_binned_cdn(page, update_baselines):
    html = vlc.vegalite_to_html(load_spec("circle_binned"), bundle=False)
    screenshot = render_html(page, html, block_network=False)
    compare_screenshot(screenshot, "circle_binned_cdn.png", update_baselines)


def test_stacked_bar_bundle(page, update_baselines):
    html = vlc.vegalite_to_html(load_spec_inline("stacked_bar_h"), bundle=True)
    screenshot = render_html(page, html, block_network=True)
    compare_screenshot(screenshot, "stacked_bar_bundle.png", update_baselines)


# --- Google Fonts tests ---


def test_google_fonts_bundle(page, update_baselines):
    vlc.configure(auto_google_fonts=True)
    html = vlc.vegalite_to_html(load_spec_inline("google_fonts"), bundle=True)
    screenshot = render_html(page, html, block_network=True)
    compare_screenshot(screenshot, "google_fonts_bundle.png", update_baselines)


def test_google_fonts_cdn(page, update_baselines):
    vlc.configure(auto_google_fonts=True)
    html = vlc.vegalite_to_html(load_spec_inline("google_fonts"), bundle=False)
    screenshot = render_html(page, html, block_network=False)
    compare_screenshot(screenshot, "google_fonts_cdn.png", update_baselines)


# --- Distinctive font + local font tests ---


def test_pacifico_bundle(page, update_baselines):
    vlc.configure(auto_google_fonts=True)
    html = vlc.vegalite_to_html(load_spec("pacifico_title"), bundle=True)
    screenshot = render_html(page, html, block_network=True)
    compare_screenshot(screenshot, "pacifico_bundle.png", update_baselines)


def test_local_font_bundle(page, update_baselines):
    vlc.register_font_directory(str(fonts_dir / "Caveat" / "static"))
    vlc.configure(html_embed_local_fonts=True, auto_google_fonts=False)
    html = vlc.vegalite_to_html(load_spec("local_font"), bundle=True)
    screenshot = render_html(page, html, block_network=True)
    compare_screenshot(screenshot, "local_font_bundle.png", update_baselines)

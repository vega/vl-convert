"""Browser-based visual regression tests for HTML export.

Renders HTML-exported charts in headless Chromium via Playwright and compares
screenshots to baselines using SSIM. Run with `pixi run test-html`.

Baselines are generated on Linux. Non-Linux platforms use a looser SSIM
threshold to tolerate font rendering differences.
"""

import http.server
import io
import socket
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
baselines_dir = tests_dir / "html-baselines"
html_dir = tests_dir / "html-baselines" / "html"
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

    # Pad smaller image with white to match dimensions if within 5px
    # (cross-platform font rendering can shift layout by a few pixels)
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


def render_html(
    page, html: str, baseline_name: str, *, block_network: bool = False
) -> bytes:
    if block_network:
        page.route("http://**/*", lambda route: route.abort())
        page.route("https://**/*", lambda route: route.abort())

    # Save HTML for manual inspection
    html_dir.mkdir(parents=True, exist_ok=True)
    html_path = html_dir / baseline_name.replace(".png", ".html")
    html_path.write_text(html)

    page.goto(f"file://{html_path}", wait_until="networkidle")
    page.wait_for_function(
        """() => {
            const svg = document.querySelector('svg');
            const canvas = document.querySelector('canvas');
            return (svg && svg.querySelectorAll('path, rect, circle, line, text').length > 0)
                || (canvas && canvas.width > 0);
        }""",
        timeout=15000,
    )
    chart = page.locator("#vega-chart")
    return chart.screenshot()


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


# --- Basic chart tests (replace Puppeteer CI validation) ---


def test_circle_binned_bundle(page, update_baselines):
    html = vlc.vegalite_to_html(load_spec_inline("circle_binned"), bundle=True)
    screenshot = render_html(page, html, "circle_binned_bundle.png", block_network=True)
    compare_screenshot(screenshot, "circle_binned_bundle.png", update_baselines)


def test_circle_binned_cdn(page, update_baselines):
    html = vlc.vegalite_to_html(load_spec("circle_binned"), bundle=False)
    screenshot = render_html(page, html, "circle_binned_cdn.png")
    compare_screenshot(screenshot, "circle_binned_cdn.png", update_baselines)


def test_stacked_bar_bundle(page, update_baselines):
    html = vlc.vegalite_to_html(load_spec_inline("stacked_bar_h"), bundle=True)
    screenshot = render_html(page, html, "stacked_bar_bundle.png", block_network=True)
    compare_screenshot(screenshot, "stacked_bar_bundle.png", update_baselines)


# --- Google Fonts tests ---
# CDN test must run before bundle test: the bundle path downloads fonts
# into fontdb, which causes the CDN fast path to classify them as local
# and skip <link> tags on subsequent calls.


def test_google_fonts_cdn(page, update_baselines):
    vlc.configure(auto_google_fonts=True)
    html = vlc.vegalite_to_html(load_spec_inline("google_fonts"), bundle=False)
    screenshot = render_html(page, html, "google_fonts_cdn.png")
    compare_screenshot(screenshot, "google_fonts_cdn.png", update_baselines)


def test_google_fonts_bundle(page, update_baselines):
    vlc.configure(auto_google_fonts=True)
    html = vlc.vegalite_to_html(load_spec_inline("google_fonts"), bundle=True)
    screenshot = render_html(page, html, "google_fonts_bundle.png", block_network=True)
    compare_screenshot(screenshot, "google_fonts_bundle.png", update_baselines)


# --- Distinctive font + local font tests ---


def test_pacifico_bundle(page, update_baselines):
    vlc.configure(auto_google_fonts=True)
    html = vlc.vegalite_to_html(load_spec("pacifico_title"), bundle=True)
    screenshot = render_html(page, html, "pacifico_bundle.png", block_network=True)
    compare_screenshot(screenshot, "pacifico_bundle.png", update_baselines)


def test_local_font_bundle(page, update_baselines):
    vlc.register_font_directory(str(fonts_dir / "Caveat" / "static"))
    vlc.configure(auto_google_fonts=False, embed_local_fonts=True)
    html = vlc.vegalite_to_html(load_spec("local_font"), bundle=True)
    screenshot = render_html(page, html, "local_font_bundle.png", block_network=True)
    compare_screenshot(screenshot, "local_font_bundle.png", update_baselines)


# ---------------------------------------------------------------------------
# Plugin tests
# ---------------------------------------------------------------------------

# Inline ESM that registers a custom color scheme.
_SCHEME_PLUGIN = (
    "export default function(vega) {"
    " vega.scheme('testscheme', ['red', 'green', 'blue']); "
    "}"
)

# Inline ESM that registers an expression function using a bundled HTTP import.
# The import from esm.sh is resolved at configure() time via deno_emit.
_HTTP_IMPORT_PLUGIN = """\
import { scaleLinear } from 'https://esm.sh/d3-scale@4';

export default function(vega) {
    const s = scaleLinear().domain([0, 10]).range([0, 200]);
    vega.expressionFunction('d3scaled', (x) => s(x));
}
"""

# Self-contained bar chart that uses the 'testscheme' color scheme.
_SCHEME_SPEC = """{
  "$schema": "https://vega.github.io/schema/vega/v5.json",
  "width": 200, "height": 150, "padding": 5,
  "data": [{"name": "t", "values": [
    {"c": "A", "v": 28}, {"c": "B", "v": 55}, {"c": "C", "v": 43}
  ]}],
  "scales": [
    {"name": "x", "type": "band", "domain": {"data": "t", "field": "c"},
     "range": "width", "padding": 0.1},
    {"name": "y", "type": "linear", "domain": {"data": "t", "field": "v"},
     "range": "height", "nice": true},
    {"name": "color", "type": "ordinal", "domain": {"data": "t", "field": "c"},
     "range": {"scheme": "testscheme"}}
  ],
  "marks": [{"type": "rect", "from": {"data": "t"}, "encode": {"enter": {
    "x": {"scale": "x", "field": "c"}, "width": {"scale": "x", "band": 1},
    "y": {"scale": "y", "field": "v"}, "y2": {"scale": "y", "value": 0},
    "fill": {"scale": "color", "field": "c"}
  }}}]
}"""

# Chart that renders two text marks using d3scaled() from the HTTP-import plugin.
_EXPR_SPEC = """{
  "$schema": "https://vega.github.io/schema/vega/v5.json",
  "width": 200, "height": 100, "padding": 5,
  "marks": [
    {"type": "text", "encode": {"enter": {
      "text": {"signal": "'d3scaled(2) = ' + d3scaled(2)"},
      "x": {"value": 10}, "y": {"value": 40},
      "fontSize": {"value": 16}, "fill": {"value": "#333"}
    }}},
    {"type": "text", "encode": {"enter": {
      "text": {"signal": "'d3scaled(8) = ' + d3scaled(8)"},
      "x": {"value": 10}, "y": {"value": 70},
      "fontSize": {"value": 16}, "fill": {"value": "#333"}
    }}}
  ]
}"""


def test_plugin_custom_scheme_bundle(page, update_baselines):
    """Plugin registers a named color scheme; bundle=True embeds all JS."""
    vlc.configure(vega_plugins=[_SCHEME_PLUGIN])
    try:
        html = vlc.vega_to_html(_SCHEME_SPEC, bundle=True)
        screenshot = render_html(
            page, html, "plugin_custom_scheme_bundle.png", block_network=True
        )
        compare_screenshot(screenshot, "plugin_custom_scheme_bundle.png", update_baselines)
    finally:
        vlc.configure(vega_plugins=None)


def test_plugin_custom_scheme_cdn(page, update_baselines):
    """Plugin registers a named color scheme; bundle=False loads Vega from CDN."""
    vlc.configure(vega_plugins=[_SCHEME_PLUGIN])
    try:
        html = vlc.vega_to_html(_SCHEME_SPEC, bundle=False)
        screenshot = render_html(page, html, "plugin_custom_scheme_cdn.png")
        compare_screenshot(screenshot, "plugin_custom_scheme_cdn.png", update_baselines)
    finally:
        vlc.configure(vega_plugins=None)


def test_plugin_http_import_bundle(page, update_baselines):
    """Plugin with HTTP import (esm.sh); bundled at configure() time via deno_emit.

    bundle=True: the fully-inlined plugin blob is embedded, so the chart
    renders correctly even with network blocked.
    """
    vlc.configure(
        vega_plugins=[_HTTP_IMPORT_PLUGIN],
        plugin_import_domains=["esm.sh"],
    )
    try:
        html = vlc.vega_to_html(_EXPR_SPEC, bundle=True)
        screenshot = render_html(
            page, html, "plugin_http_import_bundle.png", block_network=True
        )
        compare_screenshot(
            screenshot, "plugin_http_import_bundle.png", update_baselines
        )
    finally:
        vlc.configure(vega_plugins=None, plugin_import_domains=[])


def test_plugin_http_import_cdn(page, update_baselines):
    """Plugin with HTTP import (esm.sh); bundle=False (Vega from CDN, plugin
    source inlined as blob URL since CDN fetch happens at configure() time, so
    the rendered HTML is self-contained w.r.t. the plugin).
    """
    vlc.configure(
        vega_plugins=[_HTTP_IMPORT_PLUGIN],
        plugin_import_domains=["esm.sh"],
    )
    try:
        html = vlc.vega_to_html(_EXPR_SPEC, bundle=False)
        screenshot = render_html(page, html, "plugin_http_import_cdn.png")
        compare_screenshot(screenshot, "plugin_http_import_cdn.png", update_baselines)
    finally:
        vlc.configure(vega_plugins=None, plugin_import_domains=[])


# ---------------------------------------------------------------------------
# URL-entry plugin tests (local HTTP server)
# ---------------------------------------------------------------------------

# Plugin served from localhost. Registers a distinct color scheme so the
# rendered chart visually confirms the URL entry was fetched and executed.
_URL_PLUGIN_SOURCE = (
    "export default function(vega) {"
    " vega.scheme('urlscheme', ['purple', 'orange', 'cyan']); "
    "}"
)

# Same bar chart spec as _SCHEME_SPEC but using 'urlscheme'.
_URL_SCHEME_SPEC = _SCHEME_SPEC.replace('"testscheme"', '"urlscheme"')

# Two-file plugin: dep.js exports colors, plugin_with_dep.js imports them via
# a relative path. Tests that deno_emit resolves relative imports against the
# entry URL rather than a synthetic local path.
_DEP_SOURCE = "export const relSchemeColors = ['gold', 'teal', 'coral'];"
_PLUGIN_WITH_DEP_SOURCE = """\
import { relSchemeColors } from './dep.js';
export default function(vega) {
    vega.scheme('relscheme', relSchemeColors);
}
"""

# Same spec as _URL_SCHEME_SPEC but using 'relscheme'.
_REL_SCHEME_SPEC = _SCHEME_SPEC.replace('"testscheme"', '"relscheme"')

_CHART_READY_JS = """() => {
    const svg = document.querySelector('svg');
    const canvas = document.querySelector('canvas');
    return (svg && svg.querySelectorAll('path, rect, circle, line, text').length > 0)
        || (canvas && canvas.width > 0);
}"""


@pytest.fixture(scope="module")
def plugin_server():
    """Local HTTP server for URL-plugin tests.

    Serves:
    - /plugin.js: single-file scheme plugin (urlscheme)
    - /dep.js: ES module exporting color array
    - /plugin_with_dep.js: imports ./dep.js (tests relative-import resolution)
    - /plugin_redirect.js: 301 redirect to /plugin.js (tests redirect handling)
    """
    tmp = tempfile.TemporaryDirectory()
    serve_dir = Path(tmp.name)
    (serve_dir / "plugin.js").write_text(_URL_PLUGIN_SOURCE)
    (serve_dir / "dep.js").write_text(_DEP_SOURCE)
    (serve_dir / "plugin_with_dep.js").write_text(_PLUGIN_WITH_DEP_SOURCE)

    class _Handler(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(serve_dir), **kwargs)

        def do_GET(self):
            if self.path == "/plugin_redirect.js":
                self.send_response(301)
                self.send_header("Location", "/plugin.js")
                self.end_headers()
            else:
                super().do_GET()

        def log_message(self, *args):
            pass  # suppress noisy output

    # Bind directly to port 0 to avoid race between finding a free port and
    # binding to it (TCPServer picks the port atomically).
    server = socketserver.TCPServer(("localhost", 0), _Handler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    yield {"port": port, "serve_dir": serve_dir}

    server.shutdown()
    thread.join()
    tmp.cleanup()


def test_plugin_url_entry_bundle(page, plugin_server, update_baselines):
    """URL-entry plugin: fetched and bundled at configure() time.

    bundle=True inlines the pre-bundled plugin source as a blob URL, so the
    page renders correctly even with network fully blocked.
    """
    port = plugin_server["port"]
    plugin_url = f"http://localhost:{port}/plugin.js"

    vlc.configure(
        vega_plugins=[plugin_url],
        plugin_import_domains=["localhost"],
    )
    try:
        html = vlc.vega_to_html(_URL_SCHEME_SPEC, bundle=True)
        screenshot = render_html(
            page, html, "plugin_url_entry_bundle.png", block_network=True
        )
        compare_screenshot(screenshot, "plugin_url_entry_bundle.png", update_baselines)
    finally:
        vlc.configure(vega_plugins=None, plugin_import_domains=[])


def test_plugin_url_entry_cdn(page, plugin_server, update_baselines):
    """URL-entry plugin with bundle=False: browser fetches plugin via import().

    The HTML is served from the same localhost origin as the plugin so that
    import('http://localhost:PORT/plugin.js') is a same-origin request and
    Chromium does not block it.
    """
    port = plugin_server["port"]
    serve_dir = plugin_server["serve_dir"]
    plugin_url = f"http://localhost:{port}/plugin.js"

    vlc.configure(
        vega_plugins=[plugin_url],
        plugin_import_domains=["localhost"],
    )
    try:
        html = vlc.vega_to_html(_URL_SCHEME_SPEC, bundle=False)
    finally:
        vlc.configure(vega_plugins=None, plugin_import_domains=[])

    # Persist HTML for manual inspection
    html_dir.mkdir(parents=True, exist_ok=True)
    (html_dir / "plugin_url_entry_cdn.html").write_text(html)

    # Write HTML into the server directory so the page and plugin share origin
    (serve_dir / "plugin_url_entry_cdn.html").write_text(html)

    page.goto(
        f"http://localhost:{port}/plugin_url_entry_cdn.html",
        wait_until="networkidle",
    )
    page.wait_for_function(_CHART_READY_JS, timeout=15000)
    screenshot = page.locator("#vega-chart").screenshot()

    compare_screenshot(screenshot, "plugin_url_entry_cdn.png", update_baselines)


def test_plugin_url_relative_imports_bundle(page, plugin_server, update_baselines):
    """URL-entry plugin that uses a relative import (./dep.js).

    Exercises the deno_emit bundler's relative-import resolution: the entry
    URL is passed as the bundle root so './dep.js' resolves against the same
    localhost origin, not a synthetic local path.
    """
    port = plugin_server["port"]
    plugin_url = f"http://localhost:{port}/plugin_with_dep.js"

    vlc.configure(
        vega_plugins=[plugin_url],
        plugin_import_domains=["localhost"],
    )
    try:
        html = vlc.vega_to_html(_REL_SCHEME_SPEC, bundle=True)
        screenshot = render_html(
            page, html, "plugin_url_relative_imports_bundle.png", block_network=True
        )
        compare_screenshot(
            screenshot, "plugin_url_relative_imports_bundle.png", update_baselines
        )
    finally:
        vlc.configure(vega_plugins=None, plugin_import_domains=[])


def test_plugin_url_redirect_bundle(page, plugin_server, update_baselines):
    """URL-entry plugin behind a 301 redirect.

    Exercises PluginBundleLoader's redirect handling: the entry URL returns
    a redirect whose Location header is validated against plugin_import_domains
    before the final plugin source is fetched and bundled.
    """
    port = plugin_server["port"]
    plugin_url = f"http://localhost:{port}/plugin_redirect.js"

    vlc.configure(
        vega_plugins=[plugin_url],
        plugin_import_domains=["localhost"],
    )
    try:
        html = vlc.vega_to_html(_URL_SCHEME_SPEC, bundle=True)
        screenshot = render_html(
            page, html, "plugin_url_redirect_bundle.png", block_network=True
        )
        compare_screenshot(
            screenshot, "plugin_url_redirect_bundle.png", update_baselines
        )
    finally:
        vlc.configure(vega_plugins=None, plugin_import_domains=[])

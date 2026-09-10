from __future__ import annotations

import sys
import tomllib
from html import escape
from pathlib import Path

from sphinx.search import js_index

project = "VlConvert"
author = "Vega"
copyright = "2026, Vega"

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).parent))

with (ROOT / "Cargo.toml").open("rb") as workspace_manifest:
    release = tomllib.load(workspace_manifest)["workspace"]["package"]["version"]

extensions = [
    "myst_parser",
    "sphinx_design",
    "sphinx_copybutton",
    "sphinx.ext.extlinks",
    "autodoc2",
    "sphinxcontrib.programoutput",
    "sphinxcontrib.openapi",
    "_ext.vl_chart",
]

html_theme = "pydata_sphinx_theme"
html_title = "VlConvert"
# Logo assets live in the top-level logo/ directory. Sphinx and the theme copy
# them into the output _static/ directory at build time.
LOGO_DIR = ROOT / "logo"
html_favicon = str(LOGO_DIR / "vl-convert-favicon.svg")
html_static_path = ["_static"]
html_css_files = ["custom.css"]
html_js_files = ["conversion-example.js", "terminal-prompts.js"]
html_theme_options = {
    "github_url": "https://github.com/vega/vl-convert",
    "navbar_align": "left",
    "show_toc_level": 2,
    "logo": {
        "image_light": str(LOGO_DIR / "vl-convert-logo.svg"),
        "image_dark": str(LOGO_DIR / "vl-convert-logo.svg"),
        "text": "VlConvert",
        "alt_text": "VlConvert",
    },
}
if "-rc" in release:
    html_theme_options["announcement"] = (
        "These docs cover the <strong>VlConvert 2.0 release candidate</strong>. "
        '<a href="https://github.com/vega/vl-convert/releases">View releases</a>.'
    )

myst_enable_extensions = ["colon_fence", "fieldlist", "deflist", "substitution"]
myst_heading_anchors = 3

extlinks = {
    "rust-api": (f"https://docs.rs/vl-convert-rs/{release}/vl_convert_rs/%s", "%s"),
    "server-api": (
        f"https://docs.rs/vl-convert-server/{release}/vl_convert_server/%s", "%s"
    ),
}

# `pixi run docs-preview-chart` writes the front-page URL output beside the
# other chart outputs.
EDITOR_URL_FILE = Path(__file__).parent / "_static" / "charts" / "stacked_bar_h.url.txt"
if not EDITOR_URL_FILE.exists():
    raise FileNotFoundError(
        f"{EDITOR_URL_FILE} is missing; run `pixi run docs-preview-chart`"
    )
editor_url = EDITOR_URL_FILE.read_text().strip()
myst_substitutions = {
    "front_page_editor_url": (
        '<a class="conversion-example__url-link" '
        f'href="{escape(editor_url, quote=True)}" '
        'target="_blank" rel="noopener">'
        f"{escape(editor_url)}</a>"
    ),
}

copybutton_prompt_text = r">>> |\.\.\. |\$ |> "
copybutton_prompt_is_regexp = True

autodoc2_packages = [
    {
        "path": "../../vl-convert-python/vl_convert.pyi",
        "module": "vl_convert",
        "auto_mode": False,
    }
]
autodoc2_render_plugin = "myst"
autodoc2_docstring_parser_regexes = [
    (r"vl_convert\..*", "_ext.google_docstring_parser")
]
autodoc2_replace_annotations = [("vl_convert.", "")]

python_maximum_signature_line_length = 88
python_trailing_comma_in_multi_line_signatures = True

exclude_patterns = [
    "_build",
    "_topics",
    "Thumbs.db",
    ".DS_Store",
]


def label_search_results(app, exception):
    """Distinguish interface pages in search without changing their headings."""
    if exception is not None or app.builder.name != "html":
        return
    path = Path(app.outdir) / "searchindex.js"
    index = js_index.loads(path.read_text(encoding="utf-8"))
    original_titles = index["titles"].copy()
    labels = {"python": "Python", "cli": "CLI", "rust": "Rust", "server": "Server"}
    for position, docname in enumerate(index["docnames"]):
        if label := labels.get(docname.split("/")[0]):
            index["titles"][position] += f" · {label}"
    all_titles = {}
    for title, entries in index["alltitles"].items():
        for position, anchor in entries:
            labeled = (
                index["titles"][position] if title == original_titles[position] else title
            )
            all_titles.setdefault(labeled, []).append((position, anchor))
    index["alltitles"] = all_titles
    path.write_text(js_index.dumps(index), encoding="utf-8")


def setup(app):
    app.connect("build-finished", label_search_results)

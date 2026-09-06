from __future__ import annotations

import sys
from pathlib import Path

project = "VlConvert"
author = "Vega"
copyright = "2026, Vega"

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).parent))

extensions = [
    "myst_parser",
    "sphinx_design",
    "sphinx_copybutton",
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

myst_enable_extensions = ["colon_fence", "fieldlist", "deflist", "substitution"]
myst_heading_anchors = 3

# The front-page example links to the Vega Editor with a URL that
# `pixi run docs-preview-chart` writes beside the other chart outputs.
EDITOR_URL_FILE = Path(__file__).parent / "_static" / "charts" / "front-page-chart.url.txt"
if not EDITOR_URL_FILE.exists():
    raise FileNotFoundError(
        f"{EDITOR_URL_FILE} is missing; run `pixi run docs-preview-chart`"
    )
myst_substitutions = {
    "front_page_editor_link": (
        '<a class="front-page-editor-link" '
        f'href="{EDITOR_URL_FILE.read_text().strip()}" '
        'target="_blank" rel="noopener">'
        '<i class="fa-solid fa-arrow-up-right-from-square"></i>'
        '<code class="docutils literal notranslate">'
        '<span class="pre">Editor URL</span></code></a>'
    ),
}

copybutton_prompt_text = r">>> |\.\.\. |\$ "
copybutton_prompt_is_regexp = True

autodoc2_packages = [
    {
        "path": "../../vl-convert-python/vl_convert.pyi",
        "module": "vl_convert",
        "auto_mode": False,
    }
]
autodoc2_render_plugin = "myst"
autodoc2_docstring_parser_regexes = [(r"vl_convert\..*", "_ext.google_docstring_parser")]
autodoc2_replace_annotations = [("vl_convert.", "")]

python_maximum_signature_line_length = 88
python_trailing_comma_in_multi_line_signatures = True

exclude_patterns = [
    "_build",
    "_topics",
    "Thumbs.db",
    ".DS_Store",
]

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
]

html_theme = "pydata_sphinx_theme"
html_title = "VlConvert"
html_favicon = "_static/vl-convert-favicon.svg"
html_static_path = ["_static"]
html_css_files = ["custom.css"]
html_theme_options = {
    "github_url": "https://github.com/vega/vl-convert",
    "navbar_align": "left",
    "show_toc_level": 2,
    "logo": {
        "image_light": "_static/vl-convert-logo.svg",
        "image_dark": "_static/vl-convert-logo.svg",
        "text": "VlConvert",
        "alt_text": "VlConvert",
    },
}

myst_enable_extensions = ["colon_fence", "fieldlist", "deflist"]
myst_heading_anchors = 3

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

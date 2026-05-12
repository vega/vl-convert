"""Docutils parser that runs Google-style docstrings through Napoleon."""

from __future__ import annotations

from docutils.parsers.rst import Parser as RstParser
from sphinx.ext.napoleon import Config
from sphinx.ext.napoleon.docstring import GoogleDocstring

NAPOLEON_CONFIG = Config(napoleon_google_docstring=True, napoleon_numpy_docstring=False)


class Parser(RstParser):
    """Parse Google-style API docstrings as reStructuredText."""

    supported = ("vlconvert-google-docstring",)

    def parse(self, inputstring: str, document) -> None:  # type: ignore[override]
        converted = str(GoogleDocstring(inputstring, config=NAPOLEON_CONFIG))
        super().parse(converted, document)

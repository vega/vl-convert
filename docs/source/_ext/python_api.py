"""Expose the sync and async type stubs as a package for autodoc2."""

from __future__ import annotations

import ast
from pathlib import Path

from sphinx.application import Sphinx


def setup(app: Sphinx) -> dict[str, bool]:
    """Generate documentation stubs without importing the Python extension."""
    root = Path(app.confdir).parents[1]
    source = (root / "vl-convert-python" / "vl_convert.pyi").read_text(encoding="utf-8")
    tree = ast.parse(source)
    functions = {
        node.name: node for node in tree.body if isinstance(node, ast.FunctionDef)
    }
    namespace = next(
        node
        for node in ast.walk(tree)
        if isinstance(node, ast.ClassDef) and node.name == "_AsyncioModule"
    )
    methods = [
        node
        for node in namespace.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
    ]
    for method in methods:
        method.args.args.pop(0)
        # Async stubs refer to the sync functions for parameter documentation.
        method.body = functions[method.name].body

    async_source = "from typing import Any, Literal\nfrom vl_convert import *\n\n"
    async_source += ast.unparse(ast.Module(body=methods, type_ignores=[])) + "\n"
    package = root / "docs" / "build" / "python-api" / "vl_convert"
    package.mkdir(parents=True, exist_ok=True)
    for name, content in {"__init__.pyi": source, "asyncio.pyi": async_source}.items():
        path = package / name
        if not path.exists() or path.read_text(encoding="utf-8") != content:
            path.write_text(content, encoding="utf-8")

    return {"parallel_read_safe": True, "parallel_write_safe": True}

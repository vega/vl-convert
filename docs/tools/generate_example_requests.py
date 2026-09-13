#!/usr/bin/env python3
"""Generate complete server request examples from canonical input files."""

from __future__ import annotations

import json
import re
from pathlib import Path

import yaml


SOURCE = Path(__file__).resolve().parents[1] / "source"
EXAMPLES = SOURCE / "_examples"
OUTPUT = SOURCE / "_generated" / "requests"


def build_requests() -> dict[str, str]:
    """Combine canonical inputs and options into formatted JSON request bodies."""
    manifest = yaml.safe_load(
        (EXAMPLES / "server-requests.yaml").read_text(encoding="utf-8")
    )
    generated = {}
    for name, entry in manifest["requests"].items():
        if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", name):
            raise ValueError(f"Invalid request name: {name!r}")
        inputs = entry["inputs"]
        options = entry.get("options", {})
        if collisions := inputs.keys() & options.keys():
            raise ValueError(
                f"{name}: options overwrite input fields: {sorted(collisions)}"
            )

        body = {}
        for field, declaration in inputs.items():
            source = Path(declaration["source"])
            if source.is_absolute() or ".." in source.parts:
                raise ValueError(f"{name}: invalid input path: {source}")
            text = (EXAMPLES / source).read_text(encoding="utf-8")
            body[field] = {"json": json.loads, "text": str}[declaration["format"]](text)
        body.update(options)
        generated[f"{name}.json"] = (
            json.dumps(body, indent=2, ensure_ascii=False) + "\n"
        )
    return generated


def main() -> None:
    generated = build_requests()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    for filename, contents in generated.items():
        path = OUTPUT / filename
        if not path.exists() or path.read_text(encoding="utf-8") != contents:
            path.write_text(contents, encoding="utf-8")
    for path in OUTPUT.glob("*.json"):
        if path.name not in generated:
            path.unlink()


if __name__ == "__main__":
    main()

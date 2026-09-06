#!/usr/bin/env python3
"""Generate complete server request examples from canonical input files."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any

import yaml


ALLOWED_INPUT_FIELDS = {"snippet", "spec", "svg", "vega_plugin"}
ALLOWED_INPUT_FORMATS = {"json", "text"}
REQUEST_NAME = re.compile(r"^[a-z0-9][a-z0-9-]*$")


class ManifestError(ValueError):
    """Report an invalid server-request manifest."""


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def default_examples_dir() -> Path:
    return repo_root() / "docs" / "source" / "_examples"


def default_manifest_path() -> Path:
    return default_examples_dir() / "server-requests.yaml"


def default_output_dir() -> Path:
    return repo_root() / "docs" / "source" / "_generated" / "requests"


def _mapping(value: Any, location: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ManifestError(f"{location} must be a mapping")
    return value


def _read_input(
    declaration: Any,
    *,
    field: str,
    request_name: str,
    examples_dir: Path,
) -> Any:
    declaration = _mapping(
        declaration, f"request {request_name!r} input {field!r}"
    )
    unknown = set(declaration) - {"source", "format"}
    if unknown:
        raise ManifestError(
            f"request {request_name!r} input {field!r} has unknown field(s): "
            f"{sorted(unknown)}"
        )

    source = declaration.get("source")
    input_format = declaration.get("format")
    if not isinstance(source, str) or not source:
        raise ManifestError(
            f"request {request_name!r} input {field!r} needs a source"
        )
    if input_format not in ALLOWED_INPUT_FORMATS:
        raise ManifestError(
            f"request {request_name!r} input {field!r} format must be one of "
            f"{sorted(ALLOWED_INPUT_FORMATS)}"
        )

    relative = Path(source)
    if relative.is_absolute() or ".." in relative.parts:
        raise ManifestError(
            f"request {request_name!r} input {field!r} has an invalid source: "
            f"{source}"
        )
    path = examples_dir / relative
    if not path.is_file():
        raise ManifestError(
            f"request {request_name!r} input {field!r} source does not exist: "
            f"{source}"
        )

    text = path.read_text(encoding="utf-8")
    if input_format == "text":
        return text
    try:
        return json.loads(text)
    except json.JSONDecodeError as exc:
        raise ManifestError(
            f"request {request_name!r} input {field!r} is not valid JSON: "
            f"{source}: {exc.msg}"
        ) from exc


def build_requests(manifest_path: Path, examples_dir: Path) -> dict[str, str]:
    """Return generated filenames and their stable JSON contents."""
    if not manifest_path.is_file():
        raise ManifestError(f"manifest does not exist: {manifest_path}")
    manifest = yaml.safe_load(manifest_path.read_text(encoding="utf-8"))
    manifest = _mapping(manifest, "manifest")
    unknown = set(manifest) - {"requests"}
    if unknown:
        raise ManifestError(f"manifest has unknown field(s): {sorted(unknown)}")

    requests = _mapping(manifest.get("requests"), "manifest requests")
    generated: dict[str, str] = {}
    for request_name, raw_entry in requests.items():
        if not isinstance(request_name, str) or not REQUEST_NAME.fullmatch(request_name):
            raise ManifestError(
                f"invalid request name {request_name!r}; use lowercase letters, "
                "digits, and hyphens"
            )
        entry = _mapping(raw_entry, f"request {request_name!r}")
        unknown = set(entry) - {"inputs", "options"}
        if unknown:
            raise ManifestError(
                f"request {request_name!r} has unknown field(s): {sorted(unknown)}"
            )

        inputs = _mapping(entry.get("inputs"), f"request {request_name!r} inputs")
        if not inputs:
            raise ManifestError(f"request {request_name!r} needs at least one input")
        invalid_fields = set(inputs) - ALLOWED_INPUT_FIELDS
        if invalid_fields:
            raise ManifestError(
                f"request {request_name!r} has invalid input field(s): "
                f"{sorted(invalid_fields)}"
            )

        options = entry.get("options", {})
        options = _mapping(options, f"request {request_name!r} options")
        collisions = set(inputs) & set(options)
        if collisions:
            raise ManifestError(
                f"request {request_name!r} options would overwrite generated "
                f"field(s): {sorted(collisions)}"
            )

        body = {
            field: _read_input(
                declaration,
                field=field,
                request_name=request_name,
                examples_dir=examples_dir,
            )
            for field, declaration in inputs.items()
        }
        body.update(options)
        generated[f"{request_name}.json"] = (
            json.dumps(body, indent=2, ensure_ascii=False) + "\n"
        )
    return generated


def write_requests(
    manifest_path: Path,
    examples_dir: Path,
    output_dir: Path,
) -> None:
    """Write generated request bodies and remove stale generated requests."""
    generated = build_requests(manifest_path, examples_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    expected = {output_dir / filename for filename in generated}
    for filename, contents in generated.items():
        path = output_dir / filename
        if not path.exists() or path.read_text(encoding="utf-8") != contents:
            path.write_text(contents, encoding="utf-8")

    for path in output_dir.glob("*.json"):
        if path not in expected:
            path.unlink()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=default_manifest_path())
    parser.add_argument("--examples-dir", type=Path, default=default_examples_dir())
    parser.add_argument("--output-dir", type=Path, default=default_output_dir())
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        write_requests(args.manifest, args.examples_dir, args.output_dir)
    except ManifestError as exc:
        raise SystemExit(f"server request manifest: {exc}") from exc
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

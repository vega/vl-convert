#!/usr/bin/env python3
"""Generate example setup files from the workspace version."""

from __future__ import annotations

import tomllib
from pathlib import Path

from packaging.version import Version


ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "docs" / "source" / "_generated" / "setup"


def build_setup_files(version: str) -> dict[str, str]:
    """Return example setup files pinned to the supplied version."""
    python_version = Version(version)

    return {
        "Cargo.toml": f"""[package]
name = "vl-convert-example"
version = "0.1.0"
edition = "2021"

[dependencies]
serde_json = "1"
tokio = {{ version = "1", features = ["macros", "rt-multi-thread"] }}
vl-convert-rs = "={version}"
""",
        "pyproject.toml": f"""[project]
name = "vl-convert-example"
version = "0.1.0"
requires-python = ">=3.10"
dependencies = ["vl-convert-python=={python_version}"]
""",
        "install-vl-convert.md": (
            "Download a [prebuilt executable]"
            "(/cli/getting-started/installation.md#prebuilt-executable), "
            "or build it with Cargo:\n\n"
            "```console\n"
            "$ cargo install vl-convert \\\n"
            f">   --version '={version}' \\\n"
            ">   --locked\n"
            "```\n"
        ),
    }


def write_setup_files(output_dir: Path, files: dict[str, str]) -> None:
    """Write setup files and remove stale generated files."""
    output_dir.mkdir(parents=True, exist_ok=True)
    expected = {output_dir / filename for filename in files}
    for filename, contents in files.items():
        path = output_dir / filename
        if not path.exists() or path.read_text(encoding="utf-8") != contents:
            path.write_text(contents, encoding="utf-8")

    for path in output_dir.iterdir():
        if path.is_file() and path not in expected:
            path.unlink()


def main() -> None:
    with (ROOT / "Cargo.toml").open("rb") as file:
        version = tomllib.load(file)["workspace"]["package"]["version"]
    write_setup_files(OUTPUT, build_setup_files(version))


if __name__ == "__main__":
    main()

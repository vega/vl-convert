#!/usr/bin/env python3
"""Generate example setup files from the workspace version."""

from __future__ import annotations

import argparse
import tomllib
from pathlib import Path

from packaging.version import InvalidVersion, Version


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def default_workspace_manifest() -> Path:
    return repo_root() / "Cargo.toml"


def default_output_dir() -> Path:
    return repo_root() / "docs" / "source" / "_generated" / "setup"


def read_workspace_version(path: Path) -> str:
    """Read the version shared by the workspace packages."""
    try:
        with path.open("rb") as file:
            manifest = tomllib.load(file)
        version = manifest["workspace"]["package"]["version"]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as exc:
        raise ValueError(f"cannot read workspace package version from {path}") from exc

    if not isinstance(version, str) or not version:
        raise ValueError(f"workspace package version in {path} must be a string")
    return version


def build_setup_files(version: str) -> dict[str, str]:
    """Return example setup files pinned to the supplied version."""
    try:
        python_version = Version(version)
    except InvalidVersion as exc:
        raise ValueError(
            f"workspace version is not valid for Python: {version}"
        ) from exc

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
            "Install the `vl-convert` binary:\n\n"
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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--workspace-manifest", type=Path, default=default_workspace_manifest()
    )
    parser.add_argument("--output-dir", type=Path, default=default_output_dir())
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        version = read_workspace_version(args.workspace_manifest)
        write_setup_files(args.output_dir, build_setup_files(version))
    except ValueError as exc:
        raise SystemExit(f"example setup: {exc}") from exc
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

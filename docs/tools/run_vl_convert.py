#!/usr/bin/env python3
"""Run the local vl-convert binary for documentation generation."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def binary_name() -> str:
    return "vl-convert.exe" if os.name == "nt" else "vl-convert"


def find_binary() -> Path:
    env_bin = os.environ.get("VL_CONVERT_BIN")
    if env_bin:
        path = Path(env_bin).expanduser()
        if path.exists():
            return path
        raise SystemExit(f"VL_CONVERT_BIN does not exist: {path}")

    root = repo_root()
    for profile in ("debug", "release"):
        path = root / "target" / profile / binary_name()
        if path.exists():
            return path

    raise SystemExit(
        "Could not find vl-convert. Run `cargo build -p vl-convert` or set "
        "VL_CONVERT_BIN."
    )


def run(args: list[str]) -> int:
    binary = find_binary()
    proc = subprocess.run([str(binary), *args], cwd=repo_root())
    return proc.returncode


if __name__ == "__main__":
    raise SystemExit(run(sys.argv[1:]))

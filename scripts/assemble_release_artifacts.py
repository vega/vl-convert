#!/usr/bin/env python3
"""Verify and assemble release artifacts."""

from __future__ import annotations

import argparse
import hashlib
import shutil
from collections import Counter
from pathlib import Path


def expected_artifacts(version: str) -> set[str]:
    return {
        "vl-convert_linux-64.zip",
        "vl-convert_linux-aarch64.zip",
        "vl-convert_osx-64.zip",
        "vl-convert_osx-arm64.zip",
        "vl-convert_win-64.zip",
        f"vl_convert_python-{version}-cp39-abi3-macosx_10_12_x86_64.whl",
        f"vl_convert_python-{version}-cp39-abi3-macosx_11_0_arm64.whl",
        f"vl_convert_python-{version}-cp39-abi3-manylinux_2_28_aarch64.whl",
        f"vl_convert_python-{version}-cp39-abi3-manylinux_2_28_x86_64.whl",
        f"vl_convert_python-{version}-cp39-abi3-win_amd64.whl",
        f"vl_convert_python-{version}.tar.gz",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("incoming", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    expected = expected_artifacts(args.version)
    files = [path for path in args.incoming.rglob("*") if path.is_file()]
    actual = Counter(path.name for path in files)
    wanted = Counter(expected)
    if actual != wanted:
        missing = sorted((wanted - actual).elements())
        extra = sorted((actual - wanted).elements())
        raise SystemExit(f"Artifact mismatch; missing={missing}, extra={extra}")

    args.output.mkdir()
    for path in files:
        shutil.copy2(path, args.output / path.name)

    with (args.output / "SHA256SUMS").open("w", encoding="utf-8") as checksums:
        for name in sorted(expected):
            digest = hashlib.sha256((args.output / name).read_bytes()).hexdigest()
            checksums.write(f"{digest}  {name}\n")

    print(f"Assembled {len(expected)} artifacts in {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

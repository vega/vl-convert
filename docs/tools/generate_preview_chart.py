#!/usr/bin/env python3
"""Regenerate the front-page chart in each documented output format."""

from __future__ import annotations

from run_vl_convert import repo_root, run


CHART_DIR = repo_root() / "docs" / "source" / "_static" / "charts"
SPEC = CHART_DIR / "front-page-chart.vl.json"


def main() -> int:
    outputs = [
        ("vl2svg", "front-page-chart.svg", []),
        ("vl2png", "front-page-chart.png", ["--scale", "2"]),
        ("vl2pdf", "front-page-chart.pdf", []),
    ]
    for command, filename, extra_args in outputs:
        status = run(
            [
                "--vlc-config",
                "disabled",
                command,
                "--input",
                str(SPEC),
                "--output",
                str(CHART_DIR / filename),
                "--vl-version",
                "6.4",
                *extra_args,
            ]
        )
        if status != 0:
            return status
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

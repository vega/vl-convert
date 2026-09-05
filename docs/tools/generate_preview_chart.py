#!/usr/bin/env python3
"""Regenerate the front-page example chart in each documented output format."""

from __future__ import annotations

from run_vl_convert import repo_root, run


CHART_DIR = repo_root() / "docs" / "source" / "_static" / "charts"
SPEC = CHART_DIR / "front-page-chart.vl.json"
VL_VERSION = ["--vl-version", "6.4"]


def main() -> int:
    outputs = [
        ("vl2svg", "front-page-chart.svg", VL_VERSION),
        ("vl2png", "front-page-chart.png", [*VL_VERSION, "--scale", "2"]),
        ("vl2pdf", "front-page-chart.pdf", VL_VERSION),
        ("vl2html", "front-page-chart.html", [*VL_VERSION, "--bundle"]),
        ("vl2vg", "front-page-chart.vg.json", [*VL_VERSION, "--pretty"]),
        # The editor URL encodes the specification itself, so it takes no version.
        ("vl2url", "front-page-chart.url.txt", []),
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
                *extra_args,
            ]
        )
        if status != 0:
            return status
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

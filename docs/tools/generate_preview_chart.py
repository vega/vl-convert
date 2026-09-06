#!/usr/bin/env python3
"""Regenerate the front-page example chart in each documented output format."""

from __future__ import annotations

from run_vl_convert import repo_root, run


DOCS_SOURCE_DIR = repo_root() / "docs" / "source"
CHART_DIR = DOCS_SOURCE_DIR / "_static" / "charts"
SPEC = DOCS_SOURCE_DIR / "_examples" / "stacked_bar_h.vl.json"
VL_VERSION = ["--vl-version", "6.4"]


def main() -> int:
    outputs = [
        ("vl2svg", "stacked_bar_h.svg", VL_VERSION),
        ("vl2png", "stacked_bar_h.png", [*VL_VERSION, "--scale", "2"]),
        ("vl2pdf", "stacked_bar_h.pdf", VL_VERSION),
        ("vl2html", "stacked_bar_h.html", [*VL_VERSION, "--bundle"]),
        ("vl2vg", "stacked_bar_h.vg.json", [*VL_VERSION, "--pretty"]),
        # The editor URL encodes the specification itself, so it takes no version.
        ("vl2url", "stacked_bar_h.url.txt", []),
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

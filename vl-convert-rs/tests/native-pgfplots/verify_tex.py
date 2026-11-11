"""Compile actual backend output and verify PDF geometry, not just TeX strings.

Requires Node >= 22, TeX Live with PGFPlots, and PyMuPDF. With --native, also
requires the patched vl-convert-python extension; a missing extension fails.
This is numerical/rendering verification, not a claim of Vega pixel identity.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import math
from pathlib import Path
import subprocess

import fitz

HERE = Path(__file__).resolve().parent
BLUE = (76 / 255, 120 / 255, 168 / 255)


def close_color(a, b):
    return a is not None and max(abs(x - y) for x, y in zip(a, b)) < 0.002


def frame(page):
    candidates = [d["rect"] for d in page.get_drawings()
                  if close_color(d["color"], (0, 0, 0))
                  and d["rect"].width > 50 and d["rect"].height > 50
                  and any(item[0] in ("qu", "re") for item in d["items"])]
    assert candidates, "no native axis frame in rendered PDF"
    return max(candidates, key=lambda r: r.get_area())


def mapped(rect, x, y, xd, yd, log_y=False):
    if log_y:
        y, yl, yh = math.log10(y), math.log10(yd[0]), math.log10(yd[1])
    else:
        yl, yh = yd
    return fitz.Point(rect.x0 + (x - xd[0]) / (xd[1] - xd[0]) * rect.width,
                      rect.y1 - (y - yl) / (yh - yl) * rect.height)


def colored_vertices(page):
    points = []
    for drawing in page.get_drawings():
        if not close_color(drawing["color"], BLUE):
            continue
        for item in drawing["items"]:
            if item[0] == "l":
                points.extend(item[1:])
            elif item[0] == "qu":
                points.extend([item[1].ul, item[1].ur, item[1].ll, item[1].lr])
            elif item[0] == "re":
                points.extend([item[1].tl, item[1].tr, item[1].bl, item[1].br])
    return points


def contains_vertex(actual, expected):
    assert any(abs(p - expected) < 0.04 for p in actual), (expected, actual)


def check_geometry(output):
    results = []
    for name, points, xd, yd, log in [
        ("line", [(1, 2), (2, 4), (3, 3)], [0, 3], [0, 4], False),
        ("log", [(1, 2), (2, 4), (3, 3)], [0, 3], [1, 10], True),
        ("errorbars", [(1, 1), (1, 3), (2, 2), (2, 5), (3, 2.5), (3, 4)], [0, 3], [0, 5], False),
        ("asymmetric", [(1, 3), (1, 6), (2, 2.5), (2, 4.5)], [0, 2], [0, 6], False),
    ]:
        with fitz.open(output / f"{name}.pdf") as doc:
            page = doc[0]
            rect = frame(page)
            assert abs(rect.width - 225) < 0.04
            assert abs(rect.height - 150) < 0.04
            vertices = colored_vertices(page)
            for x, y in points:
                contains_vertex(vertices, mapped(rect, x, y, xd, yd, log))
            assert page.get_text().strip(), "labels must remain text, not outlines"
            assert not page.get_images(), "no image embedding fallback"
        results.append({"probe": name, "vertices": len(points), "tolerance_bp": 0.04})

    with fitz.open(output / "histogram.pdf") as doc:
        page = doc[0]
        rect = frame(page)
        vertices = colored_vertices(page)
        # Expected bins are [0,1), [1,2), [2,3), [3,4], including the upper edge.
        for left, count in enumerate([2, 1, 2, 2]):
            for x in (left, left + 1):
                contains_vertex(vertices, mapped(rect, x, count, [0, 4], [0, 2]))
        results.append({"probe": "histogram", "vertices": 8, "tolerance_bp": 0.04})

    with fitz.open(output / "horizontal_boxplot.pdf") as doc:
        page = doc[0]
        rect = frame(page)
        expected = mapped(rect, 100, 0, [0, 100], [-0.5, 0.5])
        marks = [d["rect"] for d in page.get_drawings()
                 if close_color(d["color"], BLUE) and any(i[0] == "c" for i in d["items"])]
        assert any(abs(fitz.Point((r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2) - expected) < 0.04 for r in marks), marks
        results.append({"probe": "horizontal_boxplot_outlier", "coordinate": [100, 0], "tolerance_bp": 0.04})

    with fitz.open(output / "heatmap.pdf") as doc:
        page = doc[0]
        rect = frame(page)
        image = page.get_pixmap(matrix=fitz.Matrix(2, 2), colorspace=fitz.csRGB)
        for x, y, z in [(0, 0, 0), (0, 1, 1), (1, 0, 2), (1, 1, 3)]:
            point = mapped(rect, x, y, [-0.5, 1.5], [-0.5, 1.5])
            sample = image.pixel(round(point.x * 2), round(point.y * 2))
            expected = (round(z / 3 * 255), 0, round((1 - z / 3) * 255))
            assert max(abs(a - b) for a, b in zip(sample, expected)) <= 3, (sample, expected)
        results.append({"probe": "heatmap", "cells": 4, "rgb_tolerance": 3})
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--native", action="store_true", help="compare built PyO3 output with the identical source under Node")
    parser.add_argument("--cli", type=Path, help="also compare output of a built vl2pgfplots binary")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    subprocess.run(["node", str(HERE / "generate.cjs"), str(output)], check=True)
    if args.native:
        import vl_convert as vlc
        for specfile in sorted(output.glob("*.vl.json")):
            name = specfile.name.removesuffix(".vl.json")
            tex = vlc.vegalite_to_pgfplots(json.loads(specfile.read_text()), standalone=True, tex_labels=name == "math")
            assert tex == (output / f"{name}.tex").read_text(), name
    if args.cli:
        for specfile in sorted(output.glob("*.vl.json")):
            name = specfile.name.removesuffix(".vl.json")
            command = [str(args.cli.resolve()), "--input", str(specfile), "--standalone"]
            if name == "math":
                command.append("--tex-labels")
            tex = subprocess.check_output(command, text=True, timeout=60)
            assert tex == (output / f"{name}.tex").read_text(), name

    def compile_tex(path):
        proc = subprocess.run(["pdflatex", "-no-shell-escape", "-interaction=nonstopmode", "-halt-on-error", path.name], cwd=output, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=60)
        path.with_suffix(".stdout").write_bytes(proc.stdout)
        assert proc.returncode == 0, f"{path.name}: see {path.with_suffix('.stdout')}"
        with fitz.open(path.with_suffix(".pdf")) as doc:
            assert len(doc) == 1, path.name
            assert not doc[0].get_images(), path.name
            pix = doc[0].get_pixmap(matrix=fitz.Matrix(1.5, 1.5))
            pix.save(path.with_suffix(".png"))
        return path.stem

    with ThreadPoolExecutor(max_workers=4) as pool:
        figures = list(pool.map(compile_tex, sorted(output.glob("*.tex"))))
    report = {
        "source": "actual native_pgfplots.js (not a substitute emitter)",
        "node": subprocess.check_output(["node", "--version"], text=True).strip(),
        "tex": subprocess.check_output(["pdflatex", "--version"], text=True).splitlines()[0],
        "figures": figures,
        "geometry": check_geometry(output),
        "native_extension_compared": args.native,
        "native_cli_compared": bool(args.cli),
    }
    (output / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()

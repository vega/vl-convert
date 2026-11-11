"""Compile actual native output and preserve paired figures and failure evidence.

Rendering success is not numerical parity or visual approval. The report keeps
those independent states. Sources and dataset identities are recorded separately.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import io
import json
from pathlib import Path
import subprocess

import fitz
from PIL import Image, ImageDraw


def image(pdf):
    with fitz.open(stream=pdf, filetype="pdf") as doc:
        if len(doc) != 1:
            raise ValueError("Expected a single-page figure")
        scale = min(2, 1400 / max(doc[0].rect.width, doc[0].rect.height))
        pix = doc[0].get_pixmap(matrix=fitz.Matrix(scale, scale), alpha=False)
        return Image.frombytes("RGB", [pix.width, pix.height], pix.samples)


def run_case(case, row):
    if row["status"] != "tex_generated":
        return row
    try:
        run = subprocess.run(
            ["pdflatex", "-no-shell-escape", "-interaction=nonstopmode", "-halt-on-error", "native.tex"],
            cwd=case, text=True, capture_output=True, timeout=40,
        )
        (case / "latex.stdout.txt").write_text(run.stdout + run.stderr)
        if run.returncode:
            raise RuntimeError(run.stdout[-2200:])
        pdf = (case / "native.pdf").read_bytes()
        with fitz.open(stream=pdf, filetype="pdf") as doc:
            if len(doc) != 1 or doc[0].get_images(full=True):
                raise AssertionError("Native figure must be single-page and contain no embedded raster images")
        row["latex"] = "passed"
        target = image(pdf)
        target.save(case / "native.png")
        if (case / "reference.svg").exists():
            with fitz.open(stream=(case / "reference.svg").read_bytes(), filetype="svg") as ref:
                reference = image(ref.convert_to_pdf())
            reference.save(case / "reference.png")
            canvas = Image.new("RGB", (1440, 660), "white")
            draw = ImageDraw.Draw(canvas)
            draw.text((20, 12), row["id"], fill="black")
            for i, (im, title) in enumerate([(reference, "Official Vega-Lite / Vega"), (target, "Native PGFPlots / LaTeX")]):
                x = 20 + 710 * i
                draw.text((x, 38), title, fill="black")
                im.thumbnail((690, 570))
                canvas.paste(im, (x + (690-im.width)//2, 70 + (570-im.height)//2))
            canvas.save(case / "pair.png")
            row["pair"] = "pair.png"
    except Exception as exc:
        row["latex"] = "failed"
        row["latex_error"] = f"{type(exc).__name__}: {exc}"
    return row


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--workers", type=int, default=4)
    args = parser.parse_args()
    report = json.loads((args.directory / "outcomes.json").read_text())
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        results = list(pool.map(lambda row: run_case(args.directory/row["id"], row), report["examples"]))
    report["examples"] = results
    (args.directory / "outcomes.json").write_text(json.dumps(report, indent=2))
    from collections import Counter
    print(dict(Counter(r.get("latex", "not_attempted") for r in results)))
    for r in results:
        if r.get("latex") == "failed":
            print(r["id"], r["latex_error"][-350:])
    html = "<html><meta charset='utf-8'><title>Actual renderer comparisons</title><body><h1>Native PGFPlots conformance</h1><p>Pairs are actual renders, not a pixel-parity certification.</p>"
    import html as h
    for row in results:
        html += f"<h2>{h.escape(row['id'])}</h2><p>{h.escape(row['status'])}; LaTeX: {row.get('latex','not attempted')}</p>"
        if row.get("pair"):
            html += f"<img width='1200' src='{row['id']}/pair.png'>"
        elif row.get("error"):
            html += f"<pre>{h.escape(row['error'].splitlines()[0])}</pre>"
    (args.directory / "index.html").write_text(html+"</body></html>")


if __name__ == "__main__":
    main()

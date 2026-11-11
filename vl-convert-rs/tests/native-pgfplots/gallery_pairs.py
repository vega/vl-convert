"""Render paired, genuinely executed Altair/Vega-Lite and native PGFPlots figures.

Examples reproduce the original gallery's data and chart definitions where noted.
The exact *same normalized Vega-Lite JSON* is passed to both renderers.
No matplotlib plotting or imitation of either renderer is used.
"""
from __future__ import annotations

import json
import math
import os
from pathlib import Path
import subprocess
import sys
import traceback

import altair as alt
import fitz
import numpy as np
import pandas as pd
from PIL import Image, ImageDraw, ImageFont
import vl_convert as vlc

OUT = Path(sys.argv[1] if len(sys.argv) > 1 else "gallery-comparisons").resolve()
OUT.mkdir(parents=True, exist_ok=True)
ROOT = Path(__file__).resolve().parents[3]
JS = ROOT / "vl-convert-rs/src/js/native_pgfplots.js"

def examples():
    # Copied from Altair's official source files at the gallery-manifest revision.
    df = pd.DataFrame({"a": list("ABCDEFGHI"),
                       "b": [28, 55, 43, 91, 81, 53, 19, 87, 52]})
    yield ("simple_bar_chart", "Simple Bar Chart", "exact gallery chart and data",
           alt.Chart(df).mark_bar().encode(x="a", y="b"))

    x = np.arange(100)
    df = pd.DataFrame({"x": x, "f(x)": np.sin(x / 5)})
    yield ("simple_line_chart", "Simple Line Chart", "exact gallery chart and data",
           alt.Chart(df).mark_line().encode(x="x", y="f(x)"))

    df = pd.DataFrame({"Category": list("AAABBBCCC"),
                       "Group": list("xyzxyzxyz"),
                       "Value": [0.1, 0.6, 0.9, 0.7, 0.2, 1.1, 0.6, 0.1, 0.2]})
    yield ("grouped_bar_chart2", "Grouped Bar Chart with xOffset", "exact gallery chart and data",
           alt.Chart(df).mark_bar().encode(
               x="Category:N", y="Value:Q", xOffset="Group:N", color="Group:N"
           ))

    x, y = np.meshgrid(range(-5, 5), range(-5, 5))
    z = x**2 + y**2
    df = pd.DataFrame({"x": x.ravel(), "y": y.ravel(), "z": z.ravel()})
    yield ("simple_heatmap", "Simple Heatmap", "exact gallery chart and data",
           alt.Chart(df).mark_rect().encode(x="x:O", y="y:O", color="z:Q"))

    # The original errorbars gallery applies transform_calculate; this adapted
    # version materializes bounds first because that transform isn't supported.
    rng = np.random.RandomState(0)
    x = np.arange(1, 6)
    y = rng.normal(10, .5, len(x))
    df = pd.DataFrame({"x": x, "y": y,
                       "ymin": y-.2, "ymax": y+.2})
    base = alt.Chart(df)
    yield ("simple_scatter_with_errorbars", "Scatter with Error Bars",
           "adapted: original calculate transform materialized to explicit bounds",
           (base.mark_point(filled=True, size=50, color="black")
                .encode(x=alt.X("x:Q", scale=alt.Scale(domain=[0, 6])),
                        y=alt.Y("y:Q", scale=alt.Scale(zero=False)))
            +base.mark_errorbar().encode(x="x:Q", y="ymin:Q", y2="ymax:Q")))

def normalize(spec):
    # Not a semantic rewrite: discard only browser-only theme/config metadata.
    # Store this exact JSON, which BOTH renderers will consume.
    spec = dict(spec)
    for key in ("config", "background", "usermeta"):
        spec.pop(key, None)
    return spec

NODE = r"""
const fs=require('fs');
require(process.argv[1]);
const spec=JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const result=globalThis.vlcNativePgfplots.compile(spec,{standalone:true,tex_labels:false});
fs.writeFileSync(process.argv[3],result);
"""

def image_from_pdf(path):
    with fitz.open(path) as pdf:
        assert len(pdf) == 1, "PGFPlots output must have one page"
        images = list(pdf[0].get_images(full=True))
        if images:
            raise AssertionError("PGFPlots PDF contains embedded images, not just native vectors")
        pix=pdf[0].get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False)
        return Image.frombytes("RGB", (pix.width, pix.height), pix.samples)

def make_pair(left, right, title, detail):
    w=660; h=470; margin=22
    result=Image.new("RGB", (w*2+margin*3, h+128), "#ffffff")
    d=ImageDraw.Draw(result)
    d.text((margin,18), title, fill="#1a2433")
    d.text((margin,48), detail, fill="#657085")
    for i,(name,im) in enumerate([("ALTAIR → VEGA-LITE SVG",left), ("OUR BACKEND → PGFPLOTS → PDF",right)]):
        x=margin+i*(w+margin)
        d.rounded_rectangle((x,83,x+w,109),radius=6,fill="#f0f3f8")
        d.text((x+12,90),name,fill="#283648")
        bg=Image.new("RGB",(w,h),"white")
        im.thumbnail((w-12,h-12))
        bg.paste(im,((w-im.width)//2,(h-im.height)//2))
        result.paste(bg,(x,111))
        d.rectangle((x,110,x+w,110+h),outline="#cbd5e1",width=2)
    return result

results=[]
for slug,title,provenance,chart in examples():
    case=OUT/slug
    case.mkdir(exist_ok=True)
    info={"id":slug,"title":title,"source_provenance":provenance,
          "url":"https://altair-viz.github.io/gallery/"+slug+".html",
          "status":"pending","reference_renderer":"vl-convert (Vega-Lite SVG)",
          "target_renderer":"native PGFPlots compiler (same JS source embedded by Rust), pdflatex"}
    try:
        spec=normalize(chart.to_dict())
        input_path=case/(slug+".vl.json")
        input_path.write_text(json.dumps(spec,indent=2,default=str))
        svg=vlc.vegalite_to_svg(spec)
        (case/(slug+".altair.svg")).write_text(svg)
        import io
        ref=Image.open(io.BytesIO(vlc.vegalite_to_png(spec,scale=2))).convert("RGB")
        ref.save(case/(slug+".altair.png"))
        tex=case/(slug+".tex")
        run=subprocess.run(["node","-e",NODE,str(JS),str(input_path),str(tex)],
                            cwd=ROOT,capture_output=True,text=True,timeout=120)
        if run.returncode:
            raise RuntimeError("Native compiler failure:\n"+run.stderr[-2000:])
        p=subprocess.run(["pdflatex","-interaction=nonstopmode","-halt-on-error",
                         "-no-shell-escape",tex.name],cwd=case,capture_output=True,text=True,timeout=120)
        if p.returncode:
            raise RuntimeError("pdflatex failure:\n"+p.stdout[-2200:])
        pdf=tex.with_suffix(".pdf")
        target=image_from_pdf(pdf)
        target.save(case/(slug+".pgfplots.png"))
        pair=make_pair(ref,target,title,provenance+" — SAME normalized spec")
        pair.save(OUT/(slug+".side_by_side.png"))
        info.update(status="passed",altair_size=list(ref.size),
                    pgfplots_size=list(target.size))
    except Exception as e:
        info.update(status="failed",error=str(e)[-3500:])
        (case/"error.txt").write_text(traceback.format_exc())
    results.append(info)
(OUT/"report.json").write_text(json.dumps({"examples":results,"engine_note":"Comparisons are visual examples, not proof of pixel parity. TeX typography and default padding differ."},indent=2))
(OUT/"index.md").write_text("# Actual side-by-side renders\n\n"+
    "\n".join(f"- [{e['title']}](./{e['id']}.side_by_side.png): **{e['status']}** — {e.get('error','')[:180]}" for e in results))
print(json.dumps([{"id":e["id"],"status":e["status"],"error":e.get("error","")[:200]} for e in results],indent=2))
if any(e["status"]!="passed" for e in results):
    sys.exit(1)

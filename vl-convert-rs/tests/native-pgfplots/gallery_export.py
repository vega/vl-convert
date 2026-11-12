"""Export pinned Altair examples unchanged; resolve datasets by verified content.

This executes trusted fixture Python. --altair-source must be the pinned CI
checkout; never point it at untrusted Python. The optional archive mode checks
the catalog identity but is not a substitute for trusting the source archive. Keep source acquisition separate from conformance tests so an
unavailable dataset cannot be misreported as a compiler failure.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import random
import sys
import subprocess
import traceback
from urllib.parse import urlparse, unquote

ALTAIR_COMMIT = "27f89b8d4f2861d446cde8a13480ca5e366eb318"
ROOT = Path(__file__).resolve().parents[3]


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    parser.add_argument("--altair-source", type=Path, required=True)
    parser.add_argument("--datasets", type=Path, required=True)
    parser.add_argument("--allow-source-archive", action="store_true")
    args = parser.parse_args()
    source = args.altair_source.resolve()
    pin = subprocess.run(
        ["git", "-C", str(source), "rev-parse", "HEAD"], capture_output=True, text=True
    )
    if pin.returncode == 0:
        if pin.stdout.strip() != ALTAIR_COMMIT:
            raise RuntimeError("Altair checkout does not match the reviewed commit")
    elif not args.allow_source_archive:
        raise RuntimeError(
            "A pinned Git checkout is required (or explicitly allow a trusted archive)"
        )
    args.output.mkdir(parents=True, exist_ok=True)
    sys.path.insert(0, str(source))
    # Source archives lack distribution metadata. This is the fallback explicitly
    # declared by Altair's build configuration; source identity is the commit/hash.
    try:
        importlib.metadata.version("altair")
    except importlib.metadata.PackageNotFoundError:
        metadata = args.output / "metadata" / "altair-4.0.0+fallback.dist-info"
        metadata.mkdir(parents=True, exist_ok=True)
        (metadata / "METADATA").write_text(
            "Metadata-Version: 2.1\nName: altair\nVersion: 4.0.0+fallback\n"
        )
        sys.path.insert(0, str(metadata.parent))
    import numpy as np
    import altair as alt
    from altair.datasets._cache import DatasetCache, csv_cache
    from altair.datasets._cache import _iter_metadata

    alt.data_transformers.disable_max_rows()
    registry = json.loads(
        (ROOT / "docs/source/_topics/altair_gallery_registry.json").read_text()
    )
    manifest_path = source / "_data/examples.json"
    if (
        sha256(manifest_path)
        != "106d2a6be9677f33f905bedcaff59e5a6c134bcd29953eac6b6466a8eff277e3"
    ):
        raise RuntimeError("Altair gallery catalog content does not match the pin")
    manifest = json.loads(manifest_path.read_text())
    entries = {e["name"]: e for e in manifest["examples"]}
    if set(entries) != {e["id"] for e in registry["examples"]}:
        raise RuntimeError(
            "Pinned gallery catalog differs from registry; review exclusions before running"
        )
    urls = {}
    used = set()
    metadata = list(csv_cache.mapping.values())
    for m in metadata:
        fp = args.datasets / "data" / m["file_name"]
        if not fp.is_file():
            continue
        b = fp.read_bytes()
        blob = hashlib.sha1(b"blob " + str(len(b)).encode() + b"\0" + b).hexdigest()
        if blob == m["sha"]:
            urls[m["url"]] = {
                "path": str(fp.resolve()),
                "sha256": sha256(fp),
                "git_blob": blob,
            }

    def local_dataset(self, meta):
        if meta["url"] not in urls:
            raise RuntimeError(
                "Pinned dataset missing or content mismatch: " + meta["url"]
            )
        used.add(meta["url"])
        return Path(urls[meta["url"]]["path"])

    DatasetCache._maybe_download = local_dataset
    results = []
    for entry in registry["examples"]:
        slug = entry["id"]
        record = {
            "id": slug,
            "classification": entry["status"],
            "url": entry["url"],
            "status": "not_run",
        }
        if entry["status"].startswith("excluded_"):
            record["status"] = entry["status"]
            results.append(record)
            continue
        used.clear()
        path = source / entries[slug]["path"]
        record["source_sha256"] = sha256(path)
        case = args.output / slug
        case.mkdir(exist_ok=True)
        try:
            random.seed(0)
            np.random.seed(0)
            module = ast.parse(path.read_text(), filename=str(path))
            if not isinstance(module.body[-1], ast.Expr):
                raise RuntimeError(
                    "Gallery example does not end with its chart expression"
                )
            last = module.body[-1]
            module.body[-1] = ast.Assign(
                targets=[ast.Name(id="__gallery_chart", ctx=ast.Store())],
                value=last.value,
            )
            ast.fix_missing_locations(module)
            scope = {"__name__": "__gallery_fixture__", "__file__": str(path)}
            exec(compile(module, str(path), "exec"), scope)
            chart = scope["__gallery_chart"]
            spec = chart.to_dict(validate=True)
            original = (
                json.dumps(spec, indent=2, ensure_ascii=False, allow_nan=False) + "\n"
            )
            (case / "original.vl.json").write_text(original)
            record.update(
                status="exported",
                spec_sha256=hashlib.sha256(original.encode()).hexdigest(),
                loaded_datasets=sorted(used),
            )
        except Exception as exc:
            record.update(status="source_error", error=f"{type(exc).__name__}: {exc}")
            (case / "source_error.txt").write_text(traceback.format_exc())
        results.append(record)
    report = {
        "altair_commit": ALTAIR_COMMIT,
        "altair_schema": alt.SCHEMA_VERSION,
        "catalog_sha256": sha256(manifest_path),
        "python": sys.version,
        "examples": results,
        "datasets": urls,
    }
    (args.output / "sources.json").write_text(json.dumps(report, indent=2))
    from collections import Counter

    print(dict(Counter(e["status"] for e in results)))
    for r in results:
        if r["status"] == "source_error":
            print(r["id"], r["error"][:240])
    if any(r["status"] == "source_error" for r in results):
        raise SystemExit(1)


if __name__ == "__main__":
    main()

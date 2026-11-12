"""Fail on unobserved cases, source/reference errors, or accepted-case regressions.

The baseline records conversion/rendering acceptance, NOT scientific parity.
Numerical probes and visual review remain separate, explicit report fields.
"""

from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--require-native", action="store_true")
    args = parser.parse_args()
    report = json.loads((args.directory / "outcomes.json").read_text())
    baseline = json.loads((HERE / "gallery-accepted.json").read_text())
    registry = json.loads(
        (
            HERE.parents[2] / "docs/source/_topics/altair_gallery_registry.json"
        ).read_text()
    )
    expected = {e["id"] for e in registry["examples"]}
    rows = report["examples"]
    observed = {r["id"] for r in rows}
    failures = []
    if observed != expected or len(rows) != len(expected):
        failures.append("Missing/duplicate/unknown gallery outcomes")
    for row in rows:
        status = row["status"]
        if status.startswith("excluded_"):
            continue
        if status not in ("tex_generated", "unsupported"):
            failures.append(f"{row['id']}: {status}: {row.get('error', '')}")
        if row.get("reference") != "rendered":
            failures.append(f"{row['id']}: official reference did not render")
        if row["id"] in baseline["accepted"] and status != "tex_generated":
            failures.append(f"{row['id']}: previously accepted original spec regressed")
        if status == "tex_generated":
            if row.get("latex") != "passed":
                failures.append(f"{row['id']}: LaTeX did not pass")
            if args.require_native and row.get("interfaces") not in (
                "identical",
                "executed_randomized_no_byte_equality_claim",
            ):
                failures.append(f"{row['id']}: native interfaces not verified")
    summary = {
        "examples": len(rows),
        "status": dict(Counter(r["status"] for r in rows)),
        "latex": dict(Counter(r.get("latex", "not_attempted") for r in rows)),
        "interfaces": dict(Counter(r.get("interfaces", "not_attempted") for r in rows)),
        "failures": failures,
        "note": "Conversion/rendering acceptance is not full numerical or visual parity.",
    }
    (args.directory / "summary.json").write_text(json.dumps(summary, indent=2))
    print(json.dumps(summary, indent=2))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    main()

"""Reject incomplete reports, reference failures and acceptance regressions."""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path

from gallery_catalog import exclusions, load_baseline, load_catalog


def check_outcome(row: dict, accepted: set[str], require_native: bool) -> list[str]:
    failures = []
    slug, status = row["id"], row["status"]
    if status not in ("tex_generated", "unsupported"):
        failures.append(f"{slug}: {status}: {row.get('error', '')}")
    if row.get("reference") != "rendered":
        failures.append(f"{slug}: official reference did not render")
    if slug in accepted and status != "tex_generated":
        failures.append(f"{slug}: previously accepted original spec regressed")
    if status == "tex_generated":
        if row.get("latex") != "passed":
            failures.append(f"{slug}: LaTeX did not pass")
        if require_native and row.get("interfaces") not in (
            "identical",
            "equivalent_coordinates",
            "executed_randomized_no_byte_equality_claim",
        ):
            failures.append(f"{slug}: native interfaces not verified")
    return failures


def check_results(
    rows: list[dict], catalog: list[dict], baseline: dict, require_native: bool
) -> list[str]:
    expected = {entry["name"] for entry in catalog}
    observed = {row["id"] for row in rows}
    failures = []
    if observed != expected or len(rows) != len(expected):
        failures.append("Missing/duplicate/unknown gallery outcomes")
    excluded = exclusions(baseline)
    accepted = set(baseline["accepted"])
    for row in rows:
        if row["id"] in excluded:
            if row["status"] != excluded[row["id"]]:
                failures.append(f"{row['id']}: exclusion differs from test policy")
        else:
            failures.extend(check_outcome(row, accepted, require_native))
    return failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--require-native", action="store_true")
    args = parser.parse_args()
    report = json.loads((args.directory / "outcomes.json").read_text(encoding="utf-8"))
    baseline = load_baseline()
    catalog = load_catalog(args.directory / "catalog.json", baseline)
    rows = report["examples"]
    failures = check_results(rows, catalog, baseline, args.require_native)
    summary = {
        "examples": len(rows),
        "status": dict(Counter(row["status"] for row in rows)),
        "latex": dict(Counter(row.get("latex", "not_attempted") for row in rows)),
        "interfaces": dict(
            Counter(row.get("interfaces", "not_attempted") for row in rows)
        ),
        "failures": failures,
        "note": "Conversion/rendering acceptance is not full numerical or visual parity.",
    }
    (args.directory / "summary.json").write_text(
        json.dumps(summary, indent=2), encoding="utf-8"
    )
    print(json.dumps(summary, indent=2))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    main()

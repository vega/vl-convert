"""Read the pinned upstream catalog and the reviewed conformance-test policy."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

BASELINE = Path(__file__).with_name("gallery-accepted.json")


def load_baseline() -> dict:
    return json.loads(BASELINE.read_text(encoding="utf-8"))


def exclusions(baseline: dict) -> dict[str, str]:
    result = {}
    for status, names in baseline["excluded"].items():
        if status not in ("excluded_map", "excluded_interactive"):
            raise ValueError(f"Unknown gallery exclusion: {status}")
        for name in names:
            if name in result:
                raise ValueError(f"Duplicate gallery exclusion: {name}")
            result[name] = status
    return result


def load_catalog(path: Path, baseline: dict) -> list[dict]:
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != baseline["catalog_sha256"]:
        raise ValueError("Altair gallery catalog content does not match the pin")
    entries = json.loads(raw)["examples"]
    names = [entry["name"] for entry in entries]
    if len(names) != len(set(names)):
        raise ValueError("Duplicate example in the gallery catalog")
    excluded = exclusions(baseline)
    accepted = baseline["accepted"]
    if len(accepted) != len(set(accepted)):
        raise ValueError("Duplicate example in the acceptance baseline")
    if set(accepted) & excluded.keys():
        raise ValueError("An accepted gallery example cannot be excluded")
    if (set(accepted) | excluded.keys()) - set(names):
        raise ValueError("Test policy refers to unknown gallery examples")
    return entries

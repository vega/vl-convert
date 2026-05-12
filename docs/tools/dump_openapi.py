#!/usr/bin/env python3
"""Generate OpenAPI documents used by the Sphinx build."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from run_vl_convert import find_binary, repo_root


GENERATED = repo_root() / "docs" / "source" / "_generated"
SURFACES = {
    "public": GENERATED / "openapi-public.json",
    "admin": GENERATED / "openapi-admin.json",
}


def dump_surface(surface: str) -> dict:
    proc = subprocess.run(
        [str(find_binary()), "serve", f"--dump-openapi={surface}"],
        cwd=repo_root(),
        text=True,
        capture_output=True,
    )
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit(proc.returncode)

    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError as err:
        raise SystemExit(
            f"`vl-convert serve --dump-openapi={surface}` did not emit JSON: {err}"
        )


def validate_surface(surface: str, spec: dict) -> None:
    paths = spec.get("paths", {})
    if surface == "public":
        admin_paths = sorted(path for path in paths if path.startswith("/admin"))
        if admin_paths:
            raise SystemExit(f"public OpenAPI spec leaked admin paths: {admin_paths}")
    elif surface == "admin":
        non_admin_paths = sorted(path for path in paths if not path.startswith("/admin"))
        if non_admin_paths:
            raise SystemExit(f"admin OpenAPI spec leaked public paths: {non_admin_paths}")


def main() -> int:
    GENERATED.mkdir(parents=True, exist_ok=True)
    for surface, output in SURFACES.items():
        spec = dump_surface(surface)
        validate_surface(surface, spec)
        output.write_text(json.dumps(spec, indent=2, sort_keys=True) + "\n")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Generate the public OpenAPI document used by the Sphinx build."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from run_vl_convert import find_binary, repo_root


OUTPUT = repo_root() / "docs" / "source" / "_generated" / "openapi-public.json"


def main() -> int:
    proc = subprocess.run(
        [str(find_binary()), "serve", "--dump-openapi"],
        cwd=repo_root(),
        text=True,
        capture_output=True,
    )
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        return proc.returncode

    try:
        spec = json.loads(proc.stdout)
    except json.JSONDecodeError as err:
        raise SystemExit(f"`vl-convert serve --dump-openapi` did not emit JSON: {err}")

    paths = spec.get("paths", {})
    admin_paths = sorted(path for path in paths if path.startswith("/admin"))
    if admin_paths:
        raise SystemExit(f"public OpenAPI spec leaked admin paths: {admin_paths}")

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(spec, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

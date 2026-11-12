"""Verify the built extension/CLI on original gallery specs using pinned data.

Only URL transport changes (to a hash-checked local copy); chart semantics are
not rewritten. Save both inputs. Never fetch an unregistered URL or execute TeX.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
from pathlib import Path
import subprocess


def materialize_urls(value, datasets):
    if isinstance(value, list):
        return [materialize_urls(x, datasets) for x in value]
    if not isinstance(value, dict):
        return value
    result = {
        k: v if k in ("values", "datasets") else materialize_urls(v, datasets)
        for k, v in value.items()
    }
    # A URL occurs only in a data source for the supported gallery subset.
    if "url" in result and isinstance(result["url"], str):
        info = datasets.get(result["url"])
        if info is None:
            raise ValueError(f"Unpinned source URL: {result['url']}")
        source = Path(info["path"]).resolve()
        if hashlib.sha256(source.read_bytes()).hexdigest() != info["sha256"]:
            raise ValueError(f"Dataset content changed: {source}")
        result["url"] = str(source)
    return result


def randomized(value):
    if isinstance(value, list):
        return any(map(randomized, value))
    if not isinstance(value, dict):
        return isinstance(value, str) and any(
            name in value for name in ("random(", "sampleNormal(", "sampleUniform(")
        )
    mark = value.get("mark")
    kind = mark.get("type") if isinstance(mark, dict) else mark
    if kind in ("errorbar", "errorband") and not any(
        key in value.get("encoding", {}) for key in ("x2", "y2", "xError", "yError")
    ):
        if not isinstance(mark, dict) or mark.get("extent", "ci") == "ci":
            return True
    if any(
        value.get(key) in ("ci0", "ci1")
        for key in ("op", "aggregate")
        if isinstance(value.get(key), str)
    ):
        return True
    return any(map(randomized, value.values()))


_COORDINATES = re.compile(r"coordinates \{([^}]*)\}", re.DOTALL)
_NUMBER = re.compile(r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?")


def compare_coordinates(expected, actual):
    """Bound Node/V8 math roundoff only within coordinate blocks.

    Axes, labels, styles, path structure, ordering and all other TeX must remain
    byte-identical. The built Python and CLI outputs are compared exactly too.
    Do not round or rewrite the scientific data in production output.
    """
    if expected == actual:
        return {"status": "identical"}
    if _COORDINATES.sub("coordinates {}", expected) != _COORDINATES.sub(
        "coordinates {}", actual
    ):
        raise AssertionError("Non-coordinate TeX differs")
    left, right = _COORDINATES.findall(expected), _COORDINATES.findall(actual)
    if len(left) != len(right):
        raise AssertionError("Coordinate block count differs")
    differences = []
    for a, b in zip(left, right):
        if _NUMBER.sub("#", a) != _NUMBER.sub("#", b):
            raise AssertionError("Coordinate structure differs")
        xs, ys = _NUMBER.findall(a), _NUMBER.findall(b)
        if len(xs) != len(ys):
            raise AssertionError("Coordinate count differs")
        for x, y in zip(xs, ys):
            x, y = float(x), float(y)
            if not (math.isfinite(x) and math.isfinite(y)) or not math.isclose(
                x, y, rel_tol=1e-12, abs_tol=1e-12
            ):
                raise AssertionError(f"Coordinate mismatch: {x!r} versus {y!r}")
            if x != y:
                differences.append(abs(x - y))
    return {
        "status": "equivalent_coordinates",
        "relative_tolerance": 1e-12,
        "absolute_tolerance": 1e-12,
        "differing_coordinates": len(differences),
        "max_absolute_difference": max(differences, default=0),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    directory = args.directory.resolve()
    sources = json.loads((directory / "sources.json").read_text())
    report = json.loads((directory / "outcomes.json").read_text())
    import vl_convert as vlc

    roots = sorted(
        {str(Path(d["path"]).resolve().parent) for d in sources["datasets"].values()}
    )
    vlc.configure(base_url=False, allowed_base_urls=roots, auto_google_fonts=False)
    config = directory / "native-config.json"
    config.write_text(
        json.dumps(
            {"base_url": False, "allowed_base_urls": roots, "auto_google_fonts": False}
        )
    )
    errors = []
    for row in report["examples"]:
        if row["status"] != "tex_generated":
            continue
        case = directory / row["id"]
        try:
            raw = (case / "original.vl.json").read_bytes()
            if hashlib.sha256(raw).hexdigest() != row["spec_sha256"]:
                raise ValueError("Original specification hash mismatch")
            original = json.loads(raw)
            spec = materialize_urls(original, sources["datasets"])
            native_input = case / "native-input.vl.json"
            native_input.write_text(json.dumps(spec, indent=2, ensure_ascii=False))
            # Render with the actual Vega engine: generic SVG rasterizers can
            # lose gradients and produce a misleading black colorbar.
            (case / "reference.png").write_bytes(
                vlc.vegalite_to_png(spec, vl_version="v6_4", scale=2)
            )
            row["reference_rasterizer"] = "built Vega PNG renderer"
            expected = (case / "native.tex").read_text()
            tex = vlc.vegalite_to_pgfplots(spec, vl_version="v6_4", standalone=True)
            cli_tex = subprocess.check_output(
                [
                    str(args.cli.resolve()),
                    "--input",
                    str(native_input),
                    "--standalone",
                    "--vl-version",
                    "v6_4",
                    "--vlc-config",
                    str(config),
                ],
                text=True,
                timeout=120,
            )
            (case / "extension.tex").write_text(tex)
            (case / "cli.tex").write_text(cli_tex)
            if randomized(original):
                row["interfaces"] = "executed_randomized_no_byte_equality_claim"
            else:
                if tex != cli_tex:
                    raise AssertionError("Built PyO3 and CLI outputs differ")
                comparison = compare_coordinates(expected, tex)
                row["interfaces"] = comparison["status"]
                row["coordinate_comparison"] = comparison
            # The rendering pass now compiles actual PyO3 output, not just Node.
            (case / "native.tex").write_text(tex)
            row["target_engine"] = "built PyO3 extension"
        except Exception as exc:
            row["interfaces"] = "failed"
            row["interface_error"] = f"{type(exc).__name__}: {exc}"
            errors.append(row["id"] + ": " + row["interface_error"])
        finally:
            (directory / "outcomes.json").write_text(json.dumps(report, indent=2))
    for error in errors:
        print(error)
    if errors:
        raise SystemExit(1)


if __name__ == "__main__":
    main()

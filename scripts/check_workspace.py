#!/usr/bin/env python3
"""Check the workspace metadata used by the release process."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[1]
ROOT_MANIFEST = ROOT / "Cargo.toml"
PYTHON_PACKAGE = "vl-convert-python"
VENDOR_PACKAGE = "vl-convert-vendor"
PRODUCT_PACKAGES = (
    "vl-convert",
    "vl-convert-canvas2d",
    "vl-convert-canvas2d-deno",
    "vl-convert-google-fonts",
    PYTHON_PACKAGE,
    "vl-convert-rs",
    "vl-convert-server",
)
INTERNAL_DEPENDENCIES = (
    "vl-convert-canvas2d",
    "vl-convert-canvas2d-deno",
    "vl-convert-google-fonts",
    "vl-convert-rs",
    "vl-convert-server",
)
PUBLISHABLE_PACKAGES = tuple(p for p in PRODUCT_PACKAGES if p != PYTHON_PACKAGE)


class WorkspaceError(RuntimeError):
    pass


def python_version(version: str) -> str:
    from packaging.version import InvalidVersion, Version

    try:
        return str(Version(version))
    except InvalidVersion as error:
        raise WorkspaceError(f"Version {version} is not valid for Python") from error


def validate_workspace() -> str:
    result = subprocess.run(
        [
            "cargo",
            "metadata",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    metadata = json.loads(result.stdout)
    manifest = tomllib.loads(ROOT_MANIFEST.read_text(encoding="utf-8"))
    packages = {package["name"]: package for package in metadata["packages"]}
    version = manifest["workspace"]["package"]["version"]
    if "+" in version:
        raise WorkspaceError("Release versions cannot contain build metadata")

    required = set(PRODUCT_PACKAGES) | {VENDOR_PACKAGE}
    if set(packages) != required:
        raise WorkspaceError(
            f"Expected workspace packages {sorted(required)}, found {sorted(packages)}"
        )

    mismatches = {
        name: packages[name]["version"]
        for name in PRODUCT_PACKAGES
        if packages[name]["version"] != version
    }
    if mismatches:
        details = ", ".join(f"{name}={value}" for name, value in mismatches.items())
        raise WorkspaceError(f"Product versions do not match {version}: {details}")

    vendor = packages[VENDOR_PACKAGE]
    if vendor["version"] != "0.0.0" or vendor.get("publish") != []:
        raise WorkspaceError(f"{VENDOR_PACKAGE} must be private at version 0.0.0")
    if packages[PYTHON_PACKAGE].get("publish") != []:
        raise WorkspaceError(f"{PYTHON_PACKAGE} must remain private on crates.io")
    for name in PUBLISHABLE_PACKAGES:
        if packages[name].get("publish") == []:
            raise WorkspaceError(f"{name} must remain publishable")

    workspace_dependencies = manifest["workspace"]["dependencies"]
    expected_requirement = f"={version}"
    for name in INTERNAL_DEPENDENCIES:
        dependency = workspace_dependencies.get(name)
        if not isinstance(dependency, dict):
            raise WorkspaceError(f"Missing workspace dependency {name}")
        if dependency.get("version") != expected_requirement:
            raise WorkspaceError(
                f"Workspace dependency {name} must require {expected_requirement}"
            )

    for package in packages.values():
        for dependency in package.get("dependencies", []):
            if (
                dependency["name"] in INTERNAL_DEPENDENCIES
                and dependency["req"] != expected_requirement
            ):
                raise WorkspaceError(
                    f"{package['name']} requires {dependency['name']} "
                    f"{dependency['req']}; expected {expected_requirement}"
                )

    return version


def main() -> int:
    try:
        version = validate_workspace()
    except WorkspaceError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as error:
        details = (error.stderr or error.stdout or str(error)).strip()
        print(f"error: {details}", file=sys.stderr)
        return error.returncode

    print(f"Validated Rust version {version}")
    print(f"Validated Python version {python_version(version)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Assemble the versioned GitHub Pages archive from a documentation build."""

from __future__ import annotations

import argparse
import json
import re
import shutil
from pathlib import Path

from packaging.version import Version

SITE_URL = "https://vega.github.io/vl-convert"


def version_key(value: str) -> Version:
    """Validate the release directory format and return its sortable version."""
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-rc\d+)?", value):
        raise ValueError(f"Unsupported documentation version: {value}")
    return Version(value)


def archive(args):
    """Replace one version and regenerate the shared menu without deleting older versions."""
    # Validate the destination name and build contents before replacing files.
    version_key(args.version)
    if not re.fullmatch(r"[0-9a-f]{40}", args.source_sha):
        raise ValueError("Expected a full source commit SHA")
    source, site = args.build.resolve(), args.site.resolve()
    if not (source / "index.html").is_file():
        raise ValueError("The version build has no index.html")
    for path in source.rglob("*"):
        if path.is_symlink() or path.name == ".git":
            raise ValueError(f"Unexpected link or Git metadata in build: {path}")

    # A retry replaces only this version and records the source used to build it.
    target = site / args.version
    if target.is_symlink():
        raise ValueError("The version destination must not be a symlink")
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(source, target)
    (target / "release.json").write_text(
        json.dumps(
            {
                "version": args.version,
                "source_sha": args.source_sha,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )

    # Derive the menu from archived builds, not from all published releases.
    versions = []
    for metadata in site.glob("*/release.json"):
        version = json.loads(metadata.read_text(encoding="utf-8"))["version"]
        version_key(version)
        if metadata.parent.name != version:
            raise ValueError(f"Version metadata does not match directory: {metadata}")
        versions.append(version)
    versions.sort(key=version_key, reverse=True)

    # Keep the newest stable patch per minor series in the menu.
    stable = [v for v in versions if not version_key(v).is_prerelease]
    latest = stable[0] if stable else None
    series = set()
    selected = []
    for version in stable:
        key = version_key(version).release[:2]
        if key not in series:
            selected.append(version)
            series.add(key)

    # Show only the newest RC, and only while it is newer than stable.
    previews = [
        v
        for v in versions
        if version_key(v).is_prerelease
        and (latest is None or version_key(v) > version_key(latest))
    ]
    if previews:
        selected.insert(1 if latest else 0, previews[0])

    # Every archived page reads this shared theme-switcher manifest.
    url = args.url.rstrip("/")
    menu = []
    for version in selected:
        entry = {"version": version, "url": f"{url}/{version}/", "name": version}
        if version == latest:
            entry.update(name=f"{version} (stable)", preferred=True)
        elif version_key(version).is_prerelease:
            entry["name"] += " (preview)"
        menu.append(entry)
    (site / "versions.json").write_text(
        json.dumps(menu, indent=2) + "\n", encoding="utf-8"
    )

    # Prereleases never replace the stable redirect at the site root.
    if latest:
        body = (
            f'<meta http-equiv="refresh" content="0; url={latest}/">'
            f'<p><a href="{latest}/">VlConvert {latest} documentation</a></p>'
        )
    else:
        body = (
            f"<h1>VlConvert preview documentation</h1>"
            f"<p>No stable documentation has been published yet. "
            f'<a href="{previews[0]}/">Read the {previews[0]} preview</a>.</p>'
        )
    (site / "index.html").write_text(
        '<!doctype html><html lang="en"><meta charset="utf-8">'
        '<meta name="viewport" content="width=device-width, initial-scale=1">'
        "<title>VlConvert documentation</title>" + body + "</html>\n",
        encoding="utf-8",
    )
    (site / ".nojekyll").touch()


if __name__ == "__main__":
    # Accept the build and release metadata supplied by the release workflow.
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--site", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--url", default=SITE_URL)

    # Update one archived version and the shared navigation files.
    archive(parser.parse_args())

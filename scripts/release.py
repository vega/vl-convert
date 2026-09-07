#!/usr/bin/env python3
"""Create a release branch and draft pull request."""

from __future__ import annotations

import argparse
import subprocess
import sys
from collections.abc import Sequence

from check_workspace import (
    INTERNAL_DEPENDENCIES,
    PRODUCT_PACKAGES,
    ROOT,
    ROOT_MANIFEST,
    WorkspaceError,
    python_version,
    validate_workspace,
)

GITHUB_REPOSITORY = "vega/vl-convert"
EXPECTED_RELEASE_CHANGES = {"Cargo.toml", "Cargo.lock"}


def run(
    args: Sequence[str], *, check: bool = True, capture: bool = True
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=ROOT,
        check=check,
        capture_output=capture,
        text=True,
    )


def git(
    *args: str, check: bool = True, capture: bool = True
) -> subprocess.CompletedProcess[str]:
    return run(["git", *args], check=check, capture=capture)


def parse_new_version(value: str, current: str) -> str:
    import semver

    if value.startswith("v"):
        raise WorkspaceError("Pass the version without the v tag prefix")
    try:
        version = semver.Version.parse(value)
        current_version = semver.Version.parse(current)
    except ValueError as error:
        raise WorkspaceError(f"Invalid Rust SemVer: {error}") from error
    if version.build is not None:
        raise WorkspaceError("Release versions cannot contain build metadata")
    if version <= current_version:
        raise WorkspaceError(f"Version {version} must be newer than {current}")
    python_version(str(version))
    return str(version)


def ensure_clean_worktree() -> None:
    status = git("status", "--porcelain", "--untracked-files=all").stdout.strip()
    if status:
        raise WorkspaceError(f"The worktree is not clean:\n{status}")


def ensure_canonical_origin() -> None:
    origin = git("remote", "get-url", "origin").stdout.strip().removesuffix(".git")
    if not origin.endswith(
        ("github.com/vega/vl-convert", "github.com:vega/vl-convert")
    ):
        raise WorkspaceError(
            f"origin must identify {GITHUB_REPOSITORY}, found {origin}"
        )


def ref_exists(ref: str) -> bool:
    return git(
        "show-ref", "--verify", "--quiet", ref, check=False
    ).returncode == 0 or bool(git("ls-remote", "origin", ref).stdout.strip())


def ensure_release_name_available(version: str) -> None:
    branch = f"release/v{version}"
    tag = f"v{version}"
    for ref, label in (
        (f"refs/heads/{branch}", f"branch {branch}"),
        (f"refs/tags/{tag}", f"tag {tag}"),
    ):
        if ref_exists(ref):
            raise WorkspaceError(f"Release {label} already exists")


def update_root_manifest(version: str) -> None:
    import tomlkit

    document = tomlkit.parse(ROOT_MANIFEST.read_text(encoding="utf-8"))
    document["workspace"]["package"]["version"] = version
    dependencies = document["workspace"]["dependencies"]
    for name in INTERNAL_DEPENDENCIES:
        dependencies[name]["version"] = f"={version}"
    ROOT_MANIFEST.write_text(tomlkit.dumps(document), encoding="utf-8")


def pull_request_body(current: str, version: str) -> str:
    release_kind = "prerelease" if "-" in version else "stable release"
    packages = "\n".join(f"- `{name}`" for name in PRODUCT_PACKAGES)
    return f"""## Summary

Prepare `{version}` as a {release_kind}. This updates the shared workspace version
and exact internal package requirements from `{current}`.

## Packages

{packages}

## After merge

- Wait for all required CI checks on `main`.
- Create a GitHub Release with a new `v{version}` tag that targets `main`.
- Mark it as a prerelease when the version contains a prerelease component.
- Publish the GitHub Release to start package and artifact publishing.
"""


def prepare_release(value: str, *, dry_run: bool) -> None:
    current = validate_workspace()
    version = parse_new_version(value, current)
    branch = f"release/v{version}"
    tag = f"v{version}"

    ensure_clean_worktree()
    ensure_canonical_origin()
    run(["gh", "auth", "status", "--hostname", "github.com"])
    ensure_release_name_available(version)

    if dry_run:
        remote_main = git("ls-remote", "origin", "refs/heads/main").stdout.split()
        if not remote_main:
            raise WorkspaceError("origin/main does not exist")
        base = remote_main[0]
    else:
        git("fetch", "origin", "main", capture=False)
        base = git("rev-parse", "origin/main").stdout.strip()

    print(f"Current version: {current}")
    print(f"New version: {version}")
    print(f"Python version: {python_version(version)}")
    print(f"Base: origin/main at {base}")
    print(f"Branch: {branch}")
    print("Files: Cargo.toml, Cargo.lock")
    print(f"Commit: chore: prepare {tag}")
    print(f"Pull request: draft against {GITHUB_REPOSITORY}:main")
    if dry_run:
        print("Dry run complete. No local or remote state changed.")
        return

    git("switch", "--create", branch, "origin/main", capture=False)
    update_root_manifest(version)
    run(["cargo", "update", "--workspace"], capture=False)
    validate_workspace()

    changed = set(git("diff", "--name-only").stdout.splitlines())
    if changed != EXPECTED_RELEASE_CHANGES:
        raise WorkspaceError(
            f"Expected changes to {sorted(EXPECTED_RELEASE_CHANGES)}, "
            f"found {sorted(changed)}"
        )
    git("diff", "--check")
    git("add", *sorted(EXPECTED_RELEASE_CHANGES), capture=False)
    git("commit", "-m", f"chore: prepare {tag}", capture=False)
    git("push", "--set-upstream", "origin", branch, capture=False)
    run(
        [
            "gh",
            "pr",
            "create",
            "--repo",
            GITHUB_REPOSITORY,
            "--base",
            "main",
            "--head",
            branch,
            "--draft",
            "--title",
            f"chore: prepare {tag}",
            "--body",
            pull_request_body(current, version),
        ],
        capture=False,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    try:
        prepare_release(args.version, dry_run=args.dry_run)
    except WorkspaceError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as error:
        details = (error.stderr or error.stdout or str(error)).strip()
        print(f"error: {details}", file=sys.stderr)
        return error.returncode
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

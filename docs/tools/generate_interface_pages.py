#!/usr/bin/env python3
"""Generate standalone interface docs from shared topic sources."""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

import yaml


INTERFACES = {
    "python": {
        "title": "Python",
        "description": "Use `vl-convert-python` from Python applications and Altair workflows.",
    },
    "cli": {
        "title": "CLI",
        "description": "Run `vl-convert` from shells, scripts, and build pipelines.",
    },
    "rust": {
        "title": "Rust",
        "description": "Embed `vl-convert-rs` directly in Rust applications.",
    },
    "server": {
        "title": "Server",
        "description": "Run `vl-convert serve` as an HTTP rendering worker.",
    },
}

SECTION_ORDER = {
    "Getting Started": 100,
    "Guides": 200,
    "Advanced": 300,
    "Server": 400,
    "API Reference": 900,
}


@dataclass(frozen=True)
class Topic:
    source: Path
    title: str
    path: str
    section: str
    order: int
    interfaces: tuple[str, ...]
    suffix: str
    body_marker: str
    body: str


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def source_root() -> Path:
    return repo_root() / "docs" / "source"


def parse_topic(path: Path) -> Topic:
    text = path.read_text()
    if not text.startswith("---\n"):
        raise SystemExit(f"{path} must start with YAML front matter")

    try:
        _, raw_meta, _ = text.split("---\n", 2)
    except ValueError as err:
        raise SystemExit(f"{path} has malformed YAML front matter") from err

    meta = yaml.safe_load(raw_meta) or {}
    missing = {"title", "path", "section", "order", "interfaces"} - set(meta)
    if missing:
        raise SystemExit(f"{path} missing metadata field(s): {sorted(missing)}")

    interfaces = tuple(meta["interfaces"])
    invalid = set(interfaces) - set(INTERFACES)
    if invalid:
        raise SystemExit(f"{path} has unknown interface(s): {sorted(invalid)}")

    topic_path = str(meta["path"])
    if topic_path.startswith("/") or ".." in Path(topic_path).parts:
        raise SystemExit(f"{path} has invalid output path: {topic_path}")

    if path.suffix == ".rst":
        body_marker = ".. topic-body"
    elif path.suffix == ".md":
        body_marker = "<!-- topic-body -->"
    else:
        raise SystemExit(f"{path} must be a .md or .rst topic")

    if body_marker not in text:
        raise SystemExit(f"{path} missing body marker: {body_marker}")
    body = text.split(body_marker, 1)[1].lstrip()

    return Topic(
        source=path,
        title=str(meta["title"]),
        path=topic_path,
        section=str(meta["section"]),
        order=int(meta["order"]),
        interfaces=interfaces,
        suffix=path.suffix,
        body_marker=body_marker,
        body=body,
    )


def topic_files() -> list[Path]:
    topics_root = source_root() / "_topics"
    return sorted(
        path
        for path in topics_root.rglob("*")
        if path.suffix in {".md", ".rst"} and path.is_file()
    )


def validate_interface_blocks(topic: Topic) -> None:
    text = topic.source.read_text()
    unknown = set()
    for marker in (
        "```{interface}",
        "```{not-interface}",
        "::::{interface}",
        "::::{not-interface}",
    ):
        start = 0
        while True:
            idx = text.find(marker, start)
            if idx == -1:
                break
            line_end = text.find("\n", idx)
            if line_end == -1:
                line_end = len(text)
            args = text[idx + len(marker) : line_end].strip()
            args = args.removesuffix("}").strip()
            unknown.update(arg for arg in args.split() if arg not in INTERFACES)
            start = line_end

    if unknown:
        raise SystemExit(
            f"{topic.source} references unknown interface(s): {sorted(unknown)}"
        )


def parse_interface_targets(line: str, directive: str) -> set[str]:
    args = line.split(directive, 1)[1].strip()
    return set(args.split())


def filtered_body(topic: Topic, interface: str) -> str:
    lines = topic.body.splitlines()
    out: list[str] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("::::{interface}"):
            targets = parse_interface_targets(line, "::::{interface}")
            include = interface in targets
        elif line.startswith("::::{not-interface}"):
            targets = parse_interface_targets(line, "::::{not-interface}")
            include = interface not in targets
        else:
            out.append(line)
            i += 1
            continue

        invalid = targets - set(INTERFACES)
        if invalid:
            raise SystemExit(
                f"{topic.source} references unknown interface(s): {sorted(invalid)}"
            )

        i += 1
        block: list[str] = []
        while i < len(lines) and lines[i] != "::::":
            block.append(lines[i])
            i += 1
        if i >= len(lines):
            raise SystemExit(f"{topic.source} has an unclosed interface block")
        i += 1

        if include:
            out.extend(block)

    compact: list[str] = []
    blank_count = 0
    for line in out:
        if line.strip():
            blank_count = 0
            compact.append(line)
        else:
            blank_count += 1
            if blank_count <= 2:
                compact.append(line)

    return "\n".join(compact).rstrip() + "\n"


def wrapper_text(interface: str, topic: Topic, out_file: Path) -> str:
    body = filtered_body(topic, interface)
    if topic.suffix == ".rst":
        return (
            f".. Generated from {topic.source.relative_to(source_root()).as_posix()}. "
            "Do not edit.\n\n"
            f"{body}"
        )

    return (
        "---\n"
        f"current_interface: {interface}\n"
        f"generated_from: {topic.source.relative_to(source_root()).as_posix()}\n"
        "---\n\n"
        f"<!-- Generated from {topic.source.relative_to(source_root()).as_posix()}. "
        "Do not edit. -->\n\n"
        f"{body}"
    )


def write_if_changed(path: Path, text: str, expected: set[Path]) -> None:
    expected.add(path)
    if path.exists() and path.read_text() == text:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def write_topic_wrappers(
    topics: list[Topic], expected: set[Path]
) -> dict[str, list[Topic]]:
    by_interface: dict[str, list[Topic]] = defaultdict(list)
    seen: set[tuple[str, str]] = set()

    for topic in topics:
        validate_interface_blocks(topic)
        for interface in topic.interfaces:
            key = (interface, topic.path)
            if key in seen:
                raise SystemExit(
                    f"duplicate generated page for {interface}/{topic.path}"
                )
            seen.add(key)

            by_interface[interface].append(topic)
            out_file = source_root() / interface / f"{topic.path}{topic.suffix}"
            write_if_changed(
                out_file,
                wrapper_text(interface, topic, out_file),
                expected,
            )

    return by_interface


def write_index(interface: str, topics: list[Topic], expected: set[Path]) -> None:
    details = INTERFACES[interface]
    grouped: dict[str, list[Topic]] = defaultdict(list)
    for topic in topics:
        grouped[topic.section].append(topic)

    lines = [
        "---",
        f"current_interface: {interface}",
        "generated_index: true",
        "---",
        "",
        f"# {details['title']}",
        "",
        details["description"],
        "",
    ]

    for section in sorted(
        grouped, key=lambda name: (SECTION_ORDER.get(name, 500), name)
    ):
        section_topics = sorted(
            grouped[section], key=lambda topic: (topic.order, topic.title)
        )
        lines.extend(
            [
                "```{toctree}",
                ":maxdepth: 2",
                f":caption: {section}",
                "",
            ]
        )
        lines.extend(topic.path for topic in section_topics)
        lines.extend(["```", ""])

    write_if_changed(
        source_root() / interface / "index.md",
        "\n".join(lines),
        expected,
    )


def remove_stale_generated_files(expected: set[Path]) -> None:
    for interface in INTERFACES:
        root = source_root() / interface
        if not root.exists():
            continue
        for path in root.rglob("*"):
            if path.is_file() and path not in expected:
                path.unlink()
        for path in sorted(root.rglob("*"), reverse=True):
            if path.is_dir() and not any(path.iterdir()):
                path.rmdir()


def main() -> int:
    topics = [parse_topic(path) for path in topic_files()]
    expected: set[Path] = set()
    by_interface = write_topic_wrappers(topics, expected)
    for interface in INTERFACES:
        write_index(interface, by_interface.get(interface, []), expected)
    remove_stale_generated_files(expected)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

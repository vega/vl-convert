"""Render chart inputs with the local vl-convert CLI at build time.

The ``vl-chart`` directive takes input from its path argument or body, renders
it with the CLI that the docs build already compiles, and inserts the result as
an image. Rendered files are cached under ``_generated/charts`` by a hash of
the input, options, and CLI version, so unchanged charts cost nothing on later
builds.

Vega and Vega-Lite specifications should carry their data inline so the build
does not depend on remote data. For SVG output that uses Google Fonts, pass
``:google-fonts:`` together with ``:bundle:`` so the subset font is embedded.
An SVG shown through an ``<img>`` element cannot load external stylesheets, so
an embedded font is the only way the typeface reaches the reader. PNG output
needs ``:google-fonts:`` but does not need ``:bundle:``.
"""

from __future__ import annotations

import hashlib
import subprocess
import sys
from pathlib import Path

from docutils import nodes
from docutils.parsers.rst import directives
from sphinx.application import Sphinx
from sphinx.util.docutils import SphinxDirective

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))

from run_vl_convert import find_binary, runtime_environment  # noqa: E402

OUTPUT_DIR = "_generated/charts"

# Options that name files. They are resolved like other Sphinx paths and passed
# as global CLI options.
PATH_OPTIONS = {"themes": "--themes", "vega-plugin": "--vega-plugin"}

# Options passed to the conversion command unchanged.
COMMAND_OPTIONS = {
    "theme": "--theme",
    "vl-version": "--vl-version",
    "scale": "--scale",
    "format-locale": "--format-locale",
    "time-format-locale": "--time-format-locale",
    "plugin-import-domains": "--plugin-import-domains",
}

_cli_version: str | None = None


class VlChart(SphinxDirective):
    """Render a chart input and insert it as an image."""

    optional_arguments = 1
    final_argument_whitespace = True
    has_content = True
    option_spec = {
        "format": lambda arg: directives.choice(arg, ("svg", "png")),
        "input-kind": lambda arg: directives.choice(
            arg, ("vegalite", "vega", "svg")
        ),
        "theme": directives.unchanged_required,
        "themes": directives.unchanged_required,
        "vl-version": directives.unchanged_required,
        "scale": directives.unchanged_required,
        "format-locale": directives.unchanged_required,
        "time-format-locale": directives.unchanged_required,
        "vega-plugin": directives.unchanged_required,
        "plugin-import-domains": directives.unchanged_required,
        "google-fonts": directives.unchanged_required,
        "auto-google-fonts": directives.flag,
        "bundle": directives.flag,
        "alt": directives.unchanged,
        "class": directives.class_option,
        "width": directives.length_or_percentage_or_unitless,
    }

    def run(self) -> list[nodes.Node]:
        input_kind = self.options.get("input-kind", "vegalite")
        image_format = self.options.get("format", "svg")
        try:
            command = conversion_command(input_kind, image_format)
        except ValueError as exc:
            raise self.error(f"vl-chart: {exc}") from exc
        binary = self.binary()
        chart_input = self.read_input()

        global_args: list[str] = []
        file_contents: list[str] = []
        for option, flag in PATH_OPTIONS.items():
            if option in self.options:
                path = self.resolve_path(self.options[option])
                global_args += [flag, path]
                file_contents.append(Path(path).read_text())
        if "google-fonts" in self.options:
            global_args += ["--google-font", self.options["google-fonts"]]
        if "auto-google-fonts" in self.options:
            global_args.append("--auto-google-fonts")
        command_args: list[str] = []
        for option, flag in COMMAND_OPTIONS.items():
            if option in self.options:
                command_args += [flag, self.options[option]]
        if "bundle" in self.options:
            command_args.append("--bundle")

        # Files named by options contribute their contents, not just their paths,
        # so editing a theme or plugin file invalidates the cached chart.
        key = [
            cli_version(binary),
            input_kind,
            image_format,
            *global_args,
            *command_args,
            *file_contents,
            chart_input,
        ]
        digest = hashlib.sha256("\0".join(key).encode()).hexdigest()[:16]
        relative = f"{OUTPUT_DIR}/{digest}.{image_format}"
        output = Path(self.env.srcdir) / relative
        if not output.exists():
            self.render(
                binary,
                chart_input,
                output,
                command,
                global_args,
                command_args,
            )

        image = nodes.image(
            "",
            uri=f"/{relative}",
            alt=self.options.get("alt", ""),
            classes=["rendered-chart", *self.options.get("class", [])],
        )
        if "width" in self.options:
            image["width"] = self.options["width"]
        elif image_format == "png":
            # A PNG rendered at scale N is shown at 1/N so it stays sharp on
            # high-density displays without growing larger than the SVG charts.
            scale = float(self.options.get("scale", 1))
            image["width"] = f"{png_width(output) / scale:g}px"
        if "width" in image:
            # Sphinx links scaled images to the full-size file, and the theme
            # renders that link as a panel around the chart. Opt out of it.
            image["classes"].append("no-scaled-link")
        return [image]

    def binary(self) -> Path:
        try:
            return find_binary()
        except SystemExit as exc:
            raise self.error(f"vl-chart: {exc}") from exc

    def read_input(self) -> str:
        if self.arguments:
            return Path(self.resolve_path(self.arguments[0])).read_text()
        if self.content:
            return "\n".join(self.content)
        raise self.error("vl-chart needs an input path or inline input")

    def resolve_path(self, value: str) -> str:
        _, absolute = self.env.relfn2path(value)
        self.env.note_dependency(absolute)
        if not Path(absolute).is_file():
            raise self.error(f"vl-chart: file not found: {value}")
        return absolute

    def render(
        self,
        binary: Path,
        chart_input: str,
        output: Path,
        command: str,
        global_args: list[str],
        command_args: list[str],
    ) -> None:
        output.parent.mkdir(parents=True, exist_ok=True)
        proc = subprocess.run(
            [
                str(binary),
                "--vlc-config",
                "disabled",
                *global_args,
                command,
                "--input",
                "-",
                "--output",
                str(output),
                *command_args,
            ],
            input=chart_input.encode(),
            capture_output=True,
            env=runtime_environment(),
        )
        if proc.returncode != 0:
            output.unlink(missing_ok=True)
            stderr = proc.stderr.decode(errors="replace").strip()
            raise self.error(f"vl-chart: vl-convert {command} failed:\n{stderr}")


def conversion_command(input_kind: str, image_format: str) -> str:
    """Return the CLI command for a supported chart input and output pair."""
    commands = {
        ("vegalite", "svg"): "vl2svg",
        ("vegalite", "png"): "vl2png",
        ("vega", "svg"): "vg2svg",
        ("vega", "png"): "vg2png",
        ("svg", "png"): "svg2png",
    }
    try:
        return commands[(input_kind, image_format)]
    except KeyError as exc:
        raise ValueError(
            f"cannot render {input_kind!r} input as {image_format!r} output"
        ) from exc


def png_width(path: Path) -> int:
    """Read the pixel width from a PNG header."""
    header = path.read_bytes()[:24]
    if header[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"{path} is not a PNG file")
    return int.from_bytes(header[16:20], "big")


def cli_version(binary: Path) -> str:
    global _cli_version
    if _cli_version is None:
        proc = subprocess.run(
            [str(binary), "--version"],
            capture_output=True,
            text=True,
            check=True,
            env=runtime_environment(),
        )
        _cli_version = proc.stdout.strip()
    return _cli_version


def setup(app: Sphinx) -> dict[str, object]:
    app.add_directive("vl-chart", VlChart)
    return {"version": "2", "parallel_read_safe": True, "parallel_write_safe": True}

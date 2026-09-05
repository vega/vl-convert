---
title: CLI Piping
path: advanced/cli-piping
section: Advanced
order: 430
interfaces: [cli]
---

<!-- topic-body -->

# Standard Input and Output

Every conversion command accepts a file path or `-` for standard input and
standard output. If `--input` or `--output` is omitted, that stream is used by
default.

Read a specification from standard input and write SVG to a file:

```bash
vl-convert vl2svg --input - --output chart.svg < chart.vl.json
```

Send SVG to another process:

```bash
vl-convert vl2svg --input chart.vl.json --output - | gzip > chart.svg.gz
```

Binary formats such as PNG, JPEG, PDF, and MessagePack can also be written to
standard output. Redirect binary output to a file or pipe it to a program that
accepts bytes. Do not let it print directly in an interactive terminal.

Logs and errors go to standard error, so they do not corrupt successful output.
In shell scripts, enable pipeline failure handling when an earlier command must
not fail silently:

```bash
set -o pipefail
vl-convert vl2png --input chart.vl.json --output - | gzip > chart.png.gz
```

Run `vl-convert config-path` to print the platform-standard JSONC configuration
path. See {doc}`configuration` for precedence rules and
`--vlc-config disabled`.

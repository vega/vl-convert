---
title: Standard Input and Output
path: advanced/cli-piping
section: Advanced
order: 430
interfaces: [cli]
---

<!-- topic-body -->

# Standard Input and Output

Every conversion command reads `--input` and writes `--output`. Pass `-` for
either to use standard input or standard output. Omitting an option selects the
same stream. The examples use `chart.vl.json` from
{doc}`../getting-started/quick-start`.

Read a specification from standard input and write SVG to a file:

```bash
vl-convert vl2svg --input - --output chart.svg < chart.vl.json
```

Send SVG to another process:

```bash
vl-convert vl2svg --input chart.vl.json --output - | gzip > chart.svg.gz
```

Binary formats such as PNG, JPEG, PDF, and MessagePack can be piped the same
way. When `--output` is omitted and standard output is an interactive terminal,
`vl-convert` refuses to write binary data and exits with an error. Pass
`--output -` to override that guard.

Logs and errors go to standard error, so they never mix with the output. Enable
`pipefail` in shell scripts so a failed conversion fails the pipeline:

```bash
set -o pipefail
vl-convert vl2png --input chart.vl.json --output - | gzip > chart.png.gz
```

Pass `--vlc-config disabled` in scripts that must not depend on the machine's
config file. See {doc}`configuration`.

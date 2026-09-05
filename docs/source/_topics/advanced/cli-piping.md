---
title: CLI Piping
path: advanced/cli-piping
section: Advanced
order: 430
interfaces: [cli]
---

<!-- topic-body -->

# CLI Piping and Config Files

CLI conversion commands accept file paths and `-` for stdin/stdout. Binary
outputs such as PNG, JPEG, PDF, and MessagePack can be written to stdout; in a
terminal, redirect them to a file or another process.

```bash
vl-convert vl2svg --input - --output chart.svg < chart.vl.json
vl-convert config-path
```

Use `--vlc-config <path>` to load a JSONC config file and
`--vlc-config disabled` to ignore the platform default config path.

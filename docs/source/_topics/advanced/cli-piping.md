---
title: CLI Piping
path: advanced/cli-piping
section: Advanced
order: 430
interfaces: [cli]
---

<!-- topic-body -->

# CLI Piping and Config Files

CLI commands accept file paths and `-` for stdin/stdout where binary safety
allows it.

```bash
vl-convert vl2svg --input - --output chart.svg < chart.vl.json
vl-convert config-path
```

Use `--vlc-config <path>` to load a JSONC config file and `--no-vlc-config`
to ignore the platform default config path.

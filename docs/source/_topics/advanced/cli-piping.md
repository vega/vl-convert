---
title: Standard Input and Output
path: advanced/cli-piping
section: Advanced
order: 430
interfaces: [cli]
---

<!-- topic-body -->

# Standard Input and Output

Conversion commands read from standard input and write to standard output by default. Use `--input` and `--output` to specify files, or pass `-` to explicitly select standard input or standard output.

For binary formats such as PNG, the CLI refuses to write directly to a terminal unless you pass `--output -`. This safeguard does not affect piped or redirected output.

The examples use `chart.vl.json` from {doc}`../getting-started/quick-start`.

Read a specification from standard input and write SVG to a file:

```console
$ vl-convert vl2svg < chart.vl.json > chart.svg
```

Compress the SVG output:

```console
$ vl-convert vl2svg < chart.vl.json | gzip > chart.svg.gz
```

Logs and errors go to standard error, keeping them separate from conversion output. In Bash or Zsh, enable `pipefail` so a failed conversion gives the pipeline a nonzero exit status:

```console
$ set -o pipefail
$ vl-convert vl2png < chart.vl.json | gzip > chart.png.gz
```

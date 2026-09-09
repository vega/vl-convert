---
title: CLI Reference
path: api-reference
section: API Reference
order: 900
interfaces: [cli]
---

<!-- topic-body -->

# CLI Reference

This reference is generated from the version 2 `vl-convert` executable used to build the documentation. Global options go before the command, and command-specific options go after it:

```text
vl-convert [GLOBAL OPTIONS] <COMMAND> [COMMAND OPTIONS]
```

Every conversion command accepts `--input -` and `--output -` for standard input and output, and uses those streams when the option is omitted. See {doc}`advanced/cli-piping`.

## Top-Level Help

```{program-output} python ../tools/run_vl_convert.py --help
```

## Vega-Lite Commands

### `vl2vg`

```{program-output} python ../tools/run_vl_convert.py vl2vg --help
```

### `vl2svg`

```{program-output} python ../tools/run_vl_convert.py vl2svg --help
```

### `vl2png`

```{program-output} python ../tools/run_vl_convert.py vl2png --help
```

### `vl2jpeg`

```{program-output} python ../tools/run_vl_convert.py vl2jpeg --help
```

### `vl2pdf`

```{program-output} python ../tools/run_vl_convert.py vl2pdf --help
```

### `vl2url`

```{program-output} python ../tools/run_vl_convert.py vl2url --help
```

### `vl2html`

```{program-output} python ../tools/run_vl_convert.py vl2html --help
```

### `vl2fonts`

```{program-output} python ../tools/run_vl_convert.py vl2fonts --help
```

### `vl2sg`

```{program-output} python ../tools/run_vl_convert.py vl2sg --help
```

## Vega Commands

### `vg2svg`

```{program-output} python ../tools/run_vl_convert.py vg2svg --help
```

### `vg2png`

```{program-output} python ../tools/run_vl_convert.py vg2png --help
```

### `vg2jpeg`

```{program-output} python ../tools/run_vl_convert.py vg2jpeg --help
```

### `vg2pdf`

```{program-output} python ../tools/run_vl_convert.py vg2pdf --help
```

### `vg2url`

```{program-output} python ../tools/run_vl_convert.py vg2url --help
```

### `vg2html`

```{program-output} python ../tools/run_vl_convert.py vg2html --help
```

### `vg2fonts`

```{program-output} python ../tools/run_vl_convert.py vg2fonts --help
```

### `vg2sg`

```{program-output} python ../tools/run_vl_convert.py vg2sg --help
```

## SVG Commands

### `svg2png`

```{program-output} python ../tools/run_vl_convert.py svg2png --help
```

### `svg2jpeg`

```{program-output} python ../tools/run_vl_convert.py svg2jpeg --help
```

### `svg2pdf`

```{program-output} python ../tools/run_vl_convert.py svg2pdf --help
```

## JavaScript Bundling

### `bundle-js`

```{program-output} python ../tools/run_vl_convert.py bundle-js --help
```

## Themes and Config

### `ls-themes`

```{program-output} python ../tools/run_vl_convert.py ls-themes --help
```

### `cat-theme`

```{program-output} python ../tools/run_vl_convert.py cat-theme --help
```

### `config-path`

```{program-output} python ../tools/run_vl_convert.py config-path --help
```

## Server

### `serve`

```{program-output} python ../tools/run_vl_convert.py serve --help
```

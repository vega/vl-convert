## Overview
This crate is a thin wrapper around the [`vl-convert-rs`](https://docs.rs/vl-convert-rs/) crate that provides a command line interface for converting Vega-Lite visualization specifications to Vega visualization specifications.

## Installation
Install `vl-convert` using cargo with:
```
$ cargo install vl-convert
```

## CLI Usage
Display the documentation for the top-level `vl-convert` command
```plain
$ vl-convert --help

vl-convert: A utility for converting Vega-Lite specifications

Usage: vl-convert <COMMAND>

Commands:
  vl2vg   Convert a Vega-Lite specification to a Vega specification
  vl2svg  Convert a Vega-Lite specification to an SVG image
  vl2png  Convert a Vega-Lite specification to an PNG image
  vg2svg  Convert a Vega specification to an SVG image
  vg2png  Convert a Vega specification to an PNG image
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```

Various conversion formats are handled by the subcommands listed above. Documentation for each subcommands is displayed using the `--help` flag.

### vl2vg
Convert a Vega-Lite JSON specification to a Vega JSON specification
```
$ vl-convert vl2vg --help 

Convert a Vega-Lite specification to a Vega specification

Usage: vl-convert vl2vg [OPTIONS] --input <INPUT> --output <OUTPUT>

Options:
  -i, --input <INPUT>            Path to input Vega-Lite file
  -o, --output <OUTPUT>          Path to output Vega file to be created
  -v, --vl-version <VL_VERSION>  Vega-Lite Version. One of 4.17, 5.0, 5.1, 5.2, 5.3, 5.4, 5.5 [default: 5.5]
  -p, --pretty                   Pretty-print JSON in output file
  -h, --help                     Print help information
```

For example, convert a Vega-Lite specification file named `in.vl.json` into a Vega specification file named `out.vg.json`. Perform the conversion using version 5.5 of the Vega-Lite JavaScript library and pretty-print the resulting JSON.

```plain
$ vl-convert vl2vg -i ./in.vl.json -o ./out.vg.json --vl-version 5.5 --pretty
```

### vl2svg
Convert a Vega-Lite specification to an SVG image

```
$ vl-convert vl2svg --help 

Convert a Vega-Lite specification to an SVG image

Usage: vl-convert vl2svg [OPTIONS] --input <INPUT> --output <OUTPUT>

Options:
  -i, --input <INPUT>            Path to input Vega-Lite file
  -o, --output <OUTPUT>          Path to output SVG file to be created
  -v, --vl-version <VL_VERSION>  Vega-Lite Version. One of 4.17, 5.0, 5.1, 5.2, 5.3, 5.4, 5.5 [default: 5.5]
  -h, --help                     Print help information
```

For example, convert a Vega-Lite specification file named `in.vl.json` into an SVG file named `out.svg`. Perform the conversion using version 5.5 of the Vega-Lite JavaScript library.

```plain
$ vl-convert vl2svg -i ./in.vl.json -o ./out.svg --vl-version 5.5
```

### vl2png
Convert a Vega-Lite specification to an PNG image

```
$ vl-convert vl2png --help

Convert a Vega-Lite specification to an PNG image

Usage: vl-convert vl2png [OPTIONS] --input <INPUT> --output <OUTPUT>

Options:
  -i, --input <INPUT>            Path to input Vega-Lite file
  -o, --output <OUTPUT>          Path to output PNG file to be created
  -v, --vl-version <VL_VERSION>  Vega-Lite Version. One of 4.17, 5.0, 5.1, 5.2, 5.3, 5.4, 5.5 [default: 5.5]
  -s, --scale <SCALE>            Image scale factor [default: 1.0]
  -h, --help                     Print help information
```

For example, convert a Vega-Lite specification file named `in.vl.json` into a PNG file named `out.png` with a scale factor of 2. Perform the conversion using version 5.5 of the Vega-Lite JavaScript library.

```plain
$ vl-convert vl2png -i ./in.vl.json -o ./out.png --vl-version 5.5 --scale 2
```

### vg2svg
Convert a Vega specification to an SVG image

```
$ vl-convert vg2svg --help

Convert a Vega specification to an SVG image

Usage: vl-convert vg2svg --input <INPUT> --output <OUTPUT>

Options:
  -i, --input <INPUT>    Path to input Vega file
  -o, --output <OUTPUT>  Path to output SVG file to be created
  -h, --help             Print help information
```

For example, convert a Vega specification file named `in.vg.json` into an SVG file named `out.svg`.

```plain
$ vl-convert vg2svg -i ./in.vg.json -o ./out.svg
```

### vg2png
```
$ vl-convert vg2png --help

Convert a Vega specification to an PNG image

Usage: vl-convert vg2png [OPTIONS] --input <INPUT> --output <OUTPUT>

Options:
  -i, --input <INPUT>    Path to input Vega file
  -o, --output <OUTPUT>  Path to output PNG file to be created
  -s, --scale <SCALE>    Image scale factor [default: 1.0]
  -h, --help             Print help information
```

For example, convert a Vega specification file named `in.vg.json` into a PNG file named `out.png` with a scale factor of 2.

```plain
$ vl-convert vg2png -i ./in.vg.json -o ./out.png --scale 2
```

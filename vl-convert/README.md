## Overview
This crate is a thin wrapper around the [`vl-convert-rs`](https://docs.rs/vl-convert-rs/) crate that provides a command line interface for converting Vega-Lite visualization specifications to Vega visualization specifications.

## Installation
Install `vl-convert` using cargo with:
```
$ cargo install vl-convert
```

## CLI Usage
Display the documentation for the `vl-convert` command
```plain
$ ./vl-convert --help
```
```plain
vl-convert: A utility for converting Vega-Lite specifications into Vega specification

Usage: vl-convert [OPTIONS] --input-vegalite-file <INPUT_VEGALITE_FILE> --output-vega-file <OUTPUT_VEGA_FILE>

Options:
  -i, --input-vegalite-file <INPUT_VEGALITE_FILE>
          Path to input Vega-Lite file
  -o, --output-vega-file <OUTPUT_VEGA_FILE>
          Path to output Vega file to be created
  -v, --vl-version <VL_VERSION>
          Vega-Lite Version. One of 4.17, 5.0, 5.1, 5.2, 5.3, 5.4, 5.5 [default: 5.5]
  -p, --pretty
          Pretty-print JSON in output file
  -h, --help
          Print help information
  -V, --version
          Print version information
```

## CLI Example
Convert a Vega-Lite specification file named `in.vl.json` into a Vega specification file named `out.vg.json`. Perform the conversion using version 5.5 of the Vega-Lite JavaScript library and pretty-print the resulting JSON.

```plain
$ ./vl-convert -i ./in.vl.json -o ./out.vg.json --vl-version 5.5 --pretty
```

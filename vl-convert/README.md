## Overview
This crate is a thin wrapper around the [`vl-convert-rs`](https://docs.rs/vl-convert-rs/) crate that provides a command line interface for converting Vega-Lite visualization specifications into various formats.

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
  vl2vg      Convert a Vega-Lite specification to a Vega specification
  vl2svg     Convert a Vega-Lite specification to an SVG image
  vl2png     Convert a Vega-Lite specification to an PNG image
  vl2jpeg    Convert a Vega-Lite specification to an JPEG image
  vl2pdf     Convert a Vega-Lite specification to a PDF image
  vl2url     Convert a Vega-Lite specification to a URL that opens the chart in the Vega editor
  vg2svg     Convert a Vega specification to an SVG image
  vg2png     Convert a Vega specification to an PNG image
  vg2jpeg    Convert a Vega specification to an JPEG image
  vg2pdf     Convert a Vega specification to an PDF image
  vg2url     Convert a Vega specification to a URL that opens the chart in the Vega editor
  ls-themes  List available themes
  cat-theme  Print the config JSON for a theme
  help       Print this message or the help of the given subcommand(s)

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
  -v, --vl-version <VL_VERSION>  Vega-Lite Version. One of 4.17, 5.0, 5.1, 5.2, 5.3, 5.4, 5.5, 5.6 [default: 5.6]
  -t, --theme <THEME>            Named theme provided by the vegaThemes package (e.g. "dark")
  -c, --config <CONFIG>          Path to Vega-Lite config file. Defaults to ~/.config/vl-convert/config.json
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
  -v, --vl-version <VL_VERSION>  Vega-Lite Version. One of 4.17, 5.0, 5.1, 5.2, 5.3, 5.4, 5.5, 5.6 [default: 5.6]
  -t, --theme <THEME>            Named theme provided by the vegaThemes package (e.g. "dark")
  -c, --config <CONFIG>          Path to Vega-Lite config file. Defaults to ~/.config/vl-convert/config.json
      --font-dir <FONT_DIR>      Additional directory to search for fonts
  -h, --help                     Print help information
```

For example, convert a Vega-Lite specification file named `in.vl.json` into an SVG file named `out.svg`. Perform the conversion using version 5.5 of the Vega-Lite JavaScript library, and apply the `dark` theme (available themes available with the `ls-themes` subcommand below).

```plain
$ vl-convert vl2svg -i ./in.vl.json -o ./out.svg --vl-version 5.5 --theme dark
```

### vl2png
Convert a Vega-Lite specification to a PNG image

```
$ vl-convert vl2png --help

Convert a Vega-Lite specification to an PNG image

Usage: vl-convert vl2png [OPTIONS] --input <INPUT> --output <OUTPUT>

Options:
  -i, --input <INPUT>            Path to input Vega-Lite file
  -o, --output <OUTPUT>          Path to output PNG file to be created
  -v, --vl-version <VL_VERSION>  Vega-Lite Version. One of 4.17, 5.0, 5.1, 5.2, 5.3, 5.4, 5.5, 5.6 [default: 5.6]
  -t, --theme <THEME>            Named theme provided by the vegaThemes package (e.g. "dark")
  -c, --config <CONFIG>          Path to Vega-Lite config file. Defaults to ~/.config/vl-convert/config.json
  -s, --scale <SCALE>            Image scale factor [default: 1.0]
      --font-dir <FONT_DIR>      Additional directory to search for fonts
  -h, --help                     Print help information
```

For example, convert a Vega-Lite specification file named `in.vl.json` into a PNG file named `out.png` with a scale factor of 2. Perform the conversion using version 5.5 of the Vega-Lite JavaScript library, and apply the [config](https://vega.github.io/vega/docs/config/) file located at `~/my-config.json`.

```plain
$ vl-convert vl2png -i ./in.vl.json -o ./out.png --vl-version 5.5 --scale 2 --config ~/my-config.json
```

### vl2pdf
Convert a Vega-Lite specification to a PDF image
```
Convert a Vega-Lite specification to a PDF image

Usage: vl-convert vl2pdf [OPTIONS] --input <INPUT> --output <OUTPUT>

Options:
  -i, --input <INPUT>            Path to input Vega-Lite file
  -o, --output <OUTPUT>          Path to output PDF file to be created
  -v, --vl-version <VL_VERSION>  Vega-Lite Version. One of 4.17, 5.6, 5.7, 5.8, 5.9, 5.10, 5.11, 5.12, 5.13, 5.14 [default: 5.14]
  -t, --theme <THEME>            Named theme provided by the vegaThemes package (e.g. "dark")
  -c, --config <CONFIG>          Path to Vega-Lite config file. Defaults to ~/.config/vl-convert/config.json
  -s, --scale <SCALE>            Image scale factor [default: 1.0]
      --font-dir <FONT_DIR>      Additional directory to search for fonts
  -h, --help                     Print help
```

For example, convert a Vega-Lite specification file named `in.vl.json` into a PNG file named `out.pdf` with a scale factor of 2.

```
$ vl-convert vl2pdf -i ./in.vl.json -o ./out.pdf --scale 2
```

### vl2url
Convert a Vega-Lite specification to a URL that opens the chart in the Vega editor

```
Convert a Vega-Lite specification to a URL that opens the chart in the Vega editor

Usage: vl-convert vl2url --input <INPUT>

Options:
  -i, --input <INPUT>  Path to input Vega-Lite file
  -h, --help           Print help
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

### vg2pdf
```
Convert a Vega specification to an PDF image

Usage: vl-convert vg2pdf [OPTIONS] --input <INPUT> --output <OUTPUT>

Options:
  -i, --input <INPUT>        Path to input Vega file
  -o, --output <OUTPUT>      Path to output PDF file to be created
  -s, --scale <SCALE>        Image scale factor [default: 1.0]
      --font-dir <FONT_DIR>  Additional directory to search for fonts
  -h, --help                 Print help
```

For example, convert a Vega specification file named `in.vg.json` into a PDF file named `out.pdf` with a scale factor of 2.

```plain
$ vl-convert vg2pdf -i ./in.vg.json -o ./out.pdf --scale 2
```

### vg2url
Convert a Vega-Lite specification to a URL that opens the chart in the Vega editor

```
Convert a Vega specification to a URL that opens the chart in the Vega editor

Usage: vl-convert vg2url --input <INPUT>

Options:
  -i, --input <INPUT>  Path to input Vega file
  -h, --help           Print help
```

### ls-themes
```
$ vl-convert ls-themes --help

List available themes

Usage: vl-convert ls-themes

Options:
  -h, --help  Print help information
```

Here is an example of listing the names of all available built-in themes.
```
$ vl-convert ls-themes

dark
excel
fivethirtyeight
ggplot2
googlecharts
latimes
powerbi
quartz
urbaninstitute
vox
```

### cat-theme
```
$ vl-convert cat-theme --help

Print the config JSON for a theme

Usage: vl-convert cat-theme <THEME>

Arguments:
  <THEME>  Name of a theme

Options:
  -h, --help  Print help information
```

For example, print the config JSON associated with the built-in `dark` theme

```
$ vl-convert cat-theme dark

{
  "background": "#333",
  "title": {
    "color": "#fff",
    "subtitleColor": "#fff"
  },
  "style": {
    "guide-label": {
      "fill": "#fff"
    },
    "guide-title": {
      "fill": "#fff"
    }
  },
  "axis": {
    "domainColor": "#fff",
    "gridColor": "#888",
    "tickColor": "#fff"
  }
}
```

## User-level config file
If a file exists at `~/.config/vl-convert/config.json`, `vl-convert` will use this path as the default value of the `--config` flag across all subcommands.

# vl-convert

`vl-convert` is the command-line interface and HTTP server for VlConvert. For
Vega and Vega-Lite charts, it produces SVG, PNG, JPEG, PDF, HTML, scenegraphs,
font metadata, and Vega Editor URLs. It also compiles Vega-Lite to Vega and
converts existing SVG to PNG, JPEG, or PDF.

The executable embeds the official Vega and Vega-Lite JavaScript libraries. It
does not require a browser or Node.js.

## Installation

Install the executable from crates.io:

```bash
cargo install vl-convert --locked
```

## Convert a Chart

Convert a Vega-Lite specification file to SVG:

```bash
vl-convert vl2svg --input chart.vl.json --output chart.svg
```

Convert a Vega specification to PNG and apply a Vega configuration object:

```bash
vl-convert vg2png \
  --input chart.vg.json \
  --output chart.png \
  --config chart-config.json \
  --scale 2
```

Input and output default to standard input and standard output. Binary output
requires a file or an explicit `--output -` when standard output is a terminal.

## Command Groups

| Commands | Purpose |
| --- | --- |
| `vl2*` | Compile or convert Vega-Lite input |
| `vg2*` | Convert Vega input |
| `svg2*` | Convert existing SVG input |
| `bundle-js` | Build the JavaScript bundle used by Vega Embed integrations |
| `ls-themes`, `cat-theme` | Inspect bundled and configured themes |
| `config-path` | Print the platform-default converter config path |
| `serve` | Run the HTTP conversion server |

Run `vl-convert --help` for global options and `vl-convert <COMMAND> --help`
for a command's options. Global options go before the command. Command-specific
options go after it.

## Chart and Converter Configuration

`--config` applies a Vega or Vega-Lite configuration object to one chart. It is
available on the Vega-Lite and Vega chart commands except `vl2url` and
`vg2url`, which only encode the input specification in a URL.

`--vlc-config` loads process-wide converter settings from a JSONC file. These
settings control worker behavior, data and image access, fonts, plugins,
themes, locales, and JavaScript limits. Relative paths resolve from the current
working directory:

```bash
vl-convert --vlc-config production.vlc.jsonc \
  vg2svg --input chart.vg.json --output chart.svg
```

When `--vlc-config` is omitted, VlConvert loads the platform-default file if it
exists. Run `vl-convert config-path` to print that location. Pass
`--vlc-config disabled` to skip config-file loading.

Most global options also have `VLC_*` environment variables. Command-line
values take priority over environment variables, which take priority over the
converter config file.

## Network and File Access

The bundled JavaScript libraries need no network access. Specifications can
still request remote data and images. Optional Google Fonts and plugins can
make additional requests.

Data and images share the `allowed_base_urls` policy. HTTP and HTTPS access is
allowed by default, and local files are blocked. Google Fonts and plugins use
separate controls. Restrict these settings before converting specifications
from untrusted sources.

## HTTP Server

Start a local server:

```bash
vl-convert serve --host 127.0.0.1 --port 3000
```

The server exposes interactive API documentation at
`http://127.0.0.1:3000/docs`. Its defaults are intended for local use. Configure
authentication, request limits, resource permissions, JavaScript limits, and a
TLS-terminating proxy before exposing it publicly.

See the repository's
[documentation source](https://github.com/vega/vl-convert/tree/main/docs) for
complete CLI and server guides.

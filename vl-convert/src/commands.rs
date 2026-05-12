use clap::{Args, Subcommand, ValueEnum};

use crate::cli_types::DEFAULT_VL_VERSION;

fn parse_non_negative_finite_f32(value: &str) -> Result<f32, String> {
    let parsed = value
        .parse::<f32>()
        .map_err(|err| format!("expected a number: {err}"))?;
    if !parsed.is_finite() {
        return Err("value must be finite".to_string());
    }
    if parsed < 0.0 {
        return Err("value must be non-negative".to_string());
    }
    Ok(parsed)
}

#[derive(Debug, Clone, Default, Args)]
pub(crate) struct RenderOverrides {
    /// Override the chart width
    #[arg(long, value_parser = parse_non_negative_finite_f32)]
    pub width: Option<f32>,

    /// Override the chart height
    #[arg(long, value_parser = parse_non_negative_finite_f32)]
    pub height: Option<f32>,

    /// Override the chart background
    #[arg(long)]
    pub background: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ScenegraphFormat {
    Json,
    Msgpack,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Convert a Vega-Lite specification to a Vega specification
    Vl2vg {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output Vega file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(short, long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// Pretty-print JSON in output file
        #[arg(short, long)]
        pretty: bool,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Convert a Vega-Lite specification to an SVG image
    Vl2svg {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output SVG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Bundle fonts and images into a self-contained SVG
        #[arg(long)]
        bundle: bool,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Convert a Vega-Lite specification to an PNG image
    Vl2png {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PNG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// Pixels per inch
        #[arg(short, long, default_value = "72.0")]
        ppi: f32,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Convert a Vega-Lite specification to an JPEG image
    Vl2jpeg {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JPEG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// JPEG Quality between 0 (worst) and 100 (best)
        #[arg(short, long, default_value = "90")]
        quality: u8,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Convert a Vega-Lite specification to a PDF image
    Vl2pdf {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PDF file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Convert a Vega-Lite specification to a URL that opens the chart in the Vega editor
    Vl2url {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Open chart in fullscreen mode
        #[arg(long, default_value = "false")]
        fullscreen: bool,
    },

    /// Convert a Vega-Lite specification to an HTML file
    Vl2html {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output HTML file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// Whether to bundle JavaScript dependencies in the HTML file
        /// instead of loading them from a CDN
        #[arg(short, long)]
        bundle: bool,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Vega renderer. One of 'svg' (default), 'canvas', or 'hybrid'
        #[arg(long)]
        renderer: Option<String>,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Return font metadata for a rendered Vega-Lite specification
    Vl2fonts {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JSON file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// Include @font-face CSS blocks in the output
        #[arg(long = "include-font-face")]
        include_font_face: bool,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Pretty-print JSON output
        #[arg(short, long)]
        pretty: bool,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Convert a Vega-Lite specification to a Vega scenegraph
    Vl2sg {
        /// Path to input Vega-Lite file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output scenegraph file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,

        /// Named theme provided by the vegaThemes package (e.g. "dark")
        #[arg(long)]
        theme: Option<String>,

        /// Path to Vega-Lite config file
        #[arg(short, long)]
        config: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Output format: json or msgpack
        #[arg(long, value_enum, default_value = "json")]
        format: ScenegraphFormat,

        /// Pretty-print JSON output
        #[arg(short, long)]
        pretty: bool,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Convert a Vega specification to an SVG image
    Vg2svg {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output SVG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Bundle fonts and images into a self-contained SVG
        #[arg(long)]
        bundle: bool,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Convert a Vega specification to an PNG image
    Vg2png {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PNG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// Pixels per inch
        #[arg(short, long, default_value = "72.0")]
        ppi: f32,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Convert a Vega specification to an JPEG image
    Vg2jpeg {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JPEG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// JPEG Quality between 0 (worst) and 100 (best)
        #[arg(short, long, default_value = "90")]
        quality: u8,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Convert a Vega specification to an PDF image
    Vg2pdf {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PDF file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Convert a Vega specification to a URL that opens the chart in the Vega editor
    Vg2url {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Open chart in fullscreen mode
        #[arg(long, default_value = "false")]
        fullscreen: bool,
    },

    /// Convert a Vega specification to an HTML file
    Vg2html {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output HTML file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Whether to bundle JavaScript dependencies in the HTML file
        /// instead of loading them from a CDN
        #[arg(short, long)]
        bundle: bool,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Vega renderer. One of 'svg' (default), 'canvas', or 'hybrid'
        #[arg(long)]
        renderer: Option<String>,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Return font metadata for a rendered Vega specification
    Vg2fonts {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JSON file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Include @font-face CSS blocks in the output
        #[arg(long = "include-font-face")]
        include_font_face: bool,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Pretty-print JSON output
        #[arg(short, long)]
        pretty: bool,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Convert a Vega specification to a Vega scenegraph
    Vg2sg {
        /// Path to input Vega file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output scenegraph file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// d3-format locale name or file with .json extension
        #[arg(long)]
        format_locale: Option<String>,

        /// d3-time-format locale name or file with .json extension
        #[arg(long)]
        time_format_locale: Option<String>,

        /// Output format: json or msgpack
        #[arg(long, value_enum, default_value = "json")]
        format: ScenegraphFormat,

        /// Pretty-print JSON output
        #[arg(short, long)]
        pretty: bool,

        #[command(flatten)]
        render: RenderOverrides,
    },

    /// Produce the JavaScript bundle used by Vega Embed integrations
    BundleJs {
        /// Path to a JavaScript snippet to bundle. Reads from stdin if set to "-"
        #[arg(long)]
        snippet: Option<String>,

        /// Path to output JavaScript file. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Vega-Lite Version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4
        #[arg(short, long, default_value = DEFAULT_VL_VERSION)]
        vl_version: String,
    },

    /// Convert an SVG image to a PNG image
    Svg2png {
        /// Path to input SVG file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PNG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// Pixels per inch
        #[arg(short, long, default_value = "72.0")]
        ppi: f32,
    },

    /// Convert an SVG image to a JPEG image
    Svg2jpeg {
        /// Path to input SVG file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output JPEG file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,

        /// Image scale factor
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// JPEG Quality between 0 (worst) and 100 (best)
        #[arg(short, long, default_value = "90")]
        quality: u8,
    },

    /// Convert an SVG image to a PDF image
    Svg2pdf {
        /// Path to input SVG file. Reads from stdin if omitted or set to "-"
        #[arg(short, long)]
        input: Option<String>,

        /// Path to output PDF file to be created. Writes to stdout if omitted or set to "-"
        #[arg(short, long)]
        output: Option<String>,
    },

    /// List available themes
    LsThemes,

    /// Print the config JSON for a theme
    #[command(arg_required_else_help = true)]
    CatTheme {
        /// Name of a theme
        theme: String,
    },

    /// Print the default vlc-config file path
    ConfigPath,

    /// Run the HTTP conversion server.
    Serve(crate::serve::ServeArgs),
}

impl Commands {
    pub(crate) fn is_serve(&self) -> bool {
        matches!(self, Commands::Serve(_))
    }
}

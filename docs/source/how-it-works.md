# How It Works

VlConvert embeds the Vega, Vega-Lite, Vega Themes, and Vega Embed JavaScript
libraries in Rust. Conversions run inside V8 through Deno runtime components.

SVG output comes from Vega. PNG and JPEG output render that SVG through
`resvg`. PDF output converts SVG to vector PDF.

Python and CLI packages wrap the same Rust implementation. The server package
uses the same converter behind HTTP routes.

use crate::image_loading::ImageAccessPolicy;
use crate::text::USVG_OPTIONS;
use deno_core::anyhow::{anyhow, bail};
use deno_core::error::AnyError;
use image::codecs::jpeg::JpegEncoder;
use image::ImageReader;
use png::{PixelDimensions, Unit};
use resvg::render;
use std::io::Cursor;
use std::panic;
use svg2pdf::{ConversionOptions, PageOptions};
use tiny_skia::{Pixmap, PremultipliedColorU8};

use super::types::UrlOpts;
use super::value_or_string::ValueOrString;

// Modified from tiny-skia-0.10.0/src/pixmap.rs to include DPI
pub fn encode_png(pixmap: Pixmap, ppi: f32) -> Result<Vec<u8>, AnyError> {
    let mut pixmap = pixmap;

    // Demultiply alpha.
    //
    // RasterPipeline is 15% faster here, but produces slightly different results
    // due to rounding. So we stick with this method for now.
    for pixel in pixmap.pixels_mut() {
        let c = pixel.demultiply();
        let alpha = c.alpha();

        // jonmmease: tiny-skia uses the private PremultipliedColorU8::from_rgba_unchecked here,
        // but we need to use from_rgba, which checks to make sure r/g/b are less then or equal
        // to alpha. Use min to ensure we don't trigger the check
        *pixel = PremultipliedColorU8::from_rgba(
            c.red().min(alpha),
            c.green().min(alpha),
            c.blue().min(alpha),
            alpha,
        )
        .expect("Failed to construct PremultipliedColorU8 from rgba");
    }

    let mut data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut data, pixmap.width(), pixmap.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let ppm = (ppi.max(0.0) / 0.0254).round() as u32;
        encoder.set_pixel_dims(Some(PixelDimensions {
            xppu: ppm,
            yppu: ppm,
            unit: Unit::Meter,
        }));

        let mut writer = encoder.write_header()?;
        writer.write_image_data(pixmap.data())?;
    }

    Ok(data)
}

pub(crate) fn default_image_access_policy() -> ImageAccessPolicy {
    // Default: allow HTTP, no filesystem (None means allow http/https, deny filesystem)
    ImageAccessPolicy {
        allowed_base_urls: None,
        filesystem_root: None,
    }
}

pub fn svg_to_png(svg: &str, scale: f32, ppi: Option<f32>) -> Result<Vec<u8>, AnyError> {
    svg_to_png_with_policy(svg, scale, ppi, &default_image_access_policy())
}

pub(crate) fn svg_to_png_with_policy(
    svg: &str,
    scale: f32,
    ppi: Option<f32>,
    policy: &ImageAccessPolicy,
) -> Result<Vec<u8>, AnyError> {
    // default ppi to 72
    let ppi = ppi.unwrap_or(72.0);
    let scale = scale * ppi / 72.0;
    let policy = policy.clone();

    // catch_unwind so that we don't poison Mutexes
    // if usvg/resvg panics
    let response = panic::catch_unwind(|| {
        let rtree = match parse_svg(svg, &policy) {
            Ok(rtree) => rtree,
            Err(err) => return Err(err),
        };

        let mut pixmap = tiny_skia::Pixmap::new(
            (rtree.size().width() * scale) as u32,
            (rtree.size().height() * scale) as u32,
        )
        .unwrap();

        let transform = tiny_skia::Transform::from_scale(scale, scale);
        render(&rtree, transform, &mut pixmap.as_mut());
        Ok(encode_png(pixmap, ppi))
    });
    match response {
        Ok(Ok(Ok(png_result))) => Ok(png_result),
        Ok(Err(err)) => Err(err),
        err => bail!("{err:?}"),
    }
}

pub fn svg_to_jpeg(svg: &str, scale: f32, quality: Option<u8>) -> Result<Vec<u8>, AnyError> {
    svg_to_jpeg_with_policy(svg, scale, quality, &default_image_access_policy())
}

pub(crate) fn svg_to_jpeg_with_policy(
    svg: &str,
    scale: f32,
    quality: Option<u8>,
    policy: &ImageAccessPolicy,
) -> Result<Vec<u8>, AnyError> {
    let png_bytes = svg_to_png_with_policy(svg, scale, None, policy)?;
    let img = ImageReader::new(Cursor::new(png_bytes))
        .with_guessed_format()?
        .decode()?;

    let quality = quality.unwrap_or(90);
    if quality > 100 {
        bail!("JPEG quality parameter must be between 0 and 100 inclusive. Received: {quality}");
    }

    let mut jpeg_bytes: Vec<u8> = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, quality);

    // Encode the image
    encoder.encode_image(&img)?;

    Ok(jpeg_bytes)
}

pub fn svg_to_pdf(svg: &str) -> Result<Vec<u8>, AnyError> {
    svg_to_pdf_with_policy(svg, &default_image_access_policy())
}

pub(crate) fn svg_to_pdf_with_policy(
    svg: &str,
    policy: &ImageAccessPolicy,
) -> Result<Vec<u8>, AnyError> {
    let tree = parse_svg(svg, policy)?;
    let pdf = svg2pdf::to_pdf(&tree, ConversionOptions::default(), PageOptions::default());
    pdf.map_err(|err| anyhow!("Failed to convert SVG to PDF: {}", err))
}

/// Helper to parse svg string to usvg Tree with more helpful error messages
pub(crate) fn parse_svg(svg: &str, policy: &ImageAccessPolicy) -> Result<usvg::Tree, AnyError> {
    let mut opts = USVG_OPTIONS
        .lock()
        .map_err(|err| anyhow!("Failed to acquire usvg options lock: {err}"))?;
    parse_svg_with_options(svg, policy, &mut opts)
}

pub(crate) fn parse_svg_with_options(
    svg: &str,
    policy: &ImageAccessPolicy,
    opts: &mut usvg::Options<'static>,
) -> Result<usvg::Tree, AnyError> {
    let xml_opt = usvg::roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };

    let doc = usvg::roxmltree::Document::parse_with_options(svg, xml_opt)?;

    match doc.root_element().tag_name().namespace() {
        Some("http://www.w3.org/2000/svg") => {
            // All good
        }
        Some(other) => {
            bail!(
                "Invalid xmlns for SVG file. \n\
                Expected \"http://www.w3.org/2000/svg\". \n\
                Found \"{other}\""
            );
        }
        None => {
            bail!(
                "SVG file must have the xmlns attribute set to \"http://www.w3.org/2000/svg\"\n\
                For example <svg width=\"100\", height=\"100\", xmlns=\"http://www.w3.org/2000/svg\">...</svg>"
            )
        }
    }

    let previous_resources_dir = opts.resources_dir.clone();
    opts.resources_dir = policy.filesystem_root.clone();
    let (result, access_errors) =
        crate::image_loading::with_image_access_policy(policy.clone(), || {
            usvg::Tree::from_xmltree(&doc, opts)
        });
    opts.resources_dir = previous_resources_dir;

    if !access_errors.is_empty() {
        bail!("{}", access_errors.join("\n"));
    }

    Ok(result?)
}

pub fn vegalite_to_url(
    vl_spec: impl Into<ValueOrString>,
    url_opts: UrlOpts,
) -> Result<String, AnyError> {
    let spec_str = match vl_spec.into() {
        ValueOrString::JsonString(s) => s,
        ValueOrString::Value(v) => serde_json::to_string(&v)?,
    };
    let compressed_data = lz_str::compress_to_encoded_uri_component(&spec_str);
    let view = if url_opts.fullscreen {
        "/view".to_string()
    } else {
        String::new()
    };
    Ok(format!(
        "https://vega.github.io/editor/#/url/vega-lite/{compressed_data}{view}"
    ))
}

pub fn vega_to_url(
    vg_spec: impl Into<ValueOrString>,
    url_opts: UrlOpts,
) -> Result<String, AnyError> {
    let spec_str = match vg_spec.into() {
        ValueOrString::JsonString(s) => s,
        ValueOrString::Value(v) => serde_json::to_string(&v)?,
    };
    let compressed_data = lz_str::compress_to_encoded_uri_component(&spec_str);
    let view = if url_opts.fullscreen {
        "/view".to_string()
    } else {
        String::new()
    };
    Ok(format!(
        "https://vega.github.io/editor/#/url/vega/{compressed_data}{view}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_vegalite_to_url() {
        let vl_spec: serde_json::Value = serde_json::from_str(r#"
{
"data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
"mark": "bar",
"encoding": {
    "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
    "y": {"aggregate": "mean", "field": "precipitation"}
}
}
    "#).unwrap();

        let url = vegalite_to_url(&vl_spec, UrlOpts { fullscreen: false }).unwrap();
        let expected = concat!(
            "https://vega.github.io/editor/#/url/vega-lite/",
            "N4IgJghgLhIFygK4CcA28QAspQA4Gc4B6I5CAdwDoBzASyk0QCNF8BTZAYwHsA7KNv0o8AtkQBubahAlSIAWkg",
            "x2UfERER8A5ESUz20KKjbzybaJg7D84kAF8ANCA3IA1hiYRkIJ4J5haXmp4UAAPEJAoWhE2AFVeegwRPgYfEAA",
            "zWjZUMAwlNjSoAE9cArgQbmQA3gh0RxAiiIhqamQ5ASTzXjTM7Nzy3DbOWlx6aFo+ezs7IA",
        );
        println!("{url}");
        assert_eq!(url, expected);
    }

    #[test]
    fn test_convert_vega_to_url() {
        let vl_spec: serde_json::Value = serde_json::from_str(
            r#"
{
  "$schema": "https://vega.github.io/schema/vega/v5.json",
  "description": "A basic stacked bar chart example.",
  "width": 500,
  "height": 200,
  "padding": 5,

  "data": [
{
  "name": "table",
  "values": [
    {"x": 0, "y": 28, "c": 0}, {"x": 0, "y": 55, "c": 1},
    {"x": 1, "y": 43, "c": 0}, {"x": 1, "y": 91, "c": 1},
    {"x": 2, "y": 81, "c": 0}, {"x": 2, "y": 53, "c": 1},
    {"x": 3, "y": 19, "c": 0}, {"x": 3, "y": 87, "c": 1},
    {"x": 4, "y": 52, "c": 0}, {"x": 4, "y": 48, "c": 1},
    {"x": 5, "y": 24, "c": 0}, {"x": 5, "y": 49, "c": 1},
    {"x": 6, "y": 87, "c": 0}, {"x": 6, "y": 66, "c": 1},
    {"x": 7, "y": 17, "c": 0}, {"x": 7, "y": 27, "c": 1},
    {"x": 8, "y": 68, "c": 0}, {"x": 8, "y": 16, "c": 1},
    {"x": 9, "y": 49, "c": 0}, {"x": 9, "y": 15, "c": 1}
  ],
  "transform": [
    {
      "type": "stack",
      "groupby": ["x"],
      "sort": {"field": "c"},
      "field": "y"
    }
  ]
}
  ],

  "scales": [
{
  "name": "x",
  "type": "band",
  "range": "width",
  "domain": {"data": "table", "field": "x"}
},
{
  "name": "y",
  "type": "linear",
  "range": "height",
  "nice": true, "zero": true,
  "domain": {"data": "table", "field": "y1"}
},
{
  "name": "color",
  "type": "ordinal",
  "range": "category",
  "domain": {"data": "table", "field": "c"}
}
  ],

  "axes": [
{"orient": "bottom", "scale": "x", "zindex": 1},
{"orient": "left", "scale": "y", "zindex": 1}
  ],

  "marks": [
{
  "type": "rect",
  "from": {"data": "table"},
  "encode": {
    "enter": {
      "x": {"scale": "x", "field": "x"},
      "width": {"scale": "x", "band": 1, "offset": -1},
      "y": {"scale": "y", "field": "y0"},
      "y2": {"scale": "y", "field": "y1"},
      "fill": {"scale": "color", "field": "c"}
    },
    "update": {
      "fillOpacity": {"value": 1}
    },
    "hover": {
      "fillOpacity": {"value": 0.5}
    }
  }
}
  ]
}
    "#,
        )
        .unwrap();

        let url = vega_to_url(&vl_spec, UrlOpts { fullscreen: true }).unwrap();
        println!("{url}");
        let expected = concat!(
            "https://vega.github.io/editor/#/url/vega/",
            "N4IgJAzgxgFgpgWwIYgFwhgF0wBwqgegIDc4BzJAOjIEtMYBXAI0poHsDp5kTykSArJQBWENgDsQAGhAATONA",
            "BONHJnaT0AQQAETJBBpRtETEigBrOLN1JF22Lcza4ADyQIcAGziVpIAO40svRoAgAMYTLwNGRYaABMETI4SLK",
            "yNOJkoTKySKZoANqg4u5waCCmTN5+xEieDAqFoC5okSAAngkAHDJQrQC+Us2tMp2oAgK9aACMg8Oo06NoACwA",
            "zFOoYXMgLQtLqACciyB9C9u78ftdx6dbQzsJ+wLrJzPnaC9j0wcbd-OfaC6AHYNrN7rtlk9Lq9Nu9UJCOisej",
            "CwfNJojUPEEbc4eixssfii4QA2K4gmF-B6oUkY4k006oqnkr7knHgtDMhKst7s1DIsbE5Fs+b8mb0nnzQn4wn",
            "CqlSmbohn9AC6MkwiiQ4ggADM2IoEE0Ku0cGV0CYzOY-GRFGwGDgmGMCg9VSAxIpMGhQNqaHBPLJyn1BiBvb7",
            "-ehOiqVTJoHVGqgiiASghTQ81caU3pxP6ZBrMinAsEYH5ZGxkBlPXI8ih0JVqjIQ37yi1+tskynOmmTeVPBk4L",
            "Y-LmyCnorEPTJxIZTeqGjIAF5wG1oadwHKlpDl1CgXL5GtIKples+xvh6YgFv3NsBtiePV+TDp8p69IlTwDzVD",
            "gN5ch6jtyNcbrcq3KWsD2DI8w1eFsXSQFw4wTPUfXED10CYNhsFLPwY2qdAWjnDJ5F2RkELgJDuzgbUx1dKBY3",
            "KX9Z3w1w3hdZBFHMCBDXvLt0EUOAoEo7UbQNTdKx3Co92qIMSKgNh5ArEjMAXCtdlALCU1wsDQybM8ZALEJhNU",
            "rSZEzMNjjYbVtQgOBkIAWjBDEVOo7DEUPTTwzCbSOniCsDPDPwGwg9pTyDb1PFffTHJTaSb0UPzwIDM8gztbdT",
            "S9GhQoAeRSKA6DGUBanqU1ZiDGA2FIGLhJCzxMrMHKK3yhpWkoAQWyg-ogA/view",
        );
        assert_eq!(url, expected);
    }
}

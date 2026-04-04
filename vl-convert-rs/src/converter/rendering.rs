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

use super::config::ValueOrString;
use super::types::UrlOpts;

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

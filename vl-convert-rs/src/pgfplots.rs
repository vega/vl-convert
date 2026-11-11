//! Strict, static PGFPlots export of evaluated Vega geometry.
//!
//! This is not a semantic Vega-to-`\\addplot` translator. Text is outlined and
//! unsupported paint operations return an error; no raster fallback is used.

use std::fmt::Write;

use crate::anyhow::{anyhow, bail, Context, Result};
use crate::{GoogleFontUsage, LogEntry, SvgOpts, SvgOutput, ValueOrString, VgOpts, VlConverter, VlOpts};
use tiny_skia::{Path, PathSegment, Point, Transform};
use usvg::{FillRule, Group, Node, Paint, PaintOrder};

const MAX_SVG_BYTES: usize = 32 * 1024 * 1024;
const MAX_DEPTH: usize = 128;
const MAX_COORD: f32 = 20_000.0;
const MAX_PATHS: usize = 200_000;
const MAX_SEGMENTS: usize = 2_000_000;

/// Options for a static PGFPlots export.
#[derive(Debug, Clone, Copy, Default)]
pub struct PgfplotsOpts {
    /// Emit a complete `standalone` document instead of a `tikzpicture` fragment.
    pub standalone: bool,
}

/// PGFPlots source and diagnostics from the original Vega evaluation.
#[derive(Debug)]
pub struct PgfplotsOutput {
    /// TeX source. No auxiliary images or font files are required.
    pub tex: String,
    /// Diagnostics from Vega/Vega-Lite, before export.
    pub logs: Vec<LogEntry>,
    /// Font usage reported by the SVG conversion.
    pub google_fonts: GoogleFontUsage,
}

impl VlConverter {
    /// Export the initial evaluated state of a Vega specification to PGFPlots.
    ///
    /// Unsupported graphics fail explicitly. Interactive behavior, source data,
    /// and semantic axes are not retained. See the PGFPlots guide for limits.
    pub async fn vega_to_pgfplots(
        &self,
        spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
        opts: PgfplotsOpts,
    ) -> Result<PgfplotsOutput> {
        finish(self.vega_to_svg(spec, vg_opts, SvgOpts::default()).await?, opts).await
    }

    /// Evaluate Vega-Lite with the existing compiler, then export static geometry.
    pub async fn vegalite_to_pgfplots(
        &self,
        spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
        opts: PgfplotsOpts,
    ) -> Result<PgfplotsOutput> {
        finish(self.vegalite_to_svg(spec, vl_opts, SvgOpts::default()).await?, opts).await
    }
}

async fn finish(svg: SvgOutput, opts: PgfplotsOpts) -> Result<PgfplotsOutput> {
    let SvgOutput { svg, logs, google_fonts } = svg;
    let tex = tokio::task::spawn_blocking(move || svg_to_pgfplots(&svg, opts))
        .await.context("PGFPlots worker failed")??;
    Ok(PgfplotsOutput { tex, logs, google_fonts })
}

/// Convert a restricted SVG to static PGFPlots without invoking Vega.
///
/// Uses vl-convert's registered font database. External images, stylesheets,
/// filters, masks, patterns, gradients, and non-normal blending are rejected.
/// SVG text is converted to paths, never interpreted as TeX.
///
/// SVG-referenced network and filesystem resources are never loaded.
/// Font registration is a separate, caller-controlled operation.
pub fn svg_to_pgfplots(svg: &str, opts: PgfplotsOpts) -> Result<String> {
    if svg.len() > MAX_SVG_BYTES {
        bail!("PGFPlots: SVG exceeds the 32 MiB input limit");
    }
    // Parse with DTDs disabled, before usvg can discard unsupported elements.
    let doc = usvg::roxmltree::Document::parse(svg).context("PGFPlots: invalid SVG")?;
    if doc.root_element().tag_name().namespace() != Some("http://www.w3.org/2000/svg")
        || doc.root_element().tag_name().name() != "svg"
    {
        bail!("PGFPlots: expected an SVG root in the SVG namespace");
    }
    for node in doc.descendants().filter(|n| n.is_element()) {
        if node.ancestors().count() > MAX_DEPTH {
            bail!("PGFPlots: SVG nesting exceeds {MAX_DEPTH}");
        }
        if node.tag_name().namespace() != Some("http://www.w3.org/2000/svg") {
            bail!("PGFPlots: foreign XML namespace is unsupported");
        }
        let tag = node.tag_name().name();
        if !matches!(tag, "svg" | "g" | "defs" | "clipPath" | "path" | "rect" |
            "circle" | "ellipse" | "line" | "polyline" | "polygon" | "text" |
            "tspan" | "a" | "title" | "desc" | "use")
        {
            bail!("PGFPlots: unsupported SVG element <{tag}>");
        }
        for attr in node.attributes() {
            if attr.name() == "href" && !attr.value().starts_with('#') {
                bail!("PGFPlots: external references are unsupported");
            }
            if matches!(attr.name(), "fill" | "stroke" | "clip-path" | "style") {
                let value = attr.value().to_ascii_lowercase();
                let mut rest = value.as_str();
                while let Some((_, after)) = rest.split_once("url(") {
                    let (argument, tail) = after.split_once(')')
                        .ok_or_else(|| anyhow!("PGFPlots: malformed URL reference"))?;
                    let argument = argument.trim().trim_matches(['\'', '"']);
                    if !argument.starts_with('#') {
                        bail!("PGFPlots: external paint or clip references are unsupported");
                    }
                    rest = tail;
                }
            }
            if matches!(attr.name(), "filter" | "mask" | "vector-effect") {
                bail!("PGFPlots: unsupported SVG attribute {}", attr.name());
            }
            if attr.name() == "style" && attr.value().split(';').any(|s| {
                matches!(s.split(':').next().unwrap_or("").trim(),
                    "filter" | "mask" | "vector-effect")
            }) {
                bail!("PGFPlots: unsupported SVG effect in style");
            }
        }
    }
    let snapshot = crate::text::get_font_baseline_snapshot()?;
    let mut options = crate::text::build_usvg_options_with_fontdb(snapshot.clone_fontdb());
    // Preflight rejects images and external hrefs; disable resolvers as well.
    options.image_href_resolver = usvg::ImageHrefResolver {
        resolve_data: Box::new(|_, _, _| None),
        resolve_string: Box::new(|_, _| None),
    };
    let tree = usvg::Tree::from_xmltree(&doc, &options)
        .context("PGFPlots: SVG normalization failed")?;
    let width = tree.size().width();
    let height = tree.size().height();
    check_coord(width)?;
    check_coord(height)?;
    let mut writer = Writer { tex: String::new(), paths: 0, segments: 0 };
    if opts.standalone {
        writer.tex.push_str("\\documentclass[border=0bp]{standalone}\n\\usepackage{pgfplots}\n\\pgfplotsset{compat=1.18}\n\\begin{document}\n");
    }
    writer.tex.push_str("% Static evaluated geometry; text is outlined. Not semantic plot data.\n\\begin{tikzpicture}\n");
    writeln!(writer.tex, "\\begin{{axis}}[name=vlcframe,anchor=north west,at={{(0,0)}},hide axis,scale only axis,\nwidth={:.6}bp,height={:.6}bp,xmin=0,xmax=1,ymin=0,ymax=1,enlargelimits=false,clip=false]\n\\end{{axis}}", width * 0.75, height * 0.75)?;
    writer.tex.push_str("\\begin{scope}[shift={(vlcframe.north west)}]\n");
    writeln!(writer.tex, "\\pgfpathrectangle{{\\pgfpoint{{0bp}}{{0bp}}}}{{\\pgfpoint{{{:.6}bp}}{{{:.6}bp}}}}\\pgfusepath{{clip}}", width * 0.75, -height * 0.75)?;
    writer.group(tree.root(), Transform::identity(), 0)?;
    writer.tex.push_str("\\end{scope}\n\\pgfresetboundingbox\n\\path[use as bounding box] (vlcframe.north west) rectangle (vlcframe.south east);\n\\end{tikzpicture}\n");
    if opts.standalone { writer.tex.push_str("\\end{document}\n"); }
    Ok(writer.tex)
}

struct Writer {
    tex: String,
    paths: usize,
    segments: usize,
}

fn check_coord(n: f32) -> Result<()> {
    if !n.is_finite() || n.abs() > MAX_COORD {
        bail!("PGFPlots: coordinate outside TeX's supported range");
    }
    Ok(())
}

impl Writer {
    fn group(&mut self, group: &Group, transform: Transform, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH { bail!("PGFPlots: normalized tree too deep"); }
        if group.mask().is_some() || !group.filters().is_empty()
            || group.blend_mode() != usvg::BlendMode::Normal
        {
            bail!("PGFPlots: masks, filters, or non-normal blending are unsupported");
        }
        let transform = transform.pre_concat(group.transform());
        let isolate = group.isolate() || group.opacity().get() != 1.0;
        if isolate {
            writeln!(self.tex, "\\begin{{scope}}[opacity={:.6},transparency group]", group.opacity().get())?;
        } else {
            self.tex.push_str("\\begin{scope}\n");
        }
        if let Some(clip) = group.clip_path() { self.clip(clip, transform, depth + 1)?; }
        for child in group.children() {
            match child {
                Node::Group(g) => self.group(g, transform, depth + 1)?,
                Node::Text(t) => self.group(t.flattened(), transform, depth + 1)?,
                Node::Path(p) => self.path(p, transform)?,
                Node::Image(_) => bail!("PGFPlots: image nodes are unsupported"),
            }
        }
        self.tex.push_str("\\end{scope}\n");
        Ok(())
    }

    fn clip(&mut self, clip: &usvg::ClipPath, transform: Transform, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH { bail!("PGFPlots: clip nesting too deep"); }
        if let Some(parent) = clip.clip_path() { self.clip(parent, transform, depth + 1)?; }
        let mut paths = Vec::new();
        collect_clip(clip.root(), transform.pre_concat(clip.transform()), &mut paths, depth)?;
        if paths.len() > 1 {
            bail!("PGFPlots: multi-element clip unions are unsupported");
        }
        if let Some((path, transform)) = paths.first() {
            self.rule(path.fill().map_or(FillRule::NonZero, |f| f.rule()));
            self.geometry(path.data(), *transform)?;
        } else {
            self.tex.push_str("\\pgfpathrectangle{\\pgfpointorigin}{\\pgfpoint{0bp}{0bp}}\n");
        }
        self.tex.push_str("\\pgfusepath{clip}\n");
        Ok(())
    }

    fn path(&mut self, path: &usvg::Path, transform: Transform) -> Result<()> {
        if !path.is_visible() { return Ok(()); }
        let fill = |writer: &mut Self| -> Result<()> {
            if let Some(fill) = path.fill() {
                writer.paint(path.data(), transform, fill.paint(), fill.opacity().get(), fill.rule())?;
            }
            Ok(())
        };
        let stroke = |writer: &mut Self| -> Result<()> {
            if let Some(stroke) = path.stroke() {
                // Outline before the affine transform: non-uniformly scaled
                // strokes, joins, caps, and dashes then retain SVG geometry.
                let resolution = transform.sx.hypot(transform.kx)
                    .max(transform.ky.hypot(transform.sy)).max(1.0);
                let mut settings = stroke.to_tiny_skia();
                // Path::stroke does NOT apply Stroke::dash. Dash explicitly,
                // then outline the resulting pieces before transforming them.
                let dashed = settings.dash.as_ref().map(|dash| {
                    path.data().dash(dash, resolution)
                        .ok_or_else(|| anyhow!("PGFPlots: dashing produced no representable path"))
                }).transpose()?;
                settings.dash = None;
                let source = dashed.as_ref().unwrap_or(path.data());
                let outline = source.stroke(&settings, resolution)
                    .ok_or_else(|| anyhow!("PGFPlots: stroke outlining failed"))?;
                writer.paint(&outline, transform, stroke.paint(), stroke.opacity().get(), FillRule::NonZero)?;
            }
            Ok(())
        };
        match path.paint_order() {
            PaintOrder::FillAndStroke => { fill(self)?; stroke(self)?; }
            PaintOrder::StrokeAndFill => { stroke(self)?; fill(self)?; }
        }
        Ok(())
    }

    fn paint(&mut self, path: &Path, transform: Transform, paint: &Paint, opacity: f32, rule: FillRule) -> Result<()> {
        let Paint::Color(color) = paint else {
            bail!("PGFPlots: gradients and patterns are unsupported");
        };
        self.paths += 1;
        if self.paths > MAX_PATHS { bail!("PGFPlots: path limit exceeded"); }
        self.tex.push_str("\\begin{scope}\n");
        writeln!(self.tex, "\\definecolor{{vlcpaint}}{{RGB}}{{{},{},{}}}\\pgfsetfillcolor{{vlcpaint}}\\pgfsetfillopacity{{{:.6}}}", color.red, color.green, color.blue, opacity)?;
        self.rule(rule);
        self.geometry(path, transform)?;
        self.tex.push_str("\\pgfusepath{fill}\n\\end{scope}\n");
        Ok(())
    }

    fn rule(&mut self, rule: FillRule) {
        self.tex.push_str(match rule {
            FillRule::EvenOdd => "\\pgfseteorule\n",
            FillRule::NonZero => "\\pgfsetnonzerorule\n",
        });
    }

    fn point(&mut self, point: Point) -> Result<()> {
        check_coord(point.x)?;
        check_coord(point.y)?;
        write!(self.tex, "{{\\pgfpoint{{{:.6}bp}}{{{:.6}bp}}}}", point.x * 0.75, -point.y * 0.75)?;
        Ok(())
    }

    fn geometry(&mut self, path: &Path, transform: Transform) -> Result<()> {
        let path = path.clone().transform(transform)
            .ok_or_else(|| anyhow!("PGFPlots: non-finite path transform"))?;
        for segment in path.segments() {
            self.segments += 1;
            if self.segments > MAX_SEGMENTS { bail!("PGFPlots: segment limit exceeded"); }
            match segment {
                PathSegment::MoveTo(p) => { self.tex.push_str("\\pgfpathmoveto"); self.point(p)?; }
                PathSegment::LineTo(p) => { self.tex.push_str("\\pgfpathlineto"); self.point(p)?; }
                PathSegment::QuadTo(a, b) => {
                    self.tex.push_str("\\pgfpathquadraticcurveto"); self.point(a)?; self.point(b)?;
                }
                PathSegment::CubicTo(a, b, c) => {
                    self.tex.push_str("\\pgfpathcurveto"); self.point(a)?; self.point(b)?; self.point(c)?;
                }
                PathSegment::Close => self.tex.push_str("\\pgfpathclose"),
            }
            self.tex.push('\n');
        }
        Ok(())
    }
}

fn collect_clip<'a>(group: &'a Group, transform: Transform,
    paths: &mut Vec<(&'a usvg::Path, Transform)>, depth: usize) -> Result<()>
{
    if depth > MAX_DEPTH { bail!("PGFPlots: clip nesting too deep"); }
    if group.clip_path().is_some() || group.mask().is_some() || !group.filters().is_empty() {
        bail!("PGFPlots: effects inside clip paths are unsupported");
    }
    let transform = transform.pre_concat(group.transform());
    for child in group.children() {
        match child {
            Node::Path(p) if p.is_visible() => paths.push((p, transform)),
            Node::Path(_) => {},
            Node::Group(g) => collect_clip(g, transform, paths, depth + 1)?,
            _ => bail!("PGFPlots: only path geometry is supported inside clip paths"),
        }
    }
    Ok(())
}

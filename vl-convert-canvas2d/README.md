# vl-convert-canvas2d

A pure Rust implementation of the HTML Canvas 2D API, designed for server-side rendering without a browser or JavaScript runtime.

## Overview

This crate provides a Canvas 2D rendering context that can generate PNG images from drawing commands. It uses:

- **tiny-skia** - 2D graphics rendering
- **cosmic-text** - Text shaping, measurement, and rendering with vector glyph paths
- **fontdb** - Font database management (can be shared with other crates)

The API closely follows the [WHATWG Canvas 2D specification](https://html.spec.whatwg.org/multipage/canvas.html), with Rust-idiomatic method names (e.g., `set_fill_style` instead of property assignment).

## Quick Example

```rust
use vl_convert_canvas2d::{Canvas2dContext, RectParams};

let mut ctx = Canvas2dContext::new(400, 300)?;
ctx.set_fill_style("#ff0000")?;
ctx.fill_rect(&RectParams { x: 10.0, y: 10.0, width: 100.0, height: 50.0 });
let png_data = ctx.to_png()?;
```

## Feature Support

### Legend
- ✅ Implemented
- ⚠️ Partial (limitations noted)
- ❌ Not implemented

### Drawing State

| Feature | Status | API |
|---------|--------|-----|
| save() | ✅ | `save()` |
| restore() | ✅ | `restore()` |
| reset() | ✅ | `reset()` |

### Transformations

| Feature | Status | API |
|---------|--------|-----|
| scale(x, y) | ✅ | `scale(x, y)` |
| rotate(angle) | ✅ | `rotate(angle)` |
| translate(x, y) | ✅ | `translate(x, y)` |
| transform(a,b,c,d,e,f) | ✅ | `transform(a, b, c, d, e, f)` |
| setTransform(a,b,c,d,e,f) | ✅ | `set_transform(a, b, c, d, e, f)` |
| setTransform(DOMMatrix) | ✅ | `set_transform_matrix(matrix)` |
| getTransform() | ✅ | `get_transform()` |
| resetTransform() | ✅ | `reset_transform()` |

### Compositing

| Feature | Status | API |
|---------|--------|-----|
| globalAlpha | ✅ | `set_global_alpha(alpha)` |
| globalCompositeOperation | ✅ | `set_global_composite_operation(op)` - all 26 blend modes |

### Image Smoothing

| Feature | Status | API |
|---------|--------|-----|
| imageSmoothingEnabled | ✅ | `set_image_smoothing_enabled(bool)` / `get_image_smoothing_enabled()` |
| imageSmoothingQuality | ✅ | `set_image_smoothing_quality(quality)` / `get_image_smoothing_quality()` |

### Fill and Stroke Styles

| Feature | Status | API |
|---------|--------|-----|
| fillStyle (color) | ✅ | `set_fill_style(css_color)` / `set_fill_style_color(color)` |
| strokeStyle (color) | ✅ | `set_stroke_style(css_color)` / `set_stroke_style_color(color)` |
| fillStyle (gradient) | ✅ | `set_fill_style_gradient(gradient)` |
| strokeStyle (gradient) | ✅ | `set_stroke_style_gradient(gradient)` |
| fillStyle (pattern) | ✅ | `set_fill_style_pattern(pattern)` |
| strokeStyle (pattern) | ✅ | `set_stroke_style_pattern(pattern)` |
| createLinearGradient() | ✅ | `create_linear_gradient(x0, y0, x1, y1)` |
| createRadialGradient() | ⚠️ | `create_radial_gradient(x0, y0, r0, x1, y1, r1)` - r0 (inner radius) not supported |
| createConicGradient() | ❌ | - |
| createPattern() | ✅ | `create_pattern(data, width, height, repetition)` |

#### CanvasGradient

| Feature | Status | API |
|---------|--------|-----|
| addColorStop() | ✅ | `add_color_stop(offset, color)` |

#### CanvasPattern

| Feature | Status | API |
|---------|--------|-----|
| setTransform() | ✅ | `set_transform(transform)` |
| repeat modes | ✅ | "repeat", "repeat-x", "repeat-y", "no-repeat" |

### Line Styles

| Feature | Status | API |
|---------|--------|-----|
| lineWidth | ✅ | `set_line_width(width)` |
| lineCap | ✅ | `set_line_cap(cap)` - Butt, Round, Square |
| lineJoin | ✅ | `set_line_join(join)` - Miter, Round, Bevel |
| miterLimit | ✅ | `set_miter_limit(limit)` |
| setLineDash() | ✅ | `set_line_dash(segments)` |
| getLineDash() | ✅ | `get_line_dash()` |
| lineDashOffset | ✅ | `set_line_dash_offset(offset)` |

### Shadows

| Feature | Status |
|---------|--------|
| shadowOffsetX | ❌ |
| shadowOffsetY | ❌ |
| shadowBlur | ❌ |
| shadowColor | ❌ |

### Filters

| Feature | Status |
|---------|--------|
| filter | ❌ |

### Rectangle Operations

| Feature | Status | API |
|---------|--------|-----|
| clearRect() | ✅ | `clear_rect(x, y, w, h)` |
| fillRect() | ✅ | `fill_rect(x, y, w, h)` |
| strokeRect() | ✅ | `stroke_rect(x, y, w, h)` |

### Path Operations

| Feature | Status | API |
|---------|--------|-----|
| beginPath() | ✅ | `begin_path()` |
| fill() | ✅ | `fill()` |
| fill(fillRule) | ✅ | `fill_with_rule(rule)` |
| fill(Path2D) | ✅ | `fill_path2d(path)` |
| fill(Path2D, fillRule) | ✅ | `fill_path2d_with_rule(path, rule)` |
| stroke() | ✅ | `stroke()` |
| stroke(Path2D) | ✅ | `stroke_path2d(path)` |
| clip() | ✅ | `clip()` |
| clip(fillRule) | ✅ | `clip_with_rule(rule)` |
| clip(Path2D) | ✅ | `clip_path2d(path)` |
| clip(Path2D, fillRule) | ✅ | `clip_path2d_with_rule(path, rule)` |
| isPointInPath() | ❌ | Not supported |
| isPointInStroke() | ❌ | Not supported |

### Path Building

| Feature | Status | API |
|---------|--------|-----|
| moveTo() | ✅ | `move_to(x, y)` |
| lineTo() | ✅ | `line_to(x, y)` |
| quadraticCurveTo() | ✅ | `quadratic_curve_to(cpx, cpy, x, y)` |
| bezierCurveTo() | ✅ | `bezier_curve_to(cp1x, cp1y, cp2x, cp2y, x, y)` |
| arcTo() | ✅ | `arc_to(x1, y1, x2, y2, radius)` |
| arc() | ✅ | `arc(x, y, radius, start_angle, end_angle, anticlockwise)` |
| ellipse() | ✅ | `ellipse(x, y, rx, ry, rotation, start, end, anticlockwise)` |
| rect() | ✅ | `rect(x, y, w, h)` |
| roundRect() | ✅ | `round_rect(x, y, w, h, radius)` / `round_rect_radii(x, y, w, h, radii)` |
| closePath() | ✅ | `close_path()` |

### Text Drawing

| Feature | Status | API |
|---------|--------|-----|
| fillText(text, x, y) | ✅ | `fill_text(text, x, y)` |
| fillText(text, x, y, maxWidth) | ✅ | `fill_text_max_width(text, x, y, max_width)` |
| strokeText(text, x, y) | ✅ | `stroke_text(text, x, y)` |
| strokeText(text, x, y, maxWidth) | ✅ | `stroke_text_max_width(text, x, y, max_width)` |
| measureText() | ✅ | `measure_text(text)` |

### Text Styles

| Feature | Status | API |
|---------|--------|-----|
| font | ✅ | `set_font(css_font_string)` |
| textAlign | ✅ | `set_text_align(align)` - Left, Right, Center, Start, End |
| textBaseline | ✅ | `set_text_baseline(baseline)` - Top, Hanging, Middle, Alphabetic, Ideographic, Bottom |
| letterSpacing | ✅ | `set_letter_spacing(spacing)` / `get_letter_spacing()` |
| direction | ❌ | - |
| wordSpacing | ❌ | - |
| fontKerning | ❌ | - |
| fontStretch | ❌ | - |
| fontVariantCaps | ❌ | - |
| textRendering | ❌ | - |

### Drawing Images

| Feature | Status | API |
|---------|--------|-----|
| drawImage(img, dx, dy) | ✅ | `draw_image(pixmap, dx, dy)` |
| drawImage(img, dx, dy, dw, dh) | ✅ | `draw_image_scaled(pixmap, dx, dy, dw, dh)` |
| drawImage(img, sx, sy, sw, sh, dx, dy, dw, dh) | ✅ | `draw_image_cropped(pixmap, sx, sy, sw, sh, dx, dy, dw, dh)` |

### Pixel Manipulation

| Feature | Status | API |
|---------|--------|-----|
| getImageData() | ✅ | `get_image_data(x, y, w, h)` - returns Vec<u8> RGBA |
| putImageData(data, dx, dy) | ✅ | `put_image_data(data, w, h, dx, dy)` |
| putImageData(data, dx, dy, dirty) | ✅ | `put_image_data_dirty(data, w, h, dx, dy, dirty_x, dirty_y, dirty_w, dirty_h)` |
| createImageData() | ✅ | `create_image_data(width, height)` |

### Path2D

| Feature | Status | API |
|---------|--------|-----|
| Path2D() | ✅ | `Path2D::new()` |
| Path2D(path) | ✅ | `Path2D::from_path(other)` |
| Path2D(svgPath) | ✅ | `Path2D::from_svg_path_data(d)` |
| All path methods | ✅ | Same as context path methods |
| addPath() | ❌ | - |

### Output

| Feature | Status | API |
|---------|--------|-----|
| PNG export | ✅ | `to_png()` |
| Pixmap access | ✅ | `pixmap()` / `pixmap_mut()` |

## Builder Pattern

For more control over context creation:

```rust
use vl_convert_canvas2d::Canvas2dContext;

let ctx = Canvas2dContext::builder(800, 600)
    .with_font_db(my_font_db)  // Share font database with other components
    .build()?;
```

## Known Limitations

1. **Radial gradients**: The inner radius (r0) is not fully supported by the underlying tiny-skia library
2. **Hit testing**: `isPointInPath()` and `isPointInStroke()` are not implemented
3. **Shadows**: Shadow effects are not supported
4. **Filters**: CSS filter property is not supported
5. **Conic gradients**: `createConicGradient()` is not implemented
6. **Pattern memory**: Pattern backing pixmaps are cached per context with an LRU byte budget (no intentional leaks)

## License

BSD-3-Clause

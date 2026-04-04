// CanvasRenderingContext2D and related class polyfills for vl-convert canvas

import {
  op_canvas_save,
  op_canvas_restore,
  op_canvas_set_fill_style,
  op_canvas_set_stroke_style,
  op_canvas_set_line_width,
  op_canvas_set_line_cap,
  op_canvas_set_line_join,
  op_canvas_set_miter_limit,
  op_canvas_set_global_alpha,
  op_canvas_set_global_composite_operation,
  op_canvas_set_font,
  op_canvas_set_text_align,
  op_canvas_set_text_baseline,
  op_canvas_set_font_stretch,
  op_canvas_get_font_stretch,
  op_canvas_measure_text,
  op_canvas_fill_text,
  op_canvas_stroke_text,
  op_canvas_begin_path,
  op_canvas_move_to,
  op_canvas_line_to,
  op_canvas_close_path,
  op_canvas_bezier_curve_to,
  op_canvas_quadratic_curve_to,
  op_canvas_rect,
  op_canvas_arc,
  op_canvas_arc_to,
  op_canvas_ellipse,
  op_canvas_fill,
  op_canvas_stroke,
  op_canvas_fill_rect,
  op_canvas_stroke_rect,
  op_canvas_clear_rect,
  op_canvas_clip,
  op_canvas_translate,
  op_canvas_rotate,
  op_canvas_scale,
  op_canvas_transform,
  op_canvas_set_transform,
  op_canvas_reset_transform,
  op_canvas_set_line_dash,
  op_canvas_set_line_dash_offset,
  op_canvas_get_image_data,
  op_canvas_create_linear_gradient,
  op_canvas_create_radial_gradient,
  op_canvas_gradient_add_color_stop,
  op_canvas_set_fill_style_gradient,
  op_canvas_set_stroke_style_gradient,
  op_canvas_fill_text_max_width,
  op_canvas_stroke_text_max_width,
  op_canvas_draw_image,
  op_canvas_draw_image_scaled,
  op_canvas_draw_image_cropped,
  op_canvas_draw_canvas,
  op_canvas_draw_canvas_scaled,
  op_canvas_draw_canvas_cropped,
  op_canvas_create_pattern,
  op_canvas_create_pattern_from_canvas,
  op_canvas_set_fill_style_pattern,
  op_canvas_set_stroke_style_pattern,
  op_canvas_put_image_data,
  op_canvas_put_image_data_dirty,
  op_canvas_set_image_smoothing_enabled,
  op_canvas_get_image_smoothing_enabled,
  op_canvas_set_image_smoothing_quality,
  op_canvas_get_image_smoothing_quality,
  op_canvas_fill_with_rule,
  op_canvas_clip_with_rule,
  op_canvas_fill_path2d,
  op_canvas_fill_path2d_with_rule,
  op_canvas_stroke_path2d,
  op_canvas_clip_path2d,
  op_canvas_clip_path2d_with_rule,
  op_canvas_reset,
  op_canvas_round_rect,
  op_canvas_round_rect_radii,
  op_canvas_set_letter_spacing,
  op_canvas_get_letter_spacing,
  op_canvas_get_transform,
  op_canvas_decode_svg_at_size,
} from "ext:core/ops";

import { Image, ImageData, unsupported, validateImageDataSettings } from "ext:vl_convert_canvas2d/image_polyfill.js";
import { Path2D } from "ext:vl_convert_canvas2d/path2d_polyfill.js";

/**
 * TextMetrics class for measureText results
 */
class TextMetrics {
  constructor(width) {
    this.width = width;
    // Approximations for other metrics
    this.actualBoundingBoxLeft = 0;
    this.actualBoundingBoxRight = width;
    this.fontBoundingBoxAscent = 0;
    this.fontBoundingBoxDescent = 0;
    this.actualBoundingBoxAscent = 0;
    this.actualBoundingBoxDescent = 0;
    this.emHeightAscent = 0;
    this.emHeightDescent = 0;
    this.hangingBaseline = 0;
    this.alphabeticBaseline = 0;
    this.ideographicBaseline = 0;
  }
}

/**
 * CanvasGradient class for gradient fill/stroke styles
 */
class CanvasGradient {
  #rid;
  #gradientId;

  constructor(rid, gradientId) {
    this.#rid = rid;
    this.#gradientId = gradientId;
  }

  addColorStop(offset, color) {
    op_canvas_gradient_add_color_stop(this.#rid, this.#gradientId, offset, color);
  }

  // Internal method to get the gradient ID for applying as fill/stroke style
  _getGradientId() {
    return this.#gradientId;
  }

  // Internal method to get the canvas resource ID
  _getRid() {
    return this.#rid;
  }
}

/**
 * CanvasPattern class for pattern fill/stroke styles
 */
class CanvasPattern {
  #rid;
  #patternId;

  constructor(rid, patternId) {
    this.#rid = rid;
    this.#patternId = patternId;
  }

  setTransform(transform) {
    // Pattern transforms are not currently supported
    // This is a no-op to maintain API compatibility
  }

  _getPatternId() {
    return this.#patternId;
  }

  _getRid() {
    return this.#rid;
  }
}

/**
 * CanvasRenderingContext2D polyfill that wraps Rust ops
 */
class CanvasRenderingContext2D {
  #rid;
  #canvas;
  #fillStyle = "#000000";
  #strokeStyle = "#000000";
  #lineWidth = 1;
  #lineCap = "butt";
  #lineJoin = "miter";
  #miterLimit = 10;
  #globalAlpha = 1;
  #globalCompositeOperation = "source-over";
  #font = "10px sans-serif";
  #textAlign = "start";
  #textBaseline = "alphabetic";
  #lineDash = [];
  #lineDashOffset = 0;
  #imageSmoothingEnabled = true;
  #imageSmoothingQuality = "low";
  #fontStretch = "normal";
  #letterSpacing = "0px";

  constructor(rid, canvas) {
    this.#rid = rid;
    this.#canvas = canvas;
  }

  get canvas() {
    return this.#canvas;
  }

  // Internal method to get the resource ID (used for canvas-to-canvas drawing)
  _getRid() {
    return this.#rid;
  }

  // Internal method to update the resource ID (used when canvas is resized)
  _setRid(rid) {
    this.#rid = rid;
  }

  get fillStyle() {
    return this.#fillStyle;
  }

  set fillStyle(value) {
    if (typeof value === "string") {
      try {
        op_canvas_set_fill_style(this.#rid, value);
        this.#fillStyle = value;
      } catch (e) {
        // Ignore invalid colors
      }
    } else if (value instanceof CanvasGradient) {
      op_canvas_set_fill_style_gradient(this.#rid, value._getGradientId());
      this.#fillStyle = value;
    } else if (value instanceof CanvasPattern) {
      op_canvas_set_fill_style_pattern(this.#rid, value._getPatternId());
      this.#fillStyle = value;
    }
  }

  get strokeStyle() {
    return this.#strokeStyle;
  }

  set strokeStyle(value) {
    if (typeof value === "string") {
      try {
        op_canvas_set_stroke_style(this.#rid, value);
        this.#strokeStyle = value;
      } catch (e) {
        // Ignore invalid colors
      }
    } else if (value instanceof CanvasGradient) {
      op_canvas_set_stroke_style_gradient(this.#rid, value._getGradientId());
      this.#strokeStyle = value;
    } else if (value instanceof CanvasPattern) {
      op_canvas_set_stroke_style_pattern(this.#rid, value._getPatternId());
      this.#strokeStyle = value;
    }
  }

  get lineWidth() {
    return this.#lineWidth;
  }

  set lineWidth(value) {
    if (!Number.isFinite(value) || value <= 0) return;
    op_canvas_set_line_width(this.#rid, value);
    this.#lineWidth = value;
  }

  get lineCap() {
    return this.#lineCap;
  }

  set lineCap(value) {
    op_canvas_set_line_cap(this.#rid, value);
    this.#lineCap = value;
  }

  get lineJoin() {
    return this.#lineJoin;
  }

  set lineJoin(value) {
    op_canvas_set_line_join(this.#rid, value);
    this.#lineJoin = value;
  }

  get miterLimit() {
    return this.#miterLimit;
  }

  set miterLimit(value) {
    if (!Number.isFinite(value) || value <= 0) return;
    op_canvas_set_miter_limit(this.#rid, value);
    this.#miterLimit = value;
  }

  get globalAlpha() {
    return this.#globalAlpha;
  }

  set globalAlpha(value) {
    if (!Number.isFinite(value) || value < 0 || value > 1) return;
    op_canvas_set_global_alpha(this.#rid, value);
    this.#globalAlpha = value;
  }

  get globalCompositeOperation() {
    return this.#globalCompositeOperation;
  }

  set globalCompositeOperation(value) {
    if (op_canvas_set_global_composite_operation(this.#rid, value)) {
      this.#globalCompositeOperation = value;
    }
  }

  get font() {
    return this.#font;
  }

  set font(value) {
    try {
      op_canvas_set_font(this.#rid, value);
      this.#font = value;
      // Refresh fontStretch from Rust since the font shorthand may include a stretch keyword
      this.#fontStretch = op_canvas_get_font_stretch(this.#rid);
    } catch (e) {
      // Ignore invalid fonts
    }
  }

  get textAlign() {
    return this.#textAlign;
  }

  set textAlign(value) {
    op_canvas_set_text_align(this.#rid, value);
    this.#textAlign = value;
  }

  get textBaseline() {
    return this.#textBaseline;
  }

  set textBaseline(value) {
    op_canvas_set_text_baseline(this.#rid, value);
    this.#textBaseline = value;
  }

  get fontStretch() {
    return this.#fontStretch;
  }

  set fontStretch(value) {
    const valid = [
      "ultra-condensed", "extra-condensed", "condensed", "semi-condensed",
      "normal", "semi-expanded", "expanded", "extra-expanded", "ultra-expanded"
    ];
    if (!valid.includes(value)) return; // Ignore invalid values per spec
    op_canvas_set_font_stretch(this.#rid, value);
    this.#fontStretch = value;
  }

  save() {
    op_canvas_save(this.#rid);
  }

  restore() {
    op_canvas_restore(this.#rid);
  }

  reset() {
    op_canvas_reset(this.#rid);
    // Reset local state tracking to defaults
    this.#fillStyle = "#000000";
    this.#strokeStyle = "#000000";
    this.#lineWidth = 1;
    this.#lineCap = "butt";
    this.#lineJoin = "miter";
    this.#miterLimit = 10;
    this.#globalAlpha = 1;
    this.#globalCompositeOperation = "source-over";
    this.#font = "10px sans-serif";
    this.#textAlign = "start";
    this.#textBaseline = "alphabetic";
    this.#lineDash = [];
    this.#lineDashOffset = 0;
    this.#imageSmoothingEnabled = true;
    this.#imageSmoothingQuality = "low";
    this.#fontStretch = "normal";
    this.#letterSpacing = "0px";
  }

  translate(x, y) {
    op_canvas_translate(this.#rid, x, y);
  }

  rotate(angle) {
    op_canvas_rotate(this.#rid, angle);
  }

  scale(x, y) {
    op_canvas_scale(this.#rid, x, y);
  }

  transform(a, b, c, d, e, f) {
    op_canvas_transform(this.#rid, a, b, c, d, e, f);
  }

  setTransform(a, b, c, d, e, f) {
    if (typeof a === "object") {
      // DOMMatrix form
      op_canvas_set_transform(this.#rid, a.a, a.b, a.c, a.d, a.e, a.f);
    } else {
      op_canvas_set_transform(this.#rid, a, b, c, d, e, f);
    }
  }

  resetTransform() {
    op_canvas_reset_transform(this.#rid);
  }

  getTransform() {
    const [a, b, c, d, e, f] = op_canvas_get_transform(this.#rid);
    // Return a DOMMatrix-like object
    return { a, b, c, d, e, f, is2D: true, isIdentity: (a === 1 && b === 0 && c === 0 && d === 1 && e === 0 && f === 0) };
  }

  beginPath() {
    op_canvas_begin_path(this.#rid);
  }

  moveTo(x, y) {
    op_canvas_move_to(this.#rid, x, y);
  }

  lineTo(x, y) {
    op_canvas_line_to(this.#rid, x, y);
  }

  closePath() {
    op_canvas_close_path(this.#rid);
  }

  bezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y) {
    op_canvas_bezier_curve_to(this.#rid, cp1x, cp1y, cp2x, cp2y, x, y);
  }

  quadraticCurveTo(cpx, cpy, x, y) {
    op_canvas_quadratic_curve_to(this.#rid, cpx, cpy, x, y);
  }

  rect(x, y, width, height) {
    op_canvas_rect(this.#rid, x, y, width, height);
  }

  arc(x, y, radius, startAngle, endAngle, anticlockwise = false) {
    op_canvas_arc(this.#rid, x, y, radius, startAngle, endAngle, anticlockwise);
  }

  arcTo(x1, y1, x2, y2, radius) {
    op_canvas_arc_to(this.#rid, x1, y1, x2, y2, radius);
  }

  ellipse(x, y, radiusX, radiusY, rotation, startAngle, endAngle, anticlockwise = false) {
    op_canvas_ellipse(this.#rid, x, y, radiusX, radiusY, rotation, startAngle, endAngle, anticlockwise);
  }

  roundRect(x, y, width, height, radii = 0) {
    if (typeof radii === "number") {
      op_canvas_round_rect(this.#rid, x, y, width, height, radii);
    } else if (Array.isArray(radii)) {
      // Handle DOMPointInit objects or numbers in array - produce [x, y] pairs
      const xyRadii = radii.map(r => {
        if (typeof r === "number") return [r, r];
        return [r.x || 0, r.y || 0];
      });
      op_canvas_round_rect_radii(this.#rid, x, y, width, height, xyRadii);
    }
  }

  fill(pathOrFillRule, fillRule) {
    if (pathOrFillRule instanceof Path2D) {
      // fill(path) or fill(path, fillRule)
      if (fillRule) {
        op_canvas_fill_path2d_with_rule(this.#rid, pathOrFillRule._getPathId(), fillRule);
      } else {
        op_canvas_fill_path2d(this.#rid, pathOrFillRule._getPathId());
      }
    } else if (typeof pathOrFillRule === "string") {
      // fill(fillRule)
      op_canvas_fill_with_rule(this.#rid, pathOrFillRule);
    } else {
      // fill()
      op_canvas_fill(this.#rid);
    }
  }

  stroke(path) {
    if (path instanceof Path2D) {
      op_canvas_stroke_path2d(this.#rid, path._getPathId());
    } else {
      op_canvas_stroke(this.#rid);
    }
  }

  fillRect(x, y, width, height) {
    op_canvas_fill_rect(this.#rid, x, y, width, height);
  }

  strokeRect(x, y, width, height) {
    op_canvas_stroke_rect(this.#rid, x, y, width, height);
  }

  clearRect(x, y, width, height) {
    op_canvas_clear_rect(this.#rid, x, y, width, height);
  }

  clip(pathOrFillRule, fillRule) {
    if (pathOrFillRule instanceof Path2D) {
      // clip(path) or clip(path, fillRule)
      if (fillRule) {
        op_canvas_clip_path2d_with_rule(this.#rid, pathOrFillRule._getPathId(), fillRule);
      } else {
        op_canvas_clip_path2d(this.#rid, pathOrFillRule._getPathId());
      }
    } else if (typeof pathOrFillRule === "string") {
      // clip(fillRule)
      op_canvas_clip_with_rule(this.#rid, pathOrFillRule);
    } else {
      // clip()
      op_canvas_clip(this.#rid);
    }
  }

  measureText(text) {
    const width = op_canvas_measure_text(this.#rid, String(text));
    return new TextMetrics(width);
  }

  fillText(text, x, y, maxWidth) {
    if (maxWidth !== undefined) {
      op_canvas_fill_text_max_width(this.#rid, String(text), x, y, maxWidth);
    } else {
      op_canvas_fill_text(this.#rid, String(text), x, y);
    }
  }

  strokeText(text, x, y, maxWidth) {
    if (maxWidth !== undefined) {
      op_canvas_stroke_text_max_width(this.#rid, String(text), x, y, maxWidth);
    } else {
      op_canvas_stroke_text(this.#rid, String(text), x, y);
    }
  }

  setLineDash(segments) {
    // Ignore if any value is non-finite or negative
    for (const v of segments) {
      if (!Number.isFinite(v) || v < 0) return;
    }
    // Duplicate odd-length arrays per spec
    let normalized = segments;
    if (segments.length % 2 !== 0) {
      normalized = [...segments, ...segments];
    }
    op_canvas_set_line_dash(this.#rid, normalized);
    this.#lineDash = normalized;
  }

  getLineDash() {
    return this.#lineDash.slice();
  }

  get lineDashOffset() {
    return this.#lineDashOffset;
  }

  set lineDashOffset(value) {
    if (!Number.isFinite(value)) return;
    op_canvas_set_line_dash_offset(this.#rid, value);
    this.#lineDashOffset = value;
  }

  get imageSmoothingEnabled() {
    return this.#imageSmoothingEnabled;
  }

  set imageSmoothingEnabled(value) {
    op_canvas_set_image_smoothing_enabled(this.#rid, !!value);
    this.#imageSmoothingEnabled = !!value;
  }

  get imageSmoothingQuality() {
    return this.#imageSmoothingQuality;
  }

  set imageSmoothingQuality(value) {
    if (value === "low" || value === "medium" || value === "high") {
      op_canvas_set_image_smoothing_quality(this.#rid, value);
      this.#imageSmoothingQuality = value;
    }
  }

  get letterSpacing() {
    return this.#letterSpacing;
  }

  set letterSpacing(value) {
    // Parse CSS length value (e.g., "2px", "0.5em")
    // For simplicity, we only support px values
    const match = String(value).match(/^(-?\d*\.?\d+)px$/);
    if (match) {
      const spacing = parseFloat(match[1]);
      op_canvas_set_letter_spacing(this.#rid, spacing);
      this.#letterSpacing = value;
    } else if (value === "normal" || value === "0") {
      op_canvas_set_letter_spacing(this.#rid, 0);
      this.#letterSpacing = "0px";
    }
  }

  getImageData(x, y, width, height, settings) {
    if (settings) validateImageDataSettings(settings);
    const data = op_canvas_get_image_data(this.#rid, x, y, width, height);
    return new ImageData(new Uint8ClampedArray(data), width, height);
  }

  createImageData(widthOrImageData, height, settings) {
    if (typeof widthOrImageData === "object") {
      // createImageData(imagedata[, settings])
      if (height) validateImageDataSettings(height);
      return new ImageData(
        new Uint8ClampedArray(widthOrImageData.width * widthOrImageData.height * 4),
        widthOrImageData.width,
        widthOrImageData.height
      );
    }
    // createImageData(width, height[, settings])
    if (settings) validateImageDataSettings(settings);
    return new ImageData(
      new Uint8ClampedArray(widthOrImageData * height * 4),
      widthOrImageData,
      height
    );
  }

  putImageData(imageData, dx, dy, dirtyX, dirtyY, dirtyWidth, dirtyHeight) {
    const data = imageData.data;
    const width = imageData.width;
    const height = imageData.height;

    if (dirtyX !== undefined) {
      // putImageData with dirty rect
      op_canvas_put_image_data_dirty(
        this.#rid, data, width, height, dx, dy,
        dirtyX, dirtyY, dirtyWidth, dirtyHeight
      );
    } else {
      // Simple putImageData
      op_canvas_put_image_data(this.#rid, data, width, height, dx, dy);
    }
  }

  createLinearGradient(x0, y0, x1, y1) {
    const gradientId = op_canvas_create_linear_gradient(this.#rid, x0, y0, x1, y1);
    return new CanvasGradient(this.#rid, gradientId);
  }

  createRadialGradient(x0, y0, r0, x1, y1, r1) {
    const gradientId = op_canvas_create_radial_gradient(this.#rid, x0, y0, r0, x1, y1, r1);
    return new CanvasGradient(this.#rid, gradientId);
  }

  createPattern(image, repetition) {
    // Normalize repetition parameter
    if (repetition === null || repetition === undefined || repetition === "") {
      repetition = "repeat";
    }

    const HTMLCanvasElement = getHTMLCanvasElementClass();
    if (HTMLCanvasElement && image instanceof HTMLCanvasElement) {
      // Create pattern from canvas
      const sourceCtx = image.getContext("2d");
      if (!sourceCtx) return null;
      const sourceRid = sourceCtx._getRid();
      const patternId = op_canvas_create_pattern_from_canvas(this.#rid, sourceRid, repetition);
      return new CanvasPattern(this.#rid, patternId);
    } else if (image instanceof ImageData || (image && image.data && image.width && image.height)) {
      // Create pattern from ImageData
      const patternId = op_canvas_create_pattern(this.#rid, image.data, image.width, image.height, repetition);
      return new CanvasPattern(this.#rid, patternId);
    }

    // Unsupported image type
    return null;
  }

  drawImage(source, ...args) {
    const HTMLCanvasElement = getHTMLCanvasElementClass();

    // Handle different source types
    if (HTMLCanvasElement && source instanceof HTMLCanvasElement) {
      const sourceCtx = source.getContext("2d");
      if (!sourceCtx) return;
      const sourceRid = sourceCtx._getRid();

      if (args.length === 2) {
        // drawImage(canvas, dx, dy)
        const [dx, dy] = args;
        op_canvas_draw_canvas(this.#rid, sourceRid, dx, dy);
      } else if (args.length === 4) {
        // drawImage(canvas, dx, dy, dw, dh)
        const [dx, dy, dw, dh] = args;
        op_canvas_draw_canvas_scaled(this.#rid, sourceRid, dx, dy, dw, dh);
      } else if (args.length === 8) {
        // drawImage(canvas, sx, sy, sw, sh, dx, dy, dw, dh)
        const [sx, sy, sw, sh, dx, dy, dw, dh] = args;
        op_canvas_draw_canvas_cropped(this.#rid, sourceRid, sx, sy, sw, sh, dx, dy, dw, dh);
      }
    } else if (source instanceof Image) {
      // Image (HTMLImageElement) source
      if (source._isSvg && source._rawBytes) {
        // SVG image - decode at target size for quality
        // Determine target dimensions
        let targetWidth, targetHeight;
        if (args.length === 2) {
          // drawImage(image, dx, dy) - use natural size
          targetWidth = source.naturalWidth;
          targetHeight = source.naturalHeight;
        } else if (args.length >= 4) {
          // drawImage(image, dx, dy, dw, dh) - use specified size
          targetWidth = Math.ceil(args[2]);
          targetHeight = Math.ceil(args[3]);
        }

        // Decode SVG at target size (with 2x supersampling done in Rust)
        const decoded = op_canvas_decode_svg_at_size(source._rawBytes, targetWidth, targetHeight);
        const pixelData = decoded.data instanceof Uint8Array
          ? decoded.data
          : new Uint8Array(decoded.data);

        if (args.length === 2) {
          const [dx, dy] = args;
          // decoded is at 2x size, scale down when drawing
          op_canvas_draw_image_scaled(this.#rid, pixelData, decoded.width, decoded.height, dx, dy, targetWidth, targetHeight);
        } else if (args.length === 4) {
          const [dx, dy, dw, dh] = args;
          op_canvas_draw_image_scaled(this.#rid, pixelData, decoded.width, decoded.height, dx, dy, dw, dh);
        } else if (args.length === 8) {
          // For cropped drawing, scale source coordinates to 2x
          const [sx, sy, sw, sh, dx, dy, dw, dh] = args;
          op_canvas_draw_image_cropped(this.#rid, pixelData, decoded.width, decoded.height, sx * 2, sy * 2, sw * 2, sh * 2, dx, dy, dw, dh);
        }
      } else {
        // Raster image - use pre-decoded image data
        const imageData = source._imageData;
        if (!imageData) {
          return;
        }
        const data = imageData.data;
        const imgWidth = imageData.width;
        const imgHeight = imageData.height;

        if (args.length === 2) {
          // drawImage(image, dx, dy)
          const [dx, dy] = args;
          op_canvas_draw_image(this.#rid, data, imgWidth, imgHeight, dx, dy);
        } else if (args.length === 4) {
          // drawImage(image, dx, dy, dw, dh)
          const [dx, dy, dw, dh] = args;
          op_canvas_draw_image_scaled(this.#rid, data, imgWidth, imgHeight, dx, dy, dw, dh);
        } else if (args.length === 8) {
          // drawImage(image, sx, sy, sw, sh, dx, dy, dw, dh)
          const [sx, sy, sw, sh, dx, dy, dw, dh] = args;
          op_canvas_draw_image_cropped(this.#rid, data, imgWidth, imgHeight, sx, sy, sw, sh, dx, dy, dw, dh);
        }
      }
    } else if (source instanceof ImageData || (source && source.data && source.width && source.height)) {
      // ImageData source - extract RGBA data
      const data = source.data;
      const imgWidth = source.width;
      const imgHeight = source.height;

      if (args.length === 2) {
        // drawImage(imageData, dx, dy)
        const [dx, dy] = args;
        op_canvas_draw_image(this.#rid, data, imgWidth, imgHeight, dx, dy);
      } else if (args.length === 4) {
        // drawImage(imageData, dx, dy, dw, dh)
        const [dx, dy, dw, dh] = args;
        op_canvas_draw_image_scaled(this.#rid, data, imgWidth, imgHeight, dx, dy, dw, dh);
      } else if (args.length === 8) {
        // drawImage(imageData, sx, sy, sw, sh, dx, dy, dw, dh)
        const [sx, sy, sw, sh, dx, dy, dw, dh] = args;
        op_canvas_draw_image_cropped(this.#rid, data, imgWidth, imgHeight, sx, sy, sw, sh, dx, dy, dw, dh);
      }
    }
    // Other source types are not supported in this polyfill
  }

  isPointInPath() {
    unsupported("CanvasRenderingContext2D.isPointInPath");
  }

  isPointInStroke() {
    unsupported("CanvasRenderingContext2D.isPointInStroke");
  }
}

// Lazy reference to HTMLCanvasElement to break circular dependency.
// context_polyfill needs HTMLCanvasElement for instanceof checks in drawImage/createPattern,
// and canvas_element_polyfill needs CanvasRenderingContext2D for getContext.
// The orchestrator (canvas_polyfill.js) calls registerCanvasElementClass after both modules load.
let _HTMLCanvasElement = null;

function getHTMLCanvasElementClass() {
  return _HTMLCanvasElement;
}

function registerCanvasElementClass(cls) {
  _HTMLCanvasElement = cls;
}

export { CanvasRenderingContext2D, TextMetrics, CanvasGradient, CanvasPattern, registerCanvasElementClass };

// Canvas 2D polyfill for vl-convert
// Provides HTMLCanvasElement and CanvasRenderingContext2D that use Rust ops

// Debug logging - set to true to trace all canvas calls
const DEBUG_CANVAS = false;
function log(...args) {
  if (DEBUG_CANVAS) console.log('[canvas]', ...args);
}

import {
  op_canvas_create,
  op_canvas_destroy,
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
  op_canvas_to_png,
  op_canvas_width,
  op_canvas_height,
  // Phase 1: Gradients
  op_canvas_create_linear_gradient,
  op_canvas_create_radial_gradient,
  op_canvas_gradient_add_color_stop,
  op_canvas_set_fill_style_gradient,
  op_canvas_set_stroke_style_gradient,
  // Phase 1: Text with maxWidth
  op_canvas_fill_text_max_width,
  op_canvas_stroke_text_max_width,
  // Phase 1: drawImage
  op_canvas_draw_image,
  op_canvas_draw_image_scaled,
  op_canvas_draw_image_cropped,
  op_canvas_draw_canvas,
  op_canvas_draw_canvas_scaled,
  op_canvas_draw_canvas_cropped,
  // Phase 2: Patterns
  op_canvas_create_pattern,
  op_canvas_create_pattern_from_canvas,
  op_canvas_set_fill_style_pattern,
  op_canvas_set_stroke_style_pattern,
  // Phase 2: putImageData
  op_canvas_put_image_data,
  op_canvas_put_image_data_dirty,
  // Phase 2: imageSmoothingEnabled/Quality
  op_canvas_set_image_smoothing_enabled,
  op_canvas_get_image_smoothing_enabled,
  op_canvas_set_image_smoothing_quality,
  op_canvas_get_image_smoothing_quality,
  // Phase 2: fillRule support
  op_canvas_fill_with_rule,
  op_canvas_clip_with_rule,
  // Phase 2: Path2D
  op_path2d_create,
  op_path2d_create_from_svg,
  op_path2d_create_from_path,
  op_path2d_destroy,
  op_path2d_move_to,
  op_path2d_line_to,
  op_path2d_close_path,
  op_path2d_bezier_curve_to,
  op_path2d_quadratic_curve_to,
  op_path2d_rect,
  op_path2d_arc,
  op_path2d_arc_to,
  op_path2d_ellipse,
  op_canvas_fill_path2d,
  op_canvas_fill_path2d_with_rule,
  op_canvas_stroke_path2d,
  op_canvas_clip_path2d,
  op_canvas_clip_path2d_with_rule,
  // Phase 3: Nice-to-Have
  op_canvas_reset,
  op_canvas_round_rect,
  op_canvas_round_rect_radii,
  op_canvas_set_letter_spacing,
  op_canvas_get_letter_spacing,
  op_canvas_get_transform,
  op_path2d_round_rect,
  op_path2d_round_rect_radii,
  // Image decoding
  op_canvas_decode_image,
} from "ext:core/ops";

/**
 * ImageData class for getImageData results
 */
class ImageData {
  constructor(data, width, height) {
    this.data = data;
    this.width = width;
    this.height = height;
    this.colorSpace = "srgb";
  }
}

/**
 * Image (HTMLImageElement) polyfill for loading remote images
 * Used by Vega's ResourceLoader to load images for image marks
 */
class Image {
  #src = "";
  #width = 0;
  #height = 0;
  #complete = false;
  #imageData = null;

  constructor(width, height) {
    if (width !== undefined) this.#width = width;
    if (height !== undefined) this.#height = height;
    this.crossOrigin = null;
    this.onload = null;
    this.onerror = null;
  }

  get src() {
    return this.#src;
  }

  set src(url) {
    this.#src = url;
    this.#complete = false;
    this.#loadImage(url);
  }

  get width() {
    return this.#width;
  }

  set width(value) {
    this.#width = value;
  }

  get height() {
    return this.#height;
  }

  set height(value) {
    this.#height = value;
  }

  get complete() {
    return this.#complete;
  }

  get naturalWidth() {
    return this.#width;
  }

  get naturalHeight() {
    return this.#height;
  }

  // Internal: get decoded image data for drawImage
  get _imageData() {
    return this.#imageData;
  }

  async #loadImage(url) {
    log('Image loading:', url);
    try {
      // Fetch the image
      const response = await fetch(url);
      if (!response.ok) {
        throw new Error(`Failed to fetch image: ${response.status}`);
      }

      // Get image data as ArrayBuffer
      const arrayBuffer = await response.arrayBuffer();
      const bytes = new Uint8Array(arrayBuffer);

      // Decode the image to get RGBA pixel data
      // We need a Deno op for this - for now use a simple decode op
      const decoded = op_canvas_decode_image(bytes);

      this.#width = decoded.width;
      this.#height = decoded.height;
      // Convert data to Uint8Array (it comes back as a regular Array from serde)
      const pixelData = decoded.data instanceof Uint8Array
        ? decoded.data
        : new Uint8Array(decoded.data);
      this.#imageData = {
        data: pixelData,
        width: decoded.width,
        height: decoded.height,
      };
      this.#complete = true;

      log('Image loaded:', url, this.#width, 'x', this.#height);

      // Call onload callback
      if (this.onload) {
        this.onload();
      }
    } catch (error) {
      log('Image error:', url, error);
      this.#complete = false;
      if (this.onerror) {
        this.onerror(error);
      }
    }
  }
}

// Alias for HTMLImageElement
const HTMLImageElement = Image;

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
 * Path2D class for reusable path objects
 */
class Path2D {
  #pathId;

  constructor(pathOrString) {
    if (pathOrString === undefined) {
      // Create empty path
      this.#pathId = op_path2d_create();
    } else if (typeof pathOrString === "string") {
      // Create from SVG path data
      this.#pathId = op_path2d_create_from_svg(pathOrString);
    } else if (pathOrString instanceof Path2D) {
      // Copy from another Path2D
      this.#pathId = op_path2d_create_from_path(pathOrString._getPathId());
    } else {
      // Unknown type, create empty
      this.#pathId = op_path2d_create();
    }
  }

  _getPathId() {
    return this.#pathId;
  }

  moveTo(x, y) {
    op_path2d_move_to(this.#pathId, x, y);
  }

  lineTo(x, y) {
    op_path2d_line_to(this.#pathId, x, y);
  }

  closePath() {
    op_path2d_close_path(this.#pathId);
  }

  bezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y) {
    op_path2d_bezier_curve_to(this.#pathId, cp1x, cp1y, cp2x, cp2y, x, y);
  }

  quadraticCurveTo(cpx, cpy, x, y) {
    op_path2d_quadratic_curve_to(this.#pathId, cpx, cpy, x, y);
  }

  rect(x, y, width, height) {
    op_path2d_rect(this.#pathId, x, y, width, height);
  }

  arc(x, y, radius, startAngle, endAngle, anticlockwise = false) {
    op_path2d_arc(this.#pathId, x, y, radius, startAngle, endAngle, anticlockwise);
  }

  arcTo(x1, y1, x2, y2, radius) {
    op_path2d_arc_to(this.#pathId, x1, y1, x2, y2, radius);
  }

  ellipse(x, y, radiusX, radiusY, rotation, startAngle, endAngle, anticlockwise = false) {
    op_path2d_ellipse(this.#pathId, x, y, radiusX, radiusY, rotation, startAngle, endAngle, anticlockwise);
  }

  roundRect(x, y, width, height, radii = 0) {
    if (typeof radii === "number") {
      op_path2d_round_rect(this.#pathId, x, y, width, height, radii);
    } else if (Array.isArray(radii)) {
      // Handle DOMPointInit objects or numbers in array
      const numericRadii = radii.map(r => typeof r === "object" ? r.x || 0 : r);
      op_path2d_round_rect_radii(this.#pathId, x, y, width, height, numericRadii);
    }
  }

  addPath(path, transform) {
    // addPath with transform is not currently supported
    // This is a no-op for API compatibility
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

  // --- Style properties ---

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
    op_canvas_set_miter_limit(this.#rid, value);
    this.#miterLimit = value;
  }

  get globalAlpha() {
    return this.#globalAlpha;
  }

  set globalAlpha(value) {
    op_canvas_set_global_alpha(this.#rid, value);
    this.#globalAlpha = value;
  }

  get globalCompositeOperation() {
    return this.#globalCompositeOperation;
  }

  set globalCompositeOperation(value) {
    op_canvas_set_global_composite_operation(this.#rid, value);
    this.#globalCompositeOperation = value;
  }

  get font() {
    return this.#font;
  }

  set font(value) {
    try {
      op_canvas_set_font(this.#rid, value);
      this.#font = value;
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

  // --- State ---

  save() {
    log('save');
    op_canvas_save(this.#rid);
  }

  restore() {
    log('restore');
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
    this.#letterSpacing = "0px";
  }

  // --- Transforms ---

  translate(x, y) {
    log('translate', x, y);
    op_canvas_translate(this.#rid, x, y);
  }

  rotate(angle) {
    log('rotate', angle);
    op_canvas_rotate(this.#rid, angle);
  }

  scale(x, y) {
    log('scale', x, y);
    op_canvas_scale(this.#rid, x, y);
  }

  transform(a, b, c, d, e, f) {
    log('transform', a, b, c, d, e, f);
    op_canvas_transform(this.#rid, a, b, c, d, e, f);
  }

  setTransform(a, b, c, d, e, f) {
    if (typeof a === "object") {
      // DOMMatrix form
      log('setTransform (DOMMatrix)', a.a, a.b, a.c, a.d, a.e, a.f);
      op_canvas_set_transform(this.#rid, a.a, a.b, a.c, a.d, a.e, a.f);
    } else {
      log('setTransform', a, b, c, d, e, f);
      op_canvas_set_transform(this.#rid, a, b, c, d, e, f);
    }
  }

  resetTransform() {
    log('resetTransform');
    op_canvas_reset_transform(this.#rid);
  }

  getTransform() {
    const [a, b, c, d, e, f] = op_canvas_get_transform(this.#rid);
    // Return a DOMMatrix-like object
    return { a, b, c, d, e, f, is2D: true, isIdentity: (a === 1 && b === 0 && c === 0 && d === 1 && e === 0 && f === 0) };
  }

  // --- Paths ---

  beginPath() {
    log('beginPath');
    op_canvas_begin_path(this.#rid);
  }

  moveTo(x, y) {
    log('moveTo', x, y);
    op_canvas_move_to(this.#rid, x, y);
  }

  lineTo(x, y) {
    log('lineTo', x, y);
    op_canvas_line_to(this.#rid, x, y);
  }

  closePath() {
    log('closePath');
    op_canvas_close_path(this.#rid);
  }

  bezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y) {
    log('bezierCurveTo', cp1x, cp1y, cp2x, cp2y, x, y);
    op_canvas_bezier_curve_to(this.#rid, cp1x, cp1y, cp2x, cp2y, x, y);
  }

  quadraticCurveTo(cpx, cpy, x, y) {
    log('quadraticCurveTo', cpx, cpy, x, y);
    op_canvas_quadratic_curve_to(this.#rid, cpx, cpy, x, y);
  }

  rect(x, y, width, height) {
    log('rect', x, y, width, height);
    op_canvas_rect(this.#rid, x, y, width, height);
  }

  arc(x, y, radius, startAngle, endAngle, anticlockwise = false) {
    log('arc', x, y, radius, startAngle, endAngle, anticlockwise);
    op_canvas_arc(this.#rid, x, y, radius, startAngle, endAngle, anticlockwise);
  }

  arcTo(x1, y1, x2, y2, radius) {
    log('arcTo', x1, y1, x2, y2, radius);
    op_canvas_arc_to(this.#rid, x1, y1, x2, y2, radius);
  }

  ellipse(x, y, radiusX, radiusY, rotation, startAngle, endAngle, anticlockwise = false) {
    log('ellipse', x, y, radiusX, radiusY, rotation, startAngle, endAngle, anticlockwise);
    op_canvas_ellipse(this.#rid, x, y, radiusX, radiusY, rotation, startAngle, endAngle, anticlockwise);
  }

  roundRect(x, y, width, height, radii = 0) {
    log('roundRect', x, y, width, height, radii);
    if (typeof radii === "number") {
      op_canvas_round_rect(this.#rid, x, y, width, height, radii);
    } else if (Array.isArray(radii)) {
      // Handle DOMPointInit objects or numbers in array
      const numericRadii = radii.map(r => typeof r === "object" ? r.x || 0 : r);
      op_canvas_round_rect_radii(this.#rid, x, y, width, height, numericRadii);
    }
  }

  // --- Drawing ---

  fill(pathOrFillRule, fillRule) {
    log('fill', pathOrFillRule instanceof Path2D ? 'Path2D' : pathOrFillRule, fillRule);
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
    log('stroke', path instanceof Path2D ? 'Path2D' : path);
    if (path instanceof Path2D) {
      op_canvas_stroke_path2d(this.#rid, path._getPathId());
    } else {
      op_canvas_stroke(this.#rid);
    }
  }

  fillRect(x, y, width, height) {
    log('fillRect', x, y, width, height);
    op_canvas_fill_rect(this.#rid, x, y, width, height);
  }

  strokeRect(x, y, width, height) {
    log('strokeRect', x, y, width, height);
    op_canvas_stroke_rect(this.#rid, x, y, width, height);
  }

  clearRect(x, y, width, height) {
    log('clearRect', x, y, width, height);
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

  // --- Text ---

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

  // --- Line dash ---

  setLineDash(segments) {
    op_canvas_set_line_dash(this.#rid, segments);
    this.#lineDash = segments;
  }

  getLineDash() {
    return this.#lineDash.slice();
  }

  get lineDashOffset() {
    return this.#lineDashOffset;
  }

  set lineDashOffset(value) {
    op_canvas_set_line_dash_offset(this.#rid, value);
    this.#lineDashOffset = value;
  }

  // --- Image smoothing ---

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

  // --- Letter spacing ---

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

  // --- Image data ---

  getImageData(x, y, width, height) {
    const data = op_canvas_get_image_data(this.#rid, x, y, width, height);
    return new ImageData(new Uint8ClampedArray(data), width, height);
  }

  createImageData(width, height) {
    if (typeof width === "object") {
      // ImageData form
      return new ImageData(
        new Uint8ClampedArray(width.width * width.height * 4),
        width.width,
        width.height
      );
    }
    return new ImageData(
      new Uint8ClampedArray(width * height * 4),
      width,
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

  // --- Gradients ---

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

    if (image instanceof HTMLCanvasElement) {
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

  // --- drawImage ---

  drawImage(source, ...args) {
    log('drawImage', source?.constructor?.name, source, args);
    // Handle different source types
    if (source instanceof HTMLCanvasElement) {
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
      // Image (HTMLImageElement) source - use decoded image data
      log('drawImage: Image instance, complete:', source.complete, 'src:', source.src);
      const imageData = source._imageData;
      log('drawImage: imageData:', imageData ? `${imageData.width}x${imageData.height}` : 'null');
      if (!imageData) {
        log('drawImage: Image not loaded yet');
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

  // --- Other stubs ---
  isPointInPath() { return false; }
  isPointInStroke() { return false; }
}

/**
 * HTMLCanvasElement polyfill
 */
class HTMLCanvasElement {
  #width;
  #height;
  #rid;
  #context;

  constructor(width = 300, height = 150) {
    this.#width = width;
    this.#height = height;
    this.#rid = null;
    this.#context = null;
  }

  // Internal getter for resource ID (used by Rust ops)
  get _rid() {
    return this.#rid;
  }

  get width() {
    return this.#width;
  }

  set width(value) {
    this.#width = value;
    // Recreate context if it exists
    if (this.#rid !== null) {
      op_canvas_destroy(this.#rid);
      this.#rid = op_canvas_create(this.#width, this.#height);
      if (this.#context) {
        // Update the existing context's resource ID instead of creating a new context
        this.#context._setRid(this.#rid);
      }
    }
  }

  get height() {
    return this.#height;
  }

  set height(value) {
    this.#height = value;
    // Recreate context if it exists
    if (this.#rid !== null) {
      op_canvas_destroy(this.#rid);
      this.#rid = op_canvas_create(this.#width, this.#height);
      if (this.#context) {
        // Update the existing context's resource ID instead of creating a new context
        this.#context._setRid(this.#rid);
      }
    }
  }

  getContext(contextType, contextAttributes) {
    if (contextType !== "2d") {
      return null;
    }

    if (this.#context === null) {
      this.#rid = op_canvas_create(this.#width, this.#height);
      this.#context = new CanvasRenderingContext2D(this.#rid, this);
    }

    return this.#context;
  }

  toDataURL(type = "image/png", quality) {
    if (this.#rid === null) {
      // Empty canvas
      return "data:,";
    }
    const pngData = op_canvas_to_png(this.#rid, null);
    const base64 = btoa(String.fromCharCode(...pngData));
    return `data:image/png;base64,${base64}`;
  }

  toBlob(callback, type = "image/png", quality) {
    if (this.#rid === null) {
      callback(null);
      return;
    }
    const pngData = op_canvas_to_png(this.#rid, null);
    const blob = new Blob([new Uint8Array(pngData)], { type: "image/png" });
    callback(blob);
  }

  // Internal method to export as PNG with PPI metadata
  _toPngWithPpi(ppi) {
    if (this.#rid === null) {
      return null;
    }
    return op_canvas_to_png(this.#rid, ppi);
  }
}

/**
 * Factory function for creating canvases (used by vega-canvas)
 */
function createCanvas(width, height) {
  const canvas = new HTMLCanvasElement(width, height);
  return canvas;
}

// Export for module usage
export { HTMLCanvasElement, CanvasRenderingContext2D, ImageData, TextMetrics, CanvasGradient, CanvasPattern, Path2D, createCanvas, Image, HTMLImageElement };

// Install on globalThis
globalThis.HTMLCanvasElement = HTMLCanvasElement;
globalThis.CanvasRenderingContext2D = CanvasRenderingContext2D;
globalThis.ImageData = ImageData;
globalThis.CanvasGradient = CanvasGradient;
globalThis.CanvasPattern = CanvasPattern;
globalThis.Path2D = Path2D;
globalThis.Image = Image;
globalThis.HTMLImageElement = HTMLImageElement;

// Provide document.createElement for vega-canvas compatibility
if (typeof globalThis.document === "undefined") {
  globalThis.document = {};
}

const originalCreateElement = globalThis.document.createElement;
globalThis.document.createElement = function(tagName) {
  if (tagName.toLowerCase() === "canvas") {
    return new HTMLCanvasElement();
  }
  if (originalCreateElement) {
    return originalCreateElement.call(this, tagName);
  }
  return null;
};

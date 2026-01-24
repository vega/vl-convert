// Canvas 2D polyfill for vl-convert
// Provides HTMLCanvasElement and CanvasRenderingContext2D that use Rust ops

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

  constructor(rid, canvas) {
    this.#rid = rid;
    this.#canvas = canvas;
  }

  get canvas() {
    return this.#canvas;
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
    op_canvas_save(this.#rid);
  }

  restore() {
    op_canvas_restore(this.#rid);
  }

  // --- Transforms ---

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

  // --- Paths ---

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

  // --- Drawing ---

  fill() {
    op_canvas_fill(this.#rid);
  }

  stroke() {
    op_canvas_stroke(this.#rid);
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

  clip() {
    op_canvas_clip(this.#rid);
  }

  // --- Text ---

  measureText(text) {
    const width = op_canvas_measure_text(this.#rid, String(text));
    return new TextMetrics(width);
  }

  fillText(text, x, y, maxWidth) {
    op_canvas_fill_text(this.#rid, String(text), x, y);
  }

  strokeText(text, x, y, maxWidth) {
    op_canvas_stroke_text(this.#rid, String(text), x, y);
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

  putImageData(imageData, dx, dy) {
    // Not implemented - would need an op
  }

  // --- Gradients (stubs) ---

  createLinearGradient(x0, y0, x1, y1) {
    // Return a stub gradient object
    return {
      addColorStop(offset, color) {},
    };
  }

  createRadialGradient(x0, y0, r0, x1, y1, r1) {
    // Return a stub gradient object
    return {
      addColorStop(offset, color) {},
    };
  }

  createPattern(image, repetition) {
    // Return a stub pattern
    return {};
  }

  // --- Not implemented stubs ---

  drawImage() {}
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
        this.#context = new CanvasRenderingContext2D(this.#rid, this);
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
        this.#context = new CanvasRenderingContext2D(this.#rid, this);
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
    const pngData = op_canvas_to_png(this.#rid);
    const base64 = btoa(String.fromCharCode(...pngData));
    return `data:image/png;base64,${base64}`;
  }

  toBlob(callback, type = "image/png", quality) {
    if (this.#rid === null) {
      callback(null);
      return;
    }
    const pngData = op_canvas_to_png(this.#rid);
    const blob = new Blob([new Uint8Array(pngData)], { type: "image/png" });
    callback(blob);
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
export { HTMLCanvasElement, CanvasRenderingContext2D, ImageData, TextMetrics, createCanvas };

// Install on globalThis
globalThis.HTMLCanvasElement = HTMLCanvasElement;
globalThis.CanvasRenderingContext2D = CanvasRenderingContext2D;
globalThis.ImageData = ImageData;

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

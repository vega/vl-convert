// HTMLCanvasElement polyfill for vl-convert canvas

import {
  op_canvas_create,
  op_canvas_destroy,
  op_canvas_to_png,
} from "ext:core/ops";

import { uint8ArrayToBase64 } from "ext:vl_convert_canvas2d/image_polyfill.js";
import { CanvasRenderingContext2D } from "ext:vl_convert_canvas2d/context_polyfill.js";

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
    const base64 = uint8ArrayToBase64(pngData);
    return `data:image/png;base64,${base64}`;
  }

  toBlob(callback, type = "image/png", quality) {
    if (this.#rid === null) {
      callback(null);
      return;
    }
    const pngData = op_canvas_to_png(this.#rid, null);
    const blob = new Blob([pngData], { type: "image/png" });
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

export { HTMLCanvasElement, createCanvas };

// Bootstrap script for vl-convert runtime extension
// This sets up the Canvas 2D polyfill and text measurement ops

import { op_text_width, op_get_json_arg } from "ext:core/ops";

// Import and initialize the canvas polyfill from the separate canvas2d extension
// This will install HTMLCanvasElement, CanvasRenderingContext2D, and ImageData on globalThis
// It also patches document.createElement to return canvas elements
import "ext:vl_convert_canvas2d/canvas_polyfill.js";

// Expose text measurement ops on globalThis for vega-scenegraph
globalThis.op_text_width = op_text_width;
globalThis.op_get_json_arg = op_get_json_arg;

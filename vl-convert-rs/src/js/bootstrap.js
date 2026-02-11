// Bootstrap script for vl-convert runtime extension
// This sets up the Canvas 2D polyfill

import { op_get_json_arg, op_set_msgpack_result } from "ext:core/ops";

// Import and initialize the canvas polyfill from the separate canvas2d extension
// This will install HTMLCanvasElement, CanvasRenderingContext2D, and ImageData on globalThis
// It also patches document.createElement to return canvas elements
import "ext:vl_convert_canvas2d/canvas_polyfill.js";

// Expose ops on globalThis
globalThis.op_get_json_arg = op_get_json_arg;
globalThis.op_set_msgpack_result = op_set_msgpack_result;

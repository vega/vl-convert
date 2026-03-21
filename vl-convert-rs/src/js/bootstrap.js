// Bootstrap script for vl-convert runtime extension
// This sets up the Canvas 2D polyfill

import {
    op_get_json_arg,
    op_set_msgpack_result,
    op_vega_data_fetch,
    op_vega_data_fetch_bytes,
    op_vega_file_read,
    op_vega_file_read_bytes,
} from "ext:core/ops";

// Import and initialize the canvas polyfill from the separate canvas2d extension
// This will install HTMLCanvasElement, CanvasRenderingContext2D, and ImageData on globalThis
// It also patches document.createElement to return canvas elements
import "ext:vl_convert_canvas2d/canvas_polyfill.js";

// Expose ops on globalThis
globalThis.op_get_json_arg = op_get_json_arg;
globalThis.op_set_msgpack_result = op_set_msgpack_result;
globalThis.op_vega_data_fetch = op_vega_data_fetch;
globalThis.op_vega_data_fetch_bytes = op_vega_data_fetch_bytes;
globalThis.op_vega_file_read = op_vega_file_read;
globalThis.op_vega_file_read_bytes = op_vega_file_read_bytes;

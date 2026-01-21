// Copyright 2024 the vl-convert authors.
// ESM file that exposes vl-convert ops on globalThis

import {
  op_text_width,
  op_get_json_arg,
} from "ext:core/ops";

// Expose ops on globalThis so user code can access them
globalThis.vlConvertOps = {
  op_text_width,
  op_get_json_arg,
};

// Also expose as individual global variables for backwards compatibility
globalThis.op_text_width = op_text_width;
globalThis.op_get_json_arg = op_get_json_arg;

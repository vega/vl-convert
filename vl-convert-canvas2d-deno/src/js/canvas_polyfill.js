// Canvas 2D polyfill orchestrator for vl-convert
// Re-exports all polyfill classes and sets up globalThis assignments

import { Image, HTMLImageElement, ImageData } from "ext:vl_convert_canvas2d/image_polyfill.js";
import { Path2D } from "ext:vl_convert_canvas2d/path2d_polyfill.js";
import { CanvasRenderingContext2D, TextMetrics, CanvasGradient, CanvasPattern, registerCanvasElementClass } from "ext:vl_convert_canvas2d/context_polyfill.js";
import { HTMLCanvasElement, createCanvas } from "ext:vl_convert_canvas2d/canvas_element_polyfill.js";

// Wire up the lazy HTMLCanvasElement reference in context_polyfill to break the circular dependency
registerCanvasElementClass(HTMLCanvasElement);

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

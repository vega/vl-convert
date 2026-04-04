// Image and ImageData polyfills for vl-convert canvas

import {
  op_canvas_decode_image,
  op_canvas_get_image_info,
  op_canvas_decode_svg_at_size,
  op_vega_data_fetch_bytes,
  op_vega_file_read_bytes,
} from "ext:core/ops";

function unsupported(methodName) {
  throw new Error(`${methodName} is not supported by vl-convert canvas polyfill`);
}

function uint8ArrayToBase64(bytes) {
  const chunkSize = 0x8000;
  let binary = "";
  for (let i = 0; i < bytes.length; i += chunkSize) {
    const chunk = bytes.subarray(i, i + chunkSize);
    binary += String.fromCharCode.apply(null, chunk);
  }
  return btoa(binary);
}

/**
 * Validate ImageDataSettings. Only colorSpace "srgb" is supported.
 * Throws on unsupported values.
 */
function validateImageDataSettings(settings) {
  if (typeof settings !== "object" || settings === null) return;
  if (settings.colorSpace !== undefined && settings.colorSpace !== "srgb") {
    throw new DOMException(
      `Unsupported color space: ${settings.colorSpace}. Only "srgb" is supported.`,
      "InvalidStateError"
    );
  }
}

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
  #rawBytes = null;  // Store raw bytes for SVG images
  #isSvg = false;
  #requestId = 0;
  #loadPromise = null;
  #lastError = null;
  #eventListeners = new Map();

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
    const nextSrc = String(url);
    this.#src = nextSrc;
    this.#requestId += 1;
    const requestId = this.#requestId;

    this.#complete = false;
    this.#lastError = null;
    this.#width = 0;
    this.#height = 0;
    this.#imageData = null;
    this.#rawBytes = null;
    this.#isSvg = false;

    const loadPromise = this.#loadImage(nextSrc).then((result) => {
      if (requestId !== this.#requestId) {
        throw new Error("Image source changed during load");
      }
      if (result.ok) {
        this.#applyLoadSuccess(result);
        this.#emitEvent("load");
        return;
      }
      this.#applyLoadFailure(result.error);
      this.#emitEvent("error", result.error);
      throw result.error;
    });

    this.#loadPromise = loadPromise;
    this.#loadPromise.catch(() => {});
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

  // Internal: check if this is an SVG image
  get _isSvg() {
    return this.#isSvg;
  }

  // Internal: get raw bytes for SVG images
  get _rawBytes() {
    return this.#rawBytes;
  }

  // Internal: get decoded image data for drawImage (raster images only)
  get _imageData() {
    return this.#imageData;
  }

  addEventListener(type, listener) {
    if (type !== "load" && type !== "error") {
      return;
    }
    const isFunctionListener = typeof listener === "function";
    const isObjectListener =
      listener != null &&
      typeof listener === "object" &&
      typeof listener.handleEvent === "function";
    if (!isFunctionListener && !isObjectListener) {
      return;
    }
    if (!this.#eventListeners.has(type)) {
      this.#eventListeners.set(type, new Set());
    }
    this.#eventListeners.get(type).add(listener);
  }

  removeEventListener(type, listener) {
    const listeners = this.#eventListeners.get(type);
    if (!listeners) {
      return;
    }
    listeners.delete(listener);
  }

  dispatchEvent(event) {
    if (event == null || typeof event.type !== "string") {
      throw new TypeError("Failed to execute 'dispatchEvent': invalid event");
    }
    const type = event.type;
    if (type !== "load" && type !== "error") {
      return !event.defaultPrevented;
    }

    if (type === "load") {
      this.#callHandler(this.onload, event);
    } else {
      this.#callHandler(this.onerror, event);
    }

    const listeners = this.#eventListeners.get(type);
    if (listeners) {
      for (const listener of [...listeners]) {
        this.#callListener(listener, event);
      }
    }

    return !event.defaultPrevented;
  }

  decode() {
    if (this.#loadPromise != null) {
      return this.#loadPromise;
    }
    if (this.#lastError != null) {
      return Promise.reject(this.#lastError);
    }
    if (this.#complete && (this.#imageData != null || this.#rawBytes != null)) {
      return Promise.resolve();
    }
    return Promise.reject(new Error("Image is not loaded"));
  }

  #callHandler(handler, event) {
    if (typeof handler !== "function") {
      return;
    }
    try {
      handler.call(this, event);
    } catch (_err) {
      // Match browser behavior: event-handler exceptions don't alter load state.
    }
  }

  #callListener(listener, event) {
    try {
      if (typeof listener === "function") {
        listener.call(this, event);
      } else if (listener && typeof listener.handleEvent === "function") {
        listener.handleEvent(event);
      }
    } catch (_err) {
      // Match browser behavior: listener exceptions don't alter load state.
    }
  }

  #createEvent(type, error) {
    let event;
    if (typeof Event === "function") {
      event = new Event(type);
    } else {
      event = { type };
    }
    if (error != null) {
      try {
        event.error = error;
      } catch (_err) {
        // Ignore if event implementation is immutable.
      }
      try {
        event.message = String(error?.message ?? error);
      } catch (_err) {
        // Ignore if event implementation is immutable.
      }
    }
    return event;
  }

  #emitEvent(type, error = null) {
    this.dispatchEvent(this.#createEvent(type, error));
  }

  #applyLoadSuccess(result) {
    this.#width = result.width;
    this.#height = result.height;
    this.#isSvg = result.isSvg;
    this.#rawBytes = result.rawBytes;
    this.#imageData = result.imageData;
    this.#lastError = null;
    this.#complete = true;
  }

  #applyLoadFailure(error) {
    this.#width = 0;
    this.#height = 0;
    this.#isSvg = false;
    this.#rawBytes = null;
    this.#imageData = null;
    this.#lastError = error;
    this.#complete = true;
  }

  async #loadImage(url) {
    try {
      const hasScheme = /^[A-Za-z][A-Za-z0-9+.-]*:/.test(url);
      const isWindowsPath = /^[A-Za-z]:[\\/]/.test(url);
      const isFileUrl = url.startsWith("file://");
      const shouldReadLocalFile =
        isFileUrl || isWindowsPath || (!hasScheme && !url.startsWith("//"));

      let bytes;
      if (shouldReadLocalFile) {
        let filePath = url;
        if (isFileUrl) {
          const fileUrl = new URL(url);
          if (fileUrl.protocol !== "file:") {
            throw new Error(`Unsupported file URL protocol: ${fileUrl.protocol}`);
          }
          filePath = decodeURIComponent(fileUrl.pathname);
          if (globalThis.Deno?.build?.os === "windows" && filePath.startsWith("/")) {
            filePath = filePath.slice(1);
          }
        }
        bytes = new Uint8Array(await op_vega_file_read_bytes(filePath));
      } else {
        let normalizedUrl = null;
        let isHttpUrl = false;
        try {
          const parsedUrl = new URL(url);
          normalizedUrl = parsedUrl.href;
          isHttpUrl = parsedUrl.protocol === "http:" || parsedUrl.protocol === "https:";
        } catch (_err) {
          // Leave invalid/non-standard URL handling to the fallback path.
        }

        if (isHttpUrl) {
          bytes = new Uint8Array(await op_vega_data_fetch_bytes(normalizedUrl));
        } else {
          // Handle data: URLs and other non-HTTP schemes via fetch
          const response = await fetch(url);
          if (!response.ok) {
            throw new Error(`Failed to fetch image: ${response.status}`);
          }
          const arrayBuffer = await response.arrayBuffer();
          bytes = new Uint8Array(arrayBuffer);
        }
      }

      // Get image info (checks if SVG and returns native dimensions)
      const info = op_canvas_get_image_info(bytes);

      if (info.is_svg) {
        // For SVG, store raw bytes - decode at drawImage time for proper scaling
        return {
          ok: true,
          width: info.width,
          height: info.height,
          isSvg: true,
          rawBytes: bytes,
          imageData: null,
        };
      }

      // For raster images, decode immediately
      const decoded = op_canvas_decode_image(bytes);
      const pixelData = decoded.data instanceof Uint8Array
        ? decoded.data
        : new Uint8Array(decoded.data);

      return {
        ok: true,
        width: info.width,
        height: info.height,
        isSvg: false,
        rawBytes: null,
        imageData: {
          data: pixelData,
          width: decoded.width,
          height: decoded.height,
        },
      };
    } catch (error) {
      return { ok: false, error };
    }
  }
}

// Alias for HTMLImageElement
const HTMLImageElement = Image;

export { Image, HTMLImageElement, ImageData, unsupported, uint8ArrayToBase64, validateImageDataSettings };

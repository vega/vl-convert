# vl-convert-canvas2d-deno

Deno extension providing a Canvas 2D API implementation for server-side rendering.

This crate wraps [vl-convert-canvas2d](../vl-convert-canvas2d) and exposes it as a Deno extension with JavaScript polyfills that implement the standard Canvas 2D API.

## Usage

### Registering the Extension

Add the extension when building your Deno runtime:

```rust
use deno_core::{Extension, JsRuntime, RuntimeOptions};

let extensions = vec![
    vl_convert_canvas2d_deno::vl_convert_canvas2d::init_ops_and_esm(),
    // ... other extensions
];

let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions,
    ..Default::default()
});
```

### Sharing a Font Database

By default, canvas contexts use system fonts. To share a custom font database (e.g., with additional fonts loaded), put a `SharedFontDb` into the runtime's `OpState` before creating any canvases:

```rust
use std::sync::Arc;
use vl_convert_canvas2d_deno::SharedFontDb;

// Create and configure your fontdb
let mut fontdb = fontdb::Database::new();
fontdb.load_system_fonts();
fontdb.load_font_file("/path/to/custom/font.ttf")?;

// Wrap in SharedFontDb and put into OpState
let shared_fontdb = SharedFontDb::from_arc(Arc::new(fontdb));
runtime.op_state().borrow_mut().put(shared_fontdb);
```

All subsequent canvas contexts created via JavaScript will use this shared font database.

### JavaScript API

Once the extension is registered, JavaScript code has access to the standard Canvas 2D API:

```javascript
// Create a canvas (provided by the polyfill)
const canvas = document.createElement('canvas');
canvas.width = 800;
canvas.height = 600;

const ctx = canvas.getContext('2d');

// Standard Canvas 2D operations
ctx.fillStyle = 'blue';
ctx.fillRect(10, 10, 100, 100);

ctx.font = '16px Arial';
ctx.fillStyle = 'black';
ctx.fillText('Hello, World!', 50, 50);

// Export to PNG
const pngData = canvas.toDataURL('image/png');
```

## Supported Features

- Basic shapes: `fillRect`, `strokeRect`, `clearRect`
- Paths: `beginPath`, `moveTo`, `lineTo`, `arc`, `arcTo`, `ellipse`, `bezierCurveTo`, `quadraticCurveTo`, `closePath`, `rect`, `roundRect`
- Path drawing: `fill`, `stroke`, `clip`
- Path2D objects
- Styles: `fillStyle`, `strokeStyle` (colors, gradients, patterns)
- Line styles: `lineWidth`, `lineCap`, `lineJoin`, `miterLimit`, `setLineDash`, `lineDashOffset`
- Text: `fillText`, `strokeText`, `measureText`, `font`, `textAlign`, `textBaseline`, `letterSpacing`, `fontStretch`
- Transforms: `translate`, `rotate`, `scale`, `transform`, `setTransform`, `getTransform`, `resetTransform`
- Compositing: `globalAlpha`, `globalCompositeOperation`
- Image operations: `drawImage`, `getImageData`, `putImageData`, `createImageData` (with `ImageDataSettings` validation)
- Image smoothing: `imageSmoothingEnabled`, `imageSmoothingQuality`
- State: `save`, `restore`, `reset`
- Gradients: `createLinearGradient`, `createRadialGradient`
- Patterns: `createPattern`
- Path2D: all path methods plus `addPath(path, transform)`
- Not supported: `isPointInPath`, `isPointInStroke` (throw explicit errors)

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     JavaScript (Deno)                        │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  canvas_polyfill.js - Canvas/CanvasRenderingContext2D│    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              vl-convert-canvas2d-deno (this crate)          │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  ops.rs - Deno ops (op_canvas_*, op_path2d_*)       │    │
│  │  resource.rs - CanvasResource, Path2DResource       │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    vl-convert-canvas2d                       │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Canvas2dContext - Pure Rust Canvas 2D impl         │    │
│  │  Uses: tiny-skia (rendering), cosmic-text (fonts)   │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Example: vl-convert-rs Integration

[vl-convert-rs](../vl-convert-rs) uses this extension to provide canvas support for Vega/Vega-Lite rendering:

```rust
// In vl-convert-rs/src/converter.rs
use vl_convert_canvas2d_deno::SharedFontDb;

// After creating the Deno worker...
let opts = USVG_OPTIONS.lock()?;
let shared_fontdb = SharedFontDb::from_arc(opts.fontdb.clone());
worker.js_runtime.op_state().borrow_mut().put(shared_fontdb);
```

This allows Vega's canvas renderer to use the same font database as the SVG renderer, ensuring consistent text measurement and rendering.

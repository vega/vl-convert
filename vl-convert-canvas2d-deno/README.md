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

### Sharing a Font Configuration

By default, canvas contexts use system fonts. To share a pre-resolved font configuration (avoiding re-scanning system fonts on every canvas creation), put a `SharedFontConfig` into the runtime's `OpState`:

```rust
use vl_convert_canvas2d::FontConfig;
use vl_convert_canvas2d_deno::SharedFontConfig;

let font_config = FontConfig::default();
let resolved = font_config.resolve();
let shared_config = SharedFontConfig::new(resolved);
runtime.op_state().borrow_mut().put(shared_config);
```

All subsequent canvas contexts created via JavaScript will clone the cached font database from this configuration.

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
- Image operations: `drawImage`, `getImageData`, `putImageData`, `createImageData`
- Image smoothing: `imageSmoothingEnabled`, `imageSmoothingQuality`
- State: `save`, `restore`, `reset`
- Gradients: `createLinearGradient`, `createRadialGradient`
- Patterns: `createPattern`
- Path2D: all path methods plus `addPath(path, transform)`
- Not supported: `isPointInPath`, `isPointInStroke`

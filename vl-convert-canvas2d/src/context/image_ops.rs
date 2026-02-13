//! Image drawing, pixel data, and PNG output operations for Canvas2dContext.

use super::Canvas2dContext;
use crate::error::Canvas2dResult;
use crate::geometry::{CanvasImageDataRef, CanvasPixmapRef, DirtyRect, ImageCropParams};
use crate::pattern::{CanvasPattern, Repetition};
use std::sync::Arc;
use tiny_skia::Transform;

impl Canvas2dContext {
    // --- Internal image drawing ---

    /// Internal: draw a premultiplied-alpha pixmap at (dx, dy).
    pub(crate) fn draw_image(&mut self, image: CanvasPixmapRef, dx: f32, dy: f32) {
        log::debug!(target: "canvas", "drawImage {}x{} at {} {}", image.width, image.height, dx, dy);
        let Some(pixmap) = tiny_skia::PixmapRef::from_bytes(image.data, image.width, image.height)
        else {
            return;
        };
        let paint = tiny_skia::PixmapPaint {
            opacity: self.state.global_alpha,
            blend_mode: self.state.global_composite_operation,
            quality: self.get_image_filter_quality(),
        };

        // Translate to destination position
        let transform = self.state.transform.pre_translate(dx, dy);

        let clip_mask = self.create_clip_mask();
        self.pixmap
            .draw_pixmap(0, 0, pixmap, &paint, transform, clip_mask.as_ref());
    }

    /// Internal: draw a premultiplied-alpha pixmap scaled.
    pub(crate) fn draw_image_scaled(
        &mut self,
        image: CanvasPixmapRef,
        dx: f32,
        dy: f32,
        dw: f32,
        dh: f32,
    ) {
        let Some(pixmap) = tiny_skia::PixmapRef::from_bytes(image.data, image.width, image.height)
        else {
            return;
        };
        let paint = tiny_skia::PixmapPaint {
            opacity: self.state.global_alpha,
            blend_mode: self.state.global_composite_operation,
            quality: self.get_image_filter_quality(),
        };

        // Calculate scale factors
        let scale_x = dw / image.width as f32;
        let scale_y = dh / image.height as f32;

        // Translate to destination position, then scale
        let transform = self
            .state
            .transform
            .pre_translate(dx, dy)
            .pre_scale(scale_x, scale_y);

        let clip_mask = self.create_clip_mask();
        self.pixmap
            .draw_pixmap(0, 0, pixmap, &paint, transform, clip_mask.as_ref());
    }

    /// Internal: draw a cropped region of a premultiplied-alpha pixmap.
    pub(crate) fn draw_image_cropped(&mut self, image: CanvasPixmapRef, params: &ImageCropParams) {
        let Some(pixmap) = tiny_skia::PixmapRef::from_bytes(image.data, image.width, image.height)
        else {
            return;
        };
        let ImageCropParams {
            sx,
            sy,
            sw,
            sh,
            dx,
            dy,
            dw,
            dh,
        } = *params;

        // Clamp source rectangle to image bounds
        let sx = sx.max(0.0);
        let sy = sy.max(0.0);
        let sw = sw.min(image.width as f32 - sx);
        let sh = sh.min(image.height as f32 - sy);

        if sw <= 0.0 || sh <= 0.0 || dw <= 0.0 || dh <= 0.0 {
            return;
        }

        // Create a sub-image by creating a temporary pixmap with just the source region
        let sub_width = sw.ceil() as u32;
        let sub_height = sh.ceil() as u32;

        if let Some(mut sub_pixmap) = tiny_skia::Pixmap::new(sub_width, sub_height) {
            // Copy the source region to the sub-pixmap
            let src_x = sx.floor() as i32;
            let src_y = sy.floor() as i32;

            // Draw the source image offset to extract the region
            let extract_paint = tiny_skia::PixmapPaint::default();
            let extract_transform = Transform::from_translate(-src_x as f32, -src_y as f32);
            sub_pixmap.draw_pixmap(0, 0, pixmap, &extract_paint, extract_transform, None);

            // Now draw the extracted region scaled to the destination
            let sub_ref = CanvasPixmapRef {
                data: sub_pixmap.data(),
                width: sub_width,
                height: sub_height,
            };
            self.draw_image_scaled(sub_ref, dx, dy, dw, dh);
        }
    }

    // --- Public draw image/canvas methods (backend-neutral) ---

    /// Draw image data at the specified position.
    pub fn draw_image_data(&mut self, image: &CanvasImageDataRef<'_>, dx: f32, dy: f32) {
        let pixmap_ref = CanvasPixmapRef {
            data: image.data,
            width: image.width,
            height: image.height,
        };
        self.draw_image(pixmap_ref, dx, dy);
    }

    /// Draw image data scaled to the specified dimensions.
    pub fn draw_image_data_scaled(
        &mut self,
        image: &CanvasImageDataRef<'_>,
        dx: f32,
        dy: f32,
        dw: f32,
        dh: f32,
    ) {
        let pixmap_ref = CanvasPixmapRef {
            data: image.data,
            width: image.width,
            height: image.height,
        };
        self.draw_image_scaled(pixmap_ref, dx, dy, dw, dh);
    }

    /// Draw a cropped region of image data to a destination rectangle.
    pub fn draw_image_data_cropped(
        &mut self,
        image: &CanvasImageDataRef<'_>,
        params: &ImageCropParams,
    ) {
        let pixmap_ref = CanvasPixmapRef {
            data: image.data,
            width: image.width,
            height: image.height,
        };
        self.draw_image_cropped(pixmap_ref, params);
    }

    /// Draw another canvas at the specified position.
    pub fn draw_canvas(&mut self, source: &Canvas2dContext, dx: f32, dy: f32) {
        let pixmap_ref = CanvasPixmapRef {
            data: source.pixmap.data(),
            width: source.width,
            height: source.height,
        };
        self.draw_image(pixmap_ref, dx, dy);
    }

    /// Draw another canvas scaled to the specified dimensions.
    pub fn draw_canvas_scaled(
        &mut self,
        source: &Canvas2dContext,
        dx: f32,
        dy: f32,
        dw: f32,
        dh: f32,
    ) {
        let pixmap_ref = CanvasPixmapRef {
            data: source.pixmap.data(),
            width: source.width,
            height: source.height,
        };
        self.draw_image_scaled(pixmap_ref, dx, dy, dw, dh);
    }

    /// Draw a cropped region of another canvas to a destination rectangle.
    pub fn draw_canvas_cropped(&mut self, source: &Canvas2dContext, params: &ImageCropParams) {
        let pixmap_ref = CanvasPixmapRef {
            data: source.pixmap.data(),
            width: source.width,
            height: source.height,
        };
        self.draw_image_cropped(pixmap_ref, params);
    }

    /// Create a pattern from another canvas.
    pub fn create_pattern_from_canvas(
        &self,
        source: &Canvas2dContext,
        repetition: &str,
    ) -> Canvas2dResult<Arc<CanvasPattern>> {
        let rep = repetition.parse::<Repetition>()?;
        let pattern = CanvasPattern::from_pixmap_ref(source.pixmap.as_ref(), rep)?;
        Ok(Arc::new(pattern))
    }

    // --- Image data ---

    /// Create a new ImageData with the specified dimensions.
    ///
    /// Returns a Vec<u8> filled with transparent black (all zeros).
    /// The data is in RGBA format with 4 bytes per pixel.
    pub fn create_image_data(&self, width: u32, height: u32) -> Vec<u8> {
        vec![0u8; (width * height * 4) as usize]
    }

    /// Get image data for a region of the canvas.
    pub fn get_image_data(&self, x: i32, y: i32, width: u32, height: u32) -> Vec<u8> {
        let mut data = vec![0u8; (width * height * 4) as usize];

        for dy in 0..height {
            for dx in 0..width {
                let src_x = x + dx as i32;
                let src_y = y + dy as i32;

                let dst_idx = ((dy * width + dx) * 4) as usize;

                if src_x >= 0
                    && src_x < self.width as i32
                    && src_y >= 0
                    && src_y < self.height as i32
                {
                    let src_idx = (src_y as u32 * self.width + src_x as u32) as usize;
                    let pixel = self.pixmap.data()[src_idx * 4..src_idx * 4 + 4].to_vec();

                    // Convert from premultiplied alpha to straight alpha
                    let a = pixel[3];
                    if a == 0 {
                        data[dst_idx..dst_idx + 4].copy_from_slice(&[0, 0, 0, 0]);
                    } else if a == 255 {
                        data[dst_idx..dst_idx + 4].copy_from_slice(&pixel);
                    } else {
                        let alpha_f = a as f32 / 255.0;
                        data[dst_idx] = (pixel[0] as f32 / alpha_f).min(255.0) as u8;
                        data[dst_idx + 1] = (pixel[1] as f32 / alpha_f).min(255.0) as u8;
                        data[dst_idx + 2] = (pixel[2] as f32 / alpha_f).min(255.0) as u8;
                        data[dst_idx + 3] = a;
                    }
                }
            }
        }

        data
    }

    /// Write image data to the canvas at the specified position.
    ///
    /// The data must be in non-premultiplied RGBA format (standard ImageData format).
    /// This bypasses compositing operations and writes pixels directly.
    ///
    /// # Arguments
    /// * `data` - RGBA pixel data (4 bytes per pixel, non-premultiplied alpha)
    /// * `width` - Width of the image data
    /// * `height` - Height of the image data
    /// * `dx` - Destination x coordinate
    /// * `dy` - Destination y coordinate
    pub fn put_image_data(&mut self, data: &[u8], width: u32, height: u32, dx: i32, dy: i32) {
        self.put_image_data_dirty(
            data,
            width,
            height,
            dx,
            dy,
            &DirtyRect {
                x: 0,
                y: 0,
                width: width as i32,
                height: height as i32,
            },
        );
    }

    /// Write a portion of image data to the canvas.
    ///
    /// The dirty rectangle specifies which portion of the source data to write.
    /// Pixels outside the canvas bounds are silently ignored.
    pub fn put_image_data_dirty(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        dx: i32,
        dy: i32,
        dirty: &DirtyRect,
    ) {
        // Clamp dirty rect to source image bounds
        let dirty_x = dirty.x.max(0).min(width as i32);
        let dirty_y = dirty.y.max(0).min(height as i32);
        let dirty_width = dirty.width.max(0).min(width as i32 - dirty_x);
        let dirty_height = dirty.height.max(0).min(height as i32 - dirty_y);

        if dirty_width <= 0 || dirty_height <= 0 {
            return; // Nothing to draw
        }

        // Calculate destination coordinates for the dirty region
        let dest_x = dx + dirty_x;
        let dest_y = dy + dirty_y;

        // Get mutable access to pixmap data
        let canvas_width = self.width as i32;
        let canvas_height = self.height as i32;
        let pixmap_data = self.pixmap.data_mut();

        for sy in 0..dirty_height {
            let src_row = dirty_y + sy;
            let dst_row = dest_y + sy;

            // Skip if destination row is out of bounds
            if dst_row < 0 || dst_row >= canvas_height {
                continue;
            }

            for sx in 0..dirty_width {
                let src_col = dirty_x + sx;
                let dst_col = dest_x + sx;

                // Skip if destination column is out of bounds
                if dst_col < 0 || dst_col >= canvas_width {
                    continue;
                }

                // Calculate source and destination indices
                let src_idx = ((src_row as u32 * width + src_col as u32) * 4) as usize;
                let dst_idx = ((dst_row as u32 * self.width + dst_col as u32) * 4) as usize;

                // Read source pixel (non-premultiplied RGBA)
                let r = data[src_idx];
                let g = data[src_idx + 1];
                let b = data[src_idx + 2];
                let a = data[src_idx + 3];

                // Convert to premultiplied alpha using integer math
                // Formula: (color * alpha + 127) / 255 for proper rounding
                let (pr, pg, pb) = if a == 255 {
                    (r, g, b) // No conversion needed for fully opaque
                } else if a == 0 {
                    (0, 0, 0) // Fully transparent
                } else {
                    let a16 = a as u16;
                    (
                        ((r as u16 * a16 + 127) / 255) as u8,
                        ((g as u16 * a16 + 127) / 255) as u8,
                        ((b as u16 * a16 + 127) / 255) as u8,
                    )
                };

                // Write to destination (bypasses compositing - direct pixel write)
                pixmap_data[dst_idx] = pr;
                pixmap_data[dst_idx + 1] = pg;
                pixmap_data[dst_idx + 2] = pb;
                pixmap_data[dst_idx + 3] = a;
            }
        }
    }

    /// Export the canvas as PNG data.
    ///
    /// # Arguments
    /// * `ppi` - Optional pixels per inch for PNG metadata. Defaults to 72 if not specified.
    pub fn to_png(&self, ppi: Option<f32>) -> Canvas2dResult<Vec<u8>> {
        let ppi = ppi.unwrap_or(72.0);

        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);

            // Set pixel density metadata (pixels per meter)
            let ppm = (ppi.max(0.0) / 0.0254).round() as u32;
            encoder.set_pixel_dims(Some(png::PixelDimensions {
                xppu: ppm,
                yppu: ppm,
                unit: png::Unit::Meter,
            }));

            let mut writer = encoder.write_header()?;

            // Convert from premultiplied to straight alpha for PNG
            let data = self.get_image_data(0, 0, self.width, self.height);
            writer.write_image_data(&data)?;
        }
        Ok(buf)
    }
}

//! Pattern types for Canvas 2D operations.

use crate::error::{Canvas2dError, Canvas2dResult};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tiny_skia::{Pixmap, PixmapRef, Shader, SpreadMode, Transform};

/// Maximum pattern size (4096x4096).
const MAX_PATTERN_SIZE: u32 = 4096;

/// Global counter for pattern IDs.
static PATTERN_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Cache key for leaked pixmaps.
#[derive(Clone, Copy, PartialEq, Eq)]
struct PixmapCacheKey {
    pattern_id: u64,
    repetition: Repetition,
    /// Canvas dimensions (0,0 for Repeat mode since it doesn't depend on canvas size)
    canvas_width: u32,
    canvas_height: u32,
}

impl Hash for PixmapCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pattern_id.hash(state);
        std::mem::discriminant(&self.repetition).hash(state);
        self.canvas_width.hash(state);
        self.canvas_height.hash(state);
    }
}

/// Global cache for leaked pixmaps to avoid leaking duplicates.
static PIXMAP_CACHE: Mutex<Option<HashMap<PixmapCacheKey, &'static Pixmap>>> = Mutex::new(None);

/// Get or create a cached leaked pixmap.
fn get_or_leak_pixmap(
    key: PixmapCacheKey,
    create_fn: impl FnOnce() -> Option<Pixmap>,
) -> Option<&'static Pixmap> {
    let mut cache_guard = PIXMAP_CACHE.lock().ok()?;
    let cache = cache_guard.get_or_insert_with(HashMap::new);

    if let Some(&pixmap_ref) = cache.get(&key) {
        return Some(pixmap_ref);
    }

    // Create and leak the pixmap
    let pixmap = create_fn()?;
    let leaked: &'static Pixmap = Box::leak(Box::new(pixmap));
    cache.insert(key, leaked);
    Some(leaked)
}

/// Pattern repetition mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Repetition {
    /// Repeat in both directions (default).
    #[default]
    Repeat,
    /// Repeat only horizontally.
    RepeatX,
    /// Repeat only vertically.
    RepeatY,
    /// No repetition (single instance).
    NoRepeat,
}

impl std::str::FromStr for Repetition {
    type Err = Canvas2dError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "repeat" | "" => Ok(Repetition::Repeat),
            "repeat-x" => Ok(Repetition::RepeatX),
            "repeat-y" => Ok(Repetition::RepeatY),
            "no-repeat" => Ok(Repetition::NoRepeat),
            _ => Err(Canvas2dError::InvalidArgument(format!(
                "Invalid repetition mode: '{}'",
                s
            ))),
        }
    }
}

/// Canvas pattern for fill/stroke operations.
#[derive(Debug, Clone)]
pub struct CanvasPattern {
    /// Unique identifier for this pattern (used for caching).
    id: u64,
    /// The pattern image.
    pixmap: Pixmap,
    /// Repetition mode.
    repetition: Repetition,
    /// Pattern transform matrix.
    transform: Transform,
}

impl CanvasPattern {
    /// Create a new pattern from pixel data.
    ///
    /// # Arguments
    /// * `data` - RGBA pixel data (4 bytes per pixel, non-premultiplied)
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `repetition` - Repetition mode
    pub fn new(
        data: &[u8],
        width: u32,
        height: u32,
        repetition: Repetition,
    ) -> Canvas2dResult<Self> {
        // Validate size
        if width > MAX_PATTERN_SIZE || height > MAX_PATTERN_SIZE {
            return Err(Canvas2dError::InvalidArgument(format!(
                "Pattern size {}x{} exceeds maximum {}x{}",
                width, height, MAX_PATTERN_SIZE, MAX_PATTERN_SIZE
            )));
        }

        if width == 0 || height == 0 {
            return Err(Canvas2dError::InvalidArgument(
                "Pattern dimensions must be non-zero".to_string(),
            ));
        }

        let expected_len = (width * height * 4) as usize;
        if data.len() != expected_len {
            return Err(Canvas2dError::InvalidArgument(format!(
                "Data length {} does not match expected {} for {}x{} RGBA image",
                data.len(),
                expected_len,
                width,
                height
            )));
        }

        // Create pixmap and convert from non-premultiplied to premultiplied alpha
        let mut pixmap = Pixmap::new(width, height)
            .ok_or_else(|| Canvas2dError::InvalidArgument("Failed to create pixmap".to_string()))?;

        let pixels = pixmap.pixels_mut();
        for (i, pixel) in pixels.iter_mut().enumerate() {
            let offset = i * 4;
            let r = data[offset];
            let g = data[offset + 1];
            let b = data[offset + 2];
            let a = data[offset + 3];

            // Convert to premultiplied alpha using integer math
            let (pr, pg, pb) = if a == 255 {
                (r, g, b)
            } else if a == 0 {
                (0, 0, 0)
            } else {
                let a16 = a as u16;
                (
                    ((r as u16 * a16 + 127) / 255) as u8,
                    ((g as u16 * a16 + 127) / 255) as u8,
                    ((b as u16 * a16 + 127) / 255) as u8,
                )
            };

            *pixel = tiny_skia::PremultipliedColorU8::from_rgba(pr, pg, pb, a).unwrap();
        }

        Ok(Self {
            id: PATTERN_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            pixmap,
            repetition,
            transform: Transform::identity(),
        })
    }

    /// Create a new pattern from a Pixmap (already premultiplied).
    pub fn from_pixmap(pixmap: Pixmap, repetition: Repetition) -> Canvas2dResult<Self> {
        let width = pixmap.width();
        let height = pixmap.height();

        if width > MAX_PATTERN_SIZE || height > MAX_PATTERN_SIZE {
            return Err(Canvas2dError::InvalidArgument(format!(
                "Pattern size {}x{} exceeds maximum {}x{}",
                width, height, MAX_PATTERN_SIZE, MAX_PATTERN_SIZE
            )));
        }

        Ok(Self {
            id: PATTERN_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            pixmap,
            repetition,
            transform: Transform::identity(),
        })
    }

    /// Create a new pattern from a PixmapRef (copies the data).
    pub fn from_pixmap_ref(pixmap_ref: PixmapRef, repetition: Repetition) -> Canvas2dResult<Self> {
        let pixmap = pixmap_ref.to_owned();
        Self::from_pixmap(pixmap, repetition)
    }

    /// Set the pattern transform matrix.
    pub fn set_transform(&mut self, transform: Transform) {
        self.transform = transform;
    }

    /// Get the pattern transform matrix.
    pub fn transform(&self) -> Transform {
        self.transform
    }

    /// Get the pattern width.
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    /// Get the pattern height.
    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    /// Get the repetition mode.
    pub fn repetition(&self) -> Repetition {
        self.repetition
    }

    /// Create a shader for this pattern.
    ///
    /// For repeat-x and repeat-y modes, this creates an extended pixmap with
    /// transparent padding to simulate partial repetition using SpreadMode::Pad.
    ///
    /// Leaked pixmaps are cached by pattern ID and canvas dimensions to avoid
    /// leaking duplicate pixmaps when the same pattern is used multiple times.
    pub fn create_shader(
        &self,
        canvas_width: u32,
        canvas_height: u32,
        context_transform: Transform,
    ) -> Option<Shader<'static>> {
        // Combine pattern transform with context transform
        let combined_transform = self.transform.post_concat(context_transform);

        match self.repetition {
            Repetition::Repeat => {
                // For Repeat mode, canvas size doesn't matter - use (0, 0) as sentinel
                let key = PixmapCacheKey {
                    pattern_id: self.id,
                    repetition: self.repetition,
                    canvas_width: 0,
                    canvas_height: 0,
                };
                let pixmap_ref = get_or_leak_pixmap(key, || Some(self.pixmap.clone()))?;
                Some(tiny_skia::Pattern::new(
                    pixmap_ref.as_ref(),
                    SpreadMode::Repeat,
                    tiny_skia::FilterQuality::Bilinear,
                    1.0, // opacity applied elsewhere via global_alpha
                    combined_transform,
                ))
            }
            Repetition::NoRepeat => {
                let key = PixmapCacheKey {
                    pattern_id: self.id,
                    repetition: self.repetition,
                    canvas_width,
                    canvas_height,
                };
                let pixmap_ref = get_or_leak_pixmap(key, || {
                    self.create_extended_pixmap_no_repeat(canvas_width, canvas_height)
                })?;
                Some(tiny_skia::Pattern::new(
                    pixmap_ref.as_ref(),
                    SpreadMode::Pad,
                    tiny_skia::FilterQuality::Bilinear,
                    1.0,
                    combined_transform,
                ))
            }
            Repetition::RepeatX => {
                let key = PixmapCacheKey {
                    pattern_id: self.id,
                    repetition: self.repetition,
                    canvas_width,
                    canvas_height,
                };
                let pixmap_ref = get_or_leak_pixmap(key, || {
                    self.create_extended_pixmap_repeat_x(canvas_width, canvas_height)
                })?;
                Some(tiny_skia::Pattern::new(
                    pixmap_ref.as_ref(),
                    SpreadMode::Pad,
                    tiny_skia::FilterQuality::Bilinear,
                    1.0,
                    combined_transform,
                ))
            }
            Repetition::RepeatY => {
                let key = PixmapCacheKey {
                    pattern_id: self.id,
                    repetition: self.repetition,
                    canvas_width,
                    canvas_height,
                };
                let pixmap_ref = get_or_leak_pixmap(key, || {
                    self.create_extended_pixmap_repeat_y(canvas_width, canvas_height)
                })?;
                Some(tiny_skia::Pattern::new(
                    pixmap_ref.as_ref(),
                    SpreadMode::Pad,
                    tiny_skia::FilterQuality::Bilinear,
                    1.0,
                    combined_transform,
                ))
            }
        }
    }

    /// Create an extended pixmap for no-repeat mode with transparent padding.
    fn create_extended_pixmap_no_repeat(
        &self,
        canvas_width: u32,
        canvas_height: u32,
    ) -> Option<Pixmap> {
        let pattern_width = self.pixmap.width();
        let pattern_height = self.pixmap.height();

        // Create pixmap large enough to cover canvas plus pattern
        let ext_width = pattern_width + canvas_width;
        let ext_height = pattern_height + canvas_height;

        // Cap at reasonable size
        let ext_width = ext_width.min(MAX_PATTERN_SIZE * 2);
        let ext_height = ext_height.min(MAX_PATTERN_SIZE * 2);

        let mut extended = Pixmap::new(ext_width, ext_height)?;
        // Pixmap is initialized to transparent (all zeros)

        // Copy pattern to top-left corner
        for y in 0..pattern_height {
            for x in 0..pattern_width {
                let src_pixel = self.pixmap.pixel(x, y)?;
                extended.pixels_mut()[(y * ext_width + x) as usize] = src_pixel;
            }
        }

        Some(extended)
    }

    /// Create an extended pixmap for repeat-x mode.
    /// The pattern is tiled horizontally, with transparent padding below.
    /// When used with Pad mode, the transparent bottom edge gets extended.
    fn create_extended_pixmap_repeat_x(
        &self,
        canvas_width: u32,
        canvas_height: u32,
    ) -> Option<Pixmap> {
        let pattern_width = self.pixmap.width();
        let pattern_height = self.pixmap.height();

        // Calculate how many tiles we need to cover the canvas width (plus some extra)
        let tiles_needed = (canvas_width / pattern_width) + 2;
        let ext_width = (pattern_width * tiles_needed).min(MAX_PATTERN_SIZE * 2);

        // Add transparent padding below the pattern
        // We need 1 row of transparency at the bottom which Pad will extend
        let ext_height = (pattern_height + canvas_height).min(MAX_PATTERN_SIZE * 2);

        let mut extended = Pixmap::new(ext_width, ext_height)?;
        // Pixmap is initialized to transparent (all zeros)

        // Tile the pattern horizontally in the top portion only
        for tile in 0..tiles_needed {
            let x_offset = tile * pattern_width;
            if x_offset >= ext_width {
                break;
            }
            for y in 0..pattern_height {
                for x in 0..pattern_width {
                    let dst_x = x_offset + x;
                    if dst_x < ext_width {
                        let src_pixel = self.pixmap.pixel(x, y)?;
                        extended.pixels_mut()[(y * ext_width + dst_x) as usize] = src_pixel;
                    }
                }
            }
        }

        Some(extended)
    }

    /// Create an extended pixmap for repeat-y mode.
    /// The pattern is tiled vertically, with transparent padding to the right.
    /// When used with Pad mode, the transparent right edge gets extended.
    fn create_extended_pixmap_repeat_y(
        &self,
        canvas_width: u32,
        canvas_height: u32,
    ) -> Option<Pixmap> {
        let pattern_width = self.pixmap.width();
        let pattern_height = self.pixmap.height();

        // Calculate how many tiles we need to cover the canvas height (plus some extra)
        let tiles_needed = (canvas_height / pattern_height) + 2;
        let ext_height = (pattern_height * tiles_needed).min(MAX_PATTERN_SIZE * 2);

        // Add transparent padding to the right of the pattern
        let ext_width = (pattern_width + canvas_width).min(MAX_PATTERN_SIZE * 2);

        let mut extended = Pixmap::new(ext_width, ext_height)?;
        // Pixmap is initialized to transparent (all zeros)

        // Tile the pattern vertically in the left portion only
        for tile in 0..tiles_needed {
            let y_offset = tile * pattern_height;
            if y_offset >= ext_height {
                break;
            }
            for y in 0..pattern_height {
                let dst_y = y_offset + y;
                if dst_y >= ext_height {
                    break;
                }
                for x in 0..pattern_width {
                    let src_pixel = self.pixmap.pixel(x, y)?;
                    extended.pixels_mut()[(dst_y * ext_width + x) as usize] = src_pixel;
                }
            }
        }

        Some(extended)
    }
}

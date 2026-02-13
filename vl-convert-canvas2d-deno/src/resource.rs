//! Canvas resource for Deno state management.

use deno_core::Resource;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;
use vl_convert_canvas2d::{Canvas2dContext, CanvasGradient, CanvasPattern, Path2D};

/// Resource wrapper for Canvas2dContext to be stored in Deno's resource table.
/// Also stores gradients and patterns with numeric IDs for JavaScript reference.
pub struct CanvasResource {
    pub ctx: RefCell<Canvas2dContext>,
    /// Gradients created on this canvas, keyed by ID
    pub gradients: RefCell<HashMap<u32, CanvasGradient>>,
    /// Patterns created on this canvas, keyed by ID
    pub patterns: RefCell<HashMap<u32, Arc<CanvasPattern>>>,
    /// Next gradient ID to assign
    next_gradient_id: Cell<u32>,
    /// Next pattern ID to assign
    next_pattern_id: Cell<u32>,
    /// Font config version this canvas was created with (or last updated to).
    pub font_config_version: Cell<u64>,
}

/// Resource wrapper for Path2D objects to be stored in Deno's resource table.
/// Path2D objects exist independently of canvases.
pub struct Path2DResource {
    pub path: RefCell<Path2D>,
}

impl CanvasResource {
    pub fn new(ctx: Canvas2dContext, font_config_version: u64) -> Self {
        Self {
            ctx: RefCell::new(ctx),
            gradients: RefCell::new(HashMap::new()),
            patterns: RefCell::new(HashMap::new()),
            next_gradient_id: Cell::new(1),
            next_pattern_id: Cell::new(1),
            font_config_version: Cell::new(font_config_version),
        }
    }

    /// Add a gradient and return its ID
    pub fn add_gradient(&self, gradient: CanvasGradient) -> u32 {
        let id = self.next_gradient_id.get();
        self.next_gradient_id.set(id + 1);
        self.gradients.borrow_mut().insert(id, gradient);
        id
    }

    /// Get a mutable reference to a gradient by ID
    pub fn get_gradient_mut(&self, id: u32) -> Option<std::cell::RefMut<'_, CanvasGradient>> {
        let gradients = self.gradients.borrow_mut();
        if gradients.contains_key(&id) {
            Some(std::cell::RefMut::map(gradients, |g| {
                g.get_mut(&id).unwrap()
            }))
        } else {
            None
        }
    }

    /// Take a gradient by ID (removes it from storage)
    pub fn take_gradient(&self, id: u32) -> Option<CanvasGradient> {
        self.gradients.borrow_mut().remove(&id)
    }

    /// Add a pattern and return its ID
    pub fn add_pattern(&self, pattern: Arc<CanvasPattern>) -> u32 {
        let id = self.next_pattern_id.get();
        self.next_pattern_id.set(id + 1);
        self.patterns.borrow_mut().insert(id, pattern);
        id
    }

    /// Get a pattern by ID
    pub fn get_pattern(&self, id: u32) -> Option<Arc<CanvasPattern>> {
        self.patterns.borrow().get(&id).cloned()
    }
}

impl Resource for CanvasResource {
    fn name(&self) -> Cow<'_, str> {
        "canvas2d".into()
    }
}

impl Path2DResource {
    pub fn new(path: Path2D) -> Self {
        Self {
            path: RefCell::new(path),
        }
    }
}

impl Resource for Path2DResource {
    fn name(&self) -> Cow<'_, str> {
        "path2d".into()
    }
}

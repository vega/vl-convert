//! Canvas resource for Deno state management.

use deno_core::Resource;
use std::borrow::Cow;
use std::cell::RefCell;
use vl_convert_canvas2d::Canvas2dContext;

/// Resource wrapper for Canvas2dContext to be stored in Deno's resource table.
pub struct CanvasResource {
    pub ctx: RefCell<Canvas2dContext>,
}

impl CanvasResource {
    pub fn new(ctx: Canvas2dContext) -> Self {
        Self {
            ctx: RefCell::new(ctx),
        }
    }
}

impl Resource for CanvasResource {
    fn name(&self) -> Cow<'_, str> {
        "canvas2d".into()
    }
}

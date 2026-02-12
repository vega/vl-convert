//! Path operations for both canvas context paths and Path2D objects.
//!
//! Includes path construction (move_to, line_to, arc, ellipse, bezier curves, etc.),
//! Path2D lifecycle management, and canvas-Path2D drawing integration.

use crate::{CanvasResource, Path2DResource};
use deno_core::op2;
use deno_core::{OpState, ResourceId};
use deno_error::JsErrorBox;
use vl_convert_canvas2d::{
    ArcParams, ArcToParams, CornerRadius, CubicBezierParams, DOMMatrix, EllipseParams,
    QuadraticBezierParams, RectParams, RoundRectParams,
};

// --- Canvas path operations ---

/// Begin a new path.
#[op2(fast)]
pub fn op_canvas_begin_path(state: &mut OpState, rid: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().begin_path();
    Ok(())
}

/// Move to a point without drawing.
#[op2(fast)]
pub fn op_canvas_move_to(state: &mut OpState, rid: u32, x: f64, y: f64) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().move_to(x as f32, y as f32);
    Ok(())
}

/// Draw a line to a point.
#[op2(fast)]
pub fn op_canvas_line_to(state: &mut OpState, rid: u32, x: f64, y: f64) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().line_to(x as f32, y as f32);
    Ok(())
}

/// Close the current subpath.
#[op2(fast)]
pub fn op_canvas_close_path(state: &mut OpState, rid: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().close_path();
    Ok(())
}

/// Add a cubic bezier curve.
#[op2(fast)]
pub fn op_canvas_bezier_curve_to(
    state: &mut OpState,
    rid: u32,
    cp1x: f64,
    cp1y: f64,
    cp2x: f64,
    cp2y: f64,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .bezier_curve_to(&CubicBezierParams {
            cp1x: cp1x as f32,
            cp1y: cp1y as f32,
            cp2x: cp2x as f32,
            cp2y: cp2y as f32,
            x: x as f32,
            y: y as f32,
        });
    Ok(())
}

/// Add a quadratic bezier curve.
#[op2(fast)]
pub fn op_canvas_quadratic_curve_to(
    state: &mut OpState,
    rid: u32,
    cpx: f64,
    cpy: f64,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource
        .ctx
        .borrow_mut()
        .quadratic_curve_to(&QuadraticBezierParams {
            cpx: cpx as f32,
            cpy: cpy as f32,
            x: x as f32,
            y: y as f32,
        });
    Ok(())
}

/// Add a rectangle to the path.
#[op2(fast)]
pub fn op_canvas_rect(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().rect(&RectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
    });
    Ok(())
}

/// Add an arc to the path.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_canvas_arc(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    anticlockwise: bool,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().arc(&ArcParams {
        x: x as f32,
        y: y as f32,
        radius: radius as f32,
        start_angle: start_angle as f32,
        end_angle: end_angle as f32,
        anticlockwise,
    });
    Ok(())
}

/// Add an arcTo segment to the path.
#[op2(fast)]
pub fn op_canvas_arc_to(
    state: &mut OpState,
    rid: u32,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    radius: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().arc_to(&ArcToParams {
        x1: x1 as f32,
        y1: y1 as f32,
        x2: x2 as f32,
        y2: y2 as f32,
        radius: radius as f32,
    });
    Ok(())
}

/// Add an ellipse to the path.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_canvas_ellipse(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    radius_x: f64,
    radius_y: f64,
    rotation: f64,
    start_angle: f64,
    end_angle: f64,
    anticlockwise: bool,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    resource.ctx.borrow_mut().ellipse(&EllipseParams {
        x: x as f32,
        y: y as f32,
        radius_x: radius_x as f32,
        radius_y: radius_y as f32,
        rotation: rotation as f32,
        start_angle: start_angle as f32,
        end_angle: end_angle as f32,
        anticlockwise,
    });
    Ok(())
}

/// Add a rounded rectangle to the path with a single radius for all corners.
#[op2(fast)]
pub fn op_canvas_round_rect(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let r = CornerRadius {
        x: radius as f32,
        y: radius as f32,
    };
    resource.ctx.borrow_mut().round_rect(&RoundRectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
        radii: [r, r, r, r],
    });
    Ok(())
}

/// Add a rounded rectangle to the path with individual corner radii.
/// Each radius is an [x, y] pair for independent horizontal/vertical radii.
#[op2]
pub fn op_canvas_round_rect_radii(
    state: &mut OpState,
    rid: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    #[serde] radii: Vec<[f64; 2]>,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let radii_array = parse_radii(&radii)?;

    resource.ctx.borrow_mut().round_rect(&RoundRectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
        radii: radii_array,
    });
    Ok(())
}

// --- Path2D lifecycle ---

/// Create an empty Path2D.
#[op2(fast)]
pub fn op_path2d_create(state: &mut OpState) -> u32 {
    let path = vl_convert_canvas2d::Path2D::new();
    let resource = Path2DResource::new(path);
    state.resource_table.add(resource)
}

/// Create a Path2D from SVG path data string.
#[op2(fast)]
pub fn op_path2d_create_from_svg(
    state: &mut OpState,
    #[string] svg_path: String,
) -> Result<u32, JsErrorBox> {
    let path = vl_convert_canvas2d::Path2D::from_svg_path_data(&svg_path)
        .map_err(|e| JsErrorBox::generic(format!("Invalid SVG path data: {}", e)))?;
    let resource = Path2DResource::new(path);
    Ok(state.resource_table.add(resource))
}

/// Create a Path2D by copying another Path2D.
#[op2(fast)]
pub fn op_path2d_create_from_path(
    state: &mut OpState,
    source_path_id: u32,
) -> Result<u32, JsErrorBox> {
    let source = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(source_path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let new_path = vl_convert_canvas2d::Path2D::from_path(&source.path.borrow());
    let resource = Path2DResource::new(new_path);
    Ok(state.resource_table.add(resource))
}

/// Destroy a Path2D.
#[op2(fast)]
pub fn op_path2d_destroy(state: &mut OpState, path_id: u32) -> Result<(), JsErrorBox> {
    state
        .resource_table
        .take::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Failed to destroy Path2D: {}", e)))?;
    Ok(())
}

// --- Path2D operations ---

/// Move to a point in Path2D.
#[op2(fast)]
pub fn op_path2d_move_to(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().move_to(x as f32, y as f32);
    Ok(())
}

/// Line to a point in Path2D.
#[op2(fast)]
pub fn op_path2d_line_to(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().line_to(x as f32, y as f32);
    Ok(())
}

/// Close the current subpath in Path2D.
#[op2(fast)]
pub fn op_path2d_close_path(state: &mut OpState, path_id: u32) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().close_path();
    Ok(())
}

/// Add a cubic bezier curve to Path2D.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_path2d_bezier_curve_to(
    state: &mut OpState,
    path_id: u32,
    cp1x: f64,
    cp1y: f64,
    cp2x: f64,
    cp2y: f64,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource
        .path
        .borrow_mut()
        .bezier_curve_to(&CubicBezierParams {
            cp1x: cp1x as f32,
            cp1y: cp1y as f32,
            cp2x: cp2x as f32,
            cp2y: cp2y as f32,
            x: x as f32,
            y: y as f32,
        });
    Ok(())
}

/// Add a quadratic bezier curve to Path2D.
#[op2(fast)]
pub fn op_path2d_quadratic_curve_to(
    state: &mut OpState,
    path_id: u32,
    cpx: f64,
    cpy: f64,
    x: f64,
    y: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource
        .path
        .borrow_mut()
        .quadratic_curve_to(&QuadraticBezierParams {
            cpx: cpx as f32,
            cpy: cpy as f32,
            x: x as f32,
            y: y as f32,
        });
    Ok(())
}

/// Add a rectangle to Path2D.
#[op2(fast)]
pub fn op_path2d_rect(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().rect(&RectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
    });
    Ok(())
}

/// Add an arc to Path2D.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_path2d_arc(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    anticlockwise: bool,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().arc(&ArcParams {
        x: x as f32,
        y: y as f32,
        radius: radius as f32,
        start_angle: start_angle as f32,
        end_angle: end_angle as f32,
        anticlockwise,
    });
    Ok(())
}

/// Add an arcTo segment to Path2D.
#[op2(fast)]
pub fn op_path2d_arc_to(
    state: &mut OpState,
    path_id: u32,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    radius: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().arc_to(&ArcToParams {
        x1: x1 as f32,
        y1: y1 as f32,
        x2: x2 as f32,
        y2: y2 as f32,
        radius: radius as f32,
    });
    Ok(())
}

/// Add an ellipse to Path2D.
#[op2(fast)]
#[allow(clippy::too_many_arguments)]
pub fn op_path2d_ellipse(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
    radius_x: f64,
    radius_y: f64,
    rotation: f64,
    start_angle: f64,
    end_angle: f64,
    anticlockwise: bool,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    resource.path.borrow_mut().ellipse(&EllipseParams {
        x: x as f32,
        y: y as f32,
        radius_x: radius_x as f32,
        radius_y: radius_y as f32,
        rotation: rotation as f32,
        start_angle: start_angle as f32,
        end_angle: end_angle as f32,
        anticlockwise,
    });
    Ok(())
}

/// Add a rounded rectangle to Path2D with a single radius.
#[op2(fast)]
pub fn op_path2d_round_rect(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let r = CornerRadius {
        x: radius as f32,
        y: radius as f32,
    };
    resource.path.borrow_mut().round_rect(&RoundRectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
        radii: [r, r, r, r],
    });
    Ok(())
}

/// Add a rounded rectangle to Path2D with individual corner radii.
/// Each radius is an [x, y] pair for independent horizontal/vertical radii.
#[op2]
pub fn op_path2d_round_rect_radii(
    state: &mut OpState,
    path_id: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    #[serde] radii: Vec<[f64; 2]>,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let radii_array = parse_radii(&radii)?;

    resource.path.borrow_mut().round_rect(&RoundRectParams {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
        radii: radii_array,
    });
    Ok(())
}

/// Add another path's segments to a Path2D, optionally with a transform.
#[op2]
pub fn op_path2d_add_path(
    state: &mut OpState,
    path_id: u32,
    other_path_id: u32,
    #[serde] transform: Option<[f64; 6]>,
) -> Result<(), JsErrorBox> {
    let resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let other_resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(other_path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid source Path2D resource: {}", e)))?;

    let dom_matrix = transform.map(|t| {
        DOMMatrix::new(
            t[0] as f32,
            t[1] as f32,
            t[2] as f32,
            t[3] as f32,
            t[4] as f32,
            t[5] as f32,
        )
    });

    let mut other_path = other_resource.path.borrow_mut();
    resource
        .path
        .borrow_mut()
        .add_path(&mut other_path, dom_matrix);
    Ok(())
}

// --- Canvas-Path2D drawing ---

/// Fill a Path2D on the canvas.
#[op2(fast)]
pub fn op_canvas_fill_path2d(
    state: &mut OpState,
    rid: u32,
    path_id: u32,
) -> Result<(), JsErrorBox> {
    let path_resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let canvas_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    canvas_resource
        .ctx
        .borrow_mut()
        .fill_path2d(&mut path_resource.path.borrow_mut());
    Ok(())
}

/// Fill a Path2D on the canvas with a fill rule.
#[op2(fast)]
pub fn op_canvas_fill_path2d_with_rule(
    state: &mut OpState,
    rid: u32,
    path_id: u32,
    #[string] fill_rule: String,
) -> Result<(), JsErrorBox> {
    let path_resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let canvas_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let rule = match fill_rule.as_str() {
        "nonzero" => vl_convert_canvas2d::CanvasFillRule::NonZero,
        "evenodd" => vl_convert_canvas2d::CanvasFillRule::EvenOdd,
        _ => {
            return Err(JsErrorBox::generic(format!(
                "Invalid fill rule: {}",
                fill_rule
            )))
        }
    };

    canvas_resource
        .ctx
        .borrow_mut()
        .fill_path2d_with_rule(&mut path_resource.path.borrow_mut(), rule);
    Ok(())
}

/// Stroke a Path2D on the canvas.
#[op2(fast)]
pub fn op_canvas_stroke_path2d(
    state: &mut OpState,
    rid: u32,
    path_id: u32,
) -> Result<(), JsErrorBox> {
    let path_resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let canvas_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    canvas_resource
        .ctx
        .borrow_mut()
        .stroke_path2d(&mut path_resource.path.borrow_mut());
    Ok(())
}

/// Clip to a Path2D on the canvas.
#[op2(fast)]
pub fn op_canvas_clip_path2d(
    state: &mut OpState,
    rid: u32,
    path_id: u32,
) -> Result<(), JsErrorBox> {
    let path_resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let canvas_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    canvas_resource
        .ctx
        .borrow_mut()
        .clip_path2d(&mut path_resource.path.borrow_mut());
    Ok(())
}

/// Clip to a Path2D on the canvas with a fill rule.
#[op2(fast)]
pub fn op_canvas_clip_path2d_with_rule(
    state: &mut OpState,
    rid: u32,
    path_id: u32,
    #[string] fill_rule: String,
) -> Result<(), JsErrorBox> {
    let path_resource = state
        .resource_table
        .get::<Path2DResource>(ResourceId::from(path_id))
        .map_err(|e| JsErrorBox::generic(format!("Invalid Path2D resource: {}", e)))?;

    let canvas_resource = state
        .resource_table
        .get::<CanvasResource>(ResourceId::from(rid))
        .map_err(|e| JsErrorBox::generic(format!("Invalid canvas resource: {}", e)))?;

    let rule = match fill_rule.as_str() {
        "nonzero" => vl_convert_canvas2d::CanvasFillRule::NonZero,
        "evenodd" => vl_convert_canvas2d::CanvasFillRule::EvenOdd,
        _ => {
            return Err(JsErrorBox::generic(format!(
                "Invalid fill rule: {}",
                fill_rule
            )))
        }
    };

    canvas_resource
        .ctx
        .borrow_mut()
        .clip_path2d_with_rule(&mut path_resource.path.borrow_mut(), rule);
    Ok(())
}

// --- Helpers ---

/// Parse a radii array (1, 2, 3, or 4 [x, y] pairs) into a fixed-size array.
fn parse_radii(radii: &[[f64; 2]]) -> Result<[CornerRadius; 4], JsErrorBox> {
    let to_cr = |pair: &[f64; 2]| CornerRadius {
        x: pair[0] as f32,
        y: pair[1] as f32,
    };
    match radii.len() {
        1 => {
            let r = to_cr(&radii[0]);
            Ok([r, r, r, r])
        }
        2 => {
            let a = to_cr(&radii[0]);
            let b = to_cr(&radii[1]);
            Ok([a, b, a, b])
        }
        3 => {
            let a = to_cr(&radii[0]);
            let b = to_cr(&radii[1]);
            let c = to_cr(&radii[2]);
            Ok([a, b, c, b])
        }
        4 => Ok([
            to_cr(&radii[0]),
            to_cr(&radii[1]),
            to_cr(&radii[2]),
            to_cr(&radii[3]),
        ]),
        _ => Err(JsErrorBox::generic("Invalid radii array length")),
    }
}

// Path2D polyfill for vl-convert canvas

import {
  op_path2d_create,
  op_path2d_create_from_svg,
  op_path2d_create_from_path,
  op_path2d_move_to,
  op_path2d_line_to,
  op_path2d_close_path,
  op_path2d_bezier_curve_to,
  op_path2d_quadratic_curve_to,
  op_path2d_rect,
  op_path2d_arc,
  op_path2d_arc_to,
  op_path2d_ellipse,
  op_path2d_round_rect,
  op_path2d_round_rect_radii,
  op_path2d_add_path,
} from "ext:core/ops";

/**
 * Path2D class for reusable path objects
 */
class Path2D {
  #pathId;

  constructor(pathOrString) {
    if (pathOrString === undefined) {
      // Create empty path
      this.#pathId = op_path2d_create();
    } else if (typeof pathOrString === "string") {
      // Create from SVG path data
      this.#pathId = op_path2d_create_from_svg(pathOrString);
    } else if (pathOrString instanceof Path2D) {
      // Copy from another Path2D
      this.#pathId = op_path2d_create_from_path(pathOrString._getPathId());
    } else {
      // Unknown type, create empty
      this.#pathId = op_path2d_create();
    }
  }

  _getPathId() {
    return this.#pathId;
  }

  moveTo(x, y) {
    op_path2d_move_to(this.#pathId, x, y);
  }

  lineTo(x, y) {
    op_path2d_line_to(this.#pathId, x, y);
  }

  closePath() {
    op_path2d_close_path(this.#pathId);
  }

  bezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y) {
    op_path2d_bezier_curve_to(this.#pathId, cp1x, cp1y, cp2x, cp2y, x, y);
  }

  quadraticCurveTo(cpx, cpy, x, y) {
    op_path2d_quadratic_curve_to(this.#pathId, cpx, cpy, x, y);
  }

  rect(x, y, width, height) {
    op_path2d_rect(this.#pathId, x, y, width, height);
  }

  arc(x, y, radius, startAngle, endAngle, anticlockwise = false) {
    op_path2d_arc(this.#pathId, x, y, radius, startAngle, endAngle, anticlockwise);
  }

  arcTo(x1, y1, x2, y2, radius) {
    op_path2d_arc_to(this.#pathId, x1, y1, x2, y2, radius);
  }

  ellipse(x, y, radiusX, radiusY, rotation, startAngle, endAngle, anticlockwise = false) {
    op_path2d_ellipse(this.#pathId, x, y, radiusX, radiusY, rotation, startAngle, endAngle, anticlockwise);
  }

  roundRect(x, y, width, height, radii = 0) {
    if (typeof radii === "number") {
      op_path2d_round_rect(this.#pathId, x, y, width, height, radii);
    } else if (Array.isArray(radii)) {
      // Handle DOMPointInit objects or numbers in array - produce [x, y] pairs
      const xyRadii = radii.map(r => {
        if (typeof r === "number") return [r, r];
        return [r.x || 0, r.y || 0];
      });
      op_path2d_round_rect_radii(this.#pathId, x, y, width, height, xyRadii);
    }
  }

  addPath(path, transform) {
    if (!(path instanceof Path2D)) return;
    const t = transform ? [transform.a, transform.b, transform.c, transform.d, transform.e, transform.f] : null;
    op_path2d_add_path(this.#pathId, path._getPathId(), t);
  }
}

export { Path2D };

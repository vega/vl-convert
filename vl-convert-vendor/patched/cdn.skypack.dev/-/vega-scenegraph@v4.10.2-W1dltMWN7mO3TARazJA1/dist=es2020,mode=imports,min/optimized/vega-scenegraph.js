import {hasOwnProperty, inherits, lruCache, isArray, error, toSet, extend, isFunction, isNumber, isObject, truthy, array, peek} from "/-/vega-util@v1.17.1-uwuqwLZrXXBeO0DFYRgh/dist=es2020,mode=imports,min/optimized/vega-util.js";
import {arc as arc$2, area as area$2, line as line$2, symbol as symbol$2, curveBasis, curveBasisClosed, curveBasisOpen, curveBundle, curveCardinal, curveCardinalOpen, curveCardinalClosed, curveCatmullRom, curveCatmullRomClosed, curveCatmullRomOpen, curveLinear, curveLinearClosed, curveMonotoneY, curveMonotoneX, curveNatural, curveStep, curveStepAfter, curveStepBefore} from "/-/d3-shape@v3.2.0-jvLE9CjF3Vp4eEpVme8s/dist=es2020,mode=imports,min/optimized/d3-shape.js";
import {path as path$3} from "/-/d3-path@v3.1.0-nHaUoYzlRDYONpece9h0/dist=es2020,mode=imports,min/optimized/d3-path.js";
export {path} from "/-/d3-path@v3.1.0-nHaUoYzlRDYONpece9h0/dist=es2020,mode=imports,min/optimized/d3-path.js";
import {image as image$1, canvas} from "/-/vega-canvas@v1.2.7-hCEcvULuKIOqBVGX1Tn8/dist=es2020,mode=imports,min/optimized/vega-canvas.js";
import {loader} from "/-/vega-loader@v4.5.1-e2JpneCYErTzObWVOVxs/dist=es2020,mode=imports,min/optimized/vega-loader.js";
import {isDiscrete, domainCaption} from "/-/vega-scale@v7.3.0-RE8rHwByiw8oUoAe4pNs/dist=es2020,mode=imports,min/optimized/vega-scale.js";
let gradient_id = 0;
function resetSVGGradientId() {
    gradient_id = 0;
}
const patternPrefix = "p_";
function isGradient(value2) {
    return value2 && value2.gradient;
}
function gradientRef(g, defs, base2) {
    const type2 = g.gradient;
    let id = g.id, prefix = type2 === "radial" ? patternPrefix : "";
    if (!id) {
        id = g.id = "gradient_" + gradient_id++;
        if (type2 === "radial") {
            g.x1 = get(g.x1, 0.5);
            g.y1 = get(g.y1, 0.5);
            g.r1 = get(g.r1, 0);
            g.x2 = get(g.x2, 0.5);
            g.y2 = get(g.y2, 0.5);
            g.r2 = get(g.r2, 0.5);
            prefix = patternPrefix;
        } else {
            g.x1 = get(g.x1, 0);
            g.y1 = get(g.y1, 0);
            g.x2 = get(g.x2, 1);
            g.y2 = get(g.y2, 0);
        }
    }
    defs[id] = g;
    return "url(" + (base2 || "") + "#" + prefix + id + ")";
}
function get(val, def2) {
    return val != null ? val : def2;
}
function Gradient(p0, p1) {
    var stops = [], gradient2;
    return gradient2 = {
        gradient: "linear",
        x1: p0 ? p0[0] : 0,
        y1: p0 ? p0[1] : 0,
        x2: p1 ? p1[0] : 1,
        y2: p1 ? p1[1] : 0,
        stops,
        stop: function(offset2, color2) {
            stops.push({
                offset: offset2,
                color: color2
            });
            return gradient2;
        }
    };
}
const lookup = {
    basis: {
        curve: curveBasis
    },
    "basis-closed": {
        curve: curveBasisClosed
    },
    "basis-open": {
        curve: curveBasisOpen
    },
    bundle: {
        curve: curveBundle,
        tension: "beta",
        value: 0.85
    },
    cardinal: {
        curve: curveCardinal,
        tension: "tension",
        value: 0
    },
    "cardinal-open": {
        curve: curveCardinalOpen,
        tension: "tension",
        value: 0
    },
    "cardinal-closed": {
        curve: curveCardinalClosed,
        tension: "tension",
        value: 0
    },
    "catmull-rom": {
        curve: curveCatmullRom,
        tension: "alpha",
        value: 0.5
    },
    "catmull-rom-closed": {
        curve: curveCatmullRomClosed,
        tension: "alpha",
        value: 0.5
    },
    "catmull-rom-open": {
        curve: curveCatmullRomOpen,
        tension: "alpha",
        value: 0.5
    },
    linear: {
        curve: curveLinear
    },
    "linear-closed": {
        curve: curveLinearClosed
    },
    monotone: {
        horizontal: curveMonotoneY,
        vertical: curveMonotoneX
    },
    natural: {
        curve: curveNatural
    },
    step: {
        curve: curveStep
    },
    "step-after": {
        curve: curveStepAfter
    },
    "step-before": {
        curve: curveStepBefore
    }
};
function curves(type2, orientation, tension) {
    var entry = hasOwnProperty(lookup, type2) && lookup[type2], curve = null;
    if (entry) {
        curve = entry.curve || entry[orientation || "vertical"];
        if (entry.tension && tension != null) {
            curve = curve[entry.tension](tension);
        }
    }
    return curve;
}
const paramCounts = {
    m: 2,
    l: 2,
    h: 1,
    v: 1,
    z: 0,
    c: 6,
    s: 4,
    q: 4,
    t: 2,
    a: 7
};
const commandPattern = /[mlhvzcsqta]([^mlhvzcsqta]+|$)/gi;
const numberPattern = /^[+-]?(([0-9]*\.[0-9]+)|([0-9]+\.)|([0-9]+))([eE][+-]?[0-9]+)?/;
const spacePattern = /^((\s+,?\s*)|(,\s*))/;
const flagPattern = /^[01]/;
function parse(path3) {
    const commands = [];
    const matches = path3.match(commandPattern) || [];
    matches.forEach((str) => {
        let cmd = str[0];
        const type2 = cmd.toLowerCase();
        const paramCount = paramCounts[type2];
        const params = parseParams(type2, paramCount, str.slice(1).trim());
        const count = params.length;
        if (count < paramCount || count && count % paramCount !== 0) {
            throw Error("Invalid SVG path, incorrect parameter count");
        }
        commands.push([cmd, ...params.slice(0, paramCount)]);
        if (count === paramCount) {
            return;
        }
        if (type2 === "m") {
            cmd = cmd === "M" ? "L" : "l";
        }
        for (let i = paramCount; i < count; i += paramCount) {
            commands.push([cmd, ...params.slice(i, i + paramCount)]);
        }
    });
    return commands;
}
function parseParams(type2, paramCount, segment) {
    const params = [];
    for (let index = 0; paramCount && index < segment.length; ) {
        for (let i = 0; i < paramCount; ++i) {
            const pattern = type2 === "a" && (i === 3 || i === 4) ? flagPattern : numberPattern;
            const match = segment.slice(index).match(pattern);
            if (match === null) {
                throw Error("Invalid SVG path, incorrect parameter type");
            }
            index += match[0].length;
            params.push(+match[0]);
            const ws = segment.slice(index).match(spacePattern);
            if (ws !== null) {
                index += ws[0].length;
            }
        }
    }
    return params;
}
const DegToRad = Math.PI / 180;
const Epsilon = 1e-14;
const HalfPi = Math.PI / 2;
const Tau = Math.PI * 2;
const HalfSqrt3 = Math.sqrt(3) / 2;
var segmentCache = {};
var bezierCache = {};
var join = [].join;
function segments(x2, y2, rx, ry, large, sweep, rotateX, ox, oy) {
    const key = join.call(arguments);
    if (segmentCache[key]) {
        return segmentCache[key];
    }
    const th = rotateX * DegToRad;
    const sin_th = Math.sin(th);
    const cos_th = Math.cos(th);
    rx = Math.abs(rx);
    ry = Math.abs(ry);
    const px2 = cos_th * (ox - x2) * 0.5 + sin_th * (oy - y2) * 0.5;
    const py2 = cos_th * (oy - y2) * 0.5 - sin_th * (ox - x2) * 0.5;
    let pl = px2 * px2 / (rx * rx) + py2 * py2 / (ry * ry);
    if (pl > 1) {
        pl = Math.sqrt(pl);
        rx *= pl;
        ry *= pl;
    }
    const a00 = cos_th / rx;
    const a01 = sin_th / rx;
    const a10 = -sin_th / ry;
    const a11 = cos_th / ry;
    const x0 = a00 * ox + a01 * oy;
    const y0 = a10 * ox + a11 * oy;
    const x1 = a00 * x2 + a01 * y2;
    const y1 = a10 * x2 + a11 * y2;
    const d = (x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0);
    let sfactor_sq = 1 / d - 0.25;
    if (sfactor_sq < 0)
        sfactor_sq = 0;
    let sfactor = Math.sqrt(sfactor_sq);
    if (sweep == large)
        sfactor = -sfactor;
    const xc = 0.5 * (x0 + x1) - sfactor * (y1 - y0);
    const yc = 0.5 * (y0 + y1) + sfactor * (x1 - x0);
    const th0 = Math.atan2(y0 - yc, x0 - xc);
    const th1 = Math.atan2(y1 - yc, x1 - xc);
    let th_arc = th1 - th0;
    if (th_arc < 0 && sweep === 1) {
        th_arc += Tau;
    } else if (th_arc > 0 && sweep === 0) {
        th_arc -= Tau;
    }
    const segs = Math.ceil(Math.abs(th_arc / (HalfPi + 1e-3)));
    const result = [];
    for (let i = 0; i < segs; ++i) {
        const th2 = th0 + i * th_arc / segs;
        const th3 = th0 + (i + 1) * th_arc / segs;
        result[i] = [xc, yc, th2, th3, rx, ry, sin_th, cos_th];
    }
    return segmentCache[key] = result;
}
function bezier(params) {
    const key = join.call(params);
    if (bezierCache[key]) {
        return bezierCache[key];
    }
    var cx = params[0], cy = params[1], th0 = params[2], th1 = params[3], rx = params[4], ry = params[5], sin_th = params[6], cos_th = params[7];
    const a00 = cos_th * rx;
    const a01 = -sin_th * ry;
    const a10 = sin_th * rx;
    const a11 = cos_th * ry;
    const cos_th0 = Math.cos(th0);
    const sin_th0 = Math.sin(th0);
    const cos_th1 = Math.cos(th1);
    const sin_th1 = Math.sin(th1);
    const th_half = 0.5 * (th1 - th0);
    const sin_th_h2 = Math.sin(th_half * 0.5);
    const t = 8 / 3 * sin_th_h2 * sin_th_h2 / Math.sin(th_half);
    const x1 = cx + cos_th0 - t * sin_th0;
    const y1 = cy + sin_th0 + t * cos_th0;
    const x3 = cx + cos_th1;
    const y3 = cy + sin_th1;
    const x2 = x3 + t * sin_th1;
    const y2 = y3 - t * cos_th1;
    return bezierCache[key] = [a00 * x1 + a01 * y1, a10 * x1 + a11 * y1, a00 * x2 + a01 * y2, a10 * x2 + a11 * y2, a00 * x3 + a01 * y3, a10 * x3 + a11 * y3];
}
const temp = ["l", 0, 0, 0, 0, 0, 0, 0];
function scale$1(current, sX, sY) {
    const c = temp[0] = current[0];
    if (c === "a" || c === "A") {
        temp[1] = sX * current[1];
        temp[2] = sY * current[2];
        temp[3] = current[3];
        temp[4] = current[4];
        temp[5] = current[5];
        temp[6] = sX * current[6];
        temp[7] = sY * current[7];
    } else if (c === "h" || c === "H") {
        temp[1] = sX * current[1];
    } else if (c === "v" || c === "V") {
        temp[1] = sY * current[1];
    } else {
        for (var i = 1, n = current.length; i < n; ++i) {
            temp[i] = (i % 2 == 1 ? sX : sY) * current[i];
        }
    }
    return temp;
}
function pathRender(context2, path3, l, t, sX, sY) {
    var current, previous = null, x2 = 0, y2 = 0, controlX = 0, controlY = 0, tempX, tempY, tempControlX, tempControlY, anchorX = 0, anchorY = 0;
    if (l == null)
        l = 0;
    if (t == null)
        t = 0;
    if (sX == null)
        sX = 1;
    if (sY == null)
        sY = sX;
    if (context2.beginPath)
        context2.beginPath();
    for (var i = 0, len = path3.length; i < len; ++i) {
        current = path3[i];
        if (sX !== 1 || sY !== 1) {
            current = scale$1(current, sX, sY);
        }
        switch (current[0]) {
            case "l":
                x2 += current[1];
                y2 += current[2];
                context2.lineTo(x2 + l, y2 + t);
                break;
            case "L":
                x2 = current[1];
                y2 = current[2];
                context2.lineTo(x2 + l, y2 + t);
                break;
            case "h":
                x2 += current[1];
                context2.lineTo(x2 + l, y2 + t);
                break;
            case "H":
                x2 = current[1];
                context2.lineTo(x2 + l, y2 + t);
                break;
            case "v":
                y2 += current[1];
                context2.lineTo(x2 + l, y2 + t);
                break;
            case "V":
                y2 = current[1];
                context2.lineTo(x2 + l, y2 + t);
                break;
            case "m":
                x2 += current[1];
                y2 += current[2];
                anchorX = x2;
                anchorY = y2;
                context2.moveTo(x2 + l, y2 + t);
                break;
            case "M":
                x2 = current[1];
                y2 = current[2];
                anchorX = x2;
                anchorY = y2;
                context2.moveTo(x2 + l, y2 + t);
                break;
            case "c":
                tempX = x2 + current[5];
                tempY = y2 + current[6];
                controlX = x2 + current[3];
                controlY = y2 + current[4];
                context2.bezierCurveTo(x2 + current[1] + l, y2 + current[2] + t, controlX + l, controlY + t, tempX + l, tempY + t);
                x2 = tempX;
                y2 = tempY;
                break;
            case "C":
                x2 = current[5];
                y2 = current[6];
                controlX = current[3];
                controlY = current[4];
                context2.bezierCurveTo(current[1] + l, current[2] + t, controlX + l, controlY + t, x2 + l, y2 + t);
                break;
            case "s":
                tempX = x2 + current[3];
                tempY = y2 + current[4];
                controlX = 2 * x2 - controlX;
                controlY = 2 * y2 - controlY;
                context2.bezierCurveTo(controlX + l, controlY + t, x2 + current[1] + l, y2 + current[2] + t, tempX + l, tempY + t);
                controlX = x2 + current[1];
                controlY = y2 + current[2];
                x2 = tempX;
                y2 = tempY;
                break;
            case "S":
                tempX = current[3];
                tempY = current[4];
                controlX = 2 * x2 - controlX;
                controlY = 2 * y2 - controlY;
                context2.bezierCurveTo(controlX + l, controlY + t, current[1] + l, current[2] + t, tempX + l, tempY + t);
                x2 = tempX;
                y2 = tempY;
                controlX = current[1];
                controlY = current[2];
                break;
            case "q":
                tempX = x2 + current[3];
                tempY = y2 + current[4];
                controlX = x2 + current[1];
                controlY = y2 + current[2];
                context2.quadraticCurveTo(controlX + l, controlY + t, tempX + l, tempY + t);
                x2 = tempX;
                y2 = tempY;
                break;
            case "Q":
                tempX = current[3];
                tempY = current[4];
                context2.quadraticCurveTo(current[1] + l, current[2] + t, tempX + l, tempY + t);
                x2 = tempX;
                y2 = tempY;
                controlX = current[1];
                controlY = current[2];
                break;
            case "t":
                tempX = x2 + current[1];
                tempY = y2 + current[2];
                if (previous[0].match(/[QqTt]/) === null) {
                    controlX = x2;
                    controlY = y2;
                } else if (previous[0] === "t") {
                    controlX = 2 * x2 - tempControlX;
                    controlY = 2 * y2 - tempControlY;
                } else if (previous[0] === "q") {
                    controlX = 2 * x2 - controlX;
                    controlY = 2 * y2 - controlY;
                }
                tempControlX = controlX;
                tempControlY = controlY;
                context2.quadraticCurveTo(controlX + l, controlY + t, tempX + l, tempY + t);
                x2 = tempX;
                y2 = tempY;
                controlX = x2 + current[1];
                controlY = y2 + current[2];
                break;
            case "T":
                tempX = current[1];
                tempY = current[2];
                controlX = 2 * x2 - controlX;
                controlY = 2 * y2 - controlY;
                context2.quadraticCurveTo(controlX + l, controlY + t, tempX + l, tempY + t);
                x2 = tempX;
                y2 = tempY;
                break;
            case "a":
                drawArc(context2, x2 + l, y2 + t, [current[1], current[2], current[3], current[4], current[5], current[6] + x2 + l, current[7] + y2 + t]);
                x2 += current[6];
                y2 += current[7];
                break;
            case "A":
                drawArc(context2, x2 + l, y2 + t, [current[1], current[2], current[3], current[4], current[5], current[6] + l, current[7] + t]);
                x2 = current[6];
                y2 = current[7];
                break;
            case "z":
            case "Z":
                x2 = anchorX;
                y2 = anchorY;
                context2.closePath();
                break;
        }
        previous = current;
    }
}
function drawArc(context2, x2, y2, coords) {
    const seg = segments(coords[5], coords[6], coords[0], coords[1], coords[3], coords[4], coords[2], x2, y2);
    for (let i = 0; i < seg.length; ++i) {
        const bez = bezier(seg[i]);
        context2.bezierCurveTo(bez[0], bez[1], bez[2], bez[3], bez[4], bez[5]);
    }
}
const Tan30 = 0.5773502691896257;
const builtins = {
    circle: {
        draw: function(context2, size) {
            const r = Math.sqrt(size) / 2;
            context2.moveTo(r, 0);
            context2.arc(0, 0, r, 0, Tau);
        }
    },
    cross: {
        draw: function(context2, size) {
            var r = Math.sqrt(size) / 2, s = r / 2.5;
            context2.moveTo(-r, -s);
            context2.lineTo(-r, s);
            context2.lineTo(-s, s);
            context2.lineTo(-s, r);
            context2.lineTo(s, r);
            context2.lineTo(s, s);
            context2.lineTo(r, s);
            context2.lineTo(r, -s);
            context2.lineTo(s, -s);
            context2.lineTo(s, -r);
            context2.lineTo(-s, -r);
            context2.lineTo(-s, -s);
            context2.closePath();
        }
    },
    diamond: {
        draw: function(context2, size) {
            const r = Math.sqrt(size) / 2;
            context2.moveTo(-r, 0);
            context2.lineTo(0, -r);
            context2.lineTo(r, 0);
            context2.lineTo(0, r);
            context2.closePath();
        }
    },
    square: {
        draw: function(context2, size) {
            var w2 = Math.sqrt(size), x2 = -w2 / 2;
            context2.rect(x2, x2, w2, w2);
        }
    },
    arrow: {
        draw: function(context2, size) {
            var r = Math.sqrt(size) / 2, s = r / 7, t = r / 2.5, v = r / 8;
            context2.moveTo(-s, r);
            context2.lineTo(s, r);
            context2.lineTo(s, -v);
            context2.lineTo(t, -v);
            context2.lineTo(0, -r);
            context2.lineTo(-t, -v);
            context2.lineTo(-s, -v);
            context2.closePath();
        }
    },
    wedge: {
        draw: function(context2, size) {
            var r = Math.sqrt(size) / 2, h2 = HalfSqrt3 * r, o = h2 - r * Tan30, b2 = r / 4;
            context2.moveTo(0, -h2 - o);
            context2.lineTo(-b2, h2 - o);
            context2.lineTo(b2, h2 - o);
            context2.closePath();
        }
    },
    triangle: {
        draw: function(context2, size) {
            var r = Math.sqrt(size) / 2, h2 = HalfSqrt3 * r, o = h2 - r * Tan30;
            context2.moveTo(0, -h2 - o);
            context2.lineTo(-r, h2 - o);
            context2.lineTo(r, h2 - o);
            context2.closePath();
        }
    },
    "triangle-up": {
        draw: function(context2, size) {
            var r = Math.sqrt(size) / 2, h2 = HalfSqrt3 * r;
            context2.moveTo(0, -h2);
            context2.lineTo(-r, h2);
            context2.lineTo(r, h2);
            context2.closePath();
        }
    },
    "triangle-down": {
        draw: function(context2, size) {
            var r = Math.sqrt(size) / 2, h2 = HalfSqrt3 * r;
            context2.moveTo(0, h2);
            context2.lineTo(-r, -h2);
            context2.lineTo(r, -h2);
            context2.closePath();
        }
    },
    "triangle-right": {
        draw: function(context2, size) {
            var r = Math.sqrt(size) / 2, h2 = HalfSqrt3 * r;
            context2.moveTo(h2, 0);
            context2.lineTo(-h2, -r);
            context2.lineTo(-h2, r);
            context2.closePath();
        }
    },
    "triangle-left": {
        draw: function(context2, size) {
            var r = Math.sqrt(size) / 2, h2 = HalfSqrt3 * r;
            context2.moveTo(-h2, 0);
            context2.lineTo(h2, -r);
            context2.lineTo(h2, r);
            context2.closePath();
        }
    },
    stroke: {
        draw: function(context2, size) {
            const r = Math.sqrt(size) / 2;
            context2.moveTo(-r, 0);
            context2.lineTo(r, 0);
        }
    }
};
function symbols(_) {
    return hasOwnProperty(builtins, _) ? builtins[_] : customSymbol(_);
}
var custom = {};
function customSymbol(path3) {
    if (!hasOwnProperty(custom, path3)) {
        const parsed = parse(path3);
        custom[path3] = {
            draw: function(context2, size) {
                pathRender(context2, parsed, 0, 0, Math.sqrt(size) / 2);
            }
        };
    }
    return custom[path3];
}
const C = 0.448084975506;
function rectangleX(d) {
    return d.x;
}
function rectangleY(d) {
    return d.y;
}
function rectangleWidth(d) {
    return d.width;
}
function rectangleHeight(d) {
    return d.height;
}
function number(_) {
    return typeof _ === "function" ? _ : () => +_;
}
function clamp(value2, min, max) {
    return Math.max(min, Math.min(value2, max));
}
function vg_rect() {
    var x2 = rectangleX, y2 = rectangleY, width = rectangleWidth, height = rectangleHeight, crTL = number(0), crTR = crTL, crBL = crTL, crBR = crTL, context2 = null;
    function rectangle2(_, x0, y0) {
        var buffer, x1 = x0 != null ? x0 : +x2.call(this, _), y1 = y0 != null ? y0 : +y2.call(this, _), w2 = +width.call(this, _), h2 = +height.call(this, _), s = Math.min(w2, h2) / 2, tl2 = clamp(+crTL.call(this, _), 0, s), tr2 = clamp(+crTR.call(this, _), 0, s), bl2 = clamp(+crBL.call(this, _), 0, s), br2 = clamp(+crBR.call(this, _), 0, s);
        if (!context2)
            context2 = buffer = path$3();
        if (tl2 <= 0 && tr2 <= 0 && bl2 <= 0 && br2 <= 0) {
            context2.rect(x1, y1, w2, h2);
        } else {
            var x22 = x1 + w2, y22 = y1 + h2;
            context2.moveTo(x1 + tl2, y1);
            context2.lineTo(x22 - tr2, y1);
            context2.bezierCurveTo(x22 - C * tr2, y1, x22, y1 + C * tr2, x22, y1 + tr2);
            context2.lineTo(x22, y22 - br2);
            context2.bezierCurveTo(x22, y22 - C * br2, x22 - C * br2, y22, x22 - br2, y22);
            context2.lineTo(x1 + bl2, y22);
            context2.bezierCurveTo(x1 + C * bl2, y22, x1, y22 - C * bl2, x1, y22 - bl2);
            context2.lineTo(x1, y1 + tl2);
            context2.bezierCurveTo(x1, y1 + C * tl2, x1 + C * tl2, y1, x1 + tl2, y1);
            context2.closePath();
        }
        if (buffer) {
            context2 = null;
            return buffer + "" || null;
        }
    }
    rectangle2.x = function(_) {
        if (arguments.length) {
            x2 = number(_);
            return rectangle2;
        } else {
            return x2;
        }
    };
    rectangle2.y = function(_) {
        if (arguments.length) {
            y2 = number(_);
            return rectangle2;
        } else {
            return y2;
        }
    };
    rectangle2.width = function(_) {
        if (arguments.length) {
            width = number(_);
            return rectangle2;
        } else {
            return width;
        }
    };
    rectangle2.height = function(_) {
        if (arguments.length) {
            height = number(_);
            return rectangle2;
        } else {
            return height;
        }
    };
    rectangle2.cornerRadius = function(tl2, tr2, br2, bl2) {
        if (arguments.length) {
            crTL = number(tl2);
            crTR = tr2 != null ? number(tr2) : crTL;
            crBR = br2 != null ? number(br2) : crTL;
            crBL = bl2 != null ? number(bl2) : crTR;
            return rectangle2;
        } else {
            return crTL;
        }
    };
    rectangle2.context = function(_) {
        if (arguments.length) {
            context2 = _ == null ? null : _;
            return rectangle2;
        } else {
            return context2;
        }
    };
    return rectangle2;
}
function vg_trail() {
    var x2, y2, size, defined, context2 = null, ready, x1, y1, r1;
    function point2(x22, y22, w2) {
        const r2 = w2 / 2;
        if (ready) {
            var ux = y1 - y22, uy = x22 - x1;
            if (ux || uy) {
                var ud = Math.sqrt(ux * ux + uy * uy), rx = (ux /= ud) * r1, ry = (uy /= ud) * r1, t = Math.atan2(uy, ux);
                context2.moveTo(x1 - rx, y1 - ry);
                context2.lineTo(x22 - ux * r2, y22 - uy * r2);
                context2.arc(x22, y22, r2, t - Math.PI, t);
                context2.lineTo(x1 + rx, y1 + ry);
                context2.arc(x1, y1, r1, t, t + Math.PI);
            } else {
                context2.arc(x22, y22, r2, 0, Tau);
            }
            context2.closePath();
        } else {
            ready = 1;
        }
        x1 = x22;
        y1 = y22;
        r1 = r2;
    }
    function trail2(data) {
        var i, n = data.length, d, defined0 = false, buffer;
        if (context2 == null)
            context2 = buffer = path$3();
        for (i = 0; i <= n; ++i) {
            if (!(i < n && defined(d = data[i], i, data)) === defined0) {
                if (defined0 = !defined0)
                    ready = 0;
            }
            if (defined0)
                point2(+x2(d, i, data), +y2(d, i, data), +size(d, i, data));
        }
        if (buffer) {
            context2 = null;
            return buffer + "" || null;
        }
    }
    trail2.x = function(_) {
        if (arguments.length) {
            x2 = _;
            return trail2;
        } else {
            return x2;
        }
    };
    trail2.y = function(_) {
        if (arguments.length) {
            y2 = _;
            return trail2;
        } else {
            return y2;
        }
    };
    trail2.size = function(_) {
        if (arguments.length) {
            size = _;
            return trail2;
        } else {
            return size;
        }
    };
    trail2.defined = function(_) {
        if (arguments.length) {
            defined = _;
            return trail2;
        } else {
            return defined;
        }
    };
    trail2.context = function(_) {
        if (arguments.length) {
            if (_ == null) {
                context2 = null;
            } else {
                context2 = _;
            }
            return trail2;
        } else {
            return context2;
        }
    };
    return trail2;
}
function value$1(a, b2) {
    return a != null ? a : b2;
}
const x = (item) => item.x || 0, y = (item) => item.y || 0, w = (item) => item.width || 0, h = (item) => item.height || 0, xw = (item) => (item.x || 0) + (item.width || 0), yh = (item) => (item.y || 0) + (item.height || 0), sa = (item) => item.startAngle || 0, ea = (item) => item.endAngle || 0, pa = (item) => item.padAngle || 0, ir = (item) => item.innerRadius || 0, or = (item) => item.outerRadius || 0, cr = (item) => item.cornerRadius || 0, tl = (item) => value$1(item.cornerRadiusTopLeft, item.cornerRadius) || 0, tr = (item) => value$1(item.cornerRadiusTopRight, item.cornerRadius) || 0, br = (item) => value$1(item.cornerRadiusBottomRight, item.cornerRadius) || 0, bl = (item) => value$1(item.cornerRadiusBottomLeft, item.cornerRadius) || 0, sz = (item) => value$1(item.size, 64), ts = (item) => item.size || 1, def = (item) => !(item.defined === false), type = (item) => symbols(item.shape || "circle");
const arcShape = arc$2().startAngle(sa).endAngle(ea).padAngle(pa).innerRadius(ir).outerRadius(or).cornerRadius(cr), areavShape = area$2().x(x).y1(y).y0(yh).defined(def), areahShape = area$2().y(y).x1(x).x0(xw).defined(def), lineShape = line$2().x(x).y(y).defined(def), rectShape = vg_rect().x(x).y(y).width(w).height(h).cornerRadius(tl, tr, br, bl), symbolShape = symbol$2().type(type).size(sz), trailShape = vg_trail().x(x).y(y).defined(def).size(ts);
function hasCornerRadius(item) {
    return item.cornerRadius || item.cornerRadiusTopLeft || item.cornerRadiusTopRight || item.cornerRadiusBottomRight || item.cornerRadiusBottomLeft;
}
function arc$1(context2, item) {
    return arcShape.context(context2)(item);
}
function area$1(context2, items) {
    const item = items[0], interp = item.interpolate || "linear";
    return (item.orient === "horizontal" ? areahShape : areavShape).curve(curves(interp, item.orient, item.tension)).context(context2)(items);
}
function line$1(context2, items) {
    const item = items[0], interp = item.interpolate || "linear";
    return lineShape.curve(curves(interp, item.orient, item.tension)).context(context2)(items);
}
function rectangle(context2, item, x2, y2) {
    return rectShape.context(context2)(item, x2, y2);
}
function shape$1(context2, item) {
    return (item.mark.shape || item.shape).context(context2)(item);
}
function symbol$1(context2, item) {
    return symbolShape.context(context2)(item);
}
function trail$1(context2, items) {
    return trailShape.context(context2)(items);
}
var clip_id = 1;
function resetSVGClipId() {
    clip_id = 1;
}
function clip$1(renderer, item, size) {
    var clip2 = item.clip, defs = renderer._defs, id = item.clip_id || (item.clip_id = "clip" + clip_id++), c = defs.clipping[id] || (defs.clipping[id] = {
        id
    });
    if (isFunction(clip2)) {
        c.path = clip2(null);
    } else if (hasCornerRadius(size)) {
        c.path = rectangle(null, size, 0, 0);
    } else {
        c.width = size.width || 0;
        c.height = size.height || 0;
    }
    return "url(#" + id + ")";
}
function Bounds(b2) {
    this.clear();
    if (b2)
        this.union(b2);
}
Bounds.prototype = {
    clone() {
        return new Bounds(this);
    },
    clear() {
        this.x1 = +Number.MAX_VALUE;
        this.y1 = +Number.MAX_VALUE;
        this.x2 = -Number.MAX_VALUE;
        this.y2 = -Number.MAX_VALUE;
        return this;
    },
    empty() {
        return this.x1 === +Number.MAX_VALUE && this.y1 === +Number.MAX_VALUE && this.x2 === -Number.MAX_VALUE && this.y2 === -Number.MAX_VALUE;
    },
    equals(b2) {
        return this.x1 === b2.x1 && this.y1 === b2.y1 && this.x2 === b2.x2 && this.y2 === b2.y2;
    },
    set(x1, y1, x2, y2) {
        if (x2 < x1) {
            this.x2 = x1;
            this.x1 = x2;
        } else {
            this.x1 = x1;
            this.x2 = x2;
        }
        if (y2 < y1) {
            this.y2 = y1;
            this.y1 = y2;
        } else {
            this.y1 = y1;
            this.y2 = y2;
        }
        return this;
    },
    add(x2, y2) {
        if (x2 < this.x1)
            this.x1 = x2;
        if (y2 < this.y1)
            this.y1 = y2;
        if (x2 > this.x2)
            this.x2 = x2;
        if (y2 > this.y2)
            this.y2 = y2;
        return this;
    },
    expand(d) {
        this.x1 -= d;
        this.y1 -= d;
        this.x2 += d;
        this.y2 += d;
        return this;
    },
    round() {
        this.x1 = Math.floor(this.x1);
        this.y1 = Math.floor(this.y1);
        this.x2 = Math.ceil(this.x2);
        this.y2 = Math.ceil(this.y2);
        return this;
    },
    scale(s) {
        this.x1 *= s;
        this.y1 *= s;
        this.x2 *= s;
        this.y2 *= s;
        return this;
    },
    translate(dx, dy) {
        this.x1 += dx;
        this.x2 += dx;
        this.y1 += dy;
        this.y2 += dy;
        return this;
    },
    rotate(angle, x2, y2) {
        const p = this.rotatedPoints(angle, x2, y2);
        return this.clear().add(p[0], p[1]).add(p[2], p[3]).add(p[4], p[5]).add(p[6], p[7]);
    },
    rotatedPoints(angle, x2, y2) {
        var {
            x1,
            y1,
            x2: x22,
            y2: y22
        } = this, cos = Math.cos(angle), sin = Math.sin(angle), cx = x2 - x2 * cos + y2 * sin, cy = y2 - x2 * sin - y2 * cos;
        return [cos * x1 - sin * y1 + cx, sin * x1 + cos * y1 + cy, cos * x1 - sin * y22 + cx, sin * x1 + cos * y22 + cy, cos * x22 - sin * y1 + cx, sin * x22 + cos * y1 + cy, cos * x22 - sin * y22 + cx, sin * x22 + cos * y22 + cy];
    },
    union(b2) {
        if (b2.x1 < this.x1)
            this.x1 = b2.x1;
        if (b2.y1 < this.y1)
            this.y1 = b2.y1;
        if (b2.x2 > this.x2)
            this.x2 = b2.x2;
        if (b2.y2 > this.y2)
            this.y2 = b2.y2;
        return this;
    },
    intersect(b2) {
        if (b2.x1 > this.x1)
            this.x1 = b2.x1;
        if (b2.y1 > this.y1)
            this.y1 = b2.y1;
        if (b2.x2 < this.x2)
            this.x2 = b2.x2;
        if (b2.y2 < this.y2)
            this.y2 = b2.y2;
        return this;
    },
    encloses(b2) {
        return b2 && this.x1 <= b2.x1 && this.x2 >= b2.x2 && this.y1 <= b2.y1 && this.y2 >= b2.y2;
    },
    alignsWith(b2) {
        return b2 && (this.x1 == b2.x1 || this.x2 == b2.x2 || this.y1 == b2.y1 || this.y2 == b2.y2);
    },
    intersects(b2) {
        return b2 && !(this.x2 < b2.x1 || this.x1 > b2.x2 || this.y2 < b2.y1 || this.y1 > b2.y2);
    },
    contains(x2, y2) {
        return !(x2 < this.x1 || x2 > this.x2 || y2 < this.y1 || y2 > this.y2);
    },
    width() {
        return this.x2 - this.x1;
    },
    height() {
        return this.y2 - this.y1;
    }
};
function Item(mark) {
    this.mark = mark;
    this.bounds = this.bounds || new Bounds();
}
function GroupItem(mark) {
    Item.call(this, mark);
    this.items = this.items || [];
}
inherits(GroupItem, Item);
function ResourceLoader(customLoader) {
    this._pending = 0;
    this._loader = customLoader || loader();
}
function increment(loader2) {
    loader2._pending += 1;
}
function decrement(loader2) {
    loader2._pending -= 1;
}
ResourceLoader.prototype = {
    pending() {
        return this._pending;
    },
    sanitizeURL(uri) {
        const loader2 = this;
        increment(loader2);
        return loader2._loader.sanitize(uri, {
            context: "href"
        }).then((opt) => {
            decrement(loader2);
            return opt;
        }).catch(() => {
            decrement(loader2);
            return null;
        });
    },
    loadImage(uri) {
        const loader2 = this, Image = image$1();
        increment(loader2);
        return loader2._loader.sanitize(uri, {
            context: "image"
        }).then((opt) => {
            const url = opt.href;
            if (!url || !Image)
                throw {
                    url
                };
            const img = new Image();
            const cors = hasOwnProperty(opt, "crossOrigin") ? opt.crossOrigin : "anonymous";
            if (cors != null)
                img.crossOrigin = cors;
            img.onload = () => decrement(loader2);
            img.onerror = () => decrement(loader2);
            img.src = url;
            return img;
        }).catch((e) => {
            decrement(loader2);
            return {
                complete: false,
                width: 0,
                height: 0,
                src: e && e.url || ""
            };
        });
    },
    ready() {
        const loader2 = this;
        return new Promise((accept) => {
            function poll(value2) {
                if (!loader2.pending())
                    accept(value2);
                else
                    setTimeout(() => {
                        poll(true);
                    }, 10);
            }
            poll(false);
        });
    }
};
function boundStroke(bounds2, item, miter) {
    if (item.stroke && item.opacity !== 0 && item.strokeOpacity !== 0) {
        const sw = item.strokeWidth != null ? +item.strokeWidth : 1;
        bounds2.expand(sw + (miter ? miterAdjustment(item, sw) : 0));
    }
    return bounds2;
}
function miterAdjustment(item, strokeWidth) {
    return item.strokeJoin && item.strokeJoin !== "miter" ? 0 : strokeWidth;
}
const circleThreshold = Tau - 1e-8;
let bounds, lx, ly, rot, ma, mb, mc, md;
const add = (x2, y2) => bounds.add(x2, y2);
const addL = (x2, y2) => add(lx = x2, ly = y2);
const addX = (x2) => add(x2, bounds.y1);
const addY = (y2) => add(bounds.x1, y2);
const px = (x2, y2) => ma * x2 + mc * y2;
const py = (x2, y2) => mb * x2 + md * y2;
const addp = (x2, y2) => add(px(x2, y2), py(x2, y2));
const addpL = (x2, y2) => addL(px(x2, y2), py(x2, y2));
function boundContext(_, deg) {
    bounds = _;
    if (deg) {
        rot = deg * DegToRad;
        ma = md = Math.cos(rot);
        mb = Math.sin(rot);
        mc = -mb;
    } else {
        ma = md = 1;
        rot = mb = mc = 0;
    }
    return context$1;
}
const context$1 = {
    beginPath() {
    },
    closePath() {
    },
    moveTo: addpL,
    lineTo: addpL,
    rect(x2, y2, w2, h2) {
        if (rot) {
            addp(x2 + w2, y2);
            addp(x2 + w2, y2 + h2);
            addp(x2, y2 + h2);
            addpL(x2, y2);
        } else {
            add(x2 + w2, y2 + h2);
            addL(x2, y2);
        }
    },
    quadraticCurveTo(x1, y1, x2, y2) {
        const px1 = px(x1, y1), py1 = py(x1, y1), px2 = px(x2, y2), py2 = py(x2, y2);
        quadExtrema(lx, px1, px2, addX);
        quadExtrema(ly, py1, py2, addY);
        addL(px2, py2);
    },
    bezierCurveTo(x1, y1, x2, y2, x3, y3) {
        const px1 = px(x1, y1), py1 = py(x1, y1), px2 = px(x2, y2), py2 = py(x2, y2), px3 = px(x3, y3), py3 = py(x3, y3);
        cubicExtrema(lx, px1, px2, px3, addX);
        cubicExtrema(ly, py1, py2, py3, addY);
        addL(px3, py3);
    },
    arc(cx, cy, r, sa2, ea2, ccw) {
        sa2 += rot;
        ea2 += rot;
        lx = r * Math.cos(ea2) + cx;
        ly = r * Math.sin(ea2) + cy;
        if (Math.abs(ea2 - sa2) > circleThreshold) {
            add(cx - r, cy - r);
            add(cx + r, cy + r);
        } else {
            const update = (a) => add(r * Math.cos(a) + cx, r * Math.sin(a) + cy);
            let s, i;
            update(sa2);
            update(ea2);
            if (ea2 !== sa2) {
                sa2 = sa2 % Tau;
                if (sa2 < 0)
                    sa2 += Tau;
                ea2 = ea2 % Tau;
                if (ea2 < 0)
                    ea2 += Tau;
                if (ea2 < sa2) {
                    ccw = !ccw;
                    s = sa2;
                    sa2 = ea2;
                    ea2 = s;
                }
                if (ccw) {
                    ea2 -= Tau;
                    s = sa2 - sa2 % HalfPi;
                    for (i = 0; i < 4 && s > ea2; ++i, s -= HalfPi)
                        update(s);
                } else {
                    s = sa2 - sa2 % HalfPi + HalfPi;
                    for (i = 0; i < 4 && s < ea2; ++i, s = s + HalfPi)
                        update(s);
                }
            }
        }
    }
};
function quadExtrema(x0, x1, x2, cb) {
    const t = (x0 - x1) / (x0 + x2 - 2 * x1);
    if (0 < t && t < 1)
        cb(x0 + (x1 - x0) * t);
}
function cubicExtrema(x0, x1, x2, x3, cb) {
    const a = x3 - x0 + 3 * x1 - 3 * x2, b2 = x0 + x2 - 2 * x1, c = x0 - x1;
    let t0 = 0, t1 = 0, r;
    if (Math.abs(a) > Epsilon) {
        r = b2 * b2 + c * a;
        if (r >= 0) {
            r = Math.sqrt(r);
            t0 = (-b2 + r) / a;
            t1 = (-b2 - r) / a;
        }
    } else {
        t0 = 0.5 * c / b2;
    }
    if (0 < t0 && t0 < 1)
        cb(cubic(t0, x0, x1, x2, x3));
    if (0 < t1 && t1 < 1)
        cb(cubic(t1, x0, x1, x2, x3));
}
function cubic(t, x0, x1, x2, x3) {
    const s = 1 - t, s2 = s * s, t2 = t * t;
    return s2 * s * x0 + 3 * s2 * t * x1 + 3 * s * t2 * x2 + t2 * t * x3;
}
var context = (context = canvas(1, 1)) ? context.getContext("2d") : null;
const b = new Bounds();
function intersectPath(draw2) {
    return function(item, brush) {
        if (!context)
            return true;
        draw2(context, item);
        b.clear().union(item.bounds).intersect(brush).round();
        const {
            x1,
            y1,
            x2,
            y2
        } = b;
        for (let y3 = y1; y3 <= y2; ++y3) {
            for (let x3 = x1; x3 <= x2; ++x3) {
                if (context.isPointInPath(x3, y3)) {
                    return true;
                }
            }
        }
        return false;
    };
}
function intersectPoint(item, box) {
    return box.contains(item.x || 0, item.y || 0);
}
function intersectRect(item, box) {
    const x2 = item.x || 0, y2 = item.y || 0, w2 = item.width || 0, h2 = item.height || 0;
    return box.intersects(b.set(x2, y2, x2 + w2, y2 + h2));
}
function intersectRule(item, box) {
    const x2 = item.x || 0, y2 = item.y || 0, x22 = item.x2 != null ? item.x2 : x2, y22 = item.y2 != null ? item.y2 : y2;
    return intersectBoxLine(box, x2, y2, x22, y22);
}
function intersectBoxLine(box, x2, y2, u, v) {
    const {
        x1,
        y1,
        x2: x22,
        y2: y22
    } = box, dx = u - x2, dy = v - y2;
    let t0 = 0, t1 = 1, p, q, r, e;
    for (e = 0; e < 4; ++e) {
        if (e === 0) {
            p = -dx;
            q = -(x1 - x2);
        }
        if (e === 1) {
            p = dx;
            q = x22 - x2;
        }
        if (e === 2) {
            p = -dy;
            q = -(y1 - y2);
        }
        if (e === 3) {
            p = dy;
            q = y22 - y2;
        }
        if (Math.abs(p) < 1e-10 && q < 0)
            return false;
        r = q / p;
        if (p < 0) {
            if (r > t1)
                return false;
            else if (r > t0)
                t0 = r;
        } else if (p > 0) {
            if (r < t0)
                return false;
            else if (r < t1)
                t1 = r;
        }
    }
    return true;
}
function blend(context2, item) {
    context2.globalCompositeOperation = item.blend || "source-over";
}
function value(value2, dflt) {
    return value2 == null ? dflt : value2;
}
function addStops(gradient2, stops) {
    const n = stops.length;
    for (let i = 0; i < n; ++i) {
        gradient2.addColorStop(stops[i].offset, stops[i].color);
    }
    return gradient2;
}
function gradient(context2, spec, bounds2) {
    const w2 = bounds2.width(), h2 = bounds2.height();
    let gradient2;
    if (spec.gradient === "radial") {
        gradient2 = context2.createRadialGradient(bounds2.x1 + value(spec.x1, 0.5) * w2, bounds2.y1 + value(spec.y1, 0.5) * h2, Math.max(w2, h2) * value(spec.r1, 0), bounds2.x1 + value(spec.x2, 0.5) * w2, bounds2.y1 + value(spec.y2, 0.5) * h2, Math.max(w2, h2) * value(spec.r2, 0.5));
    } else {
        const x1 = value(spec.x1, 0), y1 = value(spec.y1, 0), x2 = value(spec.x2, 1), y2 = value(spec.y2, 0);
        if (x1 === x2 || y1 === y2 || w2 === h2) {
            gradient2 = context2.createLinearGradient(bounds2.x1 + x1 * w2, bounds2.y1 + y1 * h2, bounds2.x1 + x2 * w2, bounds2.y1 + y2 * h2);
        } else {
            const image2 = canvas(Math.ceil(w2), Math.ceil(h2)), ictx = image2.getContext("2d");
            ictx.scale(w2, h2);
            ictx.fillStyle = addStops(ictx.createLinearGradient(x1, y1, x2, y2), spec.stops);
            ictx.fillRect(0, 0, w2, h2);
            return context2.createPattern(image2, "no-repeat");
        }
    }
    return addStops(gradient2, spec.stops);
}
function color(context2, item, value2) {
    return isGradient(value2) ? gradient(context2, value2, item.bounds) : value2;
}
function fill(context2, item, opacity) {
    opacity *= item.fillOpacity == null ? 1 : item.fillOpacity;
    if (opacity > 0) {
        context2.globalAlpha = opacity;
        context2.fillStyle = color(context2, item, item.fill);
        return true;
    } else {
        return false;
    }
}
var Empty = [];
function stroke(context2, item, opacity) {
    var lw = (lw = item.strokeWidth) != null ? lw : 1;
    if (lw <= 0)
        return false;
    opacity *= item.strokeOpacity == null ? 1 : item.strokeOpacity;
    if (opacity > 0) {
        context2.globalAlpha = opacity;
        context2.strokeStyle = color(context2, item, item.stroke);
        context2.lineWidth = lw;
        context2.lineCap = item.strokeCap || "butt";
        context2.lineJoin = item.strokeJoin || "miter";
        context2.miterLimit = item.strokeMiterLimit || 10;
        if (context2.setLineDash) {
            context2.setLineDash(item.strokeDash || Empty);
            context2.lineDashOffset = item.strokeDashOffset || 0;
        }
        return true;
    } else {
        return false;
    }
}
function compare(a, b2) {
    return a.zindex - b2.zindex || a.index - b2.index;
}
function zorder(scene) {
    if (!scene.zdirty)
        return scene.zitems;
    var items = scene.items, output = [], item, i, n;
    for (i = 0, n = items.length; i < n; ++i) {
        item = items[i];
        item.index = i;
        if (item.zindex)
            output.push(item);
    }
    scene.zdirty = false;
    return scene.zitems = output.sort(compare);
}
function visit(scene, visitor) {
    var items = scene.items, i, n;
    if (!items || !items.length)
        return;
    const zitems = zorder(scene);
    if (zitems && zitems.length) {
        for (i = 0, n = items.length; i < n; ++i) {
            if (!items[i].zindex)
                visitor(items[i]);
        }
        items = zitems;
    }
    for (i = 0, n = items.length; i < n; ++i) {
        visitor(items[i]);
    }
}
function pickVisit(scene, visitor) {
    var items = scene.items, hit2, i;
    if (!items || !items.length)
        return null;
    const zitems = zorder(scene);
    if (zitems && zitems.length)
        items = zitems;
    for (i = items.length; --i >= 0; ) {
        if (hit2 = visitor(items[i]))
            return hit2;
    }
    if (items === zitems) {
        for (items = scene.items, i = items.length; --i >= 0; ) {
            if (!items[i].zindex) {
                if (hit2 = visitor(items[i]))
                    return hit2;
            }
        }
    }
    return null;
}
function drawAll(path3) {
    return function(context2, scene, bounds2) {
        visit(scene, (item) => {
            if (!bounds2 || bounds2.intersects(item.bounds)) {
                drawPath(path3, context2, item, item);
            }
        });
    };
}
function drawOne(path3) {
    return function(context2, scene, bounds2) {
        if (scene.items.length && (!bounds2 || bounds2.intersects(scene.bounds))) {
            drawPath(path3, context2, scene.items[0], scene.items);
        }
    };
}
function drawPath(path3, context2, item, items) {
    var opacity = item.opacity == null ? 1 : item.opacity;
    if (opacity === 0)
        return;
    if (path3(context2, items))
        return;
    blend(context2, item);
    if (item.fill && fill(context2, item, opacity)) {
        context2.fill();
    }
    if (item.stroke && stroke(context2, item, opacity)) {
        context2.stroke();
    }
}
function pick$1(test) {
    test = test || truthy;
    return function(context2, scene, x2, y2, gx, gy) {
        x2 *= context2.pixelRatio;
        y2 *= context2.pixelRatio;
        return pickVisit(scene, (item) => {
            const b2 = item.bounds;
            if (b2 && !b2.contains(gx, gy) || !b2)
                return;
            if (test(context2, item, x2, y2, gx, gy))
                return item;
        });
    };
}
function hitPath(path3, filled) {
    return function(context2, o, x2, y2) {
        var item = Array.isArray(o) ? o[0] : o, fill2 = filled == null ? item.fill : filled, stroke2 = item.stroke && context2.isPointInStroke, lw, lc;
        if (stroke2) {
            lw = item.strokeWidth;
            lc = item.strokeCap;
            context2.lineWidth = lw != null ? lw : 1;
            context2.lineCap = lc != null ? lc : "butt";
        }
        return path3(context2, o) ? false : fill2 && context2.isPointInPath(x2, y2) || stroke2 && context2.isPointInStroke(x2, y2);
    };
}
function pickPath(path3) {
    return pick$1(hitPath(path3));
}
function translate(x2, y2) {
    return "translate(" + x2 + "," + y2 + ")";
}
function rotate(a) {
    return "rotate(" + a + ")";
}
function scale(scaleX, scaleY) {
    return "scale(" + scaleX + "," + scaleY + ")";
}
function translateItem(item) {
    return translate(item.x || 0, item.y || 0);
}
function rotateItem(item) {
    return translate(item.x || 0, item.y || 0) + (item.angle ? " " + rotate(item.angle) : "");
}
function transformItem(item) {
    return translate(item.x || 0, item.y || 0) + (item.angle ? " " + rotate(item.angle) : "") + (item.scaleX || item.scaleY ? " " + scale(item.scaleX || 1, item.scaleY || 1) : "");
}
function markItemPath(type2, shape2, isect) {
    function attr2(emit2, item) {
        emit2("transform", rotateItem(item));
        emit2("d", shape2(null, item));
    }
    function bound2(bounds2, item) {
        shape2(boundContext(bounds2, item.angle), item);
        return boundStroke(bounds2, item).translate(item.x || 0, item.y || 0);
    }
    function draw2(context2, item) {
        var x2 = item.x || 0, y2 = item.y || 0, a = item.angle || 0;
        context2.translate(x2, y2);
        if (a)
            context2.rotate(a *= DegToRad);
        context2.beginPath();
        shape2(context2, item);
        if (a)
            context2.rotate(-a);
        context2.translate(-x2, -y2);
    }
    return {
        type: type2,
        tag: "path",
        nested: false,
        attr: attr2,
        bound: bound2,
        draw: drawAll(draw2),
        pick: pickPath(draw2),
        isect: isect || intersectPath(draw2)
    };
}
var arc = markItemPath("arc", arc$1);
function pickArea(a, p) {
    var v = a[0].orient === "horizontal" ? p[1] : p[0], z = a[0].orient === "horizontal" ? "y" : "x", i = a.length, min = Infinity, hit2, d;
    while (--i >= 0) {
        if (a[i].defined === false)
            continue;
        d = Math.abs(a[i][z] - v);
        if (d < min) {
            min = d;
            hit2 = a[i];
        }
    }
    return hit2;
}
function pickLine(a, p) {
    var t = Math.pow(a[0].strokeWidth || 1, 2), i = a.length, dx, dy, dd;
    while (--i >= 0) {
        if (a[i].defined === false)
            continue;
        dx = a[i].x - p[0];
        dy = a[i].y - p[1];
        dd = dx * dx + dy * dy;
        if (dd < t)
            return a[i];
    }
    return null;
}
function pickTrail(a, p) {
    var i = a.length, dx, dy, dd;
    while (--i >= 0) {
        if (a[i].defined === false)
            continue;
        dx = a[i].x - p[0];
        dy = a[i].y - p[1];
        dd = dx * dx + dy * dy;
        dx = a[i].size || 1;
        if (dd < dx * dx)
            return a[i];
    }
    return null;
}
function markMultiItemPath(type2, shape2, tip) {
    function attr2(emit2, item) {
        var items = item.mark.items;
        if (items.length)
            emit2("d", shape2(null, items));
    }
    function bound2(bounds2, mark) {
        var items = mark.items;
        if (items.length === 0) {
            return bounds2;
        } else {
            shape2(boundContext(bounds2), items);
            return boundStroke(bounds2, items[0]);
        }
    }
    function draw2(context2, items) {
        context2.beginPath();
        shape2(context2, items);
    }
    const hit2 = hitPath(draw2);
    function pick2(context2, scene, x2, y2, gx, gy) {
        var items = scene.items, b2 = scene.bounds;
        if (!items || !items.length || b2 && !b2.contains(gx, gy)) {
            return null;
        }
        x2 *= context2.pixelRatio;
        y2 *= context2.pixelRatio;
        return hit2(context2, items, x2, y2) ? items[0] : null;
    }
    return {
        type: type2,
        tag: "path",
        nested: true,
        attr: attr2,
        bound: bound2,
        draw: drawOne(draw2),
        pick: pick2,
        isect: intersectPoint,
        tip
    };
}
var area = markMultiItemPath("area", area$1, pickArea);
function clip(context2, scene) {
    var clip2 = scene.clip;
    context2.save();
    if (isFunction(clip2)) {
        context2.beginPath();
        clip2(context2);
        context2.clip();
    } else {
        clipGroup(context2, scene.group);
    }
}
function clipGroup(context2, group2) {
    context2.beginPath();
    hasCornerRadius(group2) ? rectangle(context2, group2, 0, 0) : context2.rect(0, 0, group2.width || 0, group2.height || 0);
    context2.clip();
}
function offset$1(item) {
    const sw = value(item.strokeWidth, 1);
    return item.strokeOffset != null ? item.strokeOffset : item.stroke && sw > 0.5 && sw < 1.5 ? 0.5 - Math.abs(sw - 1) : 0;
}
function attr$5(emit2, item) {
    emit2("transform", translateItem(item));
}
function emitRectangle(emit2, item) {
    const off = offset$1(item);
    emit2("d", rectangle(null, item, off, off));
}
function background(emit2, item) {
    emit2("class", "background");
    emit2("aria-hidden", true);
    emitRectangle(emit2, item);
}
function foreground(emit2, item) {
    emit2("class", "foreground");
    emit2("aria-hidden", true);
    if (item.strokeForeground) {
        emitRectangle(emit2, item);
    } else {
        emit2("d", "");
    }
}
function content(emit2, item, renderer) {
    const url = item.clip ? clip$1(renderer, item, item) : null;
    emit2("clip-path", url);
}
function bound$5(bounds2, group2) {
    if (!group2.clip && group2.items) {
        const items = group2.items, m = items.length;
        for (let j = 0; j < m; ++j) {
            bounds2.union(items[j].bounds);
        }
    }
    if ((group2.clip || group2.width || group2.height) && !group2.noBound) {
        bounds2.add(0, 0).add(group2.width || 0, group2.height || 0);
    }
    boundStroke(bounds2, group2);
    return bounds2.translate(group2.x || 0, group2.y || 0);
}
function rectanglePath(context2, group2, x2, y2) {
    const off = offset$1(group2);
    context2.beginPath();
    rectangle(context2, group2, (x2 || 0) + off, (y2 || 0) + off);
}
const hitBackground = hitPath(rectanglePath);
const hitForeground = hitPath(rectanglePath, false);
const hitCorner = hitPath(rectanglePath, true);
function draw$4(context2, scene, bounds2) {
    visit(scene, (group2) => {
        const gx = group2.x || 0, gy = group2.y || 0, fore = group2.strokeForeground, opacity = group2.opacity == null ? 1 : group2.opacity;
        if ((group2.stroke || group2.fill) && opacity) {
            rectanglePath(context2, group2, gx, gy);
            blend(context2, group2);
            if (group2.fill && fill(context2, group2, opacity)) {
                context2.fill();
            }
            if (group2.stroke && !fore && stroke(context2, group2, opacity)) {
                context2.stroke();
            }
        }
        context2.save();
        context2.translate(gx, gy);
        if (group2.clip)
            clipGroup(context2, group2);
        if (bounds2)
            bounds2.translate(-gx, -gy);
        visit(group2, (item) => {
            this.draw(context2, item, bounds2);
        });
        if (bounds2)
            bounds2.translate(gx, gy);
        context2.restore();
        if (fore && group2.stroke && opacity) {
            rectanglePath(context2, group2, gx, gy);
            blend(context2, group2);
            if (stroke(context2, group2, opacity)) {
                context2.stroke();
            }
        }
    });
}
function pick(context2, scene, x2, y2, gx, gy) {
    if (scene.bounds && !scene.bounds.contains(gx, gy) || !scene.items) {
        return null;
    }
    const cx = x2 * context2.pixelRatio, cy = y2 * context2.pixelRatio;
    return pickVisit(scene, (group2) => {
        let hit2, dx, dy;
        const b2 = group2.bounds;
        if (b2 && !b2.contains(gx, gy))
            return;
        dx = group2.x || 0;
        dy = group2.y || 0;
        const dw = dx + (group2.width || 0), dh = dy + (group2.height || 0), c = group2.clip;
        if (c && (gx < dx || gx > dw || gy < dy || gy > dh))
            return;
        context2.save();
        context2.translate(dx, dy);
        dx = gx - dx;
        dy = gy - dy;
        if (c && hasCornerRadius(group2) && !hitCorner(context2, group2, cx, cy)) {
            context2.restore();
            return null;
        }
        const fore = group2.strokeForeground, ix = scene.interactive !== false;
        if (ix && fore && group2.stroke && hitForeground(context2, group2, cx, cy)) {
            context2.restore();
            return group2;
        }
        hit2 = pickVisit(group2, (mark) => pickMark(mark, dx, dy) ? this.pick(mark, x2, y2, dx, dy) : null);
        if (!hit2 && ix && (group2.fill || !fore && group2.stroke) && hitBackground(context2, group2, cx, cy)) {
            hit2 = group2;
        }
        context2.restore();
        return hit2 || null;
    });
}
function pickMark(mark, x2, y2) {
    return (mark.interactive !== false || mark.marktype === "group") && mark.bounds && mark.bounds.contains(x2, y2);
}
var group = {
    type: "group",
    tag: "g",
    nested: false,
    attr: attr$5,
    bound: bound$5,
    draw: draw$4,
    pick,
    isect: intersectRect,
    content,
    background,
    foreground
};
var metadata = {
    xmlns: "http://www.w3.org/2000/svg",
    "xmlns:xlink": "http://www.w3.org/1999/xlink",
    version: "1.1"
};
function getImage(item, renderer) {
    var image2 = item.image;
    if (!image2 || item.url && item.url !== image2.url) {
        image2 = {
            complete: false,
            width: 0,
            height: 0
        };
        renderer.loadImage(item.url).then((image3) => {
            item.image = image3;
            item.image.url = item.url;
        });
    }
    return image2;
}
function imageWidth(item, image2) {
    return item.width != null ? item.width : !image2 || !image2.width ? 0 : item.aspect !== false && item.height ? item.height * image2.width / image2.height : image2.width;
}
function imageHeight(item, image2) {
    return item.height != null ? item.height : !image2 || !image2.height ? 0 : item.aspect !== false && item.width ? item.width * image2.height / image2.width : image2.height;
}
function imageXOffset(align, w2) {
    return align === "center" ? w2 / 2 : align === "right" ? w2 : 0;
}
function imageYOffset(baseline, h2) {
    return baseline === "middle" ? h2 / 2 : baseline === "bottom" ? h2 : 0;
}
function attr$4(emit2, item, renderer) {
    const img = getImage(item, renderer), w2 = imageWidth(item, img), h2 = imageHeight(item, img), x2 = (item.x || 0) - imageXOffset(item.align, w2), y2 = (item.y || 0) - imageYOffset(item.baseline, h2), i = !img.src && img.toDataURL ? img.toDataURL() : img.src || "";
    emit2("href", i, metadata["xmlns:xlink"], "xlink:href");
    emit2("transform", translate(x2, y2));
    emit2("width", w2);
    emit2("height", h2);
    emit2("preserveAspectRatio", item.aspect === false ? "none" : "xMidYMid");
}
function bound$4(bounds2, item) {
    const img = item.image, w2 = imageWidth(item, img), h2 = imageHeight(item, img), x2 = (item.x || 0) - imageXOffset(item.align, w2), y2 = (item.y || 0) - imageYOffset(item.baseline, h2);
    return bounds2.set(x2, y2, x2 + w2, y2 + h2);
}
function draw$3(context2, scene, bounds2) {
    visit(scene, (item) => {
        if (bounds2 && !bounds2.intersects(item.bounds))
            return;
        const img = getImage(item, this);
        let w2 = imageWidth(item, img);
        let h2 = imageHeight(item, img);
        if (w2 === 0 || h2 === 0)
            return;
        let x2 = (item.x || 0) - imageXOffset(item.align, w2), y2 = (item.y || 0) - imageYOffset(item.baseline, h2), opacity, ar0, ar1, t;
        if (item.aspect !== false) {
            ar0 = img.width / img.height;
            ar1 = item.width / item.height;
            if (ar0 === ar0 && ar1 === ar1 && ar0 !== ar1) {
                if (ar1 < ar0) {
                    t = w2 / ar0;
                    y2 += (h2 - t) / 2;
                    h2 = t;
                } else {
                    t = h2 * ar0;
                    x2 += (w2 - t) / 2;
                    w2 = t;
                }
            }
        }
        if (img.complete || img.toDataURL) {
            blend(context2, item);
            context2.globalAlpha = (opacity = item.opacity) != null ? opacity : 1;
            context2.imageSmoothingEnabled = item.smooth !== false;
            context2.drawImage(img, x2, y2, w2, h2);
        }
    });
}
var image = {
    type: "image",
    tag: "image",
    nested: false,
    attr: attr$4,
    bound: bound$4,
    draw: draw$3,
    pick: pick$1(),
    isect: truthy,
    get: getImage,
    xOffset: imageXOffset,
    yOffset: imageYOffset
};
var line = markMultiItemPath("line", line$1, pickLine);
function attr$3(emit2, item) {
    var sx = item.scaleX || 1, sy = item.scaleY || 1;
    if (sx !== 1 || sy !== 1) {
        emit2("vector-effect", "non-scaling-stroke");
    }
    emit2("transform", transformItem(item));
    emit2("d", item.path);
}
function path$1(context2, item) {
    var path3 = item.path;
    if (path3 == null)
        return true;
    var x2 = item.x || 0, y2 = item.y || 0, sx = item.scaleX || 1, sy = item.scaleY || 1, a = (item.angle || 0) * DegToRad, cache = item.pathCache;
    if (!cache || cache.path !== path3) {
        (item.pathCache = cache = parse(path3)).path = path3;
    }
    if (a && context2.rotate && context2.translate) {
        context2.translate(x2, y2);
        context2.rotate(a);
        pathRender(context2, cache, 0, 0, sx, sy);
        context2.rotate(-a);
        context2.translate(-x2, -y2);
    } else {
        pathRender(context2, cache, x2, y2, sx, sy);
    }
}
function bound$3(bounds2, item) {
    return path$1(boundContext(bounds2, item.angle), item) ? bounds2.set(0, 0, 0, 0) : boundStroke(bounds2, item, true);
}
var path$2 = {
    type: "path",
    tag: "path",
    nested: false,
    attr: attr$3,
    bound: bound$3,
    draw: drawAll(path$1),
    pick: pickPath(path$1),
    isect: intersectPath(path$1)
};
function attr$2(emit2, item) {
    emit2("d", rectangle(null, item));
}
function bound$2(bounds2, item) {
    var x2, y2;
    return boundStroke(bounds2.set(x2 = item.x || 0, y2 = item.y || 0, x2 + item.width || 0, y2 + item.height || 0), item);
}
function draw$2(context2, item) {
    context2.beginPath();
    rectangle(context2, item);
}
var rect = {
    type: "rect",
    tag: "path",
    nested: false,
    attr: attr$2,
    bound: bound$2,
    draw: drawAll(draw$2),
    pick: pickPath(draw$2),
    isect: intersectRect
};
function attr$1(emit2, item) {
    emit2("transform", translateItem(item));
    emit2("x2", item.x2 != null ? item.x2 - (item.x || 0) : 0);
    emit2("y2", item.y2 != null ? item.y2 - (item.y || 0) : 0);
}
function bound$1(bounds2, item) {
    var x1, y1;
    return boundStroke(bounds2.set(x1 = item.x || 0, y1 = item.y || 0, item.x2 != null ? item.x2 : x1, item.y2 != null ? item.y2 : y1), item);
}
function path2(context2, item, opacity) {
    var x1, y1, x2, y2;
    if (item.stroke && stroke(context2, item, opacity)) {
        x1 = item.x || 0;
        y1 = item.y || 0;
        x2 = item.x2 != null ? item.x2 : x1;
        y2 = item.y2 != null ? item.y2 : y1;
        context2.beginPath();
        context2.moveTo(x1, y1);
        context2.lineTo(x2, y2);
        return true;
    }
    return false;
}
function draw$1(context2, scene, bounds2) {
    visit(scene, (item) => {
        if (bounds2 && !bounds2.intersects(item.bounds))
            return;
        var opacity = item.opacity == null ? 1 : item.opacity;
        if (opacity && path2(context2, item, opacity)) {
            blend(context2, item);
            context2.stroke();
        }
    });
}
function hit$1(context2, item, x2, y2) {
    if (!context2.isPointInStroke)
        return false;
    return path2(context2, item, 1) && context2.isPointInStroke(x2, y2);
}
var rule = {
    type: "rule",
    tag: "line",
    nested: false,
    attr: attr$1,
    bound: bound$1,
    draw: draw$1,
    pick: pick$1(hit$1),
    isect: intersectRule
};
var shape = markItemPath("shape", shape$1);
var symbol = markItemPath("symbol", symbol$1, intersectPoint);
const widthCache = lruCache();
var textMetrics = {
    height: fontSize,
    measureWidth,
    estimateWidth,
    width: estimateWidth,
    canvas: useCanvas
};
useCanvas(true);
function useCanvas(use) {
    textMetrics.width = use && context ? measureWidth : estimateWidth;
}
function estimateWidth(item, text2) {
    return _estimateWidth(textValue(item, text2), fontSize(item));
}
function _estimateWidth(text2, currentFontHeight) {
    return ~~(0.8 * text2.length * currentFontHeight);
}
function measureWidth(item, text2) {
    return fontSize(item) <= 0 || !(text2 = textValue(item, text2)) ? 0 : _measureWidth(text2, font(item));
}
function _measureWidth(text2, currentFont) {
    const key = `(${currentFont}) ${text2}`;
    let width = widthCache.get(key);
    if (width === void 0) {
        context.font = currentFont;
        width = context.measureText(text2).width;
        widthCache.set(key, width);
    }
    return width;
}
function fontSize(item) {
    return item.fontSize != null ? +item.fontSize || 0 : 11;
}
function lineHeight(item) {
    return item.lineHeight != null ? item.lineHeight : fontSize(item) + 2;
}
function lineArray(_) {
    return isArray(_) ? _.length > 1 ? _ : _[0] : _;
}
function textLines(item) {
    return lineArray(item.lineBreak && item.text && !isArray(item.text) ? item.text.split(item.lineBreak) : item.text);
}
function multiLineOffset(item) {
    const tl2 = textLines(item);
    return (isArray(tl2) ? tl2.length - 1 : 0) * lineHeight(item);
}
function textValue(item, line2) {
    const text2 = line2 == null ? "" : (line2 + "").trim();
    return item.limit > 0 && text2.length ? truncate(item, text2) : text2;
}
function widthGetter(item) {
    if (textMetrics.width === measureWidth) {
        const currentFont = font(item);
        return (text2) => _measureWidth(text2, currentFont);
    } else if (textMetrics.width === estimateWidth) {
        // we are relying on estimates
        const currentFontHeight = fontSize(item);
        return text => _estimateWidth(text, currentFontHeight);
    } else {
        // User defined textMetrics.width function in use (e.g. vl-convert)
        return text => textMetrics.width(item, text);
    }
}
function truncate(item, text2) {
    var limit = +item.limit, width = widthGetter(item);
    if (width(text2) < limit)
        return text2;
    var ellipsis = item.ellipsis || "\u2026", rtl = item.dir === "rtl", lo = 0, hi = text2.length, mid;
    limit -= width(ellipsis);
    if (rtl) {
        while (lo < hi) {
            mid = lo + hi >>> 1;
            if (width(text2.slice(mid)) > limit)
                lo = mid + 1;
            else
                hi = mid;
        }
        return ellipsis + text2.slice(lo);
    } else {
        while (lo < hi) {
            mid = 1 + (lo + hi >>> 1);
            if (width(text2.slice(0, mid)) < limit)
                lo = mid;
            else
                hi = mid - 1;
        }
        return text2.slice(0, lo) + ellipsis;
    }
}
function fontFamily(item, quote) {
    var font2 = item.font;
    return (quote && font2 ? String(font2).replace(/"/g, "'") : font2) || "sans-serif";
}
function font(item, quote) {
    return "" + (item.fontStyle ? item.fontStyle + " " : "") + (item.fontVariant ? item.fontVariant + " " : "") + (item.fontWeight ? item.fontWeight + " " : "") + fontSize(item) + "px " + fontFamily(item, quote);
}
function offset(item) {
    var baseline = item.baseline, h2 = fontSize(item);
    return Math.round(baseline === "top" ? 0.79 * h2 : baseline === "middle" ? 0.3 * h2 : baseline === "bottom" ? -0.21 * h2 : baseline === "line-top" ? 0.29 * h2 + 0.5 * lineHeight(item) : baseline === "line-bottom" ? 0.29 * h2 - 0.5 * lineHeight(item) : 0);
}
const textAlign = {
    left: "start",
    center: "middle",
    right: "end"
};
const tempBounds = new Bounds();
function anchorPoint(item) {
    var x2 = item.x || 0, y2 = item.y || 0, r = item.radius || 0, t;
    if (r) {
        t = (item.theta || 0) - HalfPi;
        x2 += r * Math.cos(t);
        y2 += r * Math.sin(t);
    }
    tempBounds.x1 = x2;
    tempBounds.y1 = y2;
    return tempBounds;
}
function attr(emit2, item) {
    var dx = item.dx || 0, dy = (item.dy || 0) + offset(item), p = anchorPoint(item), x2 = p.x1, y2 = p.y1, a = item.angle || 0, t;
    emit2("text-anchor", textAlign[item.align] || "start");
    if (a) {
        t = translate(x2, y2) + " " + rotate(a);
        if (dx || dy)
            t += " " + translate(dx, dy);
    } else {
        t = translate(x2 + dx, y2 + dy);
    }
    emit2("transform", t);
}
function bound(bounds2, item, mode) {
    var h2 = textMetrics.height(item), a = item.align, p = anchorPoint(item), x2 = p.x1, y2 = p.y1, dx = item.dx || 0, dy = (item.dy || 0) + offset(item) - Math.round(0.8 * h2), tl2 = textLines(item), w2;
    if (isArray(tl2)) {
        h2 += lineHeight(item) * (tl2.length - 1);
        w2 = tl2.reduce((w3, t) => Math.max(w3, textMetrics.width(item, t)), 0);
    } else {
        w2 = textMetrics.width(item, tl2);
    }
    if (a === "center") {
        dx -= w2 / 2;
    } else if (a === "right") {
        dx -= w2;
    } else
        ;
    bounds2.set(dx += x2, dy += y2, dx + w2, dy + h2);
    if (item.angle && !mode) {
        bounds2.rotate(item.angle * DegToRad, x2, y2);
    } else if (mode === 2) {
        return bounds2.rotatedPoints(item.angle * DegToRad, x2, y2);
    }
    return bounds2;
}
function draw(context2, scene, bounds2) {
    visit(scene, (item) => {
        var opacity = item.opacity == null ? 1 : item.opacity, p, x2, y2, i, lh, tl2, str;
        if (bounds2 && !bounds2.intersects(item.bounds) || opacity === 0 || item.fontSize <= 0 || item.text == null || item.text.length === 0)
            return;
        context2.font = font(item);
        context2.textAlign = item.align || "left";
        p = anchorPoint(item);
        x2 = p.x1, y2 = p.y1;
        if (item.angle) {
            context2.save();
            context2.translate(x2, y2);
            context2.rotate(item.angle * DegToRad);
            x2 = y2 = 0;
        }
        x2 += item.dx || 0;
        y2 += (item.dy || 0) + offset(item);
        tl2 = textLines(item);
        blend(context2, item);
        if (isArray(tl2)) {
            lh = lineHeight(item);
            for (i = 0; i < tl2.length; ++i) {
                str = textValue(item, tl2[i]);
                if (item.fill && fill(context2, item, opacity)) {
                    context2.fillText(str, x2, y2);
                }
                if (item.stroke && stroke(context2, item, opacity)) {
                    context2.strokeText(str, x2, y2);
                }
                y2 += lh;
            }
        } else {
            str = textValue(item, tl2);
            if (item.fill && fill(context2, item, opacity)) {
                context2.fillText(str, x2, y2);
            }
            if (item.stroke && stroke(context2, item, opacity)) {
                context2.strokeText(str, x2, y2);
            }
        }
        if (item.angle)
            context2.restore();
    });
}
function hit(context2, item, x2, y2, gx, gy) {
    if (item.fontSize <= 0)
        return false;
    if (!item.angle)
        return true;
    var p = anchorPoint(item), ax = p.x1, ay = p.y1, b2 = bound(tempBounds, item, 1), a = -item.angle * DegToRad, cos = Math.cos(a), sin = Math.sin(a), px2 = cos * gx - sin * gy + (ax - cos * ax + sin * ay), py2 = sin * gx + cos * gy + (ay - sin * ax - cos * ay);
    return b2.contains(px2, py2);
}
function intersectText(item, box) {
    const p = bound(tempBounds, item, 2);
    return intersectBoxLine(box, p[0], p[1], p[2], p[3]) || intersectBoxLine(box, p[0], p[1], p[4], p[5]) || intersectBoxLine(box, p[4], p[5], p[6], p[7]) || intersectBoxLine(box, p[2], p[3], p[6], p[7]);
}
var text = {
    type: "text",
    tag: "text",
    nested: false,
    attr,
    bound,
    draw,
    pick: pick$1(hit),
    isect: intersectText
};
var trail = markMultiItemPath("trail", trail$1, pickTrail);
var Marks = {
    arc,
    area,
    group,
    image,
    line,
    path: path$2,
    rect,
    rule,
    shape,
    symbol,
    text,
    trail
};
function boundItem(item, func, opt) {
    var type2 = Marks[item.mark.marktype], bound2 = func || type2.bound;
    if (type2.nested)
        item = item.mark;
    return bound2(item.bounds || (item.bounds = new Bounds()), item, opt);
}
var DUMMY = {
    mark: null
};
function boundMark(mark, bounds2, opt) {
    var type2 = Marks[mark.marktype], bound2 = type2.bound, items = mark.items, hasItems = items && items.length, i, n, item, b2;
    if (type2.nested) {
        if (hasItems) {
            item = items[0];
        } else {
            DUMMY.mark = mark;
            item = DUMMY;
        }
        b2 = boundItem(item, bound2, opt);
        bounds2 = bounds2 && bounds2.union(b2) || b2;
        return bounds2;
    }
    bounds2 = bounds2 || mark.bounds && mark.bounds.clear() || new Bounds();
    if (hasItems) {
        for (i = 0, n = items.length; i < n; ++i) {
            bounds2.union(boundItem(items[i], bound2, opt));
        }
    }
    return mark.bounds = bounds2;
}
const keys = [
    "marktype",
    "name",
    "role",
    "interactive",
    "clip",
    "items",
    "zindex",
    "x",
    "y",
    "width",
    "height",
    "align",
    "baseline",
    "fill",
    "fillOpacity",
    "opacity",
    "blend",
    "stroke",
    "strokeOpacity",
    "strokeWidth",
    "strokeCap",
    "strokeDash",
    "strokeDashOffset",
    "strokeForeground",
    "strokeOffset",
    "startAngle",
    "endAngle",
    "innerRadius",
    "outerRadius",
    "cornerRadius",
    "padAngle",
    "cornerRadiusTopLeft",
    "cornerRadiusTopRight",
    "cornerRadiusBottomLeft",
    "cornerRadiusBottomRight",
    "interpolate",
    "tension",
    "orient",
    "defined",
    "url",
    "aspect",
    "smooth",
    "path",
    "scaleX",
    "scaleY",
    "x2",
    "y2",
    "size",
    "shape",
    "text",
    "angle",
    "theta",
    "radius",
    "dir",
    "dx",
    "dy",
    "ellipsis",
    "limit",
    "lineBreak",
    "lineHeight",
    "font",
    "fontSize",
    "fontWeight",
    "fontStyle",
    "fontVariant",
    "description",
    "aria",
    "ariaRole",
    "ariaRoleDescription"
];
function sceneToJSON(scene, indent) {
    return JSON.stringify(scene, keys, indent);
}
function sceneFromJSON(json) {
    const scene = typeof json === "string" ? JSON.parse(json) : json;
    return initialize(scene);
}
function initialize(scene) {
    var type2 = scene.marktype, items = scene.items, parent, i, n;
    if (items) {
        for (i = 0, n = items.length; i < n; ++i) {
            parent = type2 ? "mark" : "group";
            items[i][parent] = scene;
            if (items[i].zindex)
                items[i][parent].zdirty = true;
            if ((type2 || parent) === "group")
                initialize(items[i]);
        }
    }
    if (type2)
        boundMark(scene);
    return scene;
}
function Scenegraph(scene) {
    if (arguments.length) {
        this.root = sceneFromJSON(scene);
    } else {
        this.root = createMark({
            marktype: "group",
            name: "root",
            role: "frame"
        });
        this.root.items = [new GroupItem(this.root)];
    }
}
Scenegraph.prototype = {
    toJSON(indent) {
        return sceneToJSON(this.root, indent || 0);
    },
    mark(markdef, group2, index) {
        group2 = group2 || this.root.items[0];
        const mark = createMark(markdef, group2);
        group2.items[index] = mark;
        if (mark.zindex)
            mark.group.zdirty = true;
        return mark;
    }
};
function createMark(def2, group2) {
    const mark = {
        bounds: new Bounds(),
        clip: !!def2.clip,
        group: group2,
        interactive: def2.interactive === false ? false : true,
        items: [],
        marktype: def2.marktype,
        name: def2.name || void 0,
        role: def2.role || void 0,
        zindex: def2.zindex || 0
    };
    if (def2.aria != null) {
        mark.aria = def2.aria;
    }
    if (def2.description) {
        mark.description = def2.description;
    }
    return mark;
}
function domCreate(doc, tag, ns) {
    if (!doc && typeof document !== "undefined" && document.createElement) {
        doc = document;
    }
    return doc ? ns ? doc.createElementNS(ns, tag) : doc.createElement(tag) : null;
}
function domFind(el, tag) {
    tag = tag.toLowerCase();
    var nodes = el.childNodes, i = 0, n = nodes.length;
    for (; i < n; ++i)
        if (nodes[i].tagName.toLowerCase() === tag) {
            return nodes[i];
        }
}
function domChild(el, index, tag, ns) {
    var a = el.childNodes[index], b2;
    if (!a || a.tagName.toLowerCase() !== tag.toLowerCase()) {
        b2 = a || null;
        a = domCreate(el.ownerDocument, tag, ns);
        el.insertBefore(a, b2);
    }
    return a;
}
function domClear(el, index) {
    var nodes = el.childNodes, curr = nodes.length;
    while (curr > index)
        el.removeChild(nodes[--curr]);
    return el;
}
function cssClass(mark) {
    return "mark-" + mark.marktype + (mark.role ? " role-" + mark.role : "") + (mark.name ? " " + mark.name : "");
}
function point(event, el) {
    const rect2 = el.getBoundingClientRect();
    return [event.clientX - rect2.left - (el.clientLeft || 0), event.clientY - rect2.top - (el.clientTop || 0)];
}
function resolveItem(item, event, el, origin) {
    var mark = item && item.mark, mdef, p;
    if (mark && (mdef = Marks[mark.marktype]).tip) {
        p = point(event, el);
        p[0] -= origin[0];
        p[1] -= origin[1];
        while (item = item.mark.group) {
            p[0] -= item.x || 0;
            p[1] -= item.y || 0;
        }
        item = mdef.tip(mark.items, p);
    }
    return item;
}
function Handler(customLoader, customTooltip) {
    this._active = null;
    this._handlers = {};
    this._loader = customLoader || loader();
    this._tooltip = customTooltip || defaultTooltip;
}
function defaultTooltip(handler, event, item, value2) {
    handler.element().setAttribute("title", value2 || "");
}
Handler.prototype = {
    initialize(el, origin, obj) {
        this._el = el;
        this._obj = obj || null;
        return this.origin(origin);
    },
    element() {
        return this._el;
    },
    canvas() {
        return this._el && this._el.firstChild;
    },
    origin(origin) {
        if (arguments.length) {
            this._origin = origin || [0, 0];
            return this;
        } else {
            return this._origin.slice();
        }
    },
    scene(scene) {
        if (!arguments.length)
            return this._scene;
        this._scene = scene;
        return this;
    },
    on() {
    },
    off() {
    },
    _handlerIndex(h2, type2, handler) {
        for (let i = h2 ? h2.length : 0; --i >= 0; ) {
            if (h2[i].type === type2 && (!handler || h2[i].handler === handler)) {
                return i;
            }
        }
        return -1;
    },
    handlers(type2) {
        const h2 = this._handlers, a = [];
        if (type2) {
            a.push(...h2[this.eventName(type2)]);
        } else {
            for (const k in h2) {
                a.push(...h2[k]);
            }
        }
        return a;
    },
    eventName(name) {
        const i = name.indexOf(".");
        return i < 0 ? name : name.slice(0, i);
    },
    handleHref(event, item, href2) {
        this._loader.sanitize(href2, {
            context: "href"
        }).then((opt) => {
            const e = new MouseEvent(event.type, event), a = domCreate(null, "a");
            for (const name in opt)
                a.setAttribute(name, opt[name]);
            a.dispatchEvent(e);
        }).catch(() => {
        });
    },
    handleTooltip(event, item, show) {
        if (item && item.tooltip != null) {
            item = resolveItem(item, event, this.canvas(), this._origin);
            const value2 = show && item && item.tooltip || null;
            this._tooltip.call(this._obj, this, event, item, value2);
        }
    },
    getItemBoundingClientRect(item) {
        const el = this.canvas();
        if (!el)
            return;
        const rect2 = el.getBoundingClientRect(), origin = this._origin, bounds2 = item.bounds, width = bounds2.width(), height = bounds2.height();
        let x2 = bounds2.x1 + origin[0] + rect2.left, y2 = bounds2.y1 + origin[1] + rect2.top;
        while (item.mark && (item = item.mark.group)) {
            x2 += item.x || 0;
            y2 += item.y || 0;
        }
        return {
            x: x2,
            y: y2,
            width,
            height,
            left: x2,
            top: y2,
            right: x2 + width,
            bottom: y2 + height
        };
    }
};
function Renderer(loader2) {
    this._el = null;
    this._bgcolor = null;
    this._loader = new ResourceLoader(loader2);
}
Renderer.prototype = {
    initialize(el, width, height, origin, scaleFactor) {
        this._el = el;
        return this.resize(width, height, origin, scaleFactor);
    },
    element() {
        return this._el;
    },
    canvas() {
        return this._el && this._el.firstChild;
    },
    background(bgcolor) {
        if (arguments.length === 0)
            return this._bgcolor;
        this._bgcolor = bgcolor;
        return this;
    },
    resize(width, height, origin, scaleFactor) {
        this._width = width;
        this._height = height;
        this._origin = origin || [0, 0];
        this._scale = scaleFactor || 1;
        return this;
    },
    dirty() {
    },
    render(scene) {
        const r = this;
        r._call = function() {
            r._render(scene);
        };
        r._call();
        r._call = null;
        return r;
    },
    _render() {
    },
    renderAsync(scene) {
        const r = this.render(scene);
        return this._ready ? this._ready.then(() => r) : Promise.resolve(r);
    },
    _load(method, uri) {
        var r = this, p = r._loader[method](uri);
        if (!r._ready) {
            const call = r._call;
            r._ready = r._loader.ready().then((redraw) => {
                if (redraw)
                    call();
                r._ready = null;
            });
        }
        return p;
    },
    sanitizeURL(uri) {
        return this._load("sanitizeURL", uri);
    },
    loadImage(uri) {
        return this._load("loadImage", uri);
    }
};
const KeyDownEvent = "keydown";
const KeyPressEvent = "keypress";
const KeyUpEvent = "keyup";
const DragEnterEvent = "dragenter";
const DragLeaveEvent = "dragleave";
const DragOverEvent = "dragover";
const MouseDownEvent = "mousedown";
const MouseUpEvent = "mouseup";
const MouseMoveEvent = "mousemove";
const MouseOutEvent = "mouseout";
const MouseOverEvent = "mouseover";
const ClickEvent = "click";
const DoubleClickEvent = "dblclick";
const WheelEvent = "wheel";
const MouseWheelEvent = "mousewheel";
const TouchStartEvent = "touchstart";
const TouchMoveEvent = "touchmove";
const TouchEndEvent = "touchend";
const Events = [KeyDownEvent, KeyPressEvent, KeyUpEvent, DragEnterEvent, DragLeaveEvent, DragOverEvent, MouseDownEvent, MouseUpEvent, MouseMoveEvent, MouseOutEvent, MouseOverEvent, ClickEvent, DoubleClickEvent, WheelEvent, MouseWheelEvent, TouchStartEvent, TouchMoveEvent, TouchEndEvent];
const TooltipShowEvent = MouseMoveEvent;
const TooltipHideEvent = MouseOutEvent;
const HrefEvent = ClickEvent;
function CanvasHandler(loader2, tooltip) {
    Handler.call(this, loader2, tooltip);
    this._down = null;
    this._touch = null;
    this._first = true;
    this._events = {};
}
const eventBundle = (type2) => type2 === TouchStartEvent || type2 === TouchMoveEvent || type2 === TouchEndEvent ? [TouchStartEvent, TouchMoveEvent, TouchEndEvent] : [type2];
function eventListenerCheck(handler, type2) {
    eventBundle(type2).forEach((_) => addEventListener(handler, _));
}
function addEventListener(handler, type2) {
    const canvas2 = handler.canvas();
    if (canvas2 && !handler._events[type2]) {
        handler._events[type2] = 1;
        canvas2.addEventListener(type2, handler[type2] ? (evt) => handler[type2](evt) : (evt) => handler.fire(type2, evt));
    }
}
function move(moveEvent, overEvent, outEvent) {
    return function(evt) {
        const a = this._active, p = this.pickEvent(evt);
        if (p === a) {
            this.fire(moveEvent, evt);
        } else {
            if (!a || !a.exit) {
                this.fire(outEvent, evt);
            }
            this._active = p;
            this.fire(overEvent, evt);
            this.fire(moveEvent, evt);
        }
    };
}
function inactive(type2) {
    return function(evt) {
        this.fire(type2, evt);
        this._active = null;
    };
}
inherits(CanvasHandler, Handler, {
    initialize(el, origin, obj) {
        this._canvas = el && domFind(el, "canvas");
        [ClickEvent, MouseDownEvent, MouseMoveEvent, MouseOutEvent, DragLeaveEvent].forEach((type2) => eventListenerCheck(this, type2));
        return Handler.prototype.initialize.call(this, el, origin, obj);
    },
    canvas() {
        return this._canvas;
    },
    context() {
        return this._canvas.getContext("2d");
    },
    events: Events,
    DOMMouseScroll(evt) {
        this.fire(MouseWheelEvent, evt);
    },
    mousemove: move(MouseMoveEvent, MouseOverEvent, MouseOutEvent),
    dragover: move(DragOverEvent, DragEnterEvent, DragLeaveEvent),
    mouseout: inactive(MouseOutEvent),
    dragleave: inactive(DragLeaveEvent),
    mousedown(evt) {
        this._down = this._active;
        this.fire(MouseDownEvent, evt);
    },
    click(evt) {
        if (this._down === this._active) {
            this.fire(ClickEvent, evt);
            this._down = null;
        }
    },
    touchstart(evt) {
        this._touch = this.pickEvent(evt.changedTouches[0]);
        if (this._first) {
            this._active = this._touch;
            this._first = false;
        }
        this.fire(TouchStartEvent, evt, true);
    },
    touchmove(evt) {
        this.fire(TouchMoveEvent, evt, true);
    },
    touchend(evt) {
        this.fire(TouchEndEvent, evt, true);
        this._touch = null;
    },
    fire(type2, evt, touch) {
        const a = touch ? this._touch : this._active, h2 = this._handlers[type2];
        evt.vegaType = type2;
        if (type2 === HrefEvent && a && a.href) {
            this.handleHref(evt, a, a.href);
        } else if (type2 === TooltipShowEvent || type2 === TooltipHideEvent) {
            this.handleTooltip(evt, a, type2 !== TooltipHideEvent);
        }
        if (h2) {
            for (let i = 0, len = h2.length; i < len; ++i) {
                h2[i].handler.call(this._obj, evt, a);
            }
        }
    },
    on(type2, handler) {
        const name = this.eventName(type2), h2 = this._handlers, i = this._handlerIndex(h2[name], type2, handler);
        if (i < 0) {
            eventListenerCheck(this, type2);
            (h2[name] || (h2[name] = [])).push({
                type: type2,
                handler
            });
        }
        return this;
    },
    off(type2, handler) {
        const name = this.eventName(type2), h2 = this._handlers[name], i = this._handlerIndex(h2, type2, handler);
        if (i >= 0) {
            h2.splice(i, 1);
        }
        return this;
    },
    pickEvent(evt) {
        const p = point(evt, this._canvas), o = this._origin;
        return this.pick(this._scene, p[0], p[1], p[0] - o[0], p[1] - o[1]);
    },
    pick(scene, x2, y2, gx, gy) {
        const g = this.context(), mark = Marks[scene.marktype];
        return mark.pick.call(this, g, scene, x2, y2, gx, gy);
    }
});
function devicePixelRatio() {
    return typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1;
}
var pixelRatio = devicePixelRatio();
function resize(canvas2, width, height, origin, scaleFactor, opt) {
    const inDOM = typeof HTMLElement !== "undefined" && canvas2 instanceof HTMLElement && canvas2.parentNode != null, context2 = canvas2.getContext("2d"), ratio = inDOM ? pixelRatio : scaleFactor;
    canvas2.width = width * ratio;
    canvas2.height = height * ratio;
    for (const key in opt) {
        context2[key] = opt[key];
    }
    if (inDOM && ratio !== 1) {
        canvas2.style.width = width + "px";
        canvas2.style.height = height + "px";
    }
    context2.pixelRatio = ratio;
    context2.setTransform(ratio, 0, 0, ratio, ratio * origin[0], ratio * origin[1]);
    return canvas2;
}
function CanvasRenderer(loader2) {
    Renderer.call(this, loader2);
    this._options = {};
    this._redraw = false;
    this._dirty = new Bounds();
    this._tempb = new Bounds();
}
const base$1 = Renderer.prototype;
const viewBounds = (origin, width, height) => new Bounds().set(0, 0, width, height).translate(-origin[0], -origin[1]);
function clipToBounds(g, b2, origin) {
    b2.expand(1).round();
    if (g.pixelRatio % 1) {
        b2.scale(g.pixelRatio).round().scale(1 / g.pixelRatio);
    }
    b2.translate(-(origin[0] % 1), -(origin[1] % 1));
    g.beginPath();
    g.rect(b2.x1, b2.y1, b2.width(), b2.height());
    g.clip();
    return b2;
}
inherits(CanvasRenderer, Renderer, {
    initialize(el, width, height, origin, scaleFactor, options) {
        this._options = options || {};
        this._canvas = this._options.externalContext ? null : canvas(1, 1, this._options.type);
        if (el && this._canvas) {
            domClear(el, 0).appendChild(this._canvas);
            this._canvas.setAttribute("class", "marks");
        }
        return base$1.initialize.call(this, el, width, height, origin, scaleFactor);
    },
    resize(width, height, origin, scaleFactor) {
        base$1.resize.call(this, width, height, origin, scaleFactor);
        if (this._canvas) {
            resize(this._canvas, this._width, this._height, this._origin, this._scale, this._options.context);
        } else {
            const ctx = this._options.externalContext;
            if (!ctx)
                error("CanvasRenderer is missing a valid canvas or context");
            ctx.scale(this._scale, this._scale);
            ctx.translate(this._origin[0], this._origin[1]);
        }
        this._redraw = true;
        return this;
    },
    canvas() {
        return this._canvas;
    },
    context() {
        return this._options.externalContext || (this._canvas ? this._canvas.getContext("2d") : null);
    },
    dirty(item) {
        const b2 = this._tempb.clear().union(item.bounds);
        let g = item.mark.group;
        while (g) {
            b2.translate(g.x || 0, g.y || 0);
            g = g.mark.group;
        }
        this._dirty.union(b2);
    },
    _render(scene) {
        const g = this.context(), o = this._origin, w2 = this._width, h2 = this._height, db = this._dirty, vb = viewBounds(o, w2, h2);
        g.save();
        const b2 = this._redraw || db.empty() ? (this._redraw = false, vb.expand(1)) : clipToBounds(g, vb.intersect(db), o);
        this.clear(-o[0], -o[1], w2, h2);
        this.draw(g, scene, b2);
        g.restore();
        db.clear();
        return this;
    },
    draw(ctx, scene, bounds2) {
        const mark = Marks[scene.marktype];
        if (scene.clip)
            clip(ctx, scene);
        mark.draw.call(this, ctx, scene, bounds2);
        if (scene.clip)
            ctx.restore();
    },
    clear(x2, y2, w2, h2) {
        const opt = this._options, g = this.context();
        if (opt.type !== "pdf" && !opt.externalContext) {
            g.clearRect(x2, y2, w2, h2);
        }
        if (this._bgcolor != null) {
            g.fillStyle = this._bgcolor;
            g.fillRect(x2, y2, w2, h2);
        }
    }
});
function SVGHandler(loader2, tooltip) {
    Handler.call(this, loader2, tooltip);
    const h2 = this;
    h2._hrefHandler = listener(h2, (evt, item) => {
        if (item && item.href)
            h2.handleHref(evt, item, item.href);
    });
    h2._tooltipHandler = listener(h2, (evt, item) => {
        h2.handleTooltip(evt, item, evt.type !== TooltipHideEvent);
    });
}
const listener = (context2, handler) => (evt) => {
    let item = evt.target.__data__;
    item = Array.isArray(item) ? item[0] : item;
    evt.vegaType = evt.type;
    handler.call(context2._obj, evt, item);
};
inherits(SVGHandler, Handler, {
    initialize(el, origin, obj) {
        let svg = this._svg;
        if (svg) {
            svg.removeEventListener(HrefEvent, this._hrefHandler);
            svg.removeEventListener(TooltipShowEvent, this._tooltipHandler);
            svg.removeEventListener(TooltipHideEvent, this._tooltipHandler);
        }
        this._svg = svg = el && domFind(el, "svg");
        if (svg) {
            svg.addEventListener(HrefEvent, this._hrefHandler);
            svg.addEventListener(TooltipShowEvent, this._tooltipHandler);
            svg.addEventListener(TooltipHideEvent, this._tooltipHandler);
        }
        return Handler.prototype.initialize.call(this, el, origin, obj);
    },
    canvas() {
        return this._svg;
    },
    on(type2, handler) {
        const name = this.eventName(type2), h2 = this._handlers, i = this._handlerIndex(h2[name], type2, handler);
        if (i < 0) {
            const x2 = {
                type: type2,
                handler,
                listener: listener(this, handler)
            };
            (h2[name] || (h2[name] = [])).push(x2);
            if (this._svg) {
                this._svg.addEventListener(name, x2.listener);
            }
        }
        return this;
    },
    off(type2, handler) {
        const name = this.eventName(type2), h2 = this._handlers[name], i = this._handlerIndex(h2, type2, handler);
        if (i >= 0) {
            if (this._svg) {
                this._svg.removeEventListener(name, h2[i].listener);
            }
            h2.splice(i, 1);
        }
        return this;
    }
});
const ARIA_HIDDEN = "aria-hidden";
const ARIA_LABEL = "aria-label";
const ARIA_ROLE = "role";
const ARIA_ROLEDESCRIPTION = "aria-roledescription";
const GRAPHICS_OBJECT = "graphics-object";
const GRAPHICS_SYMBOL = "graphics-symbol";
const bundle = (role, roledesc, label) => ({
    [ARIA_ROLE]: role,
    [ARIA_ROLEDESCRIPTION]: roledesc,
    [ARIA_LABEL]: label || void 0
});
const AriaIgnore = toSet(["axis-domain", "axis-grid", "axis-label", "axis-tick", "axis-title", "legend-band", "legend-entry", "legend-gradient", "legend-label", "legend-title", "legend-symbol", "title"]);
const AriaGuides = {
    axis: {
        desc: "axis",
        caption: axisCaption
    },
    legend: {
        desc: "legend",
        caption: legendCaption
    },
    "title-text": {
        desc: "title",
        caption: (item) => `Title text '${titleCaption(item)}'`
    },
    "title-subtitle": {
        desc: "subtitle",
        caption: (item) => `Subtitle text '${titleCaption(item)}'`
    }
};
const AriaEncode = {
    ariaRole: ARIA_ROLE,
    ariaRoleDescription: ARIA_ROLEDESCRIPTION,
    description: ARIA_LABEL
};
function ariaItemAttributes(emit2, item) {
    const hide = item.aria === false;
    emit2(ARIA_HIDDEN, hide || void 0);
    if (hide || item.description == null) {
        for (const prop in AriaEncode) {
            emit2(AriaEncode[prop], void 0);
        }
    } else {
        const type2 = item.mark.marktype;
        emit2(ARIA_LABEL, item.description);
        emit2(ARIA_ROLE, item.ariaRole || (type2 === "group" ? GRAPHICS_OBJECT : GRAPHICS_SYMBOL));
        emit2(ARIA_ROLEDESCRIPTION, item.ariaRoleDescription || `${type2} mark`);
    }
}
function ariaMarkAttributes(mark) {
    return mark.aria === false ? {
        [ARIA_HIDDEN]: true
    } : AriaIgnore[mark.role] ? null : AriaGuides[mark.role] ? ariaGuide(mark, AriaGuides[mark.role]) : ariaMark(mark);
}
function ariaMark(mark) {
    const type2 = mark.marktype;
    const recurse2 = type2 === "group" || type2 === "text" || mark.items.some((_) => _.description != null && _.aria !== false);
    return bundle(recurse2 ? GRAPHICS_OBJECT : GRAPHICS_SYMBOL, `${type2} mark container`, mark.description);
}
function ariaGuide(mark, opt) {
    try {
        const item = mark.items[0], caption = opt.caption || (() => "");
        return bundle(opt.role || GRAPHICS_SYMBOL, opt.desc, item.description || caption(item));
    } catch (err) {
        return null;
    }
}
function titleCaption(item) {
    return array(item.text).join(" ");
}
function axisCaption(item) {
    const datum = item.datum, orient = item.orient, title = datum.title ? extractTitle(item) : null, ctx = item.context, scale2 = ctx.scales[datum.scale].value, locale = ctx.dataflow.locale(), type2 = scale2.type, xy = orient === "left" || orient === "right" ? "Y" : "X";
    return `${xy}-axis` + (title ? ` titled '${title}'` : "") + ` for a ${isDiscrete(type2) ? "discrete" : type2} scale with ${domainCaption(locale, scale2, item)}`;
}
function legendCaption(item) {
    const datum = item.datum, title = datum.title ? extractTitle(item) : null, type2 = `${datum.type || ""} legend`.trim(), scales = datum.scales, props = Object.keys(scales), ctx = item.context, scale2 = ctx.scales[scales[props[0]]].value, locale = ctx.dataflow.locale();
    return capitalize(type2) + (title ? ` titled '${title}'` : "") + ` for ${channelCaption(props)} with ${domainCaption(locale, scale2, item)}`;
}
function extractTitle(item) {
    try {
        return array(peek(item.items).items[0].text).join(" ");
    } catch (err) {
        return null;
    }
}
function channelCaption(props) {
    props = props.map((p) => p + (p === "fill" || p === "stroke" ? " color" : ""));
    return props.length < 2 ? props[0] : props.slice(0, -1).join(", ") + " and " + peek(props);
}
function capitalize(s) {
    return s.length ? s[0].toUpperCase() + s.slice(1) : s;
}
const innerText = (val) => (val + "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const attrText = (val) => innerText(val).replace(/"/g, "&quot;").replace(/\t/g, "&#x9;").replace(/\n/g, "&#xA;").replace(/\r/g, "&#xD;");
function markup() {
    let buf = "", outer = "", inner = "";
    const stack = [], clear = () => outer = inner = "", push = (tag) => {
        if (outer) {
            buf += `${outer}>${inner}`;
            clear();
        }
        stack.push(tag);
    }, attr2 = (name, value2) => {
        if (value2 != null)
            outer += ` ${name}="${attrText(value2)}"`;
        return m;
    }, m = {
        open(tag) {
            push(tag);
            outer = "<" + tag;
            for (var _len = arguments.length, attrs = new Array(_len > 1 ? _len - 1 : 0), _key = 1; _key < _len; _key++) {
                attrs[_key - 1] = arguments[_key];
            }
            for (const set of attrs) {
                for (const key in set)
                    attr2(key, set[key]);
            }
            return m;
        },
        close() {
            const tag = stack.pop();
            if (outer) {
                buf += outer + (inner ? `>${inner}</${tag}>` : "/>");
            } else {
                buf += `</${tag}>`;
            }
            clear();
            return m;
        },
        attr: attr2,
        text: (t) => (inner += innerText(t), m),
        toString: () => buf
    };
    return m;
}
const serializeXML = (node) => _serialize(markup(), node) + "";
function _serialize(m, node) {
    m.open(node.tagName);
    if (node.hasAttributes()) {
        const attrs = node.attributes, n = attrs.length;
        for (let i = 0; i < n; ++i) {
            m.attr(attrs[i].name, attrs[i].value);
        }
    }
    if (node.hasChildNodes()) {
        const children = node.childNodes;
        for (const child of children) {
            child.nodeType === 3 ? m.text(child.nodeValue) : _serialize(m, child);
        }
    }
    return m.close();
}
const stylesAttr = {
    fill: "fill",
    fillOpacity: "fill-opacity",
    stroke: "stroke",
    strokeOpacity: "stroke-opacity",
    strokeWidth: "stroke-width",
    strokeCap: "stroke-linecap",
    strokeJoin: "stroke-linejoin",
    strokeDash: "stroke-dasharray",
    strokeDashOffset: "stroke-dashoffset",
    strokeMiterLimit: "stroke-miterlimit",
    opacity: "opacity"
};
const stylesCss = {
    blend: "mix-blend-mode"
};
const rootAttributes = {
    fill: "none",
    "stroke-miterlimit": 10
};
const RootIndex = 0, xmlns = "http://www.w3.org/2000/xmlns/", svgns = metadata.xmlns;
function SVGRenderer(loader2) {
    Renderer.call(this, loader2);
    this._dirtyID = 0;
    this._dirty = [];
    this._svg = null;
    this._root = null;
    this._defs = null;
}
const base = Renderer.prototype;
inherits(SVGRenderer, Renderer, {
    initialize(el, width, height, origin, scaleFactor) {
        this._defs = {};
        this._clearDefs();
        if (el) {
            this._svg = domChild(el, 0, "svg", svgns);
            this._svg.setAttributeNS(xmlns, "xmlns", svgns);
            this._svg.setAttributeNS(xmlns, "xmlns:xlink", metadata["xmlns:xlink"]);
            this._svg.setAttribute("version", metadata["version"]);
            this._svg.setAttribute("class", "marks");
            domClear(el, 1);
            this._root = domChild(this._svg, RootIndex, "g", svgns);
            setAttributes(this._root, rootAttributes);
            domClear(this._svg, RootIndex + 1);
        }
        this.background(this._bgcolor);
        return base.initialize.call(this, el, width, height, origin, scaleFactor);
    },
    background(bgcolor) {
        if (arguments.length && this._svg) {
            this._svg.style.setProperty("background-color", bgcolor);
        }
        return base.background.apply(this, arguments);
    },
    resize(width, height, origin, scaleFactor) {
        base.resize.call(this, width, height, origin, scaleFactor);
        if (this._svg) {
            setAttributes(this._svg, {
                width: this._width * this._scale,
                height: this._height * this._scale,
                viewBox: `0 0 ${this._width} ${this._height}`
            });
            this._root.setAttribute("transform", `translate(${this._origin})`);
        }
        this._dirty = [];
        return this;
    },
    canvas() {
        return this._svg;
    },
    svg() {
        const svg = this._svg, bg = this._bgcolor;
        if (!svg)
            return null;
        let node;
        if (bg) {
            svg.removeAttribute("style");
            node = domChild(svg, RootIndex, "rect", svgns);
            setAttributes(node, {
                width: this._width,
                height: this._height,
                fill: bg
            });
        }
        const text2 = serializeXML(svg);
        if (bg) {
            svg.removeChild(node);
            this._svg.style.setProperty("background-color", bg);
        }
        return text2;
    },
    _render(scene) {
        if (this._dirtyCheck()) {
            if (this._dirtyAll)
                this._clearDefs();
            this.mark(this._root, scene);
            domClear(this._root, 1);
        }
        this.defs();
        this._dirty = [];
        ++this._dirtyID;
        return this;
    },
    dirty(item) {
        if (item.dirty !== this._dirtyID) {
            item.dirty = this._dirtyID;
            this._dirty.push(item);
        }
    },
    isDirty(item) {
        return this._dirtyAll || !item._svg || !item._svg.ownerSVGElement || item.dirty === this._dirtyID;
    },
    _dirtyCheck() {
        this._dirtyAll = true;
        const items = this._dirty;
        if (!items.length || !this._dirtyID)
            return true;
        const id = ++this._dirtyID;
        let item, mark, type2, mdef, i, n, o;
        for (i = 0, n = items.length; i < n; ++i) {
            item = items[i];
            mark = item.mark;
            if (mark.marktype !== type2) {
                type2 = mark.marktype;
                mdef = Marks[type2];
            }
            if (mark.zdirty && mark.dirty !== id) {
                this._dirtyAll = false;
                dirtyParents(item, id);
                mark.items.forEach((i2) => {
                    i2.dirty = id;
                });
            }
            if (mark.zdirty)
                continue;
            if (item.exit) {
                if (mdef.nested && mark.items.length) {
                    o = mark.items[0];
                    if (o._svg)
                        this._update(mdef, o._svg, o);
                } else if (item._svg) {
                    o = item._svg.parentNode;
                    if (o)
                        o.removeChild(item._svg);
                }
                item._svg = null;
                continue;
            }
            item = mdef.nested ? mark.items[0] : item;
            if (item._update === id)
                continue;
            if (!item._svg || !item._svg.ownerSVGElement) {
                this._dirtyAll = false;
                dirtyParents(item, id);
            } else {
                this._update(mdef, item._svg, item);
            }
            item._update = id;
        }
        return !this._dirtyAll;
    },
    mark(el, scene, prev) {
        if (!this.isDirty(scene)) {
            return scene._svg;
        }
        const svg = this._svg, mdef = Marks[scene.marktype], events = scene.interactive === false ? "none" : null, isGroup = mdef.tag === "g";
        const parent = bind(scene, el, prev, "g", svg);
        parent.setAttribute("class", cssClass(scene));
        const aria = ariaMarkAttributes(scene);
        for (const key in aria)
            setAttribute(parent, key, aria[key]);
        if (!isGroup) {
            setAttribute(parent, "pointer-events", events);
        }
        setAttribute(parent, "clip-path", scene.clip ? clip$1(this, scene, scene.group) : null);
        let sibling = null, i = 0;
        const process = (item) => {
            const dirty = this.isDirty(item), node = bind(item, parent, sibling, mdef.tag, svg);
            if (dirty) {
                this._update(mdef, node, item);
                if (isGroup)
                    recurse(this, node, item);
            }
            sibling = node;
            ++i;
        };
        if (mdef.nested) {
            if (scene.items.length)
                process(scene.items[0]);
        } else {
            visit(scene, process);
        }
        domClear(parent, i);
        return parent;
    },
    _update(mdef, el, item) {
        element = el;
        values = el.__values__;
        ariaItemAttributes(emit, item);
        mdef.attr(emit, item, this);
        const extra = mark_extras[mdef.type];
        if (extra)
            extra.call(this, mdef, el, item);
        if (element)
            this.style(element, item);
    },
    style(el, item) {
        if (item == null)
            return;
        for (const prop in stylesAttr) {
            let value2 = prop === "font" ? fontFamily(item) : item[prop];
            if (value2 === values[prop])
                continue;
            const name = stylesAttr[prop];
            if (value2 == null) {
                el.removeAttribute(name);
            } else {
                if (isGradient(value2)) {
                    value2 = gradientRef(value2, this._defs.gradient, href());
                }
                el.setAttribute(name, value2 + "");
            }
            values[prop] = value2;
        }
        for (const prop in stylesCss) {
            setStyle(el, stylesCss[prop], item[prop]);
        }
    },
    defs() {
        const svg = this._svg, defs = this._defs;
        let el = defs.el, index = 0;
        for (const id in defs.gradient) {
            if (!el)
                defs.el = el = domChild(svg, RootIndex + 1, "defs", svgns);
            index = updateGradient(el, defs.gradient[id], index);
        }
        for (const id in defs.clipping) {
            if (!el)
                defs.el = el = domChild(svg, RootIndex + 1, "defs", svgns);
            index = updateClipping(el, defs.clipping[id], index);
        }
        if (el) {
            index === 0 ? (svg.removeChild(el), defs.el = null) : domClear(el, index);
        }
    },
    _clearDefs() {
        const def2 = this._defs;
        def2.gradient = {};
        def2.clipping = {};
    }
});
function dirtyParents(item, id) {
    for (; item && item.dirty !== id; item = item.mark.group) {
        item.dirty = id;
        if (item.mark && item.mark.dirty !== id) {
            item.mark.dirty = id;
        } else
            return;
    }
}
function updateGradient(el, grad, index) {
    let i, n, stop;
    if (grad.gradient === "radial") {
        let pt = domChild(el, index++, "pattern", svgns);
        setAttributes(pt, {
            id: patternPrefix + grad.id,
            viewBox: "0,0,1,1",
            width: "100%",
            height: "100%",
            preserveAspectRatio: "xMidYMid slice"
        });
        pt = domChild(pt, 0, "rect", svgns);
        setAttributes(pt, {
            width: 1,
            height: 1,
            fill: `url(${href()}#${grad.id})`
        });
        el = domChild(el, index++, "radialGradient", svgns);
        setAttributes(el, {
            id: grad.id,
            fx: grad.x1,
            fy: grad.y1,
            fr: grad.r1,
            cx: grad.x2,
            cy: grad.y2,
            r: grad.r2
        });
    } else {
        el = domChild(el, index++, "linearGradient", svgns);
        setAttributes(el, {
            id: grad.id,
            x1: grad.x1,
            x2: grad.x2,
            y1: grad.y1,
            y2: grad.y2
        });
    }
    for (i = 0, n = grad.stops.length; i < n; ++i) {
        stop = domChild(el, i, "stop", svgns);
        stop.setAttribute("offset", grad.stops[i].offset);
        stop.setAttribute("stop-color", grad.stops[i].color);
    }
    domClear(el, i);
    return index;
}
function updateClipping(el, clip2, index) {
    let mask;
    el = domChild(el, index, "clipPath", svgns);
    el.setAttribute("id", clip2.id);
    if (clip2.path) {
        mask = domChild(el, 0, "path", svgns);
        mask.setAttribute("d", clip2.path);
    } else {
        mask = domChild(el, 0, "rect", svgns);
        setAttributes(mask, {
            x: 0,
            y: 0,
            width: clip2.width,
            height: clip2.height
        });
    }
    domClear(el, 1);
    return index + 1;
}
function recurse(renderer, el, group2) {
    el = el.lastChild.previousSibling;
    let prev, idx = 0;
    visit(group2, (item) => {
        prev = renderer.mark(el, item, prev);
        ++idx;
    });
    domClear(el, 1 + idx);
}
function bind(item, el, sibling, tag, svg) {
    let node = item._svg, doc;
    if (!node) {
        doc = el.ownerDocument;
        node = domCreate(doc, tag, svgns);
        item._svg = node;
        if (item.mark) {
            node.__data__ = item;
            node.__values__ = {
                fill: "default"
            };
            if (tag === "g") {
                const bg = domCreate(doc, "path", svgns);
                node.appendChild(bg);
                bg.__data__ = item;
                const cg = domCreate(doc, "g", svgns);
                node.appendChild(cg);
                cg.__data__ = item;
                const fg = domCreate(doc, "path", svgns);
                node.appendChild(fg);
                fg.__data__ = item;
                fg.__values__ = {
                    fill: "default"
                };
            }
        }
    }
    if (node.ownerSVGElement !== svg || siblingCheck(node, sibling)) {
        el.insertBefore(node, sibling ? sibling.nextSibling : el.firstChild);
    }
    return node;
}
function siblingCheck(node, sibling) {
    return node.parentNode && node.parentNode.childNodes.length > 1 && node.previousSibling != sibling;
}
let element = null, values = null;
const mark_extras = {
    group(mdef, el, item) {
        const fg = element = el.childNodes[2];
        values = fg.__values__;
        mdef.foreground(emit, item, this);
        values = el.__values__;
        element = el.childNodes[1];
        mdef.content(emit, item, this);
        const bg = element = el.childNodes[0];
        mdef.background(emit, item, this);
        const value2 = item.mark.interactive === false ? "none" : null;
        if (value2 !== values.events) {
            setAttribute(fg, "pointer-events", value2);
            setAttribute(bg, "pointer-events", value2);
            values.events = value2;
        }
        if (item.strokeForeground && item.stroke) {
            const fill2 = item.fill;
            setAttribute(fg, "display", null);
            this.style(bg, item);
            setAttribute(bg, "stroke", null);
            if (fill2)
                item.fill = null;
            values = fg.__values__;
            this.style(fg, item);
            if (fill2)
                item.fill = fill2;
            element = null;
        } else {
            setAttribute(fg, "display", "none");
        }
    },
    image(mdef, el, item) {
        if (item.smooth === false) {
            setStyle(el, "image-rendering", "optimizeSpeed");
            setStyle(el, "image-rendering", "pixelated");
        } else {
            setStyle(el, "image-rendering", null);
        }
    },
    text(mdef, el, item) {
        const tl2 = textLines(item);
        let key, value2, doc, lh;
        if (isArray(tl2)) {
            value2 = tl2.map((_) => textValue(item, _));
            key = value2.join("\n");
            if (key !== values.text) {
                domClear(el, 0);
                doc = el.ownerDocument;
                lh = lineHeight(item);
                value2.forEach((t, i) => {
                    const ts2 = domCreate(doc, "tspan", svgns);
                    ts2.__data__ = item;
                    ts2.textContent = t;
                    if (i) {
                        ts2.setAttribute("x", 0);
                        ts2.setAttribute("dy", lh);
                    }
                    el.appendChild(ts2);
                });
                values.text = key;
            }
        } else {
            value2 = textValue(item, tl2);
            if (value2 !== values.text) {
                el.textContent = value2;
                values.text = value2;
            }
        }
        setAttribute(el, "font-family", fontFamily(item));
        setAttribute(el, "font-size", fontSize(item) + "px");
        setAttribute(el, "font-style", item.fontStyle);
        setAttribute(el, "font-variant", item.fontVariant);
        setAttribute(el, "font-weight", item.fontWeight);
    }
};
function emit(name, value2, ns) {
    if (value2 === values[name])
        return;
    if (ns) {
        setAttributeNS(element, name, value2, ns);
    } else {
        setAttribute(element, name, value2);
    }
    values[name] = value2;
}
function setStyle(el, name, value2) {
    if (value2 !== values[name]) {
        if (value2 == null) {
            el.style.removeProperty(name);
        } else {
            el.style.setProperty(name, value2 + "");
        }
        values[name] = value2;
    }
}
function setAttributes(el, attrs) {
    for (const key in attrs) {
        setAttribute(el, key, attrs[key]);
    }
}
function setAttribute(el, name, value2) {
    if (value2 != null) {
        el.setAttribute(name, value2);
    } else {
        el.removeAttribute(name);
    }
}
function setAttributeNS(el, name, value2, ns) {
    if (value2 != null) {
        el.setAttributeNS(ns, name, value2);
    } else {
        el.removeAttributeNS(ns, name);
    }
}
function href() {
    let loc;
    return typeof window === "undefined" ? "" : (loc = window.location).hash ? loc.href.slice(0, -loc.hash.length) : loc.href;
}
function SVGStringRenderer(loader2) {
    Renderer.call(this, loader2);
    this._text = null;
    this._defs = {
        gradient: {},
        clipping: {}
    };
}
inherits(SVGStringRenderer, Renderer, {
    svg() {
        return this._text;
    },
    _render(scene) {
        const m = markup();
        m.open("svg", extend({}, metadata, {
            class: "marks",
            width: this._width * this._scale,
            height: this._height * this._scale,
            viewBox: `0 0 ${this._width} ${this._height}`
        }));
        const bg = this._bgcolor;
        if (bg && bg !== "transparent" && bg !== "none") {
            m.open("rect", {
                width: this._width,
                height: this._height,
                fill: bg
            }).close();
        }
        m.open("g", rootAttributes, {
            transform: "translate(" + this._origin + ")"
        });
        this.mark(m, scene);
        m.close();
        this.defs(m);
        this._text = m.close() + "";
        return this;
    },
    mark(m, scene) {
        const mdef = Marks[scene.marktype], tag = mdef.tag, attrList = [ariaItemAttributes, mdef.attr];
        m.open("g", {
            class: cssClass(scene),
            "clip-path": scene.clip ? clip$1(this, scene, scene.group) : null
        }, ariaMarkAttributes(scene), {
            "pointer-events": tag !== "g" && scene.interactive === false ? "none" : null
        });
        const process = (item) => {
            const href2 = this.href(item);
            if (href2)
                m.open("a", href2);
            m.open(tag, this.attr(scene, item, attrList, tag !== "g" ? tag : null));
            if (tag === "text") {
                const tl2 = textLines(item);
                if (isArray(tl2)) {
                    const attrs = {
                        x: 0,
                        dy: lineHeight(item)
                    };
                    for (let i = 0; i < tl2.length; ++i) {
                        m.open("tspan", i ? attrs : null).text(textValue(item, tl2[i])).close();
                    }
                } else {
                    m.text(textValue(item, tl2));
                }
            } else if (tag === "g") {
                const fore = item.strokeForeground, fill2 = item.fill, stroke2 = item.stroke;
                if (fore && stroke2) {
                    item.stroke = null;
                }
                m.open("path", this.attr(scene, item, mdef.background, "bgrect")).close();
                m.open("g", this.attr(scene, item, mdef.content));
                visit(item, (scene2) => this.mark(m, scene2));
                m.close();
                if (fore && stroke2) {
                    if (fill2)
                        item.fill = null;
                    item.stroke = stroke2;
                    m.open("path", this.attr(scene, item, mdef.foreground, "bgrect")).close();
                    if (fill2)
                        item.fill = fill2;
                } else {
                    m.open("path", this.attr(scene, item, mdef.foreground, "bgfore")).close();
                }
            }
            m.close();
            if (href2)
                m.close();
        };
        if (mdef.nested) {
            if (scene.items && scene.items.length)
                process(scene.items[0]);
        } else {
            visit(scene, process);
        }
        return m.close();
    },
    href(item) {
        const href2 = item.href;
        let attr2;
        if (href2) {
            if (attr2 = this._hrefs && this._hrefs[href2]) {
                return attr2;
            } else {
                this.sanitizeURL(href2).then((attr3) => {
                    attr3["xlink:href"] = attr3.href;
                    attr3.href = null;
                    (this._hrefs || (this._hrefs = {}))[href2] = attr3;
                });
            }
        }
        return null;
    },
    attr(scene, item, attrs, tag) {
        const object = {}, emit2 = (name, value2, ns, prefixed) => {
            object[prefixed || name] = value2;
        };
        if (Array.isArray(attrs)) {
            attrs.forEach((fn) => fn(emit2, item, this));
        } else {
            attrs(emit2, item, this);
        }
        if (tag) {
            style(object, item, scene, tag, this._defs);
        }
        return object;
    },
    defs(m) {
        const gradient2 = this._defs.gradient, clipping = this._defs.clipping, count = Object.keys(gradient2).length + Object.keys(clipping).length;
        if (count === 0)
            return;
        m.open("defs");
        for (const id in gradient2) {
            const def2 = gradient2[id], stops = def2.stops;
            if (def2.gradient === "radial") {
                m.open("pattern", {
                    id: patternPrefix + id,
                    viewBox: "0,0,1,1",
                    width: "100%",
                    height: "100%",
                    preserveAspectRatio: "xMidYMid slice"
                });
                m.open("rect", {
                    width: "1",
                    height: "1",
                    fill: "url(#" + id + ")"
                }).close();
                m.close();
                m.open("radialGradient", {
                    id,
                    fx: def2.x1,
                    fy: def2.y1,
                    fr: def2.r1,
                    cx: def2.x2,
                    cy: def2.y2,
                    r: def2.r2
                });
            } else {
                m.open("linearGradient", {
                    id,
                    x1: def2.x1,
                    x2: def2.x2,
                    y1: def2.y1,
                    y2: def2.y2
                });
            }
            for (let i = 0; i < stops.length; ++i) {
                m.open("stop", {
                    offset: stops[i].offset,
                    "stop-color": stops[i].color
                }).close();
            }
            m.close();
        }
        for (const id in clipping) {
            const def2 = clipping[id];
            m.open("clipPath", {
                id
            });
            if (def2.path) {
                m.open("path", {
                    d: def2.path
                }).close();
            } else {
                m.open("rect", {
                    x: 0,
                    y: 0,
                    width: def2.width,
                    height: def2.height
                }).close();
            }
            m.close();
        }
        m.close();
    }
});
function style(s, item, scene, tag, defs) {
    let styleList;
    if (item == null)
        return s;
    if (tag === "bgrect" && scene.interactive === false) {
        s["pointer-events"] = "none";
    }
    if (tag === "bgfore") {
        if (scene.interactive === false) {
            s["pointer-events"] = "none";
        }
        s.display = "none";
        if (item.fill !== null)
            return s;
    }
    if (tag === "image" && item.smooth === false) {
        styleList = ["image-rendering: optimizeSpeed;", "image-rendering: pixelated;"];
    }
    if (tag === "text") {
        s["font-family"] = fontFamily(item);
        s["font-size"] = fontSize(item) + "px";
        s["font-style"] = item.fontStyle;
        s["font-variant"] = item.fontVariant;
        s["font-weight"] = item.fontWeight;
    }
    for (const prop in stylesAttr) {
        let value2 = item[prop];
        const name = stylesAttr[prop];
        if (value2 === "transparent" && (name === "fill" || name === "stroke"))
            ;
        else if (value2 != null) {
            if (isGradient(value2)) {
                value2 = gradientRef(value2, defs.gradient, "");
            }
            s[name] = value2;
        }
    }
    for (const prop in stylesCss) {
        const value2 = item[prop];
        if (value2 != null) {
            styleList = styleList || [];
            styleList.push(`${stylesCss[prop]}: ${value2};`);
        }
    }
    if (styleList) {
        s.style = styleList.join(" ");
    }
    return s;
}
const Canvas = "canvas";
const PNG = "png";
const SVG = "svg";
const None = "none";
const RenderType = {
    Canvas,
    PNG,
    SVG,
    None
};
const modules = {};
modules[Canvas] = modules[PNG] = {
    renderer: CanvasRenderer,
    headless: CanvasRenderer,
    handler: CanvasHandler
};
modules[SVG] = {
    renderer: SVGRenderer,
    headless: SVGStringRenderer,
    handler: SVGHandler
};
modules[None] = {};
function renderModule(name, _) {
    name = String(name || "").toLowerCase();
    if (arguments.length > 1) {
        modules[name] = _;
        return this;
    } else {
        return modules[name];
    }
}
function intersect(scene, bounds2, filter) {
    const hits = [], box = new Bounds().union(bounds2), type2 = scene.marktype;
    return type2 ? intersectMark(scene, box, filter, hits) : type2 === "group" ? intersectGroup(scene, box, filter, hits) : error("Intersect scene must be mark node or group item.");
}
function intersectMark(mark, box, filter, hits) {
    if (visitMark(mark, box, filter)) {
        const items = mark.items, type2 = mark.marktype, n = items.length;
        let i = 0;
        if (type2 === "group") {
            for (; i < n; ++i) {
                intersectGroup(items[i], box, filter, hits);
            }
        } else {
            for (const test = Marks[type2].isect; i < n; ++i) {
                const item = items[i];
                if (intersectItem(item, box, test))
                    hits.push(item);
            }
        }
    }
    return hits;
}
function visitMark(mark, box, filter) {
    return mark.bounds && box.intersects(mark.bounds) && (mark.marktype === "group" || mark.interactive !== false && (!filter || filter(mark)));
}
function intersectGroup(group2, box, filter, hits) {
    if (filter && filter(group2.mark) && intersectItem(group2, box, Marks.group.isect)) {
        hits.push(group2);
    }
    const marks = group2.items, n = marks && marks.length;
    if (n) {
        const x2 = group2.x || 0, y2 = group2.y || 0;
        box.translate(-x2, -y2);
        for (let i = 0; i < n; ++i) {
            intersectMark(marks[i], box, filter, hits);
        }
        box.translate(x2, y2);
    }
    return hits;
}
function intersectItem(item, box, test) {
    const bounds2 = item.bounds;
    return box.encloses(bounds2) || box.intersects(bounds2) && test(item, box);
}
const clipBounds = new Bounds();
function boundClip(mark) {
    const clip2 = mark.clip;
    if (isFunction(clip2)) {
        clip2(boundContext(clipBounds.clear()));
    } else if (clip2) {
        clipBounds.set(0, 0, mark.group.width, mark.group.height);
    } else
        return;
    mark.bounds.intersect(clipBounds);
}
const TOLERANCE = 1e-9;
function sceneEqual(a, b2, key) {
    return a === b2 ? true : key === "path" ? pathEqual(a, b2) : a instanceof Date && b2 instanceof Date ? +a === +b2 : isNumber(a) && isNumber(b2) ? Math.abs(a - b2) <= TOLERANCE : !a || !b2 || !isObject(a) && !isObject(b2) ? a == b2 : objectEqual(a, b2);
}
function pathEqual(a, b2) {
    return sceneEqual(parse(a), parse(b2));
}
function objectEqual(a, b2) {
    var ka = Object.keys(a), kb = Object.keys(b2), key, i;
    if (ka.length !== kb.length)
        return false;
    ka.sort();
    kb.sort();
    for (i = ka.length - 1; i >= 0; i--) {
        if (ka[i] != kb[i])
            return false;
    }
    for (i = ka.length - 1; i >= 0; i--) {
        key = ka[i];
        if (!sceneEqual(a[key], b2[key], key))
            return false;
    }
    return typeof a === typeof b2;
}
function resetSVGDefIds() {
    resetSVGClipId();
    resetSVGGradientId();
}
export {Bounds, CanvasHandler, CanvasRenderer, Gradient, GroupItem, Handler, Item, Marks, RenderType, Renderer, ResourceLoader, SVGHandler, SVGRenderer, SVGStringRenderer, Scenegraph, boundClip, boundContext, boundItem, boundMark, boundStroke, domChild, domClear, domCreate, domFind, font, fontFamily, fontSize, intersect, intersectBoxLine, intersectPath, intersectPoint, intersectRule, lineHeight, markup, multiLineOffset, curves as pathCurves, pathEqual, parse as pathParse, vg_rect as pathRectangle, pathRender, symbols as pathSymbols, vg_trail as pathTrail, point, renderModule, resetSVGClipId, resetSVGDefIds, sceneEqual, sceneFromJSON, pickVisit as scenePickVisit, sceneToJSON, visit as sceneVisit, zorder as sceneZOrder, serializeXML, textMetrics};
export default null;
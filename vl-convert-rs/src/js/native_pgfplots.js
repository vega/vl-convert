// Native, data-coordinate PGFPlots backend. This exact source runs in Deno and
// in the dependency-free Node test suite. Never evaluate input expressions.
globalThis.vlcNativePgfplots = (() => {
  "use strict";
  const LIMIT = { rows: 100000, panels: 64, depth: 32, output: 32 * 1024 * 1024 };
  const PALETTE = ["#4c78a8", "#f58518", "#e45756", "#72b7b2", "#54a24b", "#eeca3b", "#b279a2", "#ff9da6", "#9d755d", "#bab0ac"];
  const own = (o, k) => Object.prototype.hasOwnProperty.call(o, k);
  const fail = (path, message) => { throw new Error(`PGFPlots: ${path}: ${message}`); };
  function object(v, p) {
    if (!v || typeof v !== "object" || Array.isArray(v)) fail(p, "expected an object");
    return v;
  }
  function keys(v, allowed, p) {
    object(v, p);
    for (const k of Object.keys(v)) if (!allowed.includes(k)) fail(`${p}.${k}`, "unsupported property (not silently discarded)");
  }
  function number(v, p, min = -Infinity, max = Infinity) {
    if (typeof v !== "number" || !Number.isFinite(v) || v < min || v > max) fail(p, `expected a finite number in [${min}, ${max}]`);
    return v;
  }
  function array(v, p) { if (!Array.isArray(v)) fail(p, "expected an array"); return v; }
  function boolean(v, p) { if (typeof v !== "boolean") fail(p, "expected a boolean"); return v; }
  function choice(v, values, p) { if (!values.includes(v)) fail(p, `unsupported value ${JSON.stringify(v)}`); return v; }
  const id = v => JSON.stringify(v, (_, x) => x && typeof x === "object" && !Array.isArray(x) ? Object.fromEntries(Object.keys(x).sort().map(k => [k, x[k]])) : x);
  const unique = a => [...new Map(a.map(v => [id(v), v])).values()];
  const compare = (a, b) => typeof a === "number" && typeof b === "number" ? a - b : String(a) < String(b) ? -1 : String(a) > String(b) ? 1 : 0;
  const numeric = v => typeof v === "number" && Number.isFinite(v);
  const fmt = v => String(Object.is(v, -0) ? 0 : number(v, "output coordinate"));
  const bp = v => `${fmt(v * 0.75)}bp`;
  function label(v, raw) {
    if (v == null) return "";
    if (Array.isArray(v)) { if (v.length > 32 || v.some(Array.isArray)) fail("label", "at most 32 scalar label lines are supported"); return "\\shortstack{" + v.map(x => label(x, raw)).join("\\\\") + "}"; }
    if (!["string", "number", "boolean"].includes(typeof v)) fail("label", "expected text");
    if (String(v).length > 65536) fail("label", "label length limit exceeded");
    if (raw) return String(v);
    if (/[\u{10000}-\u{10ffff}]/u.test(String(v))) fail("label", "non-BMP glyphs require a trusted TeX label; the standalone pdfLaTeX font cannot render them");
    const escapes = { "\\": "\\textbackslash{}", "{": "\\{", "}": "\\}", "$": "\\$", "&": "\\&", "#": "\\#", "%": "\\%", "_": "\\_", "^": "\\textasciicircum{}", "~": "\\textasciitilde{}" };
    const math = {α:"alpha",β:"beta",γ:"gamma",δ:"delta",ε:"epsilon",θ:"theta",λ:"lambda",μ:"mu",µ:"mu",π:"pi",σ:"sigma",φ:"phi",ω:"omega",Γ:"Gamma",Δ:"Delta",Θ:"Theta",Λ:"Lambda",Π:"Pi",Σ:"Sigma",Φ:"Phi",Ω:"Omega","±":"pm","×":"times","≤":"leq","≥":"geq","≠":"neq","∞":"infty"};
    return String(v).replace(/[\\{}$&#%_^~]/g, c => escapes[c]).replace(/[\r\n\t\u2028\u2029]/g, " ")
      .replace(/[αβγδεθλμµπσφωΓΔΘΛΠΣΦΩ±×≤≥≠∞]/g,c=>`\\ensuremath{\\${math[c]}}`).replace(/−/g,"-");
  }
  function color(value, p) {
    const named = { black: "#000000", white: "#ffffff", red: "#ff0000", green: "#008000", blue: "#0000ff", steelblue: "#4682b4", orange: "#ffa500", gray: "#808080", grey: "#808080", purple: "#800080", yellow: "#ffff00", cyan: "#00ffff", magenta: "#ff00ff", navy: "#000080", teal: "#008080", brown: "#a52a2a" };
    let s = typeof value === "string" ? value.toLowerCase() : "";
    s = named[s] || s;
    if (/^#[0-9a-f]{3}$/.test(s)) s = "#" + [...s.slice(1)].map(c => c + c).join("");
    if (!/^#[0-9a-f]{6}$/.test(s)) fail(p, "use a supported color name or #RRGGBB (CSS expressions are not TeX)");
    return [1, 3, 5].map(i => parseInt(s.slice(i, i + 2), 16));
  }
  const texColor = rgb => rgb === null ? "none" : `{rgb,255:red,${rgb[0]};green,${rgb[1]};blue,${rgb[2]}}`;
  function field(row, name, p) {
    if (typeof name === "string" && /[.\[\]\\]/.test(name)) fail(p,"nested/escaped field paths are unsupported; materialize a flat field name");
    if (typeof name !== "string" || !own(row, name)) fail(p, `missing literal data field ${JSON.stringify(name)}`);
    return row[name];
  }
  function value(row, enc, p) {
    if (!enc) return null;
    if (own(enc, "datum")) return enc.datum;
    if (own(enc, "value")) return enc.value;
    return field(row, enc.field, p);
  }
  function groups(rows, fields) {
    const result = new Map();
    for (const row of rows) {
      const key = id(fields.map(f => field(row, f, "groupby")));
      if (!result.has(key)) result.set(key, []);
      result.get(key).push(row);
    }
    return [...result.values()];
  }
  function sum(values) {
    let total = 0, correction = 0;
    for (const v of values) { const x = v - correction, t = total + x; correction = (t - total) - x; total = t; }
    return number(total, "aggregate sum");
  }
  function quantile(sorted, q) {
    if (!sorted.length) return null;
    const i = (sorted.length - 1) * q, lo = Math.floor(i), hi = Math.ceil(i);
    return sorted[lo] * (1 - (i - lo)) + sorted[hi] * (i - lo);
  }
  function statistic(rows, op, name, p) {
    choice(op, ["count", "sum", "mean", "average", "min", "max", "median", "q1", "q3", "stdev", "stderr"], p);
    if (op === "count") return rows.length;
    const values = rows.map(r => field(r, name, p)).filter(v => v != null).map(v => number(v, p));
    if (!values.length) return null;
    switch (op) {
      case "sum": return sum(values);
      case "mean": case "average": return sum(values) / values.length;
      case "min": return values.reduce((a, b) => Math.min(a, b));
      case "max": return values.reduce((a, b) => Math.max(a, b));
      case "median": case "q1": case "q3": return quantile(values.sort((a, b) => a - b), {median: .5, q1: .25, q3: .75}[op]);
      default: {
        if (values.length < 2) fail(p, "sample standard deviation/standard error requires at least two observations");
        const mean = sum(values) / values.length;
        const sd = Math.sqrt(sum(values.map(v => (v - mean) ** 2)) / (values.length - 1));
        return op === "stderr" ? sd / Math.sqrt(values.length) : sd;
      }
    }
  }
  function predicate(pred, row, p, depth = 0) {
    if (depth > LIMIT.depth) fail(p, "predicate nesting limit exceeded");
    object(pred, p);
    if (own(pred, "and") || own(pred, "or")) {
      const op = own(pred, "and") ? "and" : "or";
      keys(pred, [op], p);
      const results = array(pred[op], p).map(v => predicate(v, row, p, depth + 1));
      return op === "and" ? results.every(Boolean) : results.some(Boolean);
    }
    if (own(pred, "not")) { keys(pred, ["not"], p); return !predicate(pred.not, row, p, depth + 1); }
    keys(pred, ["field", "equal", "oneOf", "range", "lt", "lte", "gt", "gte", "valid"], p);
    const ops = Object.keys(pred).filter(k => k !== "field");
    if (ops.length !== 1) fail(p, "a field predicate must have exactly one operation");
    const v = field(row, pred.field, p), op = ops[0], a = pred[op];
    if (op === "equal") return id(v) === id(a);
    if (op === "oneOf") return array(a, p).some(x => id(x) === id(v));
    if (op === "valid") return (v != null && !(typeof v === "number" && !Number.isFinite(v))) === boolean(a, p);
    if (v == null) return false;
    number(v, p);
    if (op === "range") {
      if (array(a, p).length !== 2) fail(p, "range needs two bounds");
      return (a[0] == null || v >= number(a[0], p)) && (a[1] == null || v <= number(a[1], p));
    }
    number(a, p);
    return {lt: v < a, lte: v <= a, gt: v > a, gte: v >= a}[op];
  }
  function validatePredicate(pred,p,depth=0) {
    if(depth>LIMIT.depth)fail(p,"predicate nesting limit exceeded");
    object(pred,p);
    for(const op of ["and","or","not"])if(own(pred,op)){
      keys(pred,[op],p);
      const children=op==="not"?[pred.not]:array(pred[op],p);
      children.forEach(c=>validatePredicate(c,p,depth+1));return;
    }
    keys(pred,["field","equal","oneOf","range","lt","lte","gt","gte","valid"],p);
    const ops=Object.keys(pred).filter(k=>k!=="field");
    if(typeof pred.field!=="string" || ops.length!==1)fail(p,"field predicates need a field and one operation");
    const op=ops[0],v=pred[op];
    if(op==="oneOf")array(v,p);
    if(op==="range"){if(array(v,p).length!==2)fail(p,"range needs two bounds");v.filter(x=>x!=null).forEach(x=>number(x,p));}
    if(["lt","lte","gt","gte"].includes(op))number(v,p);
    if(op==="valid")boolean(v,p);
  }
  function data(input, inherited, datasets, p) {
    if (input === undefined) return inherited || [{}];
    keys(input, ["values", "name"], p);
    if (own(input, "values") === own(input, "name")) fail(p, "provide either inline values or a named inline dataset");
    const rows = own(input, "values") ? input.values : datasets[input.name];
    array(rows, p);
    if (rows.length > LIMIT.rows) fail(p, "row limit exceeded");
    return rows.map((r, i) => ({...object(r, `${p}[${i}]`)}));
  }
  function transforms(rows, list, p) {
    for (const [i, t] of array(list || [], p).entries()) {
      const at = `${p}[${i}]`;
      object(t, at);
      if (own(t, "filter")) { keys(t, ["filter"], at); validatePredicate(t.filter,at); rows = rows.filter(r => predicate(t.filter, r, at)); }
      else if (own(t, "aggregate")) {
        keys(t, ["aggregate", "groupby"], at);
        const by = array(t.groupby || [], at);
        const terms = array(t.aggregate, at);
        for(const a of terms) choice(a.op,["count","sum","mean","average","min","max","median","q1","q3","stdev","stderr"],at);
        for (const a of terms) {
          keys(a, ["op", "field", "as"], at);
          if (typeof a.as !== "string" || by.includes(a.as)) fail(at, "aggregate output names must be strings distinct from grouping fields");
        }
        if (unique(terms.map(a => a.as)).length !== terms.length) fail(at, "duplicate aggregate output names");
        rows = groups(rows, by).map(g => Object.fromEntries([
          ...by.map(f => [f, g[0][f]]), ...terms.map(a => [a.as, statistic(g, a.op, a.field, at)]),
        ]));
      } else if (own(t, "fold")) {
        keys(t, ["fold", "as"], at);
        const fields = array(t.fold, at), names = t.as || ["key", "value"];
        if (array(names, at).length !== 2 || names.some(n => typeof n !== "string") || names[0] === names[1]) fail(at, "fold.as requires two distinct field names");
        if (rows.length * fields.length > LIMIT.rows) fail(at, "fold row limit exceeded");
        rows = rows.flatMap(r => fields.map(f => ({...r, [names[0]]: f, [names[1]]: field(r, f, at)})));
      } else fail(at, "only structured filter, aggregate, and fold transforms are supported; materialize other transforms first");
    }
    return rows;
  }
  const ENCODINGS = ["x", "y", "xOffset", "yOffset", "x2", "y2", "xError", "xError2", "yError", "yError2", "color", "detail", "order", "text", "opacity"];
  function encodings(input, p) {
    keys(input, ENCODINGS, p);
    const result = {};
    for (const [channel, def] of Object.entries(input)) {
      const at = `${p}.${channel}`;
      const allowed = channel === "x" || channel === "y"
        ? ["field", "type", "datum", "value", "title", "axis", "scale", "sort", "aggregate", "bin", "stack"]
        : channel === "color" ? ["field", "type", "datum", "value", "title", "scale", "sort", "aggregate", "legend"]
        : channel === "xOffset" || channel === "yOffset" ? ["field", "type"]
        : channel === "order" ? ["field", "type", "sort"]
        : ["field", "type", "datum", "value", "aggregate"];
      keys(def, allowed, at);
      const sources = ["field", "datum", "value"].filter(k => own(def, k));
      if (sources.length !== 1 && !(sources.length === 0 && def.aggregate === "count")) fail(at, "expected exactly one of field/datum/value");
      if (own(def, "value") && !["color", "text", "opacity"].includes(channel)) fail(at, "pixel-position value encodings are not semantic coordinates; use datum");
      if (def.field !== undefined && typeof def.field !== "string") fail(at, "field must be a literal string");
      if (def.type !== undefined) choice(def.type, ["quantitative", "ordinal", "nominal"], `${at}.type`);
      if (def.scale !== undefined) {
        keys(def.scale, ["type", "domain", "range", "scheme", "reverse", "zero", "nice", "base"], `${at}.scale`);
        if (def.scale.type !== undefined) choice(def.scale.type, ["linear", "log", "band", "point", "ordinal"], `${at}.scale.type`);
        for (const key of ["reverse", "zero", "nice"]) if (own(def.scale, key)) boolean(def.scale[key], `${at}.scale.${key}`);
        if (def.scale.base !== undefined) {
          if (def.scale.type !== "log") fail(at, "scale.base only applies to logarithmic scales");
          number(def.scale.base, at, 1.00000001);
        }
        if (def.scale.type === "log" && def.scale.zero === true) fail(at, "log scales cannot include zero");
        if (def.scale.scheme !== undefined && channel !== "color") fail(at, "color schemes only apply to color encodings");
        if (def.scale.range !== undefined && channel !== "color") fail(at, "explicit pixel scale ranges are not native plot semantics");
      }
      if (def.axis !== undefined && def.axis !== null) {
        keys(def.axis, ["title", "grid", "values", "labels", "labelAngle", "format", "_time", "tickLabels", "domain", "ticks", "tickSize", "titleColor", "labelColor", "gridColor", "domainColor", "tickColor", "labelFontSize", "titleFontSize", "labelPadding", "titlePadding"], `${at}.axis`);
        for (const key of ["grid", "labels"]) if (own(def.axis, key)) boolean(def.axis[key], `${at}.axis.${key}`);
        if (def.axis.labelAngle !== undefined) number(def.axis.labelAngle, at, -360, 360);
      }
      if (def.legend !== undefined && def.legend !== null) keys(def.legend, ["title", "orient", "direction"], `${at}.legend`);
      if (def.stack !== undefined && def.stack !== null) choice(def.stack, ["zero", "normalize"], `${at}.stack`);
      result[channel] = {...def, title: own(def, "title") ? def.title : def.aggregate ? `${def.aggregate}(${def.field || "rows"})` : def.field};
    }
    return result;
  }
  function encodeTransforms(unit, p) {
    let {rows, enc} = unit;
    for (const channel of ["x", "y"]) {
      const def = enc[channel];
      if (!def || !def.bin) continue;
      keys(def.bin, ["step", "extent"], `${p}.${channel}.bin`);
      const step = number(def.bin.step, p, Number.MIN_VALUE);
      const extent = array(def.bin.extent, p);
      if (extent.length !== 2) fail(p, "bin extent needs two bounds");
      const lo = number(extent[0], p), hi = number(extent[1], p), count = (hi - lo) / step;
      if (!(count >= 1 && count <= 10000) || Math.abs(count - Math.round(count)) > 1e-9) fail(p, "bin extent must contain an integer number of positive steps (at most 10,000)");
      const a = `__vlc_${channel}_bin`, b = a + "_end";
      if (rows.some(r => own(r, a) || own(r, b))) fail(p, "reserved bin field collision");
      rows = rows.filter(r => value(r, def, p) != null).map(r => {
        const v = number(value(r, def, p), p, lo, hi);
        const quotient = (v - lo) / step, nearest = Math.round(quotient);
        // Snap only division roundoff at an exact bin boundary.
        const stable = Math.abs(quotient - nearest) <= 8 * Number.EPSILON * Math.max(1, Math.abs(quotient)) ? nearest : quotient;
        const i = Math.min(Math.round(count) - 1, Math.floor(stable));
        return {...r, [a]: lo + i * step, [b]: lo + (i + 1) * step};
      });
      enc[channel] = {...def, field: a, bin: undefined, type: "quantitative"};
      if (enc[channel + "2"]) fail(p, "binning and an explicit secondary channel cannot be combined");
      enc[channel + "2"] = {field: b};
    }
    const terms = Object.entries(enc).filter(([, d]) => d.aggregate);
    for(const [,d] of terms) choice(d.aggregate,["count","sum","mean","average","min","max","median","q1","q3","stdev","stderr"],p);
    if (terms.length) {
      for (const [c] of terms) if (rows.some(r => own(r, `__vlc_aggregate_${c}`))) fail(p, "reserved aggregate field collision");
      const by = unique(Object.values(enc).filter(d => !d.aggregate && d.field).map(d => d.field));
      rows = groups(rows, by).map(g => Object.fromEntries([
        ...by.map(f => [f, g[0][f]]),
        ...terms.map(([c, d]) => [`__vlc_aggregate_${c}`, statistic(g, d.aggregate, d.field, p)]),
      ]));
      for (const [c, d] of terms) enc[c] = {...d, field: `__vlc_aggregate_${c}`, aggregate: undefined};
    }
    return {...unit, rows, enc};
  }
  function mark(input, p) {
    const m = typeof input === "string" ? {type: input} : input;
    keys(m, ["type", "color", "fill", "stroke", "opacity", "fillOpacity", "strokeOpacity", "strokeWidth", "strokeDash", "size", "shape", "filled", "point", "interpolate", "orient", "extent", "center", "text", "align", "baseline", "dx", "dy", "angle", "fontSize", "clip", "thickness", "_styleField", "_symbolLegends"], p);
    choice(m.type, ["line", "point", "circle", "square", "bar", "area", "rect", "rule", "text", "tick", "errorbar", "errorband", "boxplot"], p);
    for (const k of ["opacity", "fillOpacity", "strokeOpacity"]) if (m[k] !== undefined) number(m[k], p, 0, 1);
    for (const k of ["strokeWidth", "size", "fontSize"]) if (m[k] !== undefined) number(m[k], p, 0, 10000);
    for (const k of ["dx", "dy", "angle"]) if (m[k] !== undefined) number(m[k], p, -10000, 10000);
    for (const k of ["filled", "point", "clip"]) if (m[k] !== undefined) boolean(m[k], p);
    if (m.clip === false) fail(p, "unclipped marks are not supported");
    if (m.interpolate !== undefined) choice(m.interpolate, ["linear", "step", "step-before", "step-after", "monotone"], p);
    if (m.orient !== undefined) choice(m.orient, ["horizontal", "vertical"], p);
    if (m.center !== undefined) choice(m.center, ["mean", "median"], p);
    if (m.align !== undefined) choice(m.align, ["left", "center", "right"], p);
    if (m.baseline !== undefined) choice(m.baseline, ["top", "middle", "bottom", "alphabetic"], p);
    if (m.strokeDash !== undefined) array(m.strokeDash, p).forEach(v => number(v, p, 0, 10000));
    const special = {
      extent: ["boxplot", "errorbar", "errorband"], center: ["errorbar", "errorband"],
      point: ["line"], interpolate: ["line", "area", "errorband"],
      shape: ["point", "circle", "square", "line"], size: ["point", "circle", "square", "bar", "line", "tick", "rule", "text"],
      text: ["text"], align: ["text"], baseline: ["text"], dx: ["text"], dy: ["text"], angle: ["text"], fontSize: ["text"],
    };
    for (const [key, marks] of Object.entries(special)) if (m[key] !== undefined && !marks.includes(m.type)) fail(p + "." + key, "not supported for this mark");
    if(m._styleField!==undefined && typeof m._styleField!=="string")fail(p,"style field must be a string");
    if(m.thickness!==undefined)number(m.thickness,p,0,10000);
    return {...m};
  }
  const VIEW_KEYS = ["$schema", "description", "name", "usermeta", "data", "datasets", "transform", "encoding", "mark", "layer", "width", "height", "title", "config", "resolve"];
  function units(spec, inherited, datasets, p, depth = 0) {
    if (depth > LIMIT.depth) fail(p, "view nesting limit exceeded");
    keys(spec, VIEW_KEYS, p);
    if (depth) for (const k of ["width", "height", "title", "config", "datasets"]) if (own(spec, k)) fail(p + "." + k, "layer-level setting is unsupported; move it to the enclosing view");
    const rows = transforms(data(spec.data, inherited.rows, datasets, `${p}.data`), spec.transform, `${p}.transform`);
    const enc = {...inherited.enc, ...(spec.encoding || {})};
    if (spec.layer) {
      if (spec.mark) fail(p, "a view cannot contain both mark and layer");
      return array(spec.layer, p).flatMap((s, i) => units(s, {rows, enc}, datasets, `${p}.layer[${i}]`, depth + 1));
    }
    return [encodeTransforms({rows, enc: encodings(enc, `${p}.encoding`), mark: mark(spec.mark, `${p}.mark`), p}, p)];
  }
  function coordType(def, p) {
    if (!def) return null;
    if (def.type) return def.type === "quantitative" ? (def.scale?.type === "log" ? "log" : "linear") : "category";
    if (own(def, "datum") && numeric(def.datum)) return "linear";
    fail(p, "x/y field encodings require quantitative, nominal, or ordinal type");
  }
  function axis(units, channel) {
    const defs = units.map(u => u.enc[channel]).filter(Boolean);
    const declared = defs.filter(d => d.field || d.type || d.scale);
    const def = declared[0] || defs[0] || {type: "quantitative"};
    const configured = defs.filter(d=>d.axis!==undefined);
    if(configured.some(d=>id(d.axis)!==id(configured[0].axis)))fail(channel,"layered views require compatible axis settings");
    const axisSetting=configured.length?configured[0].axis:def.axis;
    const kind = coordType(def, channel), scale = def.scale || {}, a = axisSetting || {};
    for (const other of declared.slice(1)) {
      if (coordType(other, channel) !== kind || id(other.scale || {}) !== id(scale)) fail(channel, "layered views require identical shared scale definitions");
    }
    if (kind === "category" && scale.type && !["band", "point", "ordinal"].includes(scale.type)) fail(channel, "categorical coordinates require a categorical scale");
    if (kind !== "category" && scale.type && !["linear", "log"].includes(scale.type)) fail(channel, "quantitative coordinates require a linear or log scale");
    const values = [];
    for (const u of units) for (const row of u.rows) for (const key of [channel, channel + "2"]) {
      if (!u.enc[key]) continue;
      const v = value(row, u.enc[key], u.p);
      if (v != null) values.push(v);
    }
    let categories = [];
    if (kind === "category") {
      if (values.some(v => !["string", "number", "boolean"].includes(typeof v))) fail(channel, "category values must be scalar");
      categories = unique(values);
      const sort = def.sort === undefined ? "ascending" : def.sort;
      if (Array.isArray(scale.domain)) categories = [...scale.domain];
      else if (Array.isArray(sort)) categories = [...sort, ...categories.filter(v => !sort.some(s => id(s) === id(v)))];
      else if (sort !== null) { choice(sort, ["ascending", "descending"], channel); categories.sort(compare); if (sort === "descending") categories.reverse(); }
      if (unique(categories).length !== categories.length) fail(channel, "duplicate category domain entries");
      for (const v of values) if (!categories.some(c => id(c) === id(v))) fail(channel, "data value is outside the categorical domain");
    }
    const titles = unique(defs.map(d=>d.title).filter(v=>v!=null));
    return {kind, categories, sort: def.sort, scale, axis: axisSetting === null ? null : a, title: own(a, "title") ? a.title : titles.length === 1 ? titles[0] : titles.join(", "), values: [], includeZero: false};
  }
  function xy(row, def, axis, p) {
    const v = value(row, def, p);
    if (v == null) return null;
    if (axis.kind === "category") {
      const index = axis.categories.findIndex(c => id(c) === id(v));
      if (index < 0) fail(p, "unknown category");
      return index;
    }
    const n = number(v, p);
    if (axis.kind === "log" && n <= 0) fail(p, "logarithmic coordinates must be positive");
    return n;
  }
  function style(m, override) {
    const base = override || m.color || PALETTE[0];
    return {
      stroke: m.stroke === null || m.stroke === "none" || m.strokeWidth === 0 ? null : color(m.stroke || base, "stroke"), fill: m.fill === null || m.fill === "none" ? null : color(m.fill || base, "fill"),
      opacity: m.opacity ?? 1, fillOpacity: m.fillOpacity ?? 1, strokeOpacity: m.strokeOpacity ?? 1,
      width: m.strokeWidth ?? (m.type === "line" ? 2 : 1), dash: m.strokeDash || [],
      size: m.size ?? 30, shape: m.shape || (m.type === "square" ? "square" : "circle"),
      filled: m.filled ?? ["circle", "square"].includes(m.type),
    };
  }
  function series(unit) {
    const {enc, rows, mark: m, p} = unit;
    if (m.color !== undefined) color(m.color,p);
    for(const k of ["fill","stroke"])if(m[k] !== undefined && m[k] !== null && m[k] !== "none")color(m[k],p);
    if (enc.opacity && !own(enc.opacity, "value")) fail(p, "only constant opacity encodings are supported");
    if (enc.opacity) m.opacity = number(enc.opacity.value, p, 0, 1);
    const c = enc.color;
    if (c && !c.field && !own(c, "value") && !own(c, "datum")) fail(p, "invalid color encoding");
    if (c?.field && c.type === "quantitative") {
      if (!["point","circle","square","rect"].includes(m.type)) fail(p, "quantitative color is supported on point and rect marks");
      if (enc.detail) fail(p, "detail with quantitative color is unsupported");
      if (c.scale?.type && c.scale.type !== "linear") fail(p, "quantitative color currently requires a linear scale");
      if (c.sort) fail(p, "quantitative color cannot be sorted categorically");
      if (c.scale && Object.keys(c.scale).some(k => !["type","domain","range","scheme"].includes(k))) fail(p, "unsupported quantitative color-scale option");
      return rows.length ? [{rows, style:style(m), legend:null, meta:c.field}] : [];
    }
    if (c?.field && !["nominal", "ordinal"].includes(c.type)) fail(p, "color fields require quantitative, nominal, or ordinal type");
    if (enc.detail && !enc.detail.field) fail(p, "detail requires a field");
    const by = [c?.field, enc.detail?.field, m._styleField].filter(Boolean);
    let domain = c?.field ? unique(rows.map(r => field(r, c.field, p))).sort(compare) : [];
    if (c?.sort !== undefined) {
      if (Array.isArray(c.sort)) domain = [...c.sort, ...domain.filter(v => !c.sort.some(a => id(a) === id(v)))];
      else if (c.sort === null) domain = unique(rows.map(r => field(r, c.field, p)));
      else { choice(c.sort, ["ascending", "descending"], p); if (c.sort === "descending") domain.reverse(); }
    }
    if (c?.scale?.domain) domain = array(c.scale.domain, p);
    if (unique(domain).length !== domain.length) fail(p, "duplicate color-domain entries");
    if (c?.scale?.type && c.scale.type !== "ordinal") fail(p, "categorical color requires an ordinal scale");
    const colors = c?.scale?.range || PALETTE;
    if (!array(colors, p).length) fail(p, "empty color range");
    if (c?.scale && Object.keys(c.scale).some(k => !["domain", "range", "type"].includes(k))) fail(p, "unsupported categorical color-scale option");
    return groups(rows, by).sort((a,b) => c?.field ? domain.findIndex(v => id(v) === id(a[0][c.field])) - domain.findIndex(v => id(v) === id(b[0][c.field])) : 0).map(g => {
      const cv = c?.field ? field(g[0], c.field, p) : undefined;
      const index = c?.field ? domain.findIndex(v => id(v) === id(cv)) : 0;
      if (index < 0) fail(p, "unknown color-domain value");
      const paint = c?.field ? colors[index % colors.length] : c ? value(g[0], c, p) : undefined;
      let resolved=style(m,paint);
      if(m._styleField) {
        const v=field(g[0],m._styleField,p);
        object(v,p);
        for(const key of ["size","strokeWidth","opacity","fillOpacity","strokeOpacity"])if(v[key]!=null)number(v[key],p,0);
        resolved=style({...m,...v,filled:v.fill!=null && v.fill!=="none"},paint);
        if(v.fill==="none" || v.fill===null)resolved.fill=null;
        if(v.stroke==="none" || v.stroke==null)resolved.stroke=null;
      }
      return {rows:g,colorValue:cv,style:resolved,legend:c?.field && c.legend!==null?String(cv):null};
    });
  }
  function numericRange(axis, p) {
    if (axis.kind === "category") { axis.domain = [-.5, Math.max(.5, axis.categories.length - .5)]; return; }
    const given = axis.scale.domain;
    let domain;
    if (given !== undefined) {
      if (array(given, p).length !== 2) fail(p, "numeric domain needs two bounds");
      domain = given.map(v => number(v, p));
      if (!(domain[0] < domain[1])) fail(p, "domain must be increasing; use scale.reverse for reversal");
    } else {
      const values = axis.values.filter(v => v != null);
      let lo = values.length ? values.reduce((a, b) => Math.min(a, b)) : (axis.kind === "log" ? 1 : 0);
      let hi = values.length ? values.reduce((a, b) => Math.max(a, b)) : (axis.kind === "log" ? 10 : 1);
      if (axis.kind === "linear" && ((axis.scale.zero ?? true) || axis.includeZero)) { lo = Math.min(lo, 0); hi = Math.max(hi, 0); }
      if (lo === hi) { const d = Math.abs(lo) * .05 || .5; lo -= d; hi += d; }
      if (axis.scale.nice) {
        if (axis.kind === "log") { const b = axis.scale.base || 10; lo = b ** Math.floor(Math.log(lo) / Math.log(b)); hi = b ** Math.ceil(Math.log(hi) / Math.log(b)); }
        else { const step = 10 ** Math.floor(Math.log10((hi - lo) / 5)); lo = Math.floor(lo / step) * step; hi = Math.ceil(hi / step) * step; }
      }
      domain = [lo, hi];
    }
    if (axis.kind === "log" && domain[0] <= 0) fail(p, "logarithmic domains must be positive");
    axis.domain = domain;
  }
  function panel(spec, datasets, p) {
    if(spec.resolve) {
      keys(spec.resolve,["scale"],p+".resolve");
      keys(spec.resolve.scale || {},["x","y","color"],p+".resolve.scale");
      if(spec.resolve.scale?.y==="independent") {
        if(!spec.layer || spec.layer.length!==2 || spec.resolve.scale?.x==="independent")fail(p,"dual axes require two layers and one shared x scale");
        const base={...spec};delete base.resolve;delete base.layer;
        const children=spec.layer.map((layer,i)=>panel({...base,...layer,encoding:{...spec.encoding,...layer.encoding}},datasets,p+`.layer[${i}]`));
        shareAxes(children,["x"]);
        children[0].secondary=children[1];children[1].title=null;
        return children[0];
      }
      if(Object.values(spec.resolve.scale || {}).some(v=>v!=="shared"))fail(p,"only independent y is supported within a layered panel");
      spec={...spec};delete spec.resolve;
    }
    return singlePanel(spec,datasets,p);
  }
  function singlePanel(spec, datasets, p) {
    const cfg = spec.config === undefined ? {} : spec.config;
    keys(cfg, ["view"], `${p}.config`);
    if (cfg.view) keys(cfg.view, ["continuousWidth", "continuousHeight", "stroke", "strokeWidth"], `${p}.config.view`);
    if(cfg.view?.stroke != null)color(cfg.view.stroke,p);
    const unitsList = units(spec, {}, datasets, p);
    if (!unitsList.length || unitsList.length > 64) fail(p, "a panel requires 1 to 64 layers");
    if (unitsList.reduce((n, u) => n + u.rows.length, 0) > 1000000) fail(p, "total layer row limit exceeded");
    const colored = unitsList.map(u => u.enc.color).filter(c => c?.field);
    for (const c of colored.slice(1)) if (id(c) !== id(colored[0])) fail(p, "layered views require one shared categorical color definition");
    const legendTitle = colored.length && colored[0].type !== "quantitative" && colored[0].legend !== null ? (colored[0].legend?.title ?? colored[0].title) : null;
    let colorAxis = null;
    if (colored[0]?.type === "quantitative") {
      const c = colored[0], values = unitsList.flatMap(u => u.enc.color?.field ? u.rows.map(r => value(r, u.enc.color, p)).filter(v => v != null).map(v => number(v,p)) : []);
      let domain = c.scale?.domain || [values.length ? values.reduce((a,b)=>Math.min(a,b)) : 0, values.length ? values.reduce((a,b)=>Math.max(a,b)) : 1];
      if (array(domain,p).length !== 2) fail(p,"color domain requires two bounds");
      domain = domain.map(v => number(v,p));
      if (domain[0] === domain[1] && !c.scale?.domain) domain = [domain[0]-.5,domain[1]+.5];
      if (domain[0] >= domain[1]) fail(p,"color domain must be increasing");
      let range = null;
      if (c.scale?.range) { range=array(c.scale.range,p).map(v=>color(v,p)); if (range.length<2 || range.length>256) fail(p,"color range requires 2 to 256 colors"); }
      if (c.scale?.scheme) { choice(c.scale.scheme,["viridis"],p); if (range) fail(p,"choose either color range or scheme"); }
      colorAxis = {domain,range,title:c.legend?.title ?? c.title,show:c.legend !== null};
    }
    const axes = {x: axis(unitsList, "x"), y: axis(unitsList, "y")};
    const plots = [];
    const add = plot => {
      plots.push(plot);
      for (const pt of [...(plot.points || []), ...(plot.lower || []), ...(plot.upper || [])]) {
        if (pt) { axes.x.values.push(pt[0]); axes.y.values.push(pt[1]); }
      }
    };
    for (const unit of unitsList) {
      const {mark: m, enc: e, p: at} = unit;
      for (const c of ["x", "y"]) if (e[c]?.sort !== undefined && axes[c].kind !== "category") fail(at + "." + c, "sort is only supported on categorical axes; use order for line ordering");
      for (const c of ["xError", "xError2", "yError", "yError2"]) if (e[c] && !["errorbar","errorband"].includes(m.type)) fail(at + "." + c, "error channels require an uncertainty mark");
      for(const c of ["x","y"]){
        if(e[c+"Error2"] && !e[c+"Error"])fail(at,"a secondary error requires a primary error");
        if(e[c+"2"] && e[c+"Error"])fail(at,"use bounds or error offsets, not both");
      }
      if ((e.xOffset || e.yOffset) && m.type !== "bar") fail(at,"offset channels are supported only on bar marks");
      if (e.text && m.type !== "text") fail(at, "text encoding requires a text mark");
      if (["line","point","circle","square","boxplot","text"].includes(m.type) && (e.x2 || e.y2)) fail(at, "secondary coordinates are unsupported for this mark");
      const allSeries = series(unit);
      const horizontal = m.orient === "horizontal" || (!m.orient && e.x?.type === "quantitative" && e.y?.type !== "quantitative" && ["bar", "boxplot", "errorbar", "errorband"].includes(m.type));
      const measure = horizontal ? "x" : "y", category = horizontal ? "y" : "x";
      if(["bar","area","boxplot","errorbar","errorband"].includes(m.type) && axes[measure].kind === "category")fail(at,"the measure axis must be quantitative");
      const pair = (a, b) => horizontal ? [b, a] : [a, b];
      const offset = e[category+"Offset"];
      if(e[measure+"Offset"])fail(at,"bar offsets belong on the categorical axis");
      if(offset && (axes[category].kind !== "category" || !offset.field || offset.field !== e.color?.field || !["nominal","ordinal"].includes(offset.type)))fail(at,"bar offsets require a categorical axis and the same nominal/ordinal field as color");
      if(offset && e[measure]?.stack != null)fail(at,"offset bars cannot also be explicitly stacked");
      if (["errorbar","errorband"].includes(m.type) && (e[category+"2"] || e[category+"Error"] || e[category+"Error2"])) fail(at, "uncertainty may only be encoded on its measure axis");
      if (m.type === "area" && e[category+"2"]) fail(at, "area secondary coordinates belong on the measure axis");
      if (["line", "point", "circle", "square", "bar", "area", "rect", "boxplot", "errorbar", "errorband", "text", "tick"].includes(m.type) && (!e.x || !e.y)) fail(at, `${m.type} requires both x and y coordinates`);
      if (e.x?.stack && measure !== "x" || e.y?.stack && measure !== "y") fail(at, "stack is only supported on the measure channel");
      const requestedStack = e[measure]?.stack;
      if (requestedStack != null && !["bar", "area"].includes(m.type)) fail(at, "stacking requires a bar or area mark");
      if (requestedStack != null && axes[measure].kind !== "linear") fail(at, "stacking requires a linear measure axis");
      const stack = !offset && axes[measure].kind === "linear" && ["bar", "area"].includes(m.type)
        ? (requestedStack === undefined ? (allSeries.length > 1 && e.color?.field ? "zero" : null) : requestedStack) : null;
      if(stack && e.order)fail(at,"custom stack order is not supported; set the color domain order");
      if (stack && m.type === "area") {
        const support = s => id(unique(s.rows.map(r => xy(r, e[category], axes[category], at))).sort((a,b)=>a-b));
        if (allSeries.some(s => support(s) !== support(allSeries[0]))) fail(at, "stacked areas require a shared sampling grid; impute missing samples first");
      }
      const accum = new Map();
      const totals = new Map();
      if (stack === "normalize") for (const s of allSeries) for (const r of s.rows) {
        const cat = xy(r, e[category], axes[category], at), val = xy(r, e[measure], axes[measure], at);
        if (cat == null || val == null) continue;
        if (val < 0) fail(at, "normalized stacks with negative values are unsupported");
        totals.set(cat, (totals.get(cat) || 0) + val);
      }
      for (const [seriesIndex, s] of allSeries.entries()) {
        const rows = [...s.rows];
        const sorted = () => rows.sort((a, b) => {
          if (e.order) {
            if (!e.order.field) fail(at, "order requires a field");
            choice(e.order.sort || "ascending", ["ascending", "descending"], at);
            return compare(field(a, e.order.field, at), field(b, e.order.field, at)) * (e.order.sort === "descending" ? -1 : 1);
          }
          return (xy(a, e[category], axes[category], at) ?? Infinity) - (xy(b, e[category], axes[category], at) ?? Infinity);
        });
        const base = {style: s.style, legend: s.legend, colorValue: s.colorValue, mark: m, horizontal};
        const coordinates = row => [xy(row, e.x, axes.x, at), xy(row, e.y, axes.y, at)];
        if (["line", "point", "circle", "square"].includes(m.type)) {
          if (m.type === "line" || e.order) sorted();
          const selected = s.meta ? rows.filter(r => value(r,e.color,at) != null) : rows;
          const points = selected.map(coordinates).map(pt => pt.some(v => v == null) ? null : pt);
          const meta = s.meta ? selected.map(r=>number(field(r,s.meta,at),at)) : null;
          add({...base, type: m.type === "line" ? "line" : "point", points, meta});
        } else if (m.type === "bar" || m.type === "area") {
          sorted();
          const lower = [], upper = [], bars = [], seen = new Set();
          for (const row of rows) {
            const c = xy(row, e[category], axes[category], at), v = xy(row, e[measure], axes[measure], at);
            if (c == null || v == null) { lower.push(null); upper.push(null); continue; }
            if (seen.has(c)) fail(at, "duplicate positions in a bar/area series; aggregate explicitly first");
            seen.add(c);
            let low = e[measure + "2"] ? xy(row, e[measure + "2"], axes[measure], at) : 0, high = v;
            if (low == null) fail(at, "null range boundary");
            if (stack) {
              if (e[measure + "2"]) fail(at, "stacking cannot be combined with a ranged measure");
              const k = `${c}:${v < 0 ? "negative" : "positive"}`;
              low = accum.get(k) || 0;
              high = low + (stack === "normalize" ? (totals.get(c) ? v / totals.get(c) : 0) : v);
              accum.set(k, high);
            }
            if (axes[measure].kind === "log" && (low <= 0 || high <= 0)) fail(at, "logarithmic bars/areas require explicit positive range boundaries");
            lower.push(pair(c, low)); upper.push(pair(c, high));
            bars.push({c, low, high, c2: e[category + "2"] ? xy(row, e[category + "2"], axes[category], at) : null});
          }
          if (m.type === "area") add({...base, type: "band", lower, upper});
          else {
            axes[category].values.push(...bars.map(b => b.c2).filter(v => v != null));
            add({...base, type: "bar", lower, upper, bars, seriesIndex, seriesCount: offset ? allSeries.length : 1});
          }
        } else if (m.type === "errorbar" || m.type === "errorband") {
          const lower = [], upper = [], points = [], errors = [];
          const prepared = e[measure + "2"] || e[measure + "Error"];
          const sets = prepared ? rows.map(r => [r]) : groups(rows, e[category].field ? [e[category].field] : []);
          sets.sort((a, b) => xy(a[0], e[category], axes[category], at) - xy(b[0], e[category], axes[category], at));
          for (const g of sets) {
            const r = g[0], c = xy(r, e[category], axes[category], at);
            let center, lo, hi;
            if (e[measure + "2"]) {
              lo = xy(r, e[measure], axes[measure], at); hi = xy(r, e[measure + "2"], axes[measure], at); center = (lo + hi) / 2;
            } else if (e[measure + "Error"]) {
              center = xy(r, e[measure], axes[measure], at);
              const plus = number(value(r, e[measure + "Error"], at), at, 0);
              const minus = e[measure + "Error2"] ? number(value(r, e[measure + "Error2"], at), at, -Infinity, 0) : -plus;
              lo = center + minus; hi = center + plus;
            } else {
              const extent = m.extent || (m.type === "errorbar" ? "stderr" : "ci");
              choice(extent, ["stdev", "stderr", "iqr"], at + ".extent (precompute confidence intervals to avoid changing their statistical definition)");
              if (!e[measure].field) fail(at, "raw uncertainty requires a measure field");
              center = statistic(g, m.center || (extent === "iqr" ? "median" : "mean"), e[measure].field, at);
              if (extent === "iqr") { lo = statistic(g, "q1", e[measure].field, at); hi = statistic(g, "q3", e[measure].field, at); }
              else { const error = statistic(g, extent, e[measure].field, at); lo = center - error; hi = center + error; }
            }
            if (c == null || lo == null || hi == null || center == null) fail(at, "null uncertainty coordinate");
            if (lo > hi || center < lo || center > hi) fail(at, "uncertainty bounds must contain the center in increasing order");
            if (axes[measure].kind === "log" && lo <= 0) fail(at, "logarithmic uncertainty bounds must be positive");
            lower.push(pair(c, lo)); upper.push(pair(c, hi)); points.push(pair(c, center)); errors.push([center - lo, hi - center]);
          }
          add({...base, type: m.type === "errorband" ? "band" : "errorbar", lower, upper, points, errors});
        } else if (m.type === "boxplot") {
          if (axes[category].kind !== "category" || !e[measure].field) fail(at, "boxplots require a categorical grouping axis and numeric sample field");
          for (const g of groups(rows, [e[category].field])) {
            const v = g.map(r => field(r, e[measure].field, at)).filter(v => v != null).map(v => number(v, at)).sort((a, b) => a - b);
            if (!v.length) continue;
            const q1 = quantile(v, .25), median = quantile(v, .5), q3 = quantile(v, .75);
            const extent = m.extent ?? 1.5;
            if (extent !== "min-max") number(extent, at, 0);
            const lowFence = extent === "min-max" ? v[0] : q1 - extent * (q3 - q1);
            const highFence = extent === "min-max" ? v[v.length - 1] : q3 + extent * (q3 - q1);
            const inside = v.filter(x => x >= lowFence && x <= highFence);
            const low = inside[0] ?? q1, high = inside[inside.length - 1] ?? q3;
            const c = xy(g[0], e[category], axes[category], at);
            if (axes[measure].kind === "log" && v[0] <= 0) fail(at, "logarithmic boxplot samples must be positive");
            add({...base, type: "boxplot", c, q1, median, q3, low, high, outliers: v.filter(x => x < low || x > high), boxWidth: .6, points: v.map(x => pair(c, x))});
          }
        } else if (m.type === "tick") {
          add({...base,type:"tick",points:rows.map(coordinates).filter(pt=>pt.every(v=>v!=null))});
        } else if (m.type === "rule") {
          for (const r of rows) {
            const a = coordinates(r), b = [e.x2 ? xy(r, e.x2, axes.x, at) : a[0], e.y2 ? xy(r, e.y2, axes.y, at) : a[1]];
            if (a.every(v => v == null)) fail(at, "rule needs at least one coordinate");
            add({...base, type: "rule", points: [a, b]});
          }
        } else if (m.type === "text") {
          for (const r of rows) {
            const pt = coordinates(r);
            if (pt.some(v => v == null)) continue;
            const visual=m._styleField ? field(r,m._styleField,at) : null;
            add({...base, type: "text", points: [pt], text: e.text ? value(r, e.text, at) : m.text ?? "",fontSize:visual?.fontSize ?? m.fontSize});
          }
        } else if (m.type === "rect") {
          for (const r of rows) {
            const [x, y] = coordinates(r);
            const x2 = e.x2 ? xy(r, e.x2, axes.x, at) : axes.x.kind === "category" ? x + .5 : null;
            const y2 = e.y2 ? xy(r, e.y2, axes.y, at) : axes.y.kind === "category" ? y + .5 : null;
            const x1 = !e.x2 && axes.x.kind === "category" ? x - .5 : x;
            const y1 = !e.y2 && axes.y.kind === "category" ? y - .5 : y;
            if ([x1, y1, x2, y2].some(v => v == null)) fail(at, "rect requires secondary bounds on quantitative axes");
            const meta = s.meta ? value(r,e.color,at) : null;
            if (s.meta && meta == null) continue;
            if (meta != null) number(meta,at);
            add({...base, type: "rect", points: [[x1, y1], [x2, y1], [x2, y2], [x1, y2], [x1, y1]], meta});
          }
        }
      }
    }
    numericRange(axes.x, p + ".x"); numericRange(axes.y, p + ".y");
    return {axes, plots, border:cfg.view?.stroke, borderWidth:cfg.view?.strokeWidth || 1, symbolLegends: unitsList.flatMap(u=>u.mark._symbolLegends || []), legendTitle, colorAxis, colorDef: colored[0] || null, title: spec.title, width: number(spec.width ?? cfg.view?.continuousWidth ?? 300, p + ".width", 1, 10000), height: number(spec.height ?? cfg.view?.continuousHeight ?? 200, p + ".height", 1, 10000)};
  }
  function shareAxes(panels, channels) {
    for (const channel of channels) {
      const first = panels[0].axes[channel], dim = channel === "x" ? 0 : 1;
      if (panels.some(p => p.axes[channel].kind !== first.kind || id(p.axes[channel].scale) !== id(first.scale))) fail(channel,"shared scales require matching types and options");
      if (first.kind !== "category") {
        const domain = [Math.min(...panels.map(p=>p.axes[channel].domain[0])), Math.max(...panels.map(p=>p.axes[channel].domain[1]))];
        for (const p of panels) p.axes[channel].domain = [...domain];
        continue;
      }
      let domain = unique(panels.flatMap(p=>p.axes[channel].categories));
      if (first.scale.domain) domain = [...first.scale.domain];
      else if (Array.isArray(first.sort)) domain = [...first.sort,...domain.filter(v=>!first.sort.some(x=>id(x)===id(v)))];
      else if (first.sort !== null) { domain.sort(compare); if (first.sort === "descending") domain.reverse(); }
      for (const p of panels) {
        const a=p.axes[channel], old=a.categories;
        const map=v=>v==null ? v : domain.findIndex(x=>id(x)===id(old[v]));
        for (const plot of p.plots) {
          const isCategory = channel === (plot.horizontal ? "y" : "x");
          if (plot.type === "rect") {
            const center = (Math.min(...plot.points.map(pt=>pt[dim]))+Math.max(...plot.points.map(pt=>pt[dim])))/2;
            const shift=map(Math.round(center))-Math.round(center);
            for(const pt of plot.points) pt[dim]+=shift;
          } else if(plot.type === "boxplot" && isCategory) {
            const shift=map(Math.round(plot.c))-Math.round(plot.c);
            plot.c+=shift; for(const pt of plot.points) pt[dim]+=shift;
          } else {
            for(const property of ["points","lower","upper"]) for(const pt of plot[property] || []) if(pt) pt[dim]=map(pt[dim]);
          }
          if(plot.type === "bar" && isCategory) for(const bar of plot.bars) { bar.c=map(bar.c); if(bar.c2!=null) bar.c2=map(bar.c2); }
        }
        a.categories=[...domain];a.domain=[-.5,Math.max(.5,domain.length-.5)];
      }
    }
  }
  function shareColors(panels) {
    const def=panels.find(p=>p.colorDef)?.colorDef;
    if(!def) return;
    if(panels.some(p=>p.colorDef && id(p.colorDef)!==id(def))) fail("color","shared color definitions must match");
    if(def.type === "quantitative") {
      const axes=panels.map(p=>p.colorAxis).filter(Boolean);
      const domain=[Math.min(...axes.map(c=>c.domain[0])),Math.max(...axes.map(c=>c.domain[1]))];
      for(const c of axes)c.domain=[...domain];
    } else {
      let domain=unique(panels.flatMap(p=>p.plots.filter(t=>t.colorValue!==undefined).map(t=>t.colorValue))).sort(compare);
      if(def.scale?.domain)domain=def.scale.domain;
      else if(Array.isArray(def.sort))domain=[...def.sort,...domain.filter(v=>!def.sort.some(x=>id(x)===id(v)))];
      else if(def.sort==="descending")domain.reverse();
      const range=def.scale?.range || PALETTE;
      for(const p of panels)for(const plot of p.plots)if(plot.colorValue!==undefined){
        const rgb=color(range[domain.findIndex(v=>id(v)===id(plot.colorValue))%range.length],"color");
        if(plot.mark.fill===undefined)plot.style.fill=rgb;
        if(plot.mark.stroke===undefined && plot.mark.strokeWidth!==0)plot.style.stroke=rgb;
      }
    }
  }
  function lower(spec, opts = {}) {
    object(spec, "$");
    keys(opts, ["standalone", "tex_labels"], "options");
    for (const k of Object.keys(opts)) boolean(opts[k], `options.${k}`);
    if (spec.$schema && !/^https:\/\/vega\.github\.io\/schema\/vega-lite\/v[56](?:\.\d+){0,2}\.json$/.test(spec.$schema)) fail("$.$schema", "expected a Vega-Lite v5/v6 schema");
    const datasets = spec.datasets || {};
    object(datasets, "datasets");
    const composition = ["hconcat", "vconcat", "concat", "facet", "repeat"].filter(k => own(spec, k));
    if (!composition.length) return {panels: [panel(spec, datasets, "$")], columns: 1};
    if (composition.length !== 1) fail("$", "use one composition operator");
    const op = composition[0];
    keys(spec, ["$schema", "description", "name", "usermeta", "title", "data", "datasets", "transform", op, "spec", "columns", "config", "resolve"], "$");
    const rows = transforms(data(spec.data, undefined, datasets, "$.data"), spec.transform, "$.transform");
    const make = (s,values=rows) => panel({config: spec.config,data:{values},...s},datasets,op);
    let panels,columns;
    if(op === "facet") {
      const f=object(spec.facet,"facet");
      const one=own(f,"field");
      if(!one)keys(f,["row","column"],"facet");
      const row=one?null:f.row,col=one?f:f.column;
      if(!row && !col)fail("facet","requires a field, row, or column");
      const vals=d=>{
        if(!d)return [null];
        keys(d,["field","type","sort","title"],"facet");choice(d.type || "nominal",["nominal","ordinal"],"facet.type");
        let v=unique(rows.map(r=>field(r,d.field,"facet")));
        if(Array.isArray(d.sort))v=[...d.sort,...v.filter(x=>!d.sort.some(y=>id(x)===id(y)))];
        else if(d.sort!==null){choice(d.sort||"ascending",["ascending","descending"],"facet.sort");v.sort(compare);if(d.sort==="descending")v.reverse();}
        return v.length?v:[null];
      };
      const rv=vals(row),cv=vals(col);
      if(rv.length*cv.length>LIMIT.panels)fail("facet","panel limit exceeded");
      panels=[];
      for(const r of rv)for(const c of cv){
        const subset=rows.filter(x=>(!row||id(field(x,row.field,"facet"))===id(r))&&(!col||id(field(x,col.field,"facet"))===id(c)));
        const header=[row?`${row.title??row.field}: ${r}`:null,col?`${col.title??col.field}: ${c}`:null].filter(Boolean).join(" / ");
        const child={...object(spec.spec,"spec")};
        if(child.data)fail("facet.spec.data","facet children inherit their partitioned data");
        child.title=child.title?`${child.title} / ${header}`:header;
        panels.push(make(child,subset));
      }
      columns=one?(spec.columns??cv.length):cv.length;
    } else if(op === "repeat") {
      const r=spec.repeat;
      let rv,cv,one=Array.isArray(r);
      if(one){rv=[null];cv=r;}else{keys(r,["row","column"],"repeat");rv=r.row||[null];cv=r.column||[null];}
      array(rv,"repeat.row");array(cv,"repeat.column");
      if(!rv.length||!cv.length||rv.length*cv.length>LIMIT.panels)fail("repeat","invalid panel count");
      const replace=(v,bindings,depth=0)=>{
        if(depth>LIMIT.depth)fail("repeat","template nesting limit exceeded");
        if(Array.isArray(v))return v.map(x=>replace(x,bindings,depth+1));
        if(v && typeof v==="object")return Object.fromEntries(Object.entries(v).map(([k,x])=>{
          if(k==="field" && x && typeof x==="object"){
            keys(x,["repeat"],"repeat field");
            if(typeof bindings[x.repeat]!=="string")fail("repeat field","unbound repeated field");
            return [k,bindings[x.repeat]];
          }
          return [k,["data","datasets","usermeta"].includes(k)?x:replace(x,bindings,depth+1)];
        }));
        return v;
      };
      panels=[];
      for(const row of rv)for(const column of cv){
        if(row!==null && typeof row!=="string" || column!==null && typeof column!=="string")fail("repeat","field names must be strings");
        panels.push(make(replace(object(spec.spec,"spec"),{row,column,repeat:column})));
      }
      columns=one?(spec.columns??cv.length):cv.length;
    } else {
      if(spec.spec)fail("spec","only facet and repeat use a child spec");
      const children = array(spec[op], op);
      if (!children.length || children.length > LIMIT.panels) fail(op, "panel count must be between 1 and 64");
      panels=children.map(s=>make(s));
      columns=op === "vconcat" ? 1 : op === "hconcat" ? panels.length : (spec.columns ?? 2);
    }
    if (!Number.isInteger(columns) || columns < 1 || columns > LIMIT.panels) fail("columns", "expected an integer from 1 to 64");
    const resolve=spec.resolve || {};
    keys(resolve,["scale"],"resolve");keys(resolve.scale||{},["x","y","color"],"resolve.scale");
    const shared=[];
    for(const c of ["x","y","color"]){
      const mode=resolve.scale?.[c] || (op === "facet" || c === "color" ? "shared" : "independent");
      choice(mode,["shared","independent"],"resolve.scale");
      if(mode==="shared"){if(c==="color")shareColors(panels);else shared.push(c);}
    }
    shareAxes(panels,shared);
    return {panels,columns,title:spec.title};
  }
  function axisOptions(a, c, opts) {
    const parts = [`${c}min=${fmt(a.domain[0])}`, `${c}max=${fmt(a.domain[1])}`];
    if (a.kind === "log") parts.push(`${c}mode=log`, `log basis ${c}=${fmt(a.scale.base || 10)}`);
    if (a.scale.reverse) parts.push(`${c} dir=reverse`);
    if (a.axis === null) return [...parts, `axis ${c} line=none`, `${c}tick=\\empty`];
    if(a.axis.domain===false)parts.push(`${c} axis line style={draw=none}`);
    if(a.axis.ticks===false)parts.push(`${c}tick style={draw=none}`);
    if(a.axis.tickSize!=null)parts.push(`${c}tick style={}`,`major tick length=${bp(number(a.axis.tickSize,c,0))}`);
    if(a.axis.titleColor)parts.push(`${c}label style={text=${texColor(color(a.axis.titleColor,c))}}`);
    if(a.axis.labelColor)parts.push(`${c}ticklabel style={text=${texColor(color(a.axis.labelColor,c))}}`);
    if(a.axis.gridColor)parts.push(`${c} grid style={draw=${texColor(color(a.axis.gridColor,c))}}`);
    if(a.axis.domainColor)parts.push(`${c} axis line style={draw=${texColor(color(a.axis.domainColor,c))}}`);
    if(a.axis.tickColor)parts.push(`${c}tick style={draw=${texColor(color(a.axis.tickColor,c))}}`);
    if(a.axis.labelFontSize)parts.push(`${c}ticklabel style={font=\\fontsize{${bp(number(a.axis.labelFontSize,c,1))}}{${bp(a.axis.labelFontSize*1.2)}}\\selectfont}`);
    if(a.axis.titleFontSize)parts.push(`${c}label style={font=\\fontsize{${bp(number(a.axis.titleFontSize,c,1))}}{${bp(a.axis.titleFontSize*1.2)}}\\selectfont}`);
    if (a.title != null) parts.push(`${c}label={${label(a.title, opts.tex_labels)}}`);
    if (a.axis.grid) parts.push(`${c}majorgrids=true`);
    if (a.axis.labels === false) parts.push(`${c}ticklabels={}`);
    if (a.axis.labelAngle) parts.push(`${c}ticklabel style={rotate=${fmt(-a.axis.labelAngle)},anchor=east}`);
    if (a.kind === "category") {
      parts.push(`${c}tick={${a.categories.map((_, i) => i).join(",")}}`);
      if (a.axis.labels !== false) parts.push(`${c}ticklabels={${a.categories.map(v => `{${label(v, opts.tex_labels)}}`).join(",")}}`);
    }
    if (a.axis.values) {
      const ticks = array(a.axis.values, `${c}.axis.values`).map(v => a.kind === "category" ? a.categories.findIndex(x => id(x) === id(v)) : number(v, `${c}.axis.values`));
      if (a.kind === "category" && ticks.some(v => v < 0)) fail(c, "unknown category tick");
      parts.push(`${c}tick={${ticks.map(fmt).join(",")}}`);
      if (a.kind === "category" && a.axis.labels !== false) parts.push(`${c}ticklabels={${a.axis.values.map(v => `{${label(v, opts.tex_labels)}}`).join(",")}}`);
    }
    if (a.axis.tickLabels && a.axis.labels !== false) {
      if (!a.axis.values || a.axis.tickLabels.length !== a.axis.values.length) fail(c, "tick labels must match tick values");
      parts.push(`${c}ticklabels={${a.axis.tickLabels.map(v => `{${label(v, opts.tex_labels)}}`).join(",")}}`);
      // Custom/date labels already carry units; suppress numeric multipliers.
      parts.push(`scaled ${c} ticks=false`);
    }
    if (a.axis.format) {
      const match = /^\.(\d{1,2})([fe])$/.exec(a.axis.format);
      if (!match || +match[1] > 15) fail(c, "axis.format supports .0f through .15f and .0e through .15e");
      parts.push(`${c}ticklabel style={/pgf/number format/${match[2] === "f" ? "fixed,fixed zerofill" : "sci"},/pgf/number format/precision=${match[1]}}`);
    }
    return parts;
  }
  function plotStyle(s, filled = false) {
    const out = ["solid", `draw=${texColor(s.stroke)}`, `line width=${bp(s.width)}`, `draw opacity=${fmt(s.opacity * s.strokeOpacity)}`];
    if (filled) out.push(`fill=${texColor(s.fill)}`, `fill opacity=${fmt(s.opacity * s.fillOpacity)}`);
    if (s.dash.length) {
      const d = s.dash.length % 2 ? [...s.dash, ...s.dash] : s.dash;
      if (!d.some(v => v > 0)) fail("strokeDash", "at least one dash length must be positive");
      out.push(`dash pattern=${d.map((v, i) => `${i % 2 ? "off" : "on"} ${bp(v)}`).join(" ")}`);
    }
    return out;
  }
  function pointStyle(s) {
    const mapping = {circle:s.filled?"*":"o",square:s.filled?"square*":"square",diamond:s.filled?"diamond*":"diamond","triangle-up":s.filled?"triangle*":"triangle","triangle-down":s.filled?"triangle*":"triangle","triangle-right":s.filled?"triangle*":"triangle","triangle-left":s.filled?"triangle*":"triangle",cross:"+",stroke:"-"};
    if (!own(mapping, s.shape)) fail("shape", "supported point shapes: circle, square, diamond, triangle-up, cross");
    return [`mark=${mapping[s.shape]}`, `mark size=${bp(Math.sqrt(s.size)/2)}`, `mark options={rotate=${({"triangle-down":180,"triangle-right":270,"triangle-left":90})[s.shape] || 0},draw=${texColor(s.stroke)},draw opacity=${fmt(s.stroke === null ? 0 : s.opacity*s.strokeOpacity)},fill=${s.filled ? texColor(s.fill) : "none"},fill opacity=${fmt(s.opacity*s.fillOpacity)}}`];
  }
  const coord = pt => pt == null ? "(nan,nan)" : `(${fmt(pt[0])},${fmt(pt[1])})`;
  function stepped(points, mode, horizontal, axes) {
    if (!mode || mode === "linear") return points;
    const ci = horizontal ? 1 : 0, vi = 1 - ci, out = [];
    let last = null;
    for (const pt of points) {
      if (last && pt) {
        if (mode === "step-before") { const mid = [...last]; mid[vi] = pt[vi]; out.push(mid); }
        else if (mode === "step-after") { const mid = [...pt]; mid[vi] = last[vi]; out.push(mid); }
        else {
          const axis = axes[horizontal ? "y" : "x"];
          const m = axis.kind === "log" ? Math.sqrt(last[ci] * pt[ci]) : (last[ci] + pt[ci]) / 2;
          const a = [...last], b = [...pt]; a[ci] = m; b[ci] = m; out.push(a, b);
        }
      }
      out.push(pt); last = pt;
    }
    return out;
  }
  function curvePath(commands) {
    return commands.map(c=>{
      const pt=(i)=>`(axis cs:${fmt(c[i])},${fmt(c[i+1])})`;
      if(c[0]==="M")return pt(1);
      if(c[0]==="L")return "-- "+pt(1);
      if(c[0]==="C")return `.. controls ${pt(1)} and ${pt(3)} .. ${pt(5)}`;
      if(c[0]==="Z")return "-- cycle";
      fail("curve","unknown command");
    }).join(" ");
  }
  function emit(figure, opts) {
    const out = [];
    if (opts.standalone) out.push("\\documentclass[border=2bp]{standalone}", "\\usepackage[T1]{fontenc}", "\\usepackage{amsmath,amssymb,pgfplots}", "\\usepgfplotslibrary{groupplots,fillbetween,statistics,patchplots,colormaps}", "\\pgfplotsset{compat=1.18}", "\\begin{document}");
    out.push("% Native axes and data coordinates. No SVG outlines or embedded pictures.", "\\begin{tikzpicture}");
    const multi = figure.panels.length > 1;
    if (multi) out.push(`\\begin{groupplot}[group style={group size=${figure.columns} by ${Math.ceil(figure.panels.length / figure.columns)},horizontal sep=2cm,vertical sep=2cm}]`);
    const tasks=figure.panels.map((panel,pi)=>({panel,pi,overlay:false}));
    for(const [pi,panel] of figure.panels.entries())if(panel.secondary)tasks.push({panel:panel.secondary,pi,overlay:true});
    let groupClosed=false;
    for (const {panel,pi,overlay} of tasks) {
      if(overlay && multi && !groupClosed){out.push("\\end{groupplot}");groupClosed=true;}
      const settings = [`width=${bp(panel.width)}`, `height=${bp(panel.height)}`, "scale only axis", "enlargelimits=false", "clip=true", "unbounded coords=jump", "legend style={at={(1.03,1)},anchor=north west}", ...axisOptions(panel.axes.x, "x", opts), ...axisOptions(panel.axes.y, "y", opts)];
      if(panel.border!=null)settings.push(`axis background/.style={draw=${texColor(color(panel.border,"view.stroke"))},line width=${bp(panel.borderWidth)}}`);
      if(overlay)settings.push(`at={(${multi?`group c${pi%figure.columns+1}r${Math.floor(pi/figure.columns)+1}`:`vlcaxis${pi}`}.south west)}`,"anchor=south west","axis y line*=right","axis x line=none","xtick=\\empty");
      else settings.push(...(!multi?[`name=vlcaxis${pi}`]:[]),...(panel.secondary?["axis y line*=left"]:[]));
      if (panel.colorAxis) {
        const c=panel.colorAxis;
        settings.push(`point meta min=${fmt(c.domain[0])}`,`point meta max=${fmt(c.domain[1])}`);
        if (c.range) settings.push(`colormap={vlcmap${pi}}{${c.range.map((rgb,i)=>`rgb255(${i}cm)=(${rgb.join(",")});`).join(" ")}}`);
        else settings.push("colormap/viridis");
        if(c.show) settings.push("colorbar",`colorbar style={ylabel={${label(c.title,opts.tex_labels)}}}`);
      }
      if (panel.title != null) settings.push(`title={${label(panel.title, opts.tex_labels)}}`);
      out.push(`${multi && !overlay ? "\\nextgroupplot" : "\\begin{axis}"}[\n  ${settings.join(",\n  ")}\n]`);
      const legends = new Set();
      if (panel.legendTitle != null) out.push("\\addlegendimage{empty legend}", `\\addlegendentry{\\textbf{${label(panel.legendTitle,opts.tex_labels)}}}`);
      for (const [i, plot] of panel.plots.entries()) {
        const {style: s, mark: m, horizontal: h} = plot;
        const legend = plot.legend != null && !legends.has(plot.legend);
        const draw = (points, extra = [], fill = false) => out.push(`\\addplot[${[...plotStyle(s, fill), ...extra, ...(legend ? [] : ["forget plot"])].join(",")}] coordinates {\n${points.map(coord).join("\n")}\n};`);
        if(plot.type==="line" && m.interpolate==="monotone") {
          if(!plot.curve)fail("interpolate","monotone requires the official normalization stage");
          out.push(`\\draw[${plotStyle(s).join(",")}] ${curvePath(plot.curve)};`);
          if(m.point)draw(plot.points,["only marks",...pointStyle(s)]);
          else if(legend)out.push(`\\addlegendimage{${plotStyle(s).join(",")}}`);
        }
        else if (plot.type === "line") draw(stepped(plot.points, m.interpolate, h, panel.axes), m.point ? pointStyle(s) : ["mark=none"]);
        else if(plot.type==="tick") draw(plot.points,["only marks",`mark=${m.orient==="vertical"?"|":"-"}`,`mark size=${bp((m.size || 20)/2)}`,`line width=${bp(m.thickness || 1)}`]);
        else if (plot.type === "point") {
          if (plot.meta) {
            const mapped = `scatter/use mapped color={draw=mapped color,fill=${s.filled ? "mapped color" : "none"}}`;
            out.push(`\\addplot[${[...plotStyle(s),"only marks",...pointStyle(s),"scatter","scatter src=explicit",mapped,"forget plot"].join(",")}] coordinates {${plot.points.map((pt,k)=>pt ? `${coord(pt)} [${fmt(plot.meta[k])}]` : "").join(" ")}};`);
          } else draw(plot.points.filter(Boolean), ["only marks", ...pointStyle(s)]);
        }
        else if (plot.type === "rect") {
          if (plot.meta != null) out.push(`\\addplot[patch,patch type=rectangle,shader=flat,point meta=explicit,draw=none,forget plot,opacity=${fmt(s.opacity*s.fillOpacity)}] coordinates {${plot.points.slice(0,4).map(pt=>`${coord(pt)} [${fmt(plot.meta)}]`).join(" ")}};`);
          else draw(plot.points, ["mark=none"], true);
        }
        else if (plot.type === "bar") {
          const ca = panel.axes[h ? "y" : "x"], measure = h ? "x" : "y";
          const full = ca.kind === "category" ? (h ? panel.height : panel.width) / Math.max(1, ca.categories.length) * .8 : 5;
          const width = m.size ?? full / plot.seriesCount;
          const shift = plot.seriesCount > 1 ? (plot.seriesIndex - (plot.seriesCount - 1) / 2) * width : 0;
          if (plot.bars.every(b => b.low === 0 && b.c2 == null)) {
            draw(plot.upper, [h ? "xbar" : "ybar", "area legend", `bar width=${bp(width)}`, `bar shift=${bp(shift)}`, "mark=none"], true);
          } else {
            // Ranged/stacked bars remain explicit data-coordinate polygons.
            // For categorical bars the width is expressed in category units.
            for (const b of plot.bars) {
              if (b.c2 != null && m.size !== undefined) fail("bar", "size and ranged category bounds cannot be combined");
              const fraction = width / (h ? panel.height : panel.width) * (ca.domain[1] - ca.domain[0]);
              const offset = shift / (h ? panel.height : panel.width) * (ca.domain[1] - ca.domain[0]);
              let a = b.c2 == null ? b.c + offset - fraction / 2 : b.c, z = b.c2 == null ? b.c + offset + fraction / 2 : b.c2;
              if(ca.kind==="log" && b.c2==null){const half=width/(h?panel.height:panel.width)*Math.log(ca.domain[1]/ca.domain[0])/2;a=b.c*Math.exp(-half);z=b.c*Math.exp(half);}
              const points = [[a,b.low],[z,b.low],[z,b.high],[a,b.high],[a,b.low]].map(pt => h ? [pt[1],pt[0]] : pt);
              out.push(`\\addplot[${[...plotStyle(s,true),"mark=none","forget plot"].join(",")}] coordinates {${points.map(coord).join(" ")}};`);
            }
            if (legend) out.push(`\\addlegendimage{area legend,fill=${texColor(s.fill)},draw=${texColor(s.stroke)}}`);
          }
        } else if(plot.type==="band" && m.interpolate==="monotone") {
          if(!plot.curvedBands)fail("interpolate","monotone requires the official normalization stage");
          for(const [bi,band] of plot.curvedBands.entries()){
            const name=`vlc${pi}p${i}b${bi}`;
            out.push(`\\path[name path=${name}lo] ${curvePath(band.lo)};`,`\\path[name path=${name}hi] ${curvePath(band.hi)};`,`\\addplot[${[...plotStyle(s,true),"draw=none","forget plot"].join(",")}] fill between[of=${name}lo and ${name}hi];`);
          }
          if(legend)out.push(`\\addlegendimage{area legend,fill=${texColor(s.fill)},draw=none}`);
        } else if (plot.type === "band") {
          const lower = stepped(plot.lower, m.interpolate, h, panel.axes), upper = stepped(plot.upper, m.interpolate, h, panel.axes);
          // Split at invalid rows so a band never bridges missing observations.
          let run = [], ri = 0;
          const flush = () => {
            if (!run.length) return;
            const name = `vlc${pi}p${i}r${ri++}`;
            for (const [suffix, index] of [["lo",0],["hi",1]]) out.push(`\\addplot[name path=${name}${suffix},draw=none,forget plot] coordinates {${run.map(pair => coord(pair[index])).join(" ")}};`);
            out.push(`\\addplot[${[...plotStyle(s,true),"draw=none","forget plot"].join(",")}] fill between[of=${name}lo and ${name}hi];`);
            run = [];
          };
          for (let k = 0; k < lower.length; k++) { if (lower[k] && upper[k]) run.push([lower[k],upper[k]]); else flush(); } flush();
          if (legend) out.push(`\\addlegendimage{area legend,fill=${texColor(s.fill)},draw=none}`);
        } else if (plot.type === "errorbar") {
          const axis = h ? "x" : "y";
          out.push(`\\addplot[${["draw=none","mark=none",...(legend?[]:["forget plot"]),"error bars/.cd",`${axis} dir=both`,`${axis} explicit`,`error bar style={${plotStyle(s).join(",")}}`].join(",")}] table[x=x,y=y,${axis} error minus=minus,${axis} error plus=plus,row sep=\\\\] {`);
          out.push("x y minus plus\\\\", ...plot.points.map((pt,k) => `${pt.map(fmt).join(" ")} ${plot.errors[k].map(fmt).join(" ")}\\\\`), "};");
        } else if (plot.type === "boxplot") {
          out.push(`\\addplot[${[...plotStyle(s,true),`boxplot/draw direction=${h?"x":"y"}`,"boxplot/data=y",`boxplot={draw position=${fmt(plot.c)},box extend=${fmt(plot.boxWidth)}}`,`boxplot prepared={lower whisker=${fmt(plot.low)},lower quartile=${fmt(plot.q1)},median=${fmt(plot.median)},upper quartile=${fmt(plot.q3)},upper whisker=${fmt(plot.high)}}`,...(legend?[]:["forget plot"])].join(",")}] coordinates {${plot.outliers.map(v => coord([0,v])).join(" ")}};`);
        } else if (plot.type === "rule") {
          const points = plot.points.map((pt,k) => pt.map((v, dim) => v == null ? panel.axes[dim ? "y" : "x"].domain[k] : v));
          draw(points, ["mark=none"]);
        } else if (plot.type === "text") {
          const anchors = {left:"west",center:"center",right:"east"};
          let anchor = anchors[m.align || "center"];
          if (m.baseline === "top") anchor = anchor === "center" ? "north" : `north ${anchor}`;
          if (m.baseline === "bottom") anchor = anchor === "center" ? "south" : `south ${anchor}`;
          if (m.baseline === "alphabetic") anchor = anchor === "center" ? "base" : `base ${anchor}`;
          out.push(`\\node[anchor=${anchor},text=${texColor(s.fill)},opacity=${fmt(s.opacity)},rotate=${fmt(-(m.angle || 0))},xshift=${bp(m.dx || 0)},yshift=${bp(-(m.dy || 0))},font=\\fontsize{${bp(plot.fontSize ?? 11)}}{${bp((plot.fontSize ?? 11)*1.2)}}\\selectfont] at (axis cs:${plot.points[0].map(fmt).join(",")}) {${label(plot.text,opts.tex_labels)}};`);
        }
        if (legend) { out.push(`\\addlegendentry{${label(plot.legend,opts.tex_labels)}}`); legends.add(plot.legend); }
      }
      for(const guide of panel.symbolLegends || []) {
        out.push("\\addlegendimage{empty legend}",`\\addlegendentry{${label(guide.title,opts.tex_labels)}}`);
        for(const item of guide.items)out.push(`\\addlegendimage{only marks,${pointStyle(style({type:"point",filled:true,...item.mark})).join(",")}}`,`\\addlegendentry{${label(item.label,opts.tex_labels)}}`);
      }
      if (!multi || overlay) out.push("\\end{axis}");
    }
    if (multi && !groupClosed) out.push("\\end{groupplot}");
    if (figure.title != null) out.push(`\\node[anchor=south] at (current bounding box.north) {${label(figure.title,opts.tex_labels)}};`);
    out.push("\\end{tikzpicture}");
    if (opts.standalone) out.push("\\end{document}");
    const tex = out.join("\n") + "\n";
    if (tex.length > LIMIT.output) fail("output", "output size limit exceeded");
    return tex;
  }
  return Object.freeze({lower, emit, compile: (spec, options = {}) => emit(lower(spec, options), options)});
})();

// Original Vega-Lite inputs exercising the official-dataflow path. These must
// reach Rust/Python/CLI unchanged; the low-level emitter cannot accept them.
const q = field => ({field, type: 'quantitative'});
const domain = (field, values) => ({...q(field), scale: {domain: values, nice: false}});
const rows = [{x: 1, y: 2}, {x: 2, y: 4}, {x: 3, y: 8}];
const base = {width: 300, height: 200, data: {values: rows}, mark: 'line',
  encoding: {x: domain('x', [0, 4]), y: domain('y', [0, 10])}};
export const normalizationFixtures = {
  default_heatmap: {width:300,height:200,data:{values:[{x:'A',y:'X',z:0},{x:'A',y:'Y',z:25},{x:'B',y:'X',z:25},{x:'B',y:'Y',z:50}]},mark:'rect',
    encoding:{x:{field:'x',type:'nominal'},y:{field:'y',type:'nominal'},color:q('z')}},
  rolling_mean: {...base, transform: [{window: [{op: 'mean', field: 'y', as: 'mean'}],
    frame: [-1, 0], sort: [{field: 'x'}]}],
    encoding: {...base.encoding, y: domain('mean', [0, 10])}},
  calculated_errors: {...base, mark: {type: 'errorbar'},
    transform: [{calculate: 'datum.y - 0.5', as: 'lo'}, {calculate: 'datum.y + 0.5', as: 'hi'}],
    encoding: {x: base.encoding.x, y: domain('lo', [0, 10]), y2: {field: 'hi'}}},
  calendar_utc: {...base, data: {values: [
    {date: '2024-02-28T00:00:00Z', y: 2}, {date: '2024-02-29T00:00:00Z', y: 4},
    {date: '2024-03-01T00:00:00Z', y: 8}]},
    encoding: {x: {field: 'date', type: 'temporal', scale: {type: 'utc', nice: false},
      axis: {format: '%Y-%m-%d'}}, y: base.encoding.y}},
  monotone_curve: {...base, mark: {type: 'line', interpolate: 'monotone'}},
  native_ticks: {...base, mark: {type: 'tick', orient: 'vertical', size: 24, thickness: 2}},
  variable_symbols: {...base, mark: {type: 'point', filled: true},
    encoding: {...base.encoding, size: {...q('y'), scale: {domain: [0, 8], range: [0, 144]}}}},
  hollow_symbols: {...base, mark: 'point',
    encoding: {...base.encoding, size: {...q('y'), scale: {domain: [0, 8], range: [0, 144]}}}},
  shared_layer_axes: {width: 300, height: 200, data: base.data,
    layer: [{mark: 'bar', encoding: base.encoding},
      {mark: {type: 'line', color: '#ff0000'}, encoding: base.encoding}]},
  independent_y_axes: {width: 300, height: 200, data: base.data,
    layer: [{mark: 'line', encoding: base.encoding},
      {mark: {type: 'line', color: '#ff0000'},
        transform: [{calculate: 'datum.y * 1000', as: 'z'}],
        encoding: {x: base.encoding.x, y: domain('z', [0, 20000])}}],
    resolve: {scale: {y: 'independent'}}},
};

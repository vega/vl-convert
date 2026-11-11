const {test}=require('node:test');
const assert=require('node:assert/strict');
require('../../src/js/native_pgfplots.js');
const {compile,lower}=globalThis.vlcNativePgfplots;
const {fixtures,basic,q,n,clone}=require('./fixtures.cjs');
for(const [name,spec] of Object.entries(fixtures())) test(`native ${name}`,()=>{
 const text=compile(spec,{standalone:true,tex_labels:name==='math'});
 assert.match(text,/\\(?:begin\{axis\}|nextgroupplot)/);
 assert.match(text,/\\addplot/);
 assert.doesNotMatch(text,/\\pgfpath|\\includegraphics|hide axis/);
 assert.equal(text,compile(spec,{standalone:true,tex_labels:name==='math'}));
 assert.match(text,/\\end\{document\}\n$/);
});
test('retains source coordinates rather than pixel positions',()=>{
 const text=compile(basic);
 for(const pair of ['(1,2)','(2,4)','(3,3)']) assert.ok(text.includes(pair));
 assert.match(text,/xlabel=\{x\}/); assert.match(text,/ylabel=\{y\}/);
});
test('native log scale, domain, and reversed axis',()=>{
 const spec=clone(basic); spec.encoding.y.scale={type:'log',domain:[1,100],reverse:true};
 const text=compile(spec); for(const x of ['ymode=log','ymin=1','ymax=100','y dir=reverse']) assert.ok(text.includes(x));
});
test('prepared asymmetric errors preserve endpoints',()=>{
 const p=lower(fixtures().asymmetric).panels[0].plots[0];
 assert.deepEqual(p.errors,[[1,2],[.5,1.5]]); assert.deepEqual(p.lower,[[1,3],[2,2.5]]); assert.deepEqual(p.upper,[[1,6],[2,4.5]]);
});
test('R7 quartiles, Tukey whiskers, and outliers',()=>{
 const p=lower(fixtures().boxplot).panels[0].plots[0];
 assert.equal(p.q1,2.5); assert.equal(p.median,4); assert.equal(p.q3,5.5); assert.equal(p.low,1); assert.equal(p.high,6); assert.deepEqual(p.outliers,[100]);
});
test('sample standard error definition',()=>{
 const spec={data:{values:[1,2,3].map(v=>({g:'A',v}))},mark:{type:'errorbar',extent:'stderr'},encoding:{x:n('g'),y:q('v')}};
 const p=lower(spec).panels[0].plots[0]; assert.equal(p.points[0][1],2); assert.ok(Math.abs(p.errors[0][0]-1/Math.sqrt(3))<1e-14);
});
test('bin counts and upper endpoint',()=>{
 const p=lower(fixtures().histogram).panels[0].plots[0];
 assert.deepEqual(p.bars.map(b=>[b.c,b.c2,b.high]),[[0,1,2],[1,2,1],[2,3,2],[3,4,2]]);
});
test('source objects are never mutated',()=>{
 const spec=fixtures().histogram,before=JSON.stringify(spec); compile(spec); assert.equal(JSON.stringify(spec),before);
});
test('default labels are TeX escaped; raw math is opt-in',()=>{
 const spec=clone(basic);spec.encoding.x.title='\\input{secret} $x_1$ & 50%';
 const text=compile(spec);assert.doesNotMatch(text,/\\input\{secret\}/);assert.match(text,/\\textbackslash\{\}input\\\{secret\\\}/);assert.match(text,/\\%/);
 const raw=compile(fixtures().math,{tex_labels:true});assert.match(raw,/\$E_\{\\mathrm\{kin\}\}\$/);
});
test('named inline datasets',()=>{
 const spec={...clone(basic),data:{name:'source'},datasets:{source:basic.data.values}};
 assert.equal(compile(spec),compile(basic));
});
test('fold and grouped legends',()=>{
 const spec={data:{values:[{x:1,a:2,b:3},{x:2,a:4,b:5}]},transform:[{fold:['a','b']}],mark:'line',encoding:{x:q('x'),y:q('value'),color:n('key')}};
 const text=compile(spec);assert.match(text,/\\addlegendentry\{a\}/);assert.match(text,/\\addlegendentry\{b\}/);assert.match(text,/\(2,5\)/);
});
test('aggregation transform',()=>{
 const spec={data:fixtures().stacked.data,transform:[{aggregate:[{op:'sum',field:'v',as:'total'}],groupby:['g']}],mark:'bar',encoding:{x:n('g'),y:q('total')}};
 assert.deepEqual(lower(spec).panels[0].plots[0].upper,[[0,5],[1,9]]);
});
const invalid=[
 ['unsupported mark',s=>s.mark='geoshape',/unsupported value/],
 ['unsupported field type',s=>s.encoding.x.type='temporal',/unsupported value/],
 ['unknown option',s=>s.encoding.y.axis={labelExpr:'datum.value'},/unsupported property/],
 ['remote data',s=>s.data={url:'https://example.invalid/data.csv'},/unsupported property/],
 ['calculate code',s=>s.transform=[{calculate:'process.exit()',as:'z'}],/materialize/],
 ['expression filter',s=>s.transform=[{filter:'datum.x>1'}],/expected an object/],
 ['selections',s=>s.params=[{name:'pick',select:'interval'}],/unsupported property/],
 ['nonpositive log',s=>{s.encoding.y.scale={type:'log'};s.data.values[0].y=0},/positive/],
 ['missing field',s=>s.encoding.y.field='nope',/missing literal data field/],
 ['string number',s=>s.data.values[0].y='1',/finite number/],
 ['NaN',s=>s.data.values[0].y=NaN,/finite number/],
 ['Infinity',s=>s.data.values[0].y=Infinity,/finite number/],
 ['raw confidence interval',s=>s.mark='errorband',/precompute confidence intervals/],
 ['pixel position',s=>s.encoding.x={value:20},/use datum/],
 ['bad color',s=>s.mark={type:'line',color:'red]\\input{x}'},/supported color/],
 ['nonmonotone domain',s=>s.encoding.x.scale={domain:[5,0]},/increasing/],
 ['unsupported scale',s=>s.encoding.x.scale={type:'symlog'},/unsupported value/],
];
for(const [name,mutate,error] of invalid)test(`rejects ${name}`,()=>{const spec=clone(basic);mutate(spec);assert.throws(()=>compile(spec),error)});
test('missing values interrupt lines',()=>assert.match(compile(fixtures().gaps),/\(1,2\)\n\(nan,nan\)\n\(2,4\)/));
test('normalized stack sums to one',()=>{
 const p=lower(fixtures().normalized).panels[0].plots;assert.deepEqual(p[1].upper,[[0,1],[1,1]]);
});
test('input limits',()=>{
 const s=clone(basic);s.data.values=Array(100001).fill({x:1,y:1});assert.throws(()=>compile(s),/row limit/);
 assert.throws(()=>compile({hconcat:Array(65).fill(basic)}),/panel count/);
 assert.throws(()=>compile(basic,{tex_labels:1}),/boolean/);
});

test('histogram includes its last upper bin boundary',()=>assert.equal(lower(fixtures().histogram).panels[0].axes.x.domain[1],4));
test('horizontal boxplot outlier stream remains in PGFPlots y data column',()=>assert.match(compile(fixtures().horizontal_boxplot),/coordinates \{\(0,100\)\}/));
test('explicit numeric metadata and native colormaps',()=>{
 const spec=fixtures().heatmap,tex=compile(spec);
 assert.match(tex,/patch type=rectangle/);assert.match(tex,/point meta min=0/);assert.match(tex,/point meta max=3/);assert.match(tex,/colorbar/);
 assert.deepEqual(lower(spec).panels[0].plots.map(p=>p.meta),[0,1,2,3]);
});
test('line data are sorted independently of source order',()=>{
 const s=clone(basic);s.data.values.reverse();assert.equal(compile(s),compile(basic));
});
test('stack order is stable under row permutation',()=>{
 const a=fixtures().stacked,b=clone(a);b.data.values.reverse();assert.equal(compile(a),compile(b));
});
test('sparse stacked areas fail rather than guessing missing samples',()=>{
 const s=fixtures().stacked;s.mark='area';s.data.values.pop();assert.throws(()=>compile(s),/shared sampling grid/);
});
test('unsupported properties on otherwise valid channels fail',()=>{
 const s=clone(basic);s.encoding.x.legend={title:'oops'};assert.throws(()=>compile(s),/unsupported property/);
});
test('unsupported layer title is not silently lost',()=>{
 assert.throws(()=>compile({layer:[{...clone(basic),title:'lost'}]}),/layer-level setting/);
});

test('facet categorical coordinates and numeric domains are shared',()=>{
 const panels=lower(fixtures().facets).panels;
 assert.deepEqual(panels.map(p=>p.axes.x.categories),[['A','B','C'],['A','B','C']]);
 assert.deepEqual(panels.map(p=>p.axes.y.domain),[[0,5],[0,5]]);
 assert.deepEqual(panels[1].plots[0].bars.map(b=>b.c),[1,2]);
});
test('repeat substitutions preserve each selected field',()=>{
 const panels=lower(fixtures().repeat).panels;
 assert.deepEqual(panels[0].plots[0].points,[[1,2],[2,4]]);
 assert.deepEqual(panels[1].plots[0].points,[[1,3],[2,5]]);
});
test('explicit independent facet scales remain independent',()=>{
 const s=fixtures().facets;s.resolve={scale:{x:'independent',y:'independent'}};
 const p=lower(s).panels;assert.deepEqual(p[0].axes.x.categories,['A','B']);assert.deepEqual(p[1].axes.x.categories,['B','C']);assert.equal(p[0].axes.y.domain[1],3);
});

test('stack null overlaps; only explicit offsets group bars',()=>{
 assert.equal(lower(fixtures().overlapping).panels[0].plots[0].seriesCount,1);
 assert.equal(lower(fixtures().grouped).panels[0].plots[0].seriesCount,2);
 assert.match(compile(fixtures().overlapping),/bar shift=0bp/);
});
test('custom stack order is rejected rather than treated as line order',()=>{
 const s=fixtures().stacked;s.encoding.order={field:'v'};assert.throws(()=>compile(s),/custom stack order/);
});
test('nested field paths cannot silently mean literal data keys',()=>{
 const s=clone(basic);s.encoding.y.field='a.b';s.data.values=s.data.values.map(r=>({...r,'a.b':3}));assert.throws(()=>compile(s),/field paths/);
});
test('safe Greek labels and multiline titles remain TeX text',()=>{
 const s=clone(basic);s.encoding.y.title=['α ± σ','measurement'];const t=compile(s);assert.match(t,/shortstack/);assert.match(t,/ensuremath\{\\alpha\}/);
});
test('empty input still validates unsupported aggregate operators',()=>{
 const s=clone(basic);s.data.values=[];s.encoding.y.aggregate='bogus';assert.throws(()=>compile(s),/unsupported value/);
});
test('conflicting layer axes are rejected',()=>{
 const s={layer:[clone(basic),clone(basic)]};s.layer.forEach(v=>{delete v.width;delete v.height});s.layer[0].encoding.x.axis={grid:true};s.layer[1].encoding.x.axis={grid:false};assert.throws(()=>compile(s),/compatible axis/);
});
test('secondary errors require primary errors',()=>{
 const s=fixtures().asymmetric;delete s.encoding.yError;assert.throws(()=>compile(s),/primary error/);
});

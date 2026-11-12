import {test} from 'node:test';
import assert from 'node:assert/strict';
import * as vega from '/npm/vega@6.4.0/+esm';
import * as vl from '/npm/vega-lite@6.4.3/+esm';
import '../../src/js/native_pgfplots.js';
import '../../src/js/native_pgfplots_normalize.js';
const core=globalThis.vlcNativePgfplots,normalize=globalThis.vlcPgfplotsNormalize;
const q=field=>({field,type:'quantitative'}),n=field=>({field,type:'nominal'});
const base={data:{values:[{x:1,y:2},{x:2,y:4},{x:3,y:8}]},mark:'line',encoding:{x:q('x'),y:q('y')}};
async function run(spec,env={}){const prepared=await normalize.prepare(spec,{vega,vl,...env});const ir=normalize.decorate(core.lower(prepared.spec),vega);return {...prepared,ir,tex:core.emit(ir,{standalone:true})};}

test('original input is immutable and produces native coordinates',async()=>{
 const input=structuredClone(base),before=JSON.stringify(input);const out=await run(input);
 assert.equal(JSON.stringify(input),before);assert.match(out.tex,/\(1,2\)/);assert.doesNotMatch(out.tex,/pgfpath|includegraphics|hide axis/);
});
test('official window ordering, partitioning and frame semantics',async()=>{
 const spec={...base,data:{values:[{x:3,y:8,g:'a'},{x:1,y:2,g:'a'},{x:2,y:4,g:'a'},{x:1,y:100,g:'b'}]},transform:[{window:[{op:'mean',field:'y',as:'average'}],frame:[-1,0],sort:[{field:'x'}],groupby:['g']}],encoding:{x:q('x'),y:q('average'),color:n('g')}};
 const out=await run(spec),rows=out.spec.data.values;
 assert.equal(rows.find(r=>r.x===3&&r.g==='a').average,6);assert.equal(rows.find(r=>r.g==='b').average,100);
});
test('calculate and joinaggregate are evaluated before encodings',async()=>{
 const out=await run({...base,transform:[{joinaggregate:[{op:'sum',field:'y',as:'total'}]},{calculate:'datum.y / datum.total',as:'fraction'}],encoding:{x:q('x'),y:q('fraction')}});
 assert.deepEqual(out.ir.panels[0].plots[0].points,[[1,2/14],[2,4/14],[3,8/14]]);
});
test('calculated uncertainty remains native error bars',async()=>{
 const out=await run({...base,transform:[{calculate:'datum.y-.25',as:'lo'},{calculate:'datum.y+.5',as:'hi'}],mark:'errorbar',encoding:{x:q('x'),y:q('lo'),y2:{field:'hi'}}});
 assert.match(out.tex,/error bars\/\.cd/);assert.deepEqual(out.ir.panels[0].plots[0].lower,[[1,1.75],[2,3.75],[3,7.75]]);
});
test('UTC timeUnit and calendar labels retain the evaluated dates',async()=>{
 const out=await run({data:{values:[{d:'2020-02-29T12:00:00Z',y:1},{d:'2021-02-28T12:00:00Z',y:2}]},mark:'line',encoding:{x:{field:'d',timeUnit:'utcyearmonth',type:'temporal'},y:q('y')}});
 const xs=out.ir.panels[0].plots[0].points.map(p=>p[0]);assert.deepEqual(xs,[Date.UTC(2020,1,1),Date.UTC(2021,1,1)]);assert.ok(out.ir.panels[0].axes.x.axis.tickLabels.length);
});
test('automatic binning uses the official bin transform',async()=>{
 const out=await run({data:{values:[{x:.2},{x:.4},{x:1.2},{x:1.3}]},mark:'bar',encoding:{x:{...q('x'),bin:{step:1,extent:[0,2]}},y:{aggregate:'count',type:'quantitative'}}});
 assert.deepEqual(out.ir.panels[0].plots[0].bars.map(b=>[b.c,b.c2,b.high]),[[0,1,2],[1,2,2]]);
});
test('stack null is not silently restacked by the native emitter',async()=>{
 const data={values:[{x:'a',y:2,g:'A'},{x:'a',y:4,g:'B'}]};const out=await run({data,mark:'bar',encoding:{x:n('x'),y:{...q('y'),stack:null},color:n('g')}});
 assert.deepEqual(out.ir.panels[0].plots.map(p=>p.bars[0].low),[0,0]);
});
test('official stack boundaries are emitted once, not double-stacked',async()=>{
 const data={values:[{x:'a',y:2,g:'A'},{x:'a',y:4,g:'B'}]};const out=await run({data,mark:'bar',encoding:{x:n('x'),y:{...q('y'),aggregate:'sum'},color:n('g')}});
 assert.equal(Math.max(...out.ir.panels[0].plots.map(p=>p.bars[0].high)),6);assert.equal(Math.min(...out.ir.panels[0].plots.map(p=>p.bars[0].low)),0);
});
test('ticks and sized points remain native marks',async()=>{
 const tick=await run({...base,mark:'tick'});assert.match(tick.tex,/mark=[|\-]/);
 const sized=await run({...base,mark:'circle',encoding:{...base.encoding,size:{...q('y'),scale:{domain:[0,8],range:[0,64]}}}});
 const sizes=sized.ir.panels[0].plots.map(p=>p.style.size);assert.deepEqual(sizes,[16,32,64]);assert.match(sized.tex,/mark size=/);
});
test('dual y axes share the same x domain, not the same y domain',async()=>{
 const out=await run({data:base.data,layer:[{mark:'bar',encoding:base.encoding},{mark:{type:'line',color:'red'},transform:[{calculate:'datum.y*1000',as:'z'}],encoding:{x:q('x'),y:q('z')}}],resolve:{scale:{y:'independent'}}});
 const p=out.ir.panels[0];assert.deepEqual(p.axes.x.domain,p.secondary.axes.x.domain);assert.ok(p.secondary.axes.y.domain[1]>p.axes.y.domain[1]);assert.match(out.tex,/axis y line\*=right/);
});
test('monotone uses official cubic controls in data coordinates',async()=>{
 const out=await run({...base,mark:{type:'line',interpolate:'monotone'}});const curve=out.ir.panels[0].plots[0].curve;
 assert.equal(curve[0][0],'M');assert.deepEqual(curve[0].slice(1),[1,2]);assert.equal(curve.at(-1)[0],'C');assert.deepEqual(curve.at(-1).slice(-2),[3,8]);assert.match(out.tex,/controls \(axis cs:/);
});
test('field-path evaluation is official and flattened without mutation',async()=>{
 const out=await run({data:{values:[{a:{x:2},y:3}]},mark:'point',encoding:{x:q('a.x'),y:q('y')}});
 assert.deepEqual(out.ir.panels[0].plots[0].points,[[2,3]]);
});
test('structured data loader policy cannot be bypassed',async()=>{
 await assert.rejects(run({...base,data:{url:'https://denied.invalid/data.csv'}},{loader:{load:async()=>{throw new Error('VLC_ACCESS_DENIED test');}}}),/VLC_ACCESS_DENIED/);
});
test('interaction rejection happens before any resource access',async()=>{
 let called=false;
 await assert.rejects(run({...base,data:{url:'https://denied.invalid/data'},params:[{name:'brush',select:'interval'}]},{loader:{load:async()=>{called=true;return '[]';}}}),/outside the native Cartesian/);
 assert.equal(called,false);
});
test('unsupported free JavaScript is rejected by Vega expression parsing',async()=>{
 await assert.rejects(run({...base,transform:[{calculate:'globalThis.process.exit()',as:'bad'}]}));
});

test('quoted labels and Unicode separators stay data, not generated expressions',async()=>{
 const title="');globalThis.compromised=1;//\n\u2028\u2029\\input{secret}";
 const input=structuredClone(base);input.encoding.x.title=title;
 const before=JSON.stringify(input);
 const out=await run(input);
 assert.equal(JSON.stringify(input),before);
 assert.equal(globalThis.compromised,undefined);
 assert.ok(out.tex.includes('textbackslash{}input'));
 assert.ok(!out.tex.includes('\\input{secret}'));
 assert.ok(!/[\u2028\u2029]/.test(out.tex));
 assert.match((await run(base)).tex,/\\addplot/);
});

test('shared layered axes remain visible unless all layers explicitly hide them',async()=>{
 const spec={data:base.data,layer:[{mark:'bar',encoding:base.encoding},{mark:'line',encoding:base.encoding}]};
 let out=await run(spec);
 for(const c of ['x','y'])assert.notEqual(out.ir.panels[0].axes[c].axis,null);
 assert.doesNotMatch(out.tex,/axis [xy] line=none/);
 for(const layer of spec.layer)layer.encoding=structuredClone(base.encoding);
 spec.layer[0].encoding.x.axis=null;
 out=await run(spec);assert.notEqual(out.ir.panels[0].axes.x.axis,null);
 spec.layer[1].encoding.x.axis=null;
 out=await run(spec);assert.equal(out.ir.panels[0].axes.x.axis,null);
});


test('unsupported non-BMP labels fail before returning uncompilable TeX',async()=>{
 const spec=structuredClone(base);spec.encoding.x.title='cattle \u{1f404}';
 await assert.rejects(run(spec),/non-BMP glyphs require a trusted TeX label/);
});


test('transparent view strokes and explicitly unfilled points are retained',async()=>{
 const out=await run({...base,mark:{type:'point',fill:'none'},config:{view:{stroke:'transparent'}}});
 assert.equal(out.spec.config.view.stroke,null);
 assert.equal(out.ir.panels[0].plots[0].style.fill,null);
});
test('color datum retains its scaled color and native legend',async()=>{
 const out=await run({...base,encoding:{...base.encoding,color:{datum:'Revenue',type:'nominal',scale:{domain:['Revenue'],range:['#123456']}}}});
 assert.deepEqual(out.ir.panels[0].plots[0].style.stroke,[18,52,86]);
 assert.match(out.tex,/\\addlegendentry\{Revenue\}/);
});


test('calendar labels have no misleading epoch-number multiplier',async()=>{
 const out=await run({data:{values:[{d:'2024-02-28',v:2},{d:'2024-03-01',v:3}]},mark:'line',encoding:{x:{field:'d',type:'temporal',scale:{type:'utc'},axis:{format:'%Y-%m-%d'}},y:q('v')}});
 assert.match(out.tex,/scaled x ticks=false/);
});


test('filled symbols without a stroke do not acquire a black TeX outline',async()=>{
 const out=await run({...base,mark:{type:'point',filled:true},encoding:{...base.encoding,size:q('y')}});
 assert.equal(out.ir.panels[0].plots[0].style.stroke,null);
 assert.match(out.tex,/mark options=\{[^}]*draw=none,draw opacity=0,/);
});


test('transparent variable-size symbols and their legends stay unfilled',async()=>{
 const spec={...base,mark:'point',encoding:{...base.encoding,size:{...q('y'),scale:{domain:[0,8],range:[0,144]}}}};
 const out=await run(spec),panel=out.ir.panels[0];
 for(const plot of panel.plots){assert.equal(plot.style.fill,null);assert.equal(plot.style.filled,false);}
 for(const item of panel.symbolLegends[0].items)assert.equal(item.mark.filled,false);
 assert.match(out.tex,/mark=o,mark size=2.25bp/);
 assert.doesNotMatch(out.tex,/mark=\*/);
});
test('Vega circle symbol size is squared diameter, not disk area',async()=>{
 const out=await run({...base,mark:{type:'point',filled:true,size:144}});
 assert.match(out.tex,/mark size=4.5bp/);
});
test('transparent constant fills do not become opaque black',async()=>{
 const out=await run({...base,mark:{type:'point',fill:'transparent'}});
 assert.equal(out.ir.panels[0].plots[0].style.fill,null);
 await assert.rejects(run({...base,mark:{type:'point',fill:'rgba(0,0,0,0.5)'}}),/explicit opacity/);
});

test('categorical default label orientation is retained without overriding explicit choices',async()=>{
 const s={data:{values:[{category:'One',v:2},{category:'Two',v:4}]},mark:'bar',encoding:{x:n('category'),y:q('v')}};
 const defaultOut=await run(s);assert.equal(defaultOut.ir.panels[0].axes.x.axis.labelAngle,270);
 assert.match(defaultOut.tex,/xticklabel style=\{rotate=-270,anchor=east\}/);
 s.encoding.x.axis={labelAngle:0};const flat=await run(s);assert.equal(flat.ir.panels[0].axes.x.axis.labelAngle,0);
 assert.doesNotMatch(flat.tex,/rotate=-270/);
 s.encoding.x.axis=null;assert.equal((await run(s)).ir.panels[0].axes.x.axis,null);
});

test('categorical y orientation follows official scales, including explicit reversal',async()=>{
 const spec={data:{values:[{g:'A',v:2},{g:'B',v:4}]},mark:'bar',encoding:{x:q('v'),y:n('g')}};
 const out=await run(spec);assert.deepEqual(out.ir.panels[0].axes.y.categories,['A','B']);assert.equal(out.ir.panels[0].axes.y.scale.reverse,true);
 spec.encoding.y.scale={reverse:true};assert.equal((await run(spec)).ir.panels[0].axes.y.scale.reverse,false);
 spec.mark={type:'errorbar',extent:'stderr'};spec.data.values=[{g:'A',v:2},{g:'A',v:3},{g:'B',v:4},{g:'B',v:5}];
 delete spec.encoding.y.scale;assert.equal((await run(spec)).ir.panels[0].axes.y.scale.reverse,true);
});
test('default and reversed quantitative color use the actual official mapping',async()=>{
 const input={data:{values:[{a:'A',b:'X',z:0},{a:'B',b:'Y',z:50}]},mark:'rect',encoding:{x:n('a'),y:n('b'),color:q('z')}};
 for(const reverse of [false,true]){
  input.encoding.color.scale={reverse};
  const expected=new vega.View(vega.parse(vl.compile(input).spec),{renderer:'none'});
  try{
   await expected.runAsync();const scale=expected.scale('color');
   const actual=(await run(input)).ir.panels[0].colorAxis;
   assert.equal(actual.range.length,256);
   for(const [i,value] of [[0,0],[127,50*127/255],[255,50]]){
    const rgb=vega.expressionFunction('rgb')(scale(value));
    assert.deepEqual(actual.range[i],[Math.round(rgb.r),Math.round(rgb.g),Math.round(rgb.b)]);
   }
  }finally{expected.finalize();}
 }
});
test('unfilled symbol color scales are found on stroke, not literal transparent fill',async()=>{
 const spec={...base,mark:'point',encoding:{...base.encoding,color:{...q('y'),scale:{range:['red','blue'],domain:[0,10]}}}};
 const out=await run(spec);assert.deepEqual(out.ir.panels[0].colorAxis.range[0],[255,0,0]);assert.deepEqual(out.ir.panels[0].colorAxis.range.at(-1),[0,0,255]);
});


// Composition, datum-dependent styling, and inferred-domain regressions.
const compositionPoint=(values,color=false)=>({data:{values},mark:'point',encoding:{x:q('x'),y:q('y'),...(color?{color:n('g')}:{})}});

test('concatenation shares the official numeric x domain when requested',async()=>{
 const input={hconcat:[compositionPoint([{x:1,y:1},{x:2,y:2}]),compositionPoint([{x:10,y:1},{x:20,y:2}])],resolve:{scale:{x:'shared'}}};
 const before=JSON.stringify(input),out=await run(input);
 assert.equal(JSON.stringify(input),before);
 assert.deepEqual(out.ir.panels.map(p=>p.axes.x.domain),[[0,20],[0,20]]);
 assert.deepEqual(out.ir.panels.map(p=>p.plots[0].points.map(v=>v[0])),[[1,2],[10,20]]);
});

test('concatenation shares categorical color mappings by default',async()=>{
 const out=await run({hconcat:[compositionPoint([{x:1,y:1,g:'B'}],true),compositionPoint([{x:10,y:2,g:'A'}],true)]});
 assert.deepEqual(out.ir.panels.map(p=>p.plots[0].style.stroke),[[245,133,24],[76,120,168]]);
});

test('faceted children inherit mark and axis configuration',async()=>{
 const input={data:{values:[{g:'a',x:1,y:2},{g:'b',x:2,y:3}]},facet:{column:{field:'g',type:'nominal'}},spec:{mark:'point',encoding:{x:q('x'),y:q('y')}},config:{point:{color:'red',size:144},axis:{labels:false}}};
 const {ir}=await run(input);
 assert.equal(ir.panels.length,2);
 for(const panel of ir.panels){
  assert.equal(panel.plots[0].style.size,144);
  assert.deepEqual(panel.plots[0].style.stroke,[255,0,0]);
  assert.equal(panel.axes.x.axis.labels,false);
 }
});

test('repeated children inherit mark and axis configuration',async()=>{
 const input={data:{values:[{x:1,z:2,y:3}]},repeat:['x','z'],spec:{mark:'point',encoding:{x:{field:{repeat:'repeat'},type:'quantitative'},y:q('y')}},config:{point:{color:'red',size:144},axis:{labels:false}}};
 const {ir}=await run(input);
 assert.equal(ir.panels.length,2);
 for(const panel of ir.panels){
  assert.equal(panel.plots[0].style.size,144);
  assert.deepEqual(panel.plots[0].style.stroke,[255,0,0]);
  assert.equal(panel.axes.x.axis.labels,false);
 }
});

test('data-driven text sizes retain the evaluated per-row font size',async()=>{
 const input={data:{values:[{x:1,y:1,t:'small',sz:10},{x:2,y:2,t:'big',sz:30}]},mark:'text',encoding:{x:q('x'),y:q('y'),text:n('t'),size:q('sz')}};
 const {ir,tex}=await run(input);
 const sizes=ir.panels[0].plots.map(p=>p.fontSize);
 assert.ok(Math.abs(sizes[0]-18.666666666666664)<1e-9);
 assert.ok(Math.abs(sizes[1]-40)<1e-9);
 const emitted=[...tex.matchAll(/font=\\fontsize\{([0-9.]+)bp\}/g)].map(x=>Number(x[1]));
 assert.equal(emitted.length,2);
 assert.ok(Math.abs(emitted[0]-14)<1e-9);
 assert.ok(Math.abs(emitted[1]-30)<1e-9);
});

test('inferred singleton and all-zero domains have positive extent',async()=>{
 for(const [y,zero] of [[5,false],[0,false],[0,undefined]]){
  const spec=compositionPoint([{x:1,y}]);spec.encoding.y.scale=zero===undefined?{}:{zero};
  const {ir,tex}=await run(spec),domain=ir.panels[0].axes.y.domain;
  assert.ok(domain[0]<y && domain[1]>y);
  assert.match(tex,/ymin=/);
  assert.match(tex,/ymax=/);
 }
});

test('explicit equal-bound domains remain rejected',async()=>{
 const spec=compositionPoint([{x:1,y:5}]);spec.encoding.y.scale={domain:[5,5]};
 await assert.rejects(run(spec),/domain must be increasing/);
});

test('shared categorical positions preserve data-category alignment',async()=>{
 const a=compositionPoint([{x:'A',y:1}]),b=compositionPoint([{x:'C',y:2}]);
 a.encoding.x=n('x');b.encoding.x=n('x');a.width=200;b.width=200;
 const {ir}=await run({hconcat:[a,b],resolve:{scale:{x:'shared'}}});
 assert.deepEqual(ir.panels.map(p=>p.axes.x.categories),[['A','C'],['A','C']]);
 assert.deepEqual(ir.panels.map(p=>p.plots[0].points[0][0]),[0,1]);
});

test('independent categorical color scales are not accidentally merged',async()=>{
 const {ir}=await run({hconcat:[compositionPoint([{x:1,y:1,g:'B'}],true),compositionPoint([{x:10,y:2,g:'A'}],true)],resolve:{scale:{color:'independent'}}});
 assert.deepEqual(ir.panels.map(p=>p.plots[0].style.stroke),[[76,120,168],[76,120,168]]);
});

test('independent position scales retain child-specific domains',async()=>{
 const {ir}=await run({hconcat:[compositionPoint([{x:1,y:1}]),compositionPoint([{x:20,y:1}])],resolve:{scale:{x:'independent'}}});
 assert.deepEqual(ir.panels.map(p=>p.axes.x.domain),[[0,1],[0,20]]);
});

test('inferred singleton logarithmic domain expands multiplicatively',async()=>{
 const spec=compositionPoint([{x:1,y:5}]);spec.encoding.y.scale={type:'log',zero:false};
 const {ir}=await run(spec),domain=ir.panels[0].axes.y.domain;
 assert.ok(domain[0]>0 && domain[0]<5 && domain[1]>5);
});

test('child mark configuration overrides facet defaults',async()=>{
 const input={data:{values:[{g:'a',x:1,y:2},{g:'b',x:2,y:3}]},facet:{column:{field:'g',type:'nominal'}},spec:{mark:{type:'point',color:'green',size:64},encoding:{x:q('x'),y:q('y')}},config:{point:{color:'red',size:144},axis:{labels:false}}};
 const {ir}=await run(input);
 for(const panel of ir.panels){
  assert.equal(panel.plots[0].style.size,64);
  assert.deepEqual(panel.plots[0].style.stroke,[0,128,0]);
  assert.equal(panel.axes.x.axis.labels,false);
 }
});

test('shared quantitative colors use one evaluated domain and palette',async()=>{
 const make=z=>({data:{values:[{x:1,y:1,z}]},mark:'circle',encoding:{x:q('x'),y:q('y'),color:q('z')}});
 const {ir}=await run({hconcat:[make(0),make(100)]});
 assert.deepEqual(ir.panels.map(p=>p.colorAxis.domain),[[0,100],[0,100]]);
 assert.deepEqual(ir.panels[0].colorAxis.range,ir.panels[1].colorAxis.range);
});

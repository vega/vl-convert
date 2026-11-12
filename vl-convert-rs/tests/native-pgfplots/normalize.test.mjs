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

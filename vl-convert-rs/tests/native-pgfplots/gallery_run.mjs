// Execute official reference and the production normalizer/emitter on each
// original specification. Nothing is rewritten merely to make a fixture pass.
import * as vega from '/npm/vega@6.4.0/+esm';
import * as vl from '/npm/vega-lite@6.4.3/+esm';
import fs from 'node:fs/promises';
import path from 'node:path';
import {createHash} from 'node:crypto';
import '../../src/js/native_pgfplots.js';
import '../../src/js/native_pgfplots_normalize.js';
const dir=path.resolve(process.argv[2] || 'gallery-audit');
const sources=JSON.parse(await fs.readFile(path.join(dir,'sources.json'),'utf8'));
const core=globalThis.vlcNativePgfplots, norm=globalThis.vlcPgfplotsNormalize;
const loader=vega.loader();
loader.load=async url=>{
 const d=sources.datasets[url];
 if(!d)throw new Error(`Unpinned dataset: ${url}`);
 const bytes=await fs.readFile(d.path);
 if(createHash('sha256').update(bytes).digest('hex')!==d.sha256)throw new Error(`Dataset hash mismatch: ${url}`);
 return bytes.toString('utf8');
};
const report=[];
for(const item of sources.examples){
 const row={...item};report.push(row);
 if(item.status!=='exported')continue;
 const caseDir=path.join(dir,item.id), spec=JSON.parse(await fs.readFile(path.join(caseDir,'original.vl.json'),'utf8'));
 try{core.compile(spec);row.baseline='compiled';}catch(e){row.baseline='unsupported';row.baseline_error=e.message;}
 try {
  const errors=[], warnings=[],logger=vega.logger(vega.Warn);
  logger.error=(...args)=>{errors.push(args.map(String).join(' '));return logger;};
  logger.warn=(...args)=>{warnings.push(args.map(String).join(' '));return logger;};
  const compiled=vl.compile(spec,{logger}).spec;
  const view=new vega.View(vega.parse(compiled),{renderer:'none',loader,logger});
  try{
   await view.runAsync();if(errors.length)throw new Error(errors.join('; '));
   await fs.writeFile(path.join(caseDir,'reference.svg'),await view.toSVG());
   await fs.writeFile(path.join(caseDir,'reference.vg.json'),JSON.stringify(compiled,null,2));
  }finally{view.finalize();}
  row.reference='rendered';row.reference_warnings=warnings;
 }catch(e){row.reference='failed';row.reference_error=e.message;}
 try{
  const result=await norm.prepare(spec,{vega,vl,loader});
  await fs.writeFile(path.join(caseDir,'normalized.vl.json'),JSON.stringify(result.spec,null,2));
  row.diagnostics=result.diagnostics;
  const ir=norm.decorate(core.lower(result.spec),vega);
  await fs.writeFile(path.join(caseDir,'native.ir.json'),JSON.stringify(ir,null,2));
  const tex=core.emit(ir,{standalone:true});
  await fs.writeFile(path.join(caseDir,'native.tex'),tex);
  row.status='tex_generated';row.latex='not_run';row.numerical_parity='not_reviewed';row.visual_review='not_reviewed';
 }catch(e){row.status=e.message.startsWith('PGFPlots:')?'unsupported':'compiler_error';row.error=e.stack;}
 console.log(item.id,row.status,row.error?.split('\n')[0] || '');
 await fs.writeFile(path.join(dir,'outcomes.json'),JSON.stringify({versions:{vega:vega.version,vegaLite:vl.version},examples:report},null,2));
}
await fs.writeFile(path.join(dir,'outcomes.json'),JSON.stringify({versions:{vega:vega.version,vegaLite:vl.version},examples:report},null,2));
console.log(report.reduce((out,r)=>(out[r.status]=(out[r.status]||0)+1,out),{}));

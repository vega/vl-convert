// Generate actual backend output for TeX/integration tests; not a second emitter.
const fs=require('node:fs'),path=require('node:path');
require('../../src/js/native_pgfplots.js');
const {fixtures}=require('./fixtures.cjs');
const dir=process.argv[2];if(!dir)throw new Error('usage: node generate.cjs OUTPUT_DIRECTORY');
fs.mkdirSync(dir,{recursive:true});
for(const [name,spec] of Object.entries(fixtures())){
 fs.writeFileSync(path.join(dir,name+'.vl.json'),JSON.stringify(spec,null,2)+'\n');
 fs.writeFileSync(path.join(dir,name+'.tex'),globalThis.vlcNativePgfplots.compile(spec,{standalone:true,tex_labels:name==='math'}));
 fs.writeFileSync(path.join(dir,name+'.ir.json'),JSON.stringify(globalThis.vlcNativePgfplots.lower(spec),null,2)+'\n');
}

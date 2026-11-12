// The exact official-dataflow normalizer and emitter embedded by Rust.
import * as vega from '/npm/vega@6.4.0/+esm';
import * as vl from '/npm/vega-lite@6.4.3/+esm';
import fs from 'node:fs/promises';
import path from 'node:path';
import fixtureModule from './fixtures.cjs';
import {normalizationFixtures} from './normalization-fixtures.mjs';
import '../../src/js/native_pgfplots.js';
import '../../src/js/native_pgfplots_normalize.js';
const dir=process.argv[2];if(!dir)throw new Error('An output directory is required');
await fs.mkdir(dir,{recursive:true});
for(const [name,spec] of Object.entries({...fixtureModule.fixtures(), ...normalizationFixtures})){
 const options={standalone:true,tex_labels:name==='math'};
 const result=await globalThis.vlcPgfplotsNormalize.prepare(spec,{vega,vl});
 const ir=globalThis.vlcPgfplotsNormalize.decorate(globalThis.vlcNativePgfplots.lower(result.spec),vega);
 await fs.writeFile(path.join(dir,name+'.vl.json'),JSON.stringify(spec,null,2)+'\n');
 await fs.writeFile(path.join(dir,name+'.tex'),globalThis.vlcNativePgfplots.emit(ir,options));
 await fs.writeFile(path.join(dir,name+'.ir.json'),JSON.stringify(ir,null,2)+'\n');
}

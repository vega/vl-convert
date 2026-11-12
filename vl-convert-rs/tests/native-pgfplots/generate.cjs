// Keep the rendering driver's stable entry point, but use production normalization.
const {spawnSync}=require('node:child_process');
const path=require('node:path');
const result=spawnSync(process.execPath,[
 '--no-warnings','--loader',path.join(__dirname,'vendor-loader.mjs'),
 path.join(__dirname,'generate-normalized.mjs'),...process.argv.slice(2),
],{stdio:'inherit',env:{...process.env,TZ:process.env.TZ || 'UTC'}});
if(result.error)throw result.error;
process.exit(result.status ?? 1);

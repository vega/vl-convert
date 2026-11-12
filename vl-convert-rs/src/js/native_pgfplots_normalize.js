// Static semantic normalization. Vega-Lite compiles the dataflow; Vega evaluates
// it. Never evaluate JavaScript supplied by a caller or infer datasets by names.
// Dependencies are supplied by the existing worker (and the vendored test loader).
globalThis.vlcPgfplotsNormalize = (() => {
  'use strict';
  const own = (o, k) => Object.prototype.hasOwnProperty.call(o, k);
  const copy = x => structuredClone(x);
  const compact = x => Array.isArray(x) ? x.map(compact) : x && typeof x === 'object' ? Object.fromEntries(Object.entries(x).filter(([,v])=>v!==undefined).map(([k,v])=>[k,compact(v)])) : x;
  const fail = (p, message) => { throw new Error(`PGFPlots: ${p}: ${message}`); };
  const MAX_ROWS = 100000, MAX_VIEWS = 64, MAX_DEPTH = 32;
  const scalar = x => x instanceof Date ? +x : x;
  const clean = rows => rows.map(row => Object.fromEntries(Object.entries(row).map(([k,v]) => [k, scalar(v)])));
  const same = (a,b) => JSON.stringify(a) === JSON.stringify(b);
  const marks = (list, roots, result = []) => {
    for (const m of list || []) {
      const source = roots || m.from?.data || m.from?.facet?.data;
      if (m.marks) marks(m.marks, source, result);
      else result.push({mark: m, data: roots || source});
    }
    return result;
  };
  function ancestry(spec, name, out = [], seen = new Set()) {
    if (seen.has(name)) fail('data', 'cyclic or ambiguous compiled dataflow');
    seen.add(name);
    const d = spec.data?.find(d => d.name === name);
    if (!d) fail('data', `compiled dataset ${name} is not at the supported scope`);
    if (d.source) {
      if (Array.isArray(d.source)) fail('data', 'multiple compiled sources require an explicit union');
      ancestry(spec, d.source, out, seen);
    }
    out.push(...(d.transform || []));
    return out;
  }
  function timeSignature(unit) {
    const u = typeof unit === 'string' ? unit : unit?.unit;
    if (typeof u !== 'string') fail('timeUnit','expected a named time unit');
    return u.replace(/^utc/,'');
  }
  function fieldMapping(def, flow, p) {
    let f = def.field, end;
    if (def.timeUnit) {
      const signature = timeSignature(def.timeUnit);
      const matches = flow.filter(t => t.type === 'timeunit' && t.field === f && t.units?.join('') === signature);
      if (matches.length !== 1) fail(p, 'cannot unambiguously map the compiled time-unit field');
      [f,end] = matches[0].as;
    }
    if (def.bin && def.bin !== 'binned') {
      const matches = flow.filter(t => t.type === 'bin' && t.field === f);
      if (matches.length !== 1) fail(p, 'cannot unambiguously map the compiled bin field');
      [f,end] = matches[0].as;
    }
    if (def.aggregate) {
      if (typeof def.aggregate !== 'string') fail(p,'object-valued aggregates are not yet supported');
      const matches = flow.flatMap(t => t.type === 'aggregate' ? t.ops.flatMap((op,i) =>
        op === def.aggregate && (op === 'count' || t.fields[i] === f) ? [t.as[i]] : []) : []);
      if (matches.length !== 1) fail(p, 'cannot unambiguously map the compiled aggregate field');
      f = matches[0];
    }
    return {field:f, end};
  }
  function noInteractive(spec, path = '$', depth = 0) {
    if (depth > MAX_DEPTH) fail(path,'specification nesting limit exceeded');
    if (!spec || typeof spec !== 'object' || Array.isArray(spec)) fail(path,'expected a chart specification');
    if (spec.params?.length || spec.selection || spec.projection) fail(path,'interactive and geographic specifications are outside the native Cartesian target');
    for (const op of ['layer','hconcat','vconcat','concat']) if (spec[op]) spec[op].forEach((c,i)=>noInteractive(c,`${path}.${op}[${i}]`,depth+1));
    if (spec.spec) noInteractive(spec.spec,path+'.spec',depth+1);
  }
  async function prepare(input, env) {
    const {vega, vl, loader, logger} = env;
    if (!vega?.View || !vl?.compile) fail('runtime','official Vega and Vega-Lite are required');
    noInteractive(input);
    const source = copy(input), datasets = source.datasets || {}, diagnostics = [];
    let views = 0, rowBudget = 0;
    const warn = message => { diagnostics.push(message); logger?.warn(message); };
    const denyLoader = {load: async () => { throw new Error('PGFPlots: external data requires the configured resource loader'); }};
    async function evaluate(spec) {
      if (++views > 512) fail('data','normalization evaluation limit exceeded');
      const errors = [];
      const log = vega.logger(vega.Warn);
      log.error = (...x) => { errors.push(x.map(String).join(' ')); return log; };
      log.warn = (...x) => { warn(x.map(String).join(' ')); return log; };
      // ARIA descriptions are browser-only. Vega-Lite builds them as expression
      // strings from labels; literal U+2028/U+2029 can break Vega's parser.
      // Disable only those probe descriptions, not the caller's labels/data.
      const compiled = vl.compile({...spec, name:'__pgf_probe',
        config:{...spec.config, aria:false}}, {logger:log}).spec;
      const baseLoader=loader || denyLoader, guardedLoader=Object.create(baseLoader);
      guardedLoader.load=async (...args)=>{try{return await baseLoader.load(...args);}catch(error){errors.push(String(error.message || error));throw error;}};
      const runtime = new vega.View(vega.parse(compiled), {renderer:'none', loader:guardedLoader, logger:log});
      try {
        await runtime.runAsync();
        if (errors.length) fail('data', errors.join('; '));
        const tables = new Map();
        for (const m of marks(compiled.marks)) {
          if (!m.data || tables.has(m.data)) continue;
          const rows = runtime.data(m.data);
          if (!Array.isArray(rows) || rows.length > MAX_ROWS) fail('data','evaluated row limit exceeded');
          rowBudget += rows.length;
          if (rowBudget > 2000000) fail('data','total evaluated row limit exceeded');
          tables.set(m.data, clean(rows));
        }
        const scales = Object.fromEntries((compiled.scales || []).map(s => {
          const scale=runtime.scale(s.name);
          return [s.name,{...s,domain:scale.domain().map(scalar),range:scale.range(),ticks:typeof scale.ticks==='function'?scale.ticks(5).map(scalar):scale.domain().map(scalar),legendValues:(typeof scale.ticks==='function'?scale.ticks(4):scale.domain()).map(v=>({value:scalar(v),mapped:scale(v)}))}];
        }));
        const painted=new Map();
        function visit(node) {
          if(node.datum && node.mark?.role==='mark') {
            const style=Object.fromEntries(['fill','stroke','size','shape','strokeWidth','opacity','fillOpacity','strokeOpacity','strokeDash','text','fontSize','align','baseline','angle'].filter(k=>node[k]!==undefined).map(k=>[k,node[k]]));
            if(!painted.has(node.datum))painted.set(node.datum,[]);
            painted.get(node.datum).push({type:node.mark.marktype,style});
          }
          for(const child of node.items || [])visit(child);
        }
        visit(runtime.scenegraph().root);
        const styles=new Map();
        for(const name of tables.keys())styles.set(name,runtime.data(name).map(row=>painted.get(row)||[]));
        return {compiled, tables, scales, styles, width:runtime.width(),height:runtime.height()};
      } finally { runtime.finalize(); }
    }
    async function dataRows(data, transforms, inherited) {
      if (data === undefined && !transforms?.length) return inherited || [{}];
      const d = data === undefined ? {values:inherited || [{}]} : data;
      const result = await evaluate({datasets,data:d,transform:transforms,mark:'point'});
      if (result.tables.size !== 1) fail('data','expected one evaluated output for data materialization');
      return [...result.tables.values()][0];
    }
    function configuration(config) {
      if(!config)return undefined;
      const c=copy(config);
      for(const key of ['axis','axisX','axisY','mark','line','point','circle','square','area','bar','rule','tick','text','errorbar','errorband','boxplot','legend'])delete c[key];
      if(c.view?.strokeWidth===0){c.view.stroke=null;delete c.view.strokeWidth;}
      if(c.view?.step!==undefined)delete c.view.step; // resolved per-view dimensions below
      return c;
    }
    function css(value,p) {
      if(value==null || value==='none')return value;
      if(typeof value!=='string')fail(p,'non-solid paint is unsupported');
      const rgb=vega.expressionFunction('rgb')(value);
      if(!Number.isFinite(rgb.r))fail(p,'invalid color');
      return rgb.formatHex();
    }
    async function unit(spec, rows, encoding, p) {
      const type = typeof spec.mark === 'string' ? spec.mark : spec.mark?.type;
      const enc = copy(encoding || {});
      for(const d of Object.values(enc))if(d && !Array.isArray(d) && d.datum!==undefined && !d.type)d.type=typeof d.datum==='number'?'quantitative':d.datum && typeof d.datum==='object'?'temporal':'nominal';
      if(['errorbar','errorband'].includes(type) && !enc.x2 && !enc.y2 && !enc.xError && !enc.yError && (spec.mark?.extent==='ci' || spec.mark?.extent===undefined)) {
        const normalized=vl.compile({...spec,data:{values:rows},encoding:enc},{logger}).normalized;
        return walk(normalized,{},p+'.uncertainty');
      }
      if(typeof spec.mark==='object' && (spec.mark.point && typeof spec.mark.point==='object' || spec.mark.line)) {
        const normalized=vl.compile({...spec,data:{values:rows},encoding:enc},{logger}).normalized;
        return walk(normalized,{},p+'.overlay');
      }
      // Channels ignored by Vega-Lite for this mark are not semantic layers.
      if (type !== 'text') delete enc.text;
      if (enc.tooltip) { delete enc.tooltip; warn(`${p}.encoding.tooltip: omitted in static output`); }
      const m = {...(spec.config?.mark || {}),...(spec.config?.[type] || {}),...(typeof spec.mark==='string'?{type}:copy(spec.mark))};
      for(const c of ['color','fill','stroke'])if(typeof m[c]==='string')m[c]=css(m[c],p);
      for(const c of ['x','y'])if(enc[c] && enc[c].axis!==null) {
        const a={...(spec.config?.axis || {}),...(spec.config?.[c==='x'?'axisX':'axisY'] || {}),...(enc[c].axis || {})};
        for(const key of ['titleColor','labelColor','gridColor','domainColor','tickColor'])if(typeof a[key]==='string')a[key]=css(a[key],p);
        if(Object.keys(a).length)enc[c].axis=a;
      }
      if(enc.color?.legend!==null && spec.config?.legend)enc.color={...enc.color,legend:{...spec.config.legend,...enc.color?.legend}};

      for(const c of ['size','shape','stroke','fill','strokeWidth'])if(enc[c]?.value!==undefined){m[c]=enc[c].value;delete enc[c];}
      for(const c of ['x','y'])if(enc[c]?.stack===false)enc[c].stack=null;
      if (!m) fail(p,'missing mark');
      for(const k of ['aria','ariaRoleDescription','description'])if(k in m){delete m[k];warn(`${p}.mark.${k}: accessibility metadata is not represented in the TeX fragment`);}
      if(m.style && ['errorband-band','errorbar-ticks','errorbar-rule'].includes(m.style))delete m.style;
      if (m.tooltip !== undefined) {delete m.tooltip;warn(`${p}.mark.tooltip: omitted in static output`);}
      if (!['line','point','circle','square','bar','area','rect','rule','text','tick','errorbar','errorband','boxplot'].includes(type)) fail(p,`unsupported mark ${type}`);
      // The semantic emitter keeps native boxplot/errorbar commands. For these
      // already-supported composites retain observations after view transforms.
      // Encoding-level transforms are evaluated below for primitive marks.
      const composite = ['boxplot','errorbar','errorband'].includes(type);
      if (composite && !Object.values(enc).some(d=>d?.timeUnit || d?.bin || d?.aggregate || d?.type==='temporal')) {
        return {...spec,config:configuration(spec.config),data:{values:rows},transform:undefined,mark:m,encoding:enc};
      }
      const result = await evaluate({data:{values:rows}, mark:m, encoding:enc, config:spec.config});
      if(spec.width && typeof spec.width==='object' || spec.config?.view?.step!==undefined)spec.width=result.width;
      if(spec.height && typeof spec.height==='object' || spec.config?.view?.step!==undefined)spec.height=result.height;
      const wanted = {line:'line',point:'symbol',circle:'symbol',square:'symbol',bar:'rect',area:'area',rect:'rect',rule:'rule',text:'text',tick:'rect'}[type];
      if (!wanted) fail(p,'encoding transforms on composite marks must currently be materialized explicitly');
      const terminal = marks(result.compiled.marks).filter(x=>x.mark.type===wanted);
      const refs = [...new Set(terminal.map(x=>x.data))];
      if (refs.length !== 1) fail(p,'ambiguous compiled mark/data mapping');
      const flow = ancestry(result.compiled,refs[0]);
      const values = result.tables.get(refs[0]);
      const compiledMark=terminal[0].mark;
      const update=compiledMark.encode?.update || {};
      const scaleFor=channel=>{
        const property=({color:m.filled===false||type==='line'?'stroke':'fill'})[channel]||channel;
        const ref=update[property] || (channel==='x'?update.xc:channel==='y'?update.yc:null);
        const name=Array.isArray(ref)?ref.find(r=>r.scale)?.scale:ref?.scale;
        return name?result.scales[name]:null;
      };
      // Resolve visual channels using the evaluated official marks, never pixel
      // coordinates. Native axes still consume source-domain data below.
      const styled=['size','shape','opacity','fill','stroke','strokeWidth'].filter(c=>enc[c] && (enc[c].field || enc[c].aggregate || enc[c].condition));
      if(enc.color?.condition || (enc.color?.type==='quantitative' && !['rect','point','circle','square'].includes(type)) || (m.color && typeof m.color==='object' && m.color.expr))styled.push('color');
      if(styled.length) {
        if(!['point','circle','square','bar','rule','text','tick'].includes(type))fail(p,'datum-dependent styles on continuous paths are unsupported');
        const key='__pgf_style';if(values.some(r=>own(r,key)))fail(p,'reserved style field collision');
        for(let i=0;i<values.length;i++) {
          const visual=result.styles.get(refs[0])[i].find(x=>x.type===wanted);
          if(!visual)fail(p,'evaluated visual style cannot be mapped to a data row');
          values[i][key]=copy(visual.style);
          for(const c of ['fill','stroke'])if(values[i][key][c]!=null && values[i][key][c]!=='none'){const color=vega.expressionFunction('rgb')(values[i][key][c]);if(!color)fail(p,'non-solid paint is unsupported');values[i][key][c]=color.formatHex();}
        }
        m._styleField=key;
        m._symbolLegends=[];
        for(const c of ['size','shape'])if(enc[c] && enc[c].legend!==null) {
          const scale=scaleFor(c);
          if(scale)m._symbolLegends.push({title:enc[c].title || enc[c].field,items:scale.legendValues.map(v=>({label:v.value,mark:{[c]:v.mapped}}))});
        }
        for(const c of styled)delete enc[c];
        if(m.color?.expr)delete m.color;
      }
      if(enc.text?.format) {
        const key='__pgf_text';if(values.some(r=>own(r,key)))fail(p,'reserved text field collision');
        for(let i=0;i<values.length;i++)values[i][key]=result.styles.get(refs[0])[i].find(x=>x.type==='text')?.style.text ?? '';
        enc.text={field:key,type:'nominal'};
      }
      for(const c of ['x','y'])if(enc[c]?.datum && typeof enc[c].datum==='object') {
        const temporal=enc[c].type==='temporal';
        if(!temporal)fail(p,'object-valued positional datum is unsupported');
        const expression=compiledMark.encode?.update?.[c];
        if(!expression?.signal || !expression.scale)fail(p,'cannot map temporal datum');
        // The compiler supplies a datetime expression; use Vega's date helper
        // with the documented DateTime fields, never JavaScript evaluation.
        const d=enc[c].datum,month=typeof d.month==='string'?['january','february','march','april','may','june','july','august','september','october','november','december'].indexOf(d.month.toLowerCase()):(d.month||1)-1;
        if(month<0)fail(p,'invalid month name');
        enc[c].datum=d.utc?Date.UTC(d.year??2012,month,d.date??1,d.hours||0,d.minutes||0,d.seconds||0,d.milliseconds||0):+new Date(d.year??2012,month,d.date??1,d.hours||0,d.minutes||0,d.seconds||0,d.milliseconds||0);
        enc[c].type='quantitative';enc[c].axis={...(enc[c].axis||{}),_time:{utc:!!d.utc,format:enc[c].axis?.format||null}};
      }
      for (const [channel,def] of Object.entries(enc)) {
        if (Array.isArray(def)) fail(`${p}.${channel}`,'array-valued channel definitions are not yet supported');
        if (!def || typeof def !== 'object') fail(p,'invalid channel definition');
        if (!def.field && !def.aggregate) continue;
        const mapped = fieldMapping(def,flow,`${p}.${channel}`);
        const originalTitle=own(def,'title') ? def.title : def.aggregate ? `${def.aggregate}(${def.field || 'rows'})` : def.field;
        def.field=mapped.field;
        if(def.type==='quantitative')for(const r of values)if(r[mapped.field]!=null)r[mapped.field]=vega.toNumber(r[mapped.field]);
        delete def.aggregate; delete def.bin; delete def.timeUnit;
        if (['x','y','color','size','shape'].includes(channel)) def.title=originalTitle;
        if (mapped.end && ['x','y'].includes(channel) && values.every(r=>own(r,mapped.end))) {
          if (['bar','rect'].includes(type)) enc[channel+'2']={field:mapped.end};
          else if(encoding[channel].bin) {
            const mid=`__pgf_${channel}_midpoint`;
            if(values.some(r=>own(r,mid)))fail(p,'reserved midpoint field collision');
            for(const row of values)row[mid]=(row[mapped.field]+row[mapped.end])/2;
            def.field=mid;
          }
        }
        if (encoding[channel].type === 'temporal' || encoding[channel].timeUnit) {
          const temporal = encoding[channel];
          def.type = temporal.type === 'temporal' ? 'quantitative' : temporal.type;
          const time={unit:temporal.timeUnit || null,utc:temporal.scale?.type === 'utc' || /^utc/.test(typeof temporal.timeUnit==='string'?temporal.timeUnit:temporal.timeUnit?.unit || ''),format:temporal.axis?.format || null};
          if (['x','y'].includes(channel)) {
            def.scale={...(def.scale||{}),zero:false};
            if (['time','utc'].includes(def.scale.type)) delete def.scale.type;
            if(def.scale.domain)def.scale.domain=def.scale.domain.map(v=>+vega.toDate(v));
            def.axis=def.axis === null ? null : {...(def.axis||{}),_time:time};
            if(def.axis)delete def.axis.format;
          }
        }
      }
      for(const c of ['x','y','color'])if(enc[c]?.field) {
        const scale=scaleFor(c);
        if(!scale)continue;
        if(c==='color') {
          if(['nominal','ordinal'].includes(enc[c].type))enc[c].scale={type:'ordinal',domain:scale.domain,range:scale.range.map(v=>vega.expressionFunction('rgb')(v).formatHex())};
          else if(enc[c].scale?.scheme && typeof enc[c].scale.scheme==='string') {
            const sampler=vega.scheme(enc[c].scale.scheme);
            if(typeof sampler!=='function')fail(p,'unsupported quantitative color scheme');
            enc[c].scale={type:'linear',domain:[scale.domain[0],scale.domain.at(-1)],range:Array.from({length:16},(_,i)=>vega.expressionFunction('rgb')(sampler(i/15)).formatHex())};
          }
        } else {
          if(!enc[c].type)enc[c].type=['band','point','ordinal'].includes(scale.type)?'nominal':'quantitative';
          enc[c].scale={...(enc[c].scale||{}),domain:scale.domain};
          if(enc[c].sort!==undefined)delete enc[c].sort;
          for(const k of ['padding','domainMin','domainMax','nice'])delete enc[c].scale[k];
        }
      }
      if(type==='tick') {
        const markItems=result.styles.get(refs[0]);
        m.size ??= 20;m.thickness ??= 1;
        m.orient ??= enc.x?.type==='quantitative'?'vertical':'horizontal';
        for(const c of ['x','y'])if(!enc[c])enc[c]={datum:0,type:'quantitative',scale:{domain:[-.5,.5]},axis:null};
      }
      if(type==='rule' && m.size!==undefined){m.strokeWidth ??= m.size;delete m.size;}
      if(type==='text' && m.size!==undefined){m.fontSize ??= m.size;delete m.size;}
      for(const c of ['x','y'])if(enc[c]){delete enc[c].stack;if(['bar','area'].includes(type))enc[c].stack=null;}
      // Keep the official stack boundaries; never stack the same data twice.
      const stack=flow.filter(t=>t.type==='stack').at(-1);
      if (stack) {
        const axis=['x','y'].find(c=>enc[c]?.field===stack.field);
        if(!axis)fail(p,'compiled stack cannot be mapped to a measure channel');
        if(type==='text') {
          const f='__pgf_stack_mid';if(values.some(r=>own(r,f)))fail(p,'reserved stack field collision');
          for(const r of values)r[f]=(r[stack.as[0]]+r[stack.as[1]])/2;
          enc[axis]={...enc[axis],field:f,stack:null};
        }else {
          enc[axis]={...enc[axis],field:stack.as[1],stack:null};
          enc[axis+'2']={field:stack.as[0]};
        }
        delete enc.order;
      }
      // Vega field paths are evaluated by Vega, then given collision-free flat
      // aliases for the native emitter. A literal dot is never confused with a path.
      let alias=0;
      for(const d of Object.values(enc))if(d?.field && /[.\[\]\\]/.test(d.field)) {
        const accessor=vega.field(d.field),name=`__pgf_flat_${alias++}`;
        if(values.some(r=>own(r,name)))fail(p,'reserved flat-field collision');
        for(const r of values)r[name]=accessor(r);
        d.field=name;
      }
      return {...spec,config:configuration(spec.config),data:{values},transform:undefined,mark:m,encoding:enc};
    }
    async function resolveShared(out, original, rows, channels) {
      const reference=await evaluate({...original,data:{values:rows},transform:undefined,datasets});
      const leaf = s=>s.layer?s.layer.flatMap(leaf):s.concat?s.concat.flatMap(leaf):s.hconcat?s.hconcat.flatMap(leaf):s.vconcat?s.vconcat.flatMap(leaf):[s];
      const leaves=leaf(out);
      for(const channel of channels) {
        const properties=channel==='color'?['fill','stroke']:[channel,channel+'c'];
        const names=new Set();
        for(const {mark} of marks(reference.compiled.marks))for(const property of properties){
          const ref=mark.encode?.update?.[property];
          for(const r of Array.isArray(ref)?ref:[ref])if(r?.scale && reference.scales[r.scale])names.add(r.scale);
        }
        if(names.size!==1)continue;
        const scale=reference.scales[[...names][0]];
        for(const l of leaves) {
          const d=l.encoding?.[channel];if(!d)continue;
          if(channel==='color') {
            if(d.field && ['nominal','ordinal'].includes(d.type))d.scale={type:'ordinal',domain:scale.domain,range:scale.range.map(v=>vega.expressionFunction('rgb')(v).formatHex())};
          }else d.scale={...(d.scale||{}),domain:scale.domain};
        }
      }
      return out;
    }
    async function walk(spec, inherited={}, p='$', depth=0) {
      if(depth>MAX_DEPTH)fail(p,'composition nesting limit exceeded');
      spec={...spec,...(spec.config || inherited.config ? {config:{...(inherited.config||{}),...(spec.config||{})}} : {})};
      const rows=await dataRows(spec.data,spec.transform,inherited.rows);
      const enc={...(inherited.enc||{}),...(spec.encoding||{})};
      const out={...spec,data:{values:rows}}; delete out.datasets; delete out.transform;
      if(spec.config)out.config=configuration(spec.config);
      if (spec.layer) {
        out.layer=[];delete out.encoding;
        for(let i=0;i<spec.layer.length;i++)out.layer.push(await walk(spec.layer[i],{rows,enc,config:spec.config},`${p}.layer[${i}]`,depth+1));
        delete out.data;
        const leaf = s=>s.layer?s.layer.flatMap(leaf):[s];
        const leaves=out.layer.flatMap(leaf);
        const titles=leaves.map(s=>s.title).filter(x=>x!=null);
        if(titles.length && !out.title)out.title=titles[0];
        if(titles.some(t=>!same(t,out.title)))fail(p,'incompatible layer titles');
        for(const l of leaves){delete l.title;delete l.config;}
        const prune=l=>{if(l.layer){delete l.config;l.layer.forEach(prune);}};out.layer.forEach(prune);
        for(const c of ['x','y']) {
          if(spec.resolve?.scale?.[c]==='independent')continue;
          const defs=leaves.map(l=>l.encoding?.[c]).filter(Boolean);
          const explicit=spec.layer.map(l=>l.encoding?.[c]?.scale).filter(Boolean);
          const settings=Object.assign({},...explicit);
          const numeric=defs.every(d=>d.type==='quantitative');
          const domains=defs.flatMap(d=>d.scale?.domain||[]);
          const domain=settings.domain || (domains.length?(numeric?[Math.min(...domains),Math.max(...domains)]:[...new Set(domains)]):undefined);
          for(const d of defs)d.scale={...settings,...(domain?{domain}:{}),...(defs[0]?.scale?.type?{type:defs[0].scale.type}:{})};
          // Undefined means default axes, not hidden axes. Only explicit null hides.
          const configs=defs.map(d=>d.axis).filter(a=>a!==null);
          const merged=Object.assign({},...configs);
          for(const d of defs)d.axis=configs.length?merged:null;
        }
        return resolveShared(out,{...spec,encoding:enc},rows,['x','y','color'].filter(c=>spec.resolve?.scale?.[c]!=='independent'));
      }
      for(const op of ['concat','hconcat','vconcat'])if(spec[op]) {
        out[op]=[];
        for(let i=0;i<spec[op].length;i++)out[op].push(await walk(spec[op][i],{rows,enc,config:spec.config},`${p}.${op}[${i}]`,depth+1));
        delete out.data;return out;
      }
      if(spec.facet || spec.repeat || enc.row || enc.column || enc.facet) {
        // Preserve partitions: transforms inside a facet must run separately.
        return await panels(out,rows,enc,p,depth);
      }
      return unit({...out,config:spec.config},rows,enc,p);
    }
    async function panels(spec,rows,enc,p,depth) {
      if(spec.repeat) {
        const r=spec.repeat,one=Array.isArray(r),rv=one?[null]:r.row||[null],cv=one?r:r.column||[null];
        if(r.layer) {
          const replace=(o,b)=>Array.isArray(o)?o.map(x=>replace(x,b)):o&&typeof o==='object'?own(o,'repeat')?b[o.repeat]:Object.fromEntries(Object.entries(o).map(([k,v])=>[k,replace(v,b)])):o;
          return walk({layer:r.layer.map(layer=>replace(spec.spec,{layer})),data:{values:rows},config:spec.config}, {},p+'.repeat',depth+1);
        }
        if(rv.length*cv.length>MAX_VIEWS)fail(p,'repeat panel limit exceeded');
        const replace=(o,b)=>Array.isArray(o)?o.map(x=>replace(x,b)):o&&typeof o==='object'?own(o,'repeat')?b[o.repeat]:Object.fromEntries(Object.entries(o).map(([k,v])=>[k,replace(v,b)])):o;
        const list=[];
        for(const row of rv)for(const column of cv)list.push(await walk(replace(spec.spec,{row,column,repeat:column}),{rows},p+'.spec',depth+1));
        const out={concat:list,columns:one?(spec.columns||cv.length):cv.length,resolve:spec.resolve,title:spec.title,config:spec.config};
        return out;
      }
      let facet=spec.facet,child=spec.spec;
      if(!facet) {
        facet=enc.facet || {row:enc.row,column:enc.column};
        child={...spec,encoding:{...enc}};
        for(const c of ['row','column','facet'])delete child.encoding[c];
        delete child.data;
      }
      const one=own(facet,'field'),rd=one?null:facet.row,cd=one?facet:facet.column;
      function values(def) {
        if(!def)return [null];
        if(def.timeUnit || def.bin)fail(p,'facet field transforms require explicit materialization');
        const v=[...new Map(rows.map(r=>[JSON.stringify(r[def.field]),r[def.field]])).values()];
        if(Array.isArray(def.sort))return [...def.sort,...v.filter(x=>!def.sort.includes(x))];
        if(def.sort!==null)v.sort((a,b)=>typeof a==='number'&&typeof b==='number'?a-b:String(a).localeCompare(String(b)));
        return def.sort==='descending'?v.reverse():v;
      }
      const rv=values(rd),cv=values(cd);
      if(rv.length*cv.length>MAX_VIEWS)fail(p,'facet panel limit exceeded');
      const list=[];
      for(const r of rv)for(const c of cv) {
        const subset=rows.filter(x=>(!rd || same(x[rd.field],r))&&(!cd || same(x[cd.field],c)));
        const panel=await walk(child,{rows:subset},p+'.spec',depth+1);
        panel.title=[rd?`${rd.title??rd.field}: ${r}`:null,cd?`${cd.title??cd.field}: ${c}`:null].filter(Boolean).join(' / ');
        list.push(panel);
      }
      return resolveShared({concat:list,columns:one?(spec.columns || facet.columns || cv.length):cv.length,title:spec.title,config:spec.config,resolve:{scale:{x:'shared',y:'shared',color:'shared',...(spec.resolve?.scale||{})}}},spec,rows,['x','y','color'].filter(c=>spec.resolve?.scale?.[c]!=='independent'));
    }
    const prepared=compact(await walk(source));
    return {spec:prepared,diagnostics,versions:{vega:vega.version,vegaLite:vl.version}};
  }
  function curves(points,panel,vega,horizontal) {
    const sx=vega.scale(panel.axes.x.kind==='log'?'log':'linear')().domain(panel.axes.x.domain).range([0,panel.width]);
    const sy=vega.scale(panel.axes.y.kind==='log'?'log':'linear')().domain(panel.axes.y.domain).range([panel.height,0]);
    const output=[],convert=(x,y)=>[sx.invert(x),sy.invert(y)];
    const context={moveTo:(x,y)=>output.push(['M',...convert(x,y)]),lineTo:(x,y)=>output.push(['L',...convert(x,y)]),bezierCurveTo:(...p)=>output.push(['C',...convert(p[0],p[1]),...convert(p[2],p[3]),...convert(p[4],p[5])]),closePath:()=>output.push(['Z'])};
    let curve;
    for(const pt of [...points,null]) {
      if(pt){if(!curve){curve=vega.pathCurves('monotone',horizontal?'horizontal':'vertical')(context);curve.lineStart();}curve.point(sx(pt[0]),sy(pt[1]));}
      else if(curve){curve.lineEnd();curve=null;}
    }
    return output;
  }
  function decorate(figure,vega) {
    const panels=figure.panels.flatMap(p=>p.secondary?[p,p.secondary]:[p]);
    for(const p of panels)for(const plot of p.plots)if(plot.mark.interpolate==='monotone') {
      if(plot.type==='line')plot.curve=curves(plot.points,p,vega,plot.horizontal);
      else if(plot.type==='band'){
        plot.curvedBands=[];let lo=[],hi=[];
        const flush=()=>{if(lo.length){plot.curvedBands.push({lo:curves(lo,p,vega,plot.horizontal),hi:curves(hi,p,vega,plot.horizontal)});lo=[];hi=[];}};
        for(let i=0;i<plot.lower.length;i++)if(plot.lower[i]&&plot.upper[i]){lo.push(plot.lower[i]);hi.push(plot.upper[i]);}else flush();flush();
      }
    }
    for(const p of panels)for(const c of ['x','y']) {
      const axis=p.axes[c], t=axis.axis?._time;
      if(!t) {
        if(axis.axis?.format) {
          const format=vega.defaultLocale().format(axis.axis.format);
          const ticks=axis.axis.values || (axis.kind==='category'?axis.categories:vega.scale(axis.kind==='log'?'log':'linear')().domain(axis.domain).ticks(5));
          axis.axis.values=ticks;axis.axis.tickLabels=ticks.map(format);delete axis.axis.format;
        }
        continue;
      }
      let format=t.format;
      if(!format && t.unit) {
        const s=timeSignature(t.unit);
        format=s==='month'?'%b':s==='day'?'%a':s==='year'?'%Y':s==='date'?'%d':s.includes('month')?'%b %Y':'%Y-%m-%d';
      }
      format ||= '%Y-%m-%d';
      const locale=vega.defaultLocale();
      const formatter=t.utc?locale.utcFormat:locale.timeFormat;
      const formatDate=formatter(format);
      const ticks=axis.kind==='category'?axis.categories:vega.scale(t.utc?'utc':'time')().domain(axis.domain.map(x=>new Date(x))).ticks(5).map(Number);
      axis.axis.values=ticks;
      axis.axis.tickLabels=ticks.map(x=>formatDate(new Date(x)));
    }
    return figure;
  }
  return Object.freeze({prepare,decorate});
})();

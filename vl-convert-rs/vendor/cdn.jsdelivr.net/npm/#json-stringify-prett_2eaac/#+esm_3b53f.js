/**
 * Bundled by jsDelivr using Rollup v4.62.2 and esbuild v0.28.1.
 * Original file: /npm/json-stringify-pretty-compact@4.0.0/index.js
 *
 * Do NOT use SRI with dynamically generated files! More information: https://www.jsdelivr.com/using-sri-with-dynamic-files
 */
const p=/("(?:[^\\"]|\\.)*")|[:,]/g;function k(J,f={}){const l=JSON.stringify([1],void 0,f.indent===void 0?2:f.indent).slice(2,-3),N=l===""?1/0:f.maxLength===void 0?80:f.maxLength;let{replacer:c}=f;return(function a(n,u,S){n&&typeof n.toJSON=="function"&&(n=n.toJSON());const t=JSON.stringify(n,c);if(t===void 0)return t;const h=N-u.length-S;if(t.length<=h){const i=t.replace(p,(r,e)=>e||`${r} `);if(i.length<=h)return i}if(c!=null&&(n=JSON.parse(t),c=void 0),typeof n=="object"&&n!==null){const i=u+l,r=[];let e=0,g,d;if(Array.isArray(n)){g="[",d="]";const{length:s}=n;for(;e<s;e++)r.push(a(n[e],i,e===s-1?0:1)||"null")}else{g="{",d="}";const s=Object.keys(n),{length:y}=s;for(;e<y;e++){const O=s[e],o=`${JSON.stringify(O)}: `,x=a(n[O],i,o.length+(e===y-1?0:1));x!==void 0&&r.push(o+x)}}if(r.length>0)return[g,l+r.join(`,
${i}`),d].join(`
${u}`)}return t})(J,"",0)}export{k as default};
//# sourceMappingURL=/sm/e4cc14757810d5c9c492f17c7cc71887c1340dc929a0a0942ad1ed252e58e272.map
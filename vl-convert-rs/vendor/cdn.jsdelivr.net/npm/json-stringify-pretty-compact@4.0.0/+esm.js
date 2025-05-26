/**
 * Bundled by jsDelivr using Rollup v2.79.2 and Terser v5.39.0.
 * Original file: /npm/json-stringify-pretty-compact@4.0.0/index.js
 *
 * Do NOT use SRI with dynamically generated files! More information: https://www.jsdelivr.com/using-sri-with-dynamic-files
 */
const n=/("(?:[^\\"]|\\.)*")|[:,]/g;function t(t,e={}){const o=JSON.stringify([1],void 0,void 0===e.indent?2:e.indent).slice(2,-3),i=""===o?1/0:void 0===e.maxLength?80:e.maxLength;let{replacer:r}=e;return function t(e,l,s){e&&"function"==typeof e.toJSON&&(e=e.toJSON());const c=JSON.stringify(e,r);if(void 0===c)return c;const f=i-l.length-s;if(c.length<=f){const t=c.replace(n,((n,t)=>t||`${n} `));if(t.length<=f)return t}if(null!=r&&(e=JSON.parse(c),r=void 0),"object"==typeof e&&null!==e){const n=l+o,i=[];let r,s,c=0;if(Array.isArray(e)){r="[",s="]";const{length:o}=e;for(;c<o;c++)i.push(t(e[c],n,c===o-1?0:1)||"null")}else{r="{",s="}";const o=Object.keys(e),{length:l}=o;for(;c<l;c++){const r=o[c],s=`${JSON.stringify(r)}: `,f=t(e[r],n,s.length+(c===l-1?0:1));void 0!==f&&i.push(s+f)}}if(i.length>0)return[r,o+i.join(`,\n${n}`),s].join(`\n${l}`)}return c}(t,"",0)}export{t as default};
//# sourceMappingURL=/sm/41daf328ec920d0650f5542c9393fa04984ee5b2c1cb22e4a4532195c469d5f5.map
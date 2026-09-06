/**
 * Bundled by jsDelivr using Rollup v4.62.2 and esbuild v0.28.1.
 * Original file: /npm/fast-json-stable-stringify@2.1.0/index.js
 *
 * Do NOT use SRI with dynamically generated files! More information: https://www.jsdelivr.com/using-sri-with-dynamic-files
 */
var c,_;function g(){return _||(_=1,c=function(v,f){f||(f={}),typeof f=="function"&&(f={cmp:f});var S=typeof f.cycles=="boolean"?f.cycles:!1,l=f.cmp&&(function(n){return function(r){return function(t,i){var a={key:t,value:r[t]},e={key:i,value:r[i]};return n(a,e)}}})(f.cmp),u=[];return(function n(r){if(r&&r.toJSON&&typeof r.toJSON=="function"&&(r=r.toJSON()),r!==void 0){if(typeof r=="number")return isFinite(r)?""+r:"null";if(typeof r!="object")return JSON.stringify(r);var t,i;if(Array.isArray(r)){for(i="[",t=0;t<r.length;t++)t&&(i+=","),i+=n(r[t])||"null";return i+"]"}if(r===null)return"null";if(u.indexOf(r)!==-1){if(S)return JSON.stringify("__cycle__");throw new TypeError("Converting circular structure to JSON")}var a=u.push(r)-1,e=Object.keys(r).sort(l&&l(r));for(i="",t=0;t<e.length;t++){var y=e[t],s=n(r[y]);s&&(i&&(i+=","),i+=JSON.stringify(y)+":"+s)}return u.splice(a,1),"{"+i+"}"}})(v)}),c}var J=g();export{J as default};
//# sourceMappingURL=/sm/d6a1c158e9f92202d08826e3e395070a3f2b89bb48acbe2df03559102a5b0e5b.map
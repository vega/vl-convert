/**
 * Bundled by jsDelivr using Rollup v2.79.2 and Terser v5.39.0.
 * Original file: /npm/fast-json-stable-stringify@2.1.0/index.js
 *
 * Do NOT use SRI with dynamically generated files! More information: https://www.jsdelivr.com/using-sri-with-dynamic-files
 */
var r=function(r,t){t||(t={}),"function"==typeof t&&(t={cmp:t});var e,n="boolean"==typeof t.cycles&&t.cycles,i=t.cmp&&(e=t.cmp,function(r){return function(t,n){var i={key:t,value:r[t]},u={key:n,value:r[n]};return e(i,u)}}),u=[];return function r(t){if(t&&t.toJSON&&"function"==typeof t.toJSON&&(t=t.toJSON()),void 0!==t){if("number"==typeof t)return isFinite(t)?""+t:"null";if("object"!=typeof t)return JSON.stringify(t);var e,o;if(Array.isArray(t)){for(o="[",e=0;e<t.length;e++)e&&(o+=","),o+=r(t[e])||"null";return o+"]"}if(null===t)return"null";if(-1!==u.indexOf(t)){if(n)return JSON.stringify("__cycle__");throw new TypeError("Converting circular structure to JSON")}var f=u.push(t)-1,c=Object.keys(t).sort(i&&i(t));for(o="",e=0;e<c.length;e++){var l=c[e],y=r(t[l]);y&&(o&&(o+=","),o+=JSON.stringify(l)+":"+y)}return u.splice(f,1),"{"+o+"}"}}(r)};export{r as default};
//# sourceMappingURL=/sm/5401f1784b959a6081b4332d4cb6b2366f8bea9c7a3ea060dff5517cba40b04f.map
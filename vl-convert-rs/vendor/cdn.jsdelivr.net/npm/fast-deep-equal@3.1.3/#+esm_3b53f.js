/**
 * Bundled by jsDelivr using Rollup v4.62.2 and esbuild v0.28.1.
 * Original file: /npm/fast-deep-equal@3.1.3/index.js
 *
 * Do NOT use SRI with dynamically generated files! More information: https://www.jsdelivr.com/using-sri-with-dynamic-files
 */
var o,c;function l(){return c||(c=1,o=function n(r,e){if(r===e)return!0;if(r&&e&&typeof r=="object"&&typeof e=="object"){if(r.constructor!==e.constructor)return!1;var u,t,f;if(Array.isArray(r)){if(u=r.length,u!=e.length)return!1;for(t=u;t--!==0;)if(!n(r[t],e[t]))return!1;return!0}if(r.constructor===RegExp)return r.source===e.source&&r.flags===e.flags;if(r.valueOf!==Object.prototype.valueOf)return r.valueOf()===e.valueOf();if(r.toString!==Object.prototype.toString)return r.toString()===e.toString();if(f=Object.keys(r),u=f.length,u!==Object.keys(e).length)return!1;for(t=u;t--!==0;)if(!Object.prototype.hasOwnProperty.call(e,f[t]))return!1;for(t=u;t--!==0;){var s=f[t];if(!n(r[s],e[s]))return!1}return!0}return r!==r&&e!==e}),o}var a=l();export{a as default};
//# sourceMappingURL=/sm/c16ab06842803af7dcae0d31e0f3305d491e7d9b3cbe58a8f0aceb92ee878926.map
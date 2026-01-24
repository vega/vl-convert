/**
 * Bundled by jsDelivr using Rollup v2.79.2 and Terser v5.39.0.
 * Original file: /npm/fast-deep-equal@3.1.3/index.js
 *
 * Do NOT use SRI with dynamically generated files! More information: https://www.jsdelivr.com/using-sri-with-dynamic-files
 */
var r=function r(t,e){if(t===e)return!0;if(t&&e&&"object"==typeof t&&"object"==typeof e){if(t.constructor!==e.constructor)return!1;var o,n,f;if(Array.isArray(t)){if((o=t.length)!=e.length)return!1;for(n=o;0!=n--;)if(!r(t[n],e[n]))return!1;return!0}if(t.constructor===RegExp)return t.source===e.source&&t.flags===e.flags;if(t.valueOf!==Object.prototype.valueOf)return t.valueOf()===e.valueOf();if(t.toString!==Object.prototype.toString)return t.toString()===e.toString();if((o=(f=Object.keys(t)).length)!==Object.keys(e).length)return!1;for(n=o;0!=n--;)if(!Object.prototype.hasOwnProperty.call(e,f[n]))return!1;for(n=o;0!=n--;){var u=f[n];if(!r(t[u],e[u]))return!1}return!0}return t!=t&&e!=e};export{r as default};
//# sourceMappingURL=/sm/07f689a8c7175ef0a58d44b272dc137ebf7ea2c25e1798f00a66b4798b1bd18f.map
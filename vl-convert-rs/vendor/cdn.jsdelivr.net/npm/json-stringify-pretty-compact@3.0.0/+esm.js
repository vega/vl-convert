/**
 * Bundled by jsDelivr using Rollup v2.79.2 and Terser v5.39.0.
 * Original file: /npm/json-stringify-pretty-compact@3.0.0/index.js
 *
 * Do NOT use SRI with dynamically generated files! More information: https://www.jsdelivr.com/using-sri-with-dynamic-files
 */
var n=/("(?:[^\\"]|\\.)*")|[:,]/g,e=function(e,t){var r,i,o;return t=t||{},r=JSON.stringify([1],void 0,void 0===t.indent?2:t.indent).slice(2,-3),i=""===r?1/0:void 0===t.maxLength?80:t.maxLength,o=t.replacer,function e(t,l,f){var u,g,a,h,s,c,d,v,p,y,O,J;if(t&&"function"==typeof t.toJSON&&(t=t.toJSON()),void 0===(O=JSON.stringify(t,o)))return O;if(d=i-l.length-f,O.length<=d&&(p=O.replace(n,(function(n,e){return e||n+" "}))).length<=d)return p;if(null!=o&&(t=JSON.parse(O),o=void 0),"object"==typeof t&&null!==t){if(v=l+r,a=[],g=0,Array.isArray(t))for(y="[",u="]",d=t.length;g<d;g++)a.push(e(t[g],v,g===d-1?0:1)||"null");else for(y="{",u="}",d=(c=Object.keys(t)).length;g<d;g++)h=c[g],s=JSON.stringify(h)+": ",void 0!==(J=e(t[h],v,s.length+(g===d-1?0:1)))&&a.push(s+J);if(a.length>0)return[y,r+a.join(",\n"+v),u].join("\n"+l)}return O}(e,"",0)};export{e as default};
//# sourceMappingURL=/sm/b24190fc8c99bdfc51cd57001e9db464aa8d1bef0d0c9f7d691d5c928cadaf01.map
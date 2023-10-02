import{isArray as x,isString as k,isObject as c}from"/-/vega-util@v1.17.2-LUfkDhormMyfWqy3Ts6U/dist=es2020,mode=imports,min/optimized/vega-util.js";var w="vega-tooltip",j="0.33.0",D="A tooltip plugin for Vega-Lite and Vega visualizations.",S=["vega-lite","vega","tooltip"],I={type:"git",url:"https://github.com/vega/vega-tooltip.git"},L={name:"UW Interactive Data Lab",url:"https://idl.cs.washington.edu"},$=["Dominik Moritz","Sira Horradarn","Zening Qu","Kanit Wongsuphasawat","Yuri Astrakhan","Jeffrey Heer"],z="BSD-3-Clause",O={url:"https://github.com/vega/vega-tooltip/issues"},E="https://github.com/vega/vega-tooltip#readme",A="build/vega-tooltip.js",C="build/vega-tooltip.module.js",M="build/vega-tooltip.min.js",H="build/vega-tooltip.min.js",T="build/vega-tooltip.module.d.ts",N=["src","build","types"],U={prebuild:"yarn clean && yarn build:style",build:"rollup -c","build:style":"./build-style.sh",clean:"rimraf build && rimraf src/style.ts","copy:data":"rsync -r node_modules/vega-datasets/data/* examples/data","copy:build":"rsync -r build/* examples/build","deploy:gh":"yarn build && yarn copy:build && gh-pages -d examples && yarn clean",prepublishOnly:"yarn clean && yarn build",preversion:"yarn lint && yarn test",serve:"browser-sync start -s -f build examples --serveStatic examples",start:"yarn build && concurrently --kill-others -n Server,Rollup 'yarn serve' 'rollup -c -w'",pretest:"yarn build:style",test:"jest","test:inspect":"node --inspect-brk ./node_modules/.bin/jest --runInBand",prepare:"yarn copy:data",prettierbase:"prettier '*.{css,scss,html}'",format:"eslint . --fix && yarn prettierbase --write",lint:"eslint . && yarn prettierbase --check",release:"release-it"},W={"@babel/core":"^7.22.10","@babel/plugin-proposal-async-generator-functions":"^7.20.7","@babel/plugin-proposal-json-strings":"^7.18.6","@babel/plugin-proposal-object-rest-spread":"^7.20.7","@babel/plugin-proposal-optional-catch-binding":"^7.18.6","@babel/plugin-transform-runtime":"^7.22.10","@babel/preset-env":"^7.22.10","@babel/preset-typescript":"^7.22.5","@release-it/conventional-changelog":"^7.0.0","@rollup/plugin-json":"^6.0.0","@rollup/plugin-node-resolve":"^15.1.0","@rollup/plugin-terser":"^0.4.3","@types/jest":"^29.5.3","@typescript-eslint/eslint-plugin":"^6.3.0","@typescript-eslint/parser":"^6.3.0","browser-sync":"^2.29.3",concurrently:"^8.2.0",eslint:"^8.46.0","eslint-config-prettier":"^9.0.0","eslint-plugin-jest":"^27.2.3","eslint-plugin-prettier":"^5.0.0","gh-pages":"^5.0.0",jest:"^29.6.2","jest-environment-jsdom":"^29.6.2",path:"^0.12.7",prettier:"^3.0.1","release-it":"^16.1.3",rollup:"^3.27.2","rollup-plugin-bundle-size":"^1.0.3","rollup-plugin-ts":"^3.4.3",sass:"^1.64.2",typescript:"~5.1.6","vega-datasets":"^2.7.0","vega-typings":"^0.24.2"},Y={"vega-util":"^1.17.2"},_={name:w,version:j,description:D,keywords:S,repository:I,author:L,collaborators:$,license:z,bugs:O,homepage:E,main:A,module:C,unpkg:M,jsdelivr:H,types:T,files:N,scripts:U,devDependencies:W,dependencies:Y};function u(i,t,n){if(x(i))return`[${i.map(e=>t(k(e)?e:p(e,n))).join(", ")}]`;if(c(i)){let e="";const{title:s,image:l,...o}=i;s&&(e+=`<h2>${t(s)}</h2>`),l&&(e+=`<img src="${t(l)}">`);const a=Object.keys(o);if(a.length>0){e+="<table>";for(const d of a){let r=o[d];if(r===void 0)continue;c(r)&&(r=p(r,n)),e+=`<tr><td class="key">${t(d)}</td><td class="value">${t(r)}</td></tr>`}e+="</table>"}return e||"{}"}return t(i)}function g(i){const t=[];return function(n,e){if(typeof e!="object"||e===null)return e;const s=t.indexOf(this)+1;return t.length=s,t.length>i?"[Object]":t.indexOf(e)>=0?"[Circular]":(t.push(e),e)}}function p(i,t){return JSON.stringify(i,g(t))}var B=`#vg-tooltip-element {
  visibility: hidden;
  padding: 8px;
  position: fixed;
  z-index: 1000;
  font-family: sans-serif;
  font-size: 11px;
  border-radius: 3px;
  box-shadow: 2px 2px 4px rgba(0, 0, 0, 0.1);
  /* The default theme is the light theme. */
  background-color: rgba(255, 255, 255, 0.95);
  border: 1px solid #d9d9d9;
  color: black;
}
#vg-tooltip-element.visible {
  visibility: visible;
}
#vg-tooltip-element h2 {
  margin-top: 0;
  margin-bottom: 10px;
  font-size: 13px;
}
#vg-tooltip-element table {
  border-spacing: 0;
}
#vg-tooltip-element table tr {
  border: none;
}
#vg-tooltip-element table tr td {
  overflow: hidden;
  text-overflow: ellipsis;
  padding-top: 2px;
  padding-bottom: 2px;
}
#vg-tooltip-element table tr td.key {
  color: #808080;
  max-width: 150px;
  text-align: right;
  padding-right: 4px;
}
#vg-tooltip-element table tr td.value {
  display: block;
  max-width: 300px;
  max-height: 7em;
  text-align: left;
}
#vg-tooltip-element.dark-theme {
  background-color: rgba(32, 32, 32, 0.9);
  border: 1px solid #f5f5f5;
  color: white;
}
#vg-tooltip-element.dark-theme td.key {
  color: #bfbfbf;
}
`;const h="vg-tooltip-element",b={offsetX:10,offsetY:10,id:h,styleId:"vega-tooltip-style",theme:"light",disableDefaultStyle:!1,sanitize:m,maxDepth:2,formatTooltip:u};function m(i){return String(i).replace(/&/g,"&amp;").replace(/</g,"&lt;")}function y(i){if(!/^[A-Za-z]+[-:.\w]*$/.test(i))throw new Error("Invalid HTML ID");return B.toString().replace(h,i)}function f(i,t,n,e){let s=i.clientX+n;s+t.width>window.innerWidth&&(s=+i.clientX-n-t.width);let l=i.clientY+e;return l+t.height>window.innerHeight&&(l=+i.clientY-e-t.height),{x:s,y:l}}class v{constructor(t){this.options={...b,...t};const n=this.options.id;if(this.el=null,this.call=this.tooltipHandler.bind(this),!this.options.disableDefaultStyle&&!document.getElementById(this.options.styleId)){const e=document.createElement("style");e.setAttribute("id",this.options.styleId),e.innerHTML=y(n);const s=document.head;s.childNodes.length>0?s.insertBefore(e,s.childNodes[0]):s.appendChild(e)}}tooltipHandler(t,n,e,s){if(this.el=document.getElementById(this.options.id),!this.el){this.el=document.createElement("div"),this.el.setAttribute("id",this.options.id),this.el.classList.add("vg-tooltip");const a=document.fullscreenElement??document.body;a.appendChild(this.el)}if(s==null||s===""){this.el.classList.remove("visible",`${this.options.theme}-theme`);return}this.el.innerHTML=this.options.formatTooltip(s,this.options.sanitize,this.options.maxDepth),this.el.classList.add("visible",`${this.options.theme}-theme`);const{x:l,y:o}=f(n,this.el.getBoundingClientRect(),this.options.offsetX,this.options.offsetY);this.el.style.top=`${o}px`,this.el.style.left=`${l}px`}}const V=_.version;function X(i,t){const n=new v(t);return i.tooltip(n.call).run(),n}export default X;export{b as DEFAULT_OPTIONS,v as Handler,f as calculatePosition,y as createDefaultStyle,m as escapeHTML,u as formatValue,g as replacer,p as stringify,V as version};

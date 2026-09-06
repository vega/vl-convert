/**
 * Bundled by jsDelivr using Rollup v4.62.2 and esbuild v0.28.1.
 * Original file: /npm/vega-tooltip@1.1.0/build/index.js
 *
 * Do NOT use SRI with dynamically generated files! More information: https://www.jsdelivr.com/using-sri-with-dynamic-files
 */
import{isArray as S,isString as E,isObject as b}from"/npm/vega-util@2.1.3/+esm";var R="1.1.0",D={version:R};function x(t,e,n,o){if(S(t))return`[${t.map(i=>e(E(i)?i:f(i,n))).join(", ")}]`;if(b(t)){let i="";const{title:r,image:s,...l}=t;r&&(i+=`<h2>${e(r)}</h2>`),s&&(i+=`<img src="${new URL(e(s),o||location.href).href}">`);const c=Object.keys(l);if(c.length>0){i+="<table>";for(const d of c){let h=l[d];h!==void 0&&(b(h)&&(h=f(h,n)),i+=`<tr><td class="key">${e(d)}</td><td class="value">${e(h)}</td></tr>`)}i+="</table>"}return i||"{}"}return e(t)}function w(t){const e=[];return function(n,o){if(typeof o!="object"||o===null)return o;const i=e.indexOf(this)+1;return e.length=i,e.length>t?"[Object]":e.indexOf(o)>=0?"[Circular]":(e.push(o),o)}}function f(t,e){return JSON.stringify(t,w(e))}var T=`#vg-tooltip-element {
  visibility: hidden;
  padding: 8px;
  position: fixed;
  z-index: 1000;
  font-family: sans-serif;
  font-size: 11px;
  border-radius: 3px;
  box-shadow: 2px 2px 4px rgba(0, 0, 0, 0.1);
  white-space: pre-line;
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
  vertical-align: text-top;
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
#vg-tooltip-element {
  /* The default theme is the light theme. */
  background-color: rgba(255, 255, 255, 0.95);
  border: 1px solid #d9d9d9;
  color: black;
}
#vg-tooltip-element.dark-theme {
  background-color: rgba(32, 32, 32, 0.9);
  border: 1px solid #f5f5f5;
  color: white;
}
#vg-tooltip-element.dark-theme td.key {
  color: #bfbfbf;
}
`;const k="vg-tooltip-element",I={offsetX:10,offsetY:10,id:k,styleId:"vega-tooltip-style",theme:"light",disableDefaultStyle:!1,sanitize:v,maxDepth:2,formatTooltip:x,baseURL:"",anchor:"cursor",position:["top","bottom","left","right","top-left","top-right","bottom-left","bottom-right"]};function v(t){return String(t).replace(/&/g,"&amp;").replace(/</g,"&lt;")}function L(t){if(!/^[A-Za-z]+[-:.\w]*$/.test(t))throw new Error("Invalid HTML ID");return T.toString().replaceAll(k,t)}function g(t,e,{offsetX:n,offsetY:o}){const i=m({x1:t.clientX,x2:t.clientX,y1:t.clientY,y2:t.clientY},e,n,o),r=["bottom-right","bottom-left","top-right","top-left"];for(const s of r)if(u(i[s],e))return i[s];return i["top-left"]}function $(t,e,n,o,i){const{position:r,offsetX:s,offsetY:l}=i,c=t._el.getBoundingClientRect(),d=t._origin,h=A(c,d,n),p=m(h,o,s,l),y=Array.isArray(r)?r:[r];for(const a of y)if(u(p[a],o)&&!C(e,p[a],o))return p[a];return g(e,o,i)}function A(t,e,n){const o=n.isVoronoi?n.datum.bounds:n.bounds;let i=t.left+e[0]+o.x1,r=t.top+e[1]+o.y1,s=n;for(;s.mark.group;)s=s.mark.group,i+=s.x??0,r+=s.y??0;const l=o.x2-o.x1,c=o.y2-o.y1;return{x1:i,x2:i+l,y1:r,y2:r+c}}function m(t,e,n,o){const i=(t.x1+t.x2)/2,r=(t.y1+t.y2)/2,s=t.x1-e.width-n,l=i-e.width/2,c=t.x2+n,d=t.y1-e.height-o,h=r-e.height/2,p=t.y2+o;return{top:{x:l,y:d},bottom:{x:l,y:p},left:{x:s,y:h},right:{x:c,y:h},"top-left":{x:s,y:d},"top-right":{x:c,y:d},"bottom-left":{x:s,y:p},"bottom-right":{x:c,y:p}}}function u(t,e){return t.x>=0&&t.y>=0&&t.x+e.width<=window.innerWidth&&t.y+e.height<=window.innerHeight}function C(t,e,n){return t.clientX>=e.x&&t.clientX<=e.x+n.width&&t.clientY>=e.y&&t.clientY<=e.y+n.height}class O{call;options;el;constructor(e){this.options={...I,...e};const n=this.options.id;if(this.el=null,this.call=this.tooltipHandler.bind(this),!this.options.disableDefaultStyle&&!document.getElementById(this.options.styleId)){const o=document.createElement("style");o.setAttribute("id",this.options.styleId),o.innerHTML=L(n);const i=document.head;i.childNodes.length>0?i.insertBefore(o,i.childNodes[0]):i.appendChild(o)}}tooltipHandler(e,n,o,i){if(this.el=document.getElementById(this.options.id),this.el||(this.el=document.createElement("div"),this.el.setAttribute("id",this.options.id),this.el.classList.add("vg-tooltip"),(document.fullscreenElement??document.body).appendChild(this.el)),i==null||i===""){this.el.classList.remove("visible",`${this.options.theme}-theme`);return}this.el.innerHTML=this.options.formatTooltip(i,this.options.sanitize,this.options.maxDepth,this.options.baseURL),this.el.classList.add("visible",`${this.options.theme}-theme`);const{x:r,y:s}=this.options.anchor==="mark"?$(e,n,o,this.el.getBoundingClientRect(),this.options):g(n,this.el.getBoundingClientRect(),this.options);this.el.style.top=`${s}px`,this.el.style.left=`${r}px`}}const z=D.version;function M(t,e){const n=new O(e);return t.tooltip(n.call).run(),n}export{I as DEFAULT_OPTIONS,O as Handler,g as calculatePositionRelativeToCursor,$ as calculatePositionRelativeToMark,L as createDefaultStyle,M as default,v as escapeHTML,x as formatValue,A as getMarkBounds,m as getPositions,C as mouseIsOnTooltip,w as replacer,f as stringify,u as tooltipIsInViewport,z as version};
//# sourceMappingURL=/sm/6d4d48a8cd4932a6c497063959a9e92a1918aa50aec095600429730fcbd9c017.map
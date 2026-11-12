// Test-only Node loader: use exactly the JavaScript modules bundled by Rust.
// Never download code or silently fall back to a different compiler version.
import {readFile} from 'node:fs/promises';
import {pathToFileURL, fileURLToPath} from 'node:url';
import {resolve as pathResolve} from 'node:path';
const vendor = fileURLToPath(new URL('../../vendor/cdn.jsdelivr.net/', import.meta.url));
export async function resolve(specifier, context, next) {
  const path = specifier.replace(/^https:\/\/cdn\.jsdelivr\.net/, '');
  if (path.startsWith('/npm/') && path.endsWith('/+esm')) {
    return {url: pathToFileURL(pathResolve(vendor, path.slice(1, -4), '#+esm_3b53f.js')).href, shortCircuit: true};
  }
  return next(specifier, context);
}
export async function load(url, context, next) {
  if (url.startsWith(pathToFileURL(vendor).href) && url.endsWith('.js')) {
    return {format:'module', source:await readFile(new URL(url), 'utf8'), shortCircuit:true};
  }
  return next(url, context);
}

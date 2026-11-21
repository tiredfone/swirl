const ALLOWED = new Set(['wasm', 'js', 'mjs', 'json', 'data', 'css', 'html', 'png', 'svg', 'txt']);

export function safeName(rel) {
  let s = String(rel).replace(/[^A-Za-z0-9._\/-]/g, '_');
  const m = /\.([^./]+)$/.exec(s);
  if (!m || !ALLOWED.has(m[1].toLowerCase())) s += '.txt';
  return s;
}

export function mapUrl(url, base) {
  if (typeof url !== 'string' || !url.startsWith(base + '/')) return url;
  const q = url.indexOf('?');
  const rel = decodeURIComponent(q < 0 ? url.slice(base.length + 1) : url.slice(base.length + 1, q));
  return base + '/' + safeName(rel).split('/').map(encodeURIComponent).join('/') + (q < 0 ? '' : url.slice(q));
}

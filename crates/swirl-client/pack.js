const base = new URL('./chocolate/', import.meta.url).href;
let map = {};
const appParam = new URLSearchParams(location.search).get('swirlapp');

export const useApp = appParam !== '0';
const skip = useApp;
try {
  if (skip) throw new Error('skip');
  map = await (await fetch(base + 'rewrite.json')).json();
} catch (e) {
  map = {};
}
const orig = window.fetch.bind(window);
window.fetch = skip ? window.fetch : (input, init) => {
  try {
    const raw = typeof input === 'string' ? input : input instanceof URL ? input.href : input && input.url;
    if (raw && raw.startsWith(base)) {
      const rel = decodeURIComponent(raw.slice(base.length).split(/[?#]/)[0]);
      const hit = map[rel];
      if (hit) return orig(base + hit.split('/').map(encodeURIComponent).join('/'), init);
    }
  } catch (e) {}
  return orig(input, init);
};

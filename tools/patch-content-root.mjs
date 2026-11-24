import fs from 'node:fs';

const file = process.argv[2];
let s = fs.readFileSync(file, 'utf8');
const a = "  if (cfg.version) settings.version = String(cfg.version);\n";
const b = "export function publicPath(p) {\n  return settings.publicPath + p;\n}";
if (!s.includes(a) || !s.includes(b)) {
  console.error('assets.js changed; content root patch does not apply');
  process.exit(1);
}
s = s.replace(a, a + "  if (cfg.contentRoot) settings.contentRoot = String(cfg.contentRoot);\n  if (cfg.contentRewrites) settings.contentRewrites = cfg.contentRewrites;\n");
s = s.replace(b, `export function publicPath(p) {
  const m = settings.contentRoot && /^\\/(content2018|content|corescripts2018)\\/([^?]*)(.*)$/.exec(p);
  if (m) {
    let rel = m[2];
    try {
      rel = decodeURIComponent(rel);
    } catch {}
    rel = rel.replace(/[/]{2,}/g, '/');
    const map = settings.contentRewrites && settings.contentRewrites[m[1]];
    if (map && map[rel]) rel = map[rel];
    return settings.contentRoot + m[1] + '/' + rel.split('/').map(encodeURIComponent).join('/') + m[3];
  }
  return settings.publicPath + p;
}`);
const c1 = "return publicPath('/content2018/') + hit.split('/').map(encodeURIComponent).join('/');";
const c2 = "return publicPath('/content/') + p.split('/').map(encodeURIComponent).join('/');";
if (!s.includes(c1) || !s.includes(c2)) {
  console.error('assets.js contentUrl changed; content root patch does not apply');
  process.exit(1);
}
s = s.replace(c1, "return publicPath('/content2018/' + hit);").replace(c2, "return publicPath('/content/' + p);");
fs.rmSync(file);
fs.writeFileSync(file, s);

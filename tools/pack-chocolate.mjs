import fs from 'fs';
import path from 'path';
import { safeName } from '/var/www/swirl/crates/swirl-client/urlmap.js';
const root = process.argv[2];
const walk = (d, out) => {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
};
for (const f of walk(root, [])) {
  const rel = path.relative(root, f);
  const to = safeName(rel);
  if (to !== rel) {
    fs.mkdirSync(path.dirname(path.join(root, to)), { recursive: true });
    fs.renameSync(f, path.join(root, to));
  }
}
const css = path.join(root, 'client/style.css');
let c = fs.readFileSync(css, 'utf8');
fs.rmSync(css);
c = c.replace(/url\("\.\.\/content2018\/fonts\/([^"]+)"\)/g, (m, n) => 'url("../' + safeName('content2018/fonts/' + n) + '")');
fs.writeFileSync(css, c);
const as = path.join(root, 'client/assets.js');
let a = fs.readFileSync(as, 'utf8');
fs.rmSync(as);
a = a.replace('export function contentUrl(path) {', "import { mapUrl as __mapUrl } from '../../urlmap.js';\nexport function contentUrl(path) {\n  return __mapUrl(contentUrlRaw(path), settings.publicPath);\n}\n\nfunction contentUrlRaw(path) {");
fs.writeFileSync(as, a);

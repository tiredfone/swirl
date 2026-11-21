import fs from 'fs';
import path from 'path';
const [src, dst] = process.argv.slice(2);
const allowed = new Set(['wasm', 'js', 'mjs', 'json', 'data', 'css', 'html', 'png', 'svg', 'txt']);
const map = {};
const walk = (rel) => {
  for (const e of fs.readdirSync(path.join(src, rel), { withFileTypes: true })) {
    const r = rel ? rel + '/' + e.name : e.name;
    if (e.isDirectory()) {
      walk(r);
      continue;
    }
    if (r === 'package.json') continue;
    const ext = path.extname(e.name).slice(1).toLowerCase();
    const safe = r.replace(/[^A-Za-z0-9._\-/]/g, '_');
    let out = r;
    if (!allowed.has(ext) || safe !== r) {
      out = allowed.has(ext) ? safe : safe + '.txt';
      if (out === r) out = safe + '.txt';
      map[r] = out;
    }
    const to = path.join(dst, out);
    fs.mkdirSync(path.dirname(to), { recursive: true });
    try {
      fs.linkSync(path.join(src, r), to);
    } catch (err) {
      fs.copyFileSync(path.join(src, r), to);
    }
  }
};
walk('');
fs.writeFileSync(path.join(dst, 'rewrite.json'), JSON.stringify(map));

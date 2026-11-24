const p = require('/tmp/claude-1000/pp/node_modules/puppeteer-core');
const http = require('http');
const fs = require('fs');
const path = require('path');
const [, , era, wsPort, pageFile, out] = process.argv;
const dist = process.env.DIST || '/var/www/swirl/dist/client';
const types = { '.js': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css', '.png': 'image/png', '.ttf': 'font/ttf', '.jpg': 'image/jpeg' };
const srv = http.createServer((req, res) => {
  const u = req.url.split('?')[0];
  if (u === '/') {
    res.writeHead(200, { 'Content-Type': 'text/html' });
    res.end(fs.readFileSync(pageFile));
    return;
  }
  if (u.startsWith('/dist/')) {
    const f = path.join(dist, decodeURIComponent(u.slice(6)));
    try {
      res.writeHead(200, { 'Content-Type': types[path.extname(f)] || 'application/octet-stream' });
      res.end(fs.readFileSync(f));
    } catch (e) {
      res.writeHead(404);
      res.end();
    }
    return;
  }
  http.get({ host: '127.0.0.1', port: 80, path: req.url, headers: { Host: 'localhost' } }, (r) => {
    res.writeHead(r.statusCode, { 'Content-Type': r.headers['content-type'] || 'application/octet-stream' });
    r.pipe(res);
  }).on('error', () => {
    res.writeHead(502);
    res.end();
  });
});
srv.listen(0, async () => {
  const port = srv.address().port;
  const b = await p.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const pg = await b.newPage();
  await pg.setViewport({ width: 1100, height: 700 });
  const errs = [];
  pg.on('pageerror', (e) => errs.push(String(e).slice(0, 300)));
  await pg.evaluateOnNewDocument(() => {
    window.__st = [];
    window.addEventListener('DOMContentLoaded', () => {
      const s = document.getElementById('swirl-status');
      new MutationObserver(() => window.__st.push(s.textContent)).observe(s, { childList: true, characterData: true, subtree: true });
    });
  });
  await pg.goto(`http://127.0.0.1:${port}/?swirlws=` + encodeURIComponent('ws://127.0.0.1:' + wsPort + '/ws'), { waitUntil: 'domcontentloaded' });
  const snap = async (n) => {
    fs.writeFileSync(out + '-' + n + '.png', await pg.screenshot());
    return pg.evaluate(() => {
      const r = document.getElementById('swirl-boot');
      return { hidden: r.hidden, cls: r.className, op: getComputedStyle(r).opacity, bricks: document.getElementById('swirl-bricks').textContent, conn: document.getElementById('swirl-connectors').textContent, status: document.getElementById('swirl-status').textContent };
    });
  };
  const marks = [];
  for (const [n, ms] of [['a', 1500], ['b', 3000], ['c', 6000], ['d', 12000]]) {
    await new Promise((r) => setTimeout(r, n === 'a' ? ms : ms - marks.at(-1).ms));
    marks.push({ n, ms, ...(await snap(n)) });
  }
  console.log(JSON.stringify({ marks, statuses: await pg.evaluate(() => window.__st), errs }, null, 1));
  await b.close();
  srv.close();
  process.exit(0);
});

const p = require('/tmp/claude-1000/pp/node_modules/puppeteer-core');
const http = require('http');
const fs = require('fs');
const path = require('path');
const dist = process.env.DIST || '/var/www/swirl/dist/client';
const [, , wsPort, mode, out] = process.argv;
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const page = `<!doctype html><meta charset=utf-8><body style="margin:0;background:#000"><canvas id=swirl-canvas style="width:100vw;height:100vh;display:block" tabindex=0></canvas><div id=swirl-status style="position:fixed;top:0;left:0;color:#fff"></div><script type=module>
import init from '/dist/entry.js';
</script></body>`;
const gui = fs.readFileSync(path.join(__dirname, 'swirl-gui-test.cjs'), 'utf8');
const pageHtml = /const page = `([\s\S]*?)`;/.exec(gui)[1];
const types = { '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css', '.png': 'image/png', '.ttf': 'font/ttf', '.otf': 'font/otf', '.svg': 'image/svg+xml', '.txt': 'text/plain' };
const srv = http.createServer((req, res) => {
  if (req.url.startsWith('/dist/')) {
    const f = path.join(dist, req.url.slice(6).split('?')[0]);
    try {
      res.writeHead(200, { 'Content-Type': types[path.extname(f)] || 'application/octet-stream' });
      res.end(fs.readFileSync(f));
    } catch (e) {
      res.writeHead(404);
      res.end();
    }
    return;
  }
  if (req.url.startsWith('/asset') || req.url.startsWith('/chocolate/')) {
    http.get({ host: '127.0.0.1', port: 80, path: req.url, headers: { Host: 'localhost' } }, (r) => {
      res.writeHead(r.statusCode, { 'Content-Type': r.headers['content-type'] || 'application/octet-stream' });
      r.pipe(res);
    }).on('error', () => { res.writeHead(502); res.end(); });
    return;
  }
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(pageHtml);
});
(async () => {
  await new Promise((r) => srv.listen(0, '127.0.0.1', r));
  const b = await p.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const pg = await b.newPage();
  await pg.setViewport({ width: 900, height: 600 });
  await pg.evaluateOnNewDocument(() => {
    window.__chars = [];
    const o = WebSocket.prototype.send;
    WebSocket.prototype.send = function (d) {
      try { const m = JSON.parse(d); if (m.t === 'char') window.__chars.push([Math.round(performance.now()), m]); } catch (e) {}
      return o.call(this, d);
    };
  });
  await pg.goto('http://127.0.0.1:' + srv.address().port + '/?swirlws=' + encodeURIComponent('ws://127.0.0.1:' + wsPort + '/ws') + '&swirlapp=' + (mode === 'app' ? 1 : 0), { waitUntil: 'domcontentloaded' });
  await pg.waitForFunction('window.__ready === true || window.__failed', { timeout: 30000 });
  await pg.waitForFunction('(window.solarLua && window.solarLua.ready && window.solarLua.ready()) || (window.solarApp && window.solarApp.vm)', { timeout: 60000 });
  await wait(9000);
  await pg.focus('#swirl-canvas');
  const seq = (process.env.SEQ || 'w:2000,wait:500,d:1500,wait:500,a:1500,wait:500').split(',');
  await pg.evaluate(() => { window.__mark = window.__chars.length; });
  const t0 = await pg.evaluate(() => Math.round(performance.now()));
  let shot = 0;
  for (const s of seq) {
    const [k, ms] = s.split(':');
    if (k === 'wait') await wait(+ms);
    else {
      await pg.keyboard.down(k);
      await wait(+ms);
      if (process.env.SHOT && !shot) { shot = 1; await pg.screenshot({ path: process.env.SHOT + '-' + mode + '.png' }); }
      await pg.keyboard.up(k);
    }
  }
  const chars = await pg.evaluate(() => window.__chars);
  fs.writeFileSync(out, JSON.stringify({ t0, chars }));
  console.log(mode, 'chars', chars.length);
  await b.close();
  srv.close();
  process.exit(0);
})();

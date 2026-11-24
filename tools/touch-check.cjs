const p = require('/tmp/claude-1000/pp/node_modules/puppeteer-core');
const http = require('http');
const fs = require('fs');
const path = require('path');
const [, , wsPort, pageFile, out] = process.argv;
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
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
srv.listen(0, async () => {
  const port = srv.address().port;
  const b = await p.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const pg = await b.newPage();
  await pg.setUserAgent('Mozilla/5.0 (Linux; Android 13; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Mobile Safari/537.36');
  await pg.setViewport({ width: 844, height: 390, isMobile: true, hasTouch: true, deviceScaleFactor: 1 });
  const errs = [];
  pg.on('pageerror', (e) => errs.push(String(e).slice(0, 300)));
  await pg.goto(`http://127.0.0.1:${port}/?swirlws=` + encodeURIComponent('ws://127.0.0.1:' + wsPort + '/ws'), { waitUntil: 'domcontentloaded' });
  await sleep(+process.env.SETTLE || 12000);
  const own = () => pg.evaluate(() => { const o = JSON.parse(window.solarApp.vm.own_state()); return o && { x: o.cf[0], y: o.cf[1], z: o.cf[2], state: o.state }; });
  const cam = () => pg.evaluate(() => window.solarApp.vm.cam_state());
  const info = await pg.evaluate(() => ({ touchClass: document.getElementById('chocolate').classList.contains('ch-touch'), uis: window.solarApp.vm.class_counts('CoreGui'), logs: (window.solarApp.logs || []).slice(0, 8).map((l) => l.kind + ': ' + l.msg.slice(0, 120)), errors: window.solarApp.errors.slice(0, 3) }));
  fs.writeFileSync(out + '-0.png', await pg.screenshot());
  const p0 = await own();
  const c0 = await cam();
  const r = await pg.touchscreen.touchStart(450, 120);
  for (let i = 1; i <= 8; i++) { await r.move(450 + i * 15, 120 + i * 3); await sleep(400); }
  await r.end();
  await sleep(2000);
  const c1pre = await cam();
  const t = await pg.touchscreen.touchStart(73, 287);
  for (let i = 1; i <= 6; i++) { await t.move(73 + i * 6, 287 - i * 8); await sleep(60); }
  await sleep(1200);
  const pMove = await own();
  fs.writeFileSync(out + '-1.png', await pg.screenshot());
  await t.end();
  await sleep(500);
  const pStop = await own();
  const c1 = c1pre;
  const jumpT = await pg.touchscreen.touchStart(720, 277);
  const pJump = [];
  for (let i = 0; i < 10; i++) { await sleep(250); const o = await own(); pJump.push(+o.y.toFixed(2)); }
  await jumpT.end();
  await sleep(600);
  console.log(JSON.stringify({ info, p0, pMove, pStop, pJump, c0, c1, errs }, null, 1));
  await b.close();
  srv.close();
  process.exit(0);
});

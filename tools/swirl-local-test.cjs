const p = require('/tmp/claude-1000/pp/node_modules/puppeteer-core');
const http = require('http');
const fs = require('fs');
const path = require('path');
const dist = process.env.DIST || '/var/www/swirl/dist/client';
const wsPort = process.argv[2];
const outPrefix = process.argv[3] || '/tmp/swirl/local';
const scenario = process.env.SCENARIO || 'tools';
const page = `<!doctype html><meta charset=utf-8><body style="margin:0;background:#000"><canvas id=swirl-canvas style="width:100vw;height:100vh;display:block"></canvas><div id=swirl-status style="position:fixed;top:0;left:0;color:#fff"></div><script>window.solarClientConfig={};window.solarClientReady=function(){window.__ready=true};window.solarClientFailed=function(r){window.__failed=r}</script><script type=module src="/dist/entry.js"></script>`;
const srv = http.createServer((req, res) => {
  if (req.url.startsWith('/dist/')) {
    const f = path.join(dist, req.url.slice(6).split('?')[0]);
    const types = { '.js': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json' };
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
    }).on('error', () => {
      res.writeHead(502);
      res.end();
    });
    return;
  }
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(page);
});
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
(async () => {
  await new Promise((r) => srv.listen(0, '127.0.0.1', r));
  const port = srv.address().port;
  const b = await p.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const open = async (label) => {
    const pg = await b.newPage();
    await pg.setViewport({ width: 900, height: 600 });
    pg.on('pageerror', (e) => console.log(label, 'pageerror', String(e).slice(0, 300)));
    pg.on('console', (m) => { if (m.type() === 'error') console.log(label, 'console', m.text().slice(0, 200)); });
    await pg.goto('http://127.0.0.1:' + port + '/?swirlws=' + encodeURIComponent('ws://127.0.0.1:' + wsPort + '/ws'), { waitUntil: 'domcontentloaded' });
    await pg.waitForFunction('window.__ready === true || window.__failed', { timeout: 30000 });
    return pg;
  };
  const A = await open('A');
  const B = await open('B');
  await wait(1500);
  const inv = (pg) => pg.evaluate(() => JSON.parse(window.solarClient.inventory()));
  if (scenario === 'look') {
    await wait(6000);
    await A.evaluate(() => window.solarClient.focus(0, 4.5, 0, +(new URLSearchParams(location.search).get('d') || 12)));
    await wait(1500);
    await A.screenshot({ path: outPrefix + '-look.png' });
    await b.close();
    srv.close();
    process.exit(0);
  }
  const res = { invA: await inv(A) };
  const si = res.invA.findIndex((t) => t.name === (process.env.TOOL || 'ClassicSword'));
  await A.bringToFront();
  await A.keyboard.press(String(si + 1));
  await wait(800);
  res.invAfterEquip = (await inv(A)).map((t) => t.name + ':' + t.equipped);
  await A.screenshot({ path: outPrefix + '-A-equipped.png' });
  await B.bringToFront();
  await wait(500);
  await B.screenshot({ path: outPrefix + '-B-sees.png' });
  await A.bringToFront();
  await A.mouse.click(450, 300);
  await wait(1500);
  res.hpOps = 'see server log';
  await A.keyboard.press(String(si + 1));
  await wait(600);
  res.invAfterUnequip = (await inv(A)).map((t) => t.name + ':' + t.equipped);
  await A.keyboard.press('/');
  await A.keyboard.type('hello from A');
  await A.keyboard.press('Enter');
  await wait(1200);
  res.chatB = await B.evaluate(() => document.querySelector('#swirl-log, #swirl-chat div').innerText);
  console.log(JSON.stringify(res, null, 1));
  await b.close();
  srv.close();
  process.exit(0);
})().catch((e) => {
  console.log('FAIL', e.message);
  process.exit(1);
});

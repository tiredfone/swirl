const p = require('/tmp/claude-1000/pp/node_modules/puppeteer-core');
const http = require('http');
const fs = require('fs');
const path = require('path');
const dist = process.env.DIST || '/var/www/swirl/dist/client';
const wsPort = process.argv[2];
const out = process.argv[3] || '/tmp/swirl/gui';
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const page = `<!doctype html><meta charset=utf-8><body style="margin:0;background:#000"><canvas id=swirl-canvas style="width:100vw;height:100vh;display:block" tabindex=0></canvas><div id=swirl-status style="position:fixed;top:0;left:0;color:#fff"></div><script>window.solarClientConfig={};window.solarClientReady=function(){window.__ready=true};window.solarClientFailed=function(r){window.__failed=r}</script><script type=module src="/dist/entry.js"></script>`;
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
      const h = { 'Content-Type': r.headers['content-type'] || 'application/octet-stream' };
      res.writeHead(r.statusCode, h);
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
(async () => {
  await new Promise((r) => srv.listen(0, '127.0.0.1', r));
  const port = srv.address().port;
  const b = await p.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const pg = await b.newPage();
  await pg.setViewport({ width: +(process.env.W || 1100), height: +(process.env.H || 700) });
  pg.on('pageerror', (e) => console.log('pageerror', String(e).slice(0, 400)));
  pg.on('console', (m) => { if (m.type() === 'error' || m.type() === 'warning') console.log('console', m.type(), m.text().slice(0, 300)); });
  await pg.evaluateOnNewDocument(() => { window.__sent = []; const o = WebSocket.prototype.send; WebSocket.prototype.send = function (d) { try { const m = JSON.parse(d); if (m.t !== 'char' && m.t !== 'ping') window.__sent.push(String(d).slice(0, 160)); } catch (e) {} return o.call(this, d); }; });
  await pg.goto('http://127.0.0.1:' + port + '/?swirlws=' + encodeURIComponent('ws://127.0.0.1:' + wsPort + '/ws') + (process.env.QS || ''), { waitUntil: 'domcontentloaded' });
  await pg.waitForFunction('window.__ready === true || window.__failed', { timeout: 30000 });
  await pg.waitForFunction('window.solarLua && window.solarLua.ready() || window.solarLuaFailed', { timeout: 60000 }).catch(() => console.log('lua not ready'));
  console.log('luaFailed', await pg.evaluate(() => window.solarLuaFailed || null));
  await wait(+(process.env.SETTLE || 8000));
  const info = () => pg.evaluate(() => {
    const L = window.solarLua;
    if (!L) return null;
    const dm = L.world.dm;
    const kids = (i) => i ? i.children.map((c) => c.props.get('Name') + ':' + c.ClassName) : null;
    const pl = L.world.player;
    const pgui = pl && pl.findChildOfClass('PlayerGui');
    return { errors: L.errors.slice(0, 20), robloxGui: kids(L.world.robloxGui), playerGui: kids(pgui), backpack: kids(pl && pl.findChildOfClass('Backpack')), logs: L.logs.slice(-15) };
  });
  console.log(JSON.stringify(await info(), null, 1));
  await pg.screenshot({ path: out + '-1.png' });
  if (process.env.HOLD) {
    await pg.focus('#swirl-canvas');
    for (const step of process.env.HOLD.split(',')) {
      const [k, ms] = step.split(':');
      if (k === 'wait') await wait(+ms);
      else {
        await pg.keyboard.down(k);
        await wait(+ms);
        await pg.keyboard.up(k);
      }
    }
  }
  if (process.env.MCLICK) {
    const [x, y, n] = process.env.MCLICK.split(',').map(Number);
    for (let i = 0; i < (n || 1); i++) {
      await pg.mouse.click(x, y);
      await wait(400);
    }
  }
  if (process.env.TAPOUT) fs.writeFileSync(process.env.TAPOUT, JSON.stringify(await pg.evaluate(() => window.solarTapLog || [])));
  if (process.env.EVAL) console.log(JSON.stringify(await pg.evaluate(process.env.EVAL), null, 1));
  if (process.env.CHAT) {
    await pg.focus('#swirl-canvas');
    await pg.keyboard.press('/');
    await wait(600);
    console.log('active', await pg.evaluate(() => document.activeElement && document.activeElement.tagName + ':' + document.activeElement.className));
    await pg.keyboard.type(process.env.CHAT);
    await wait(400);
    await pg.screenshot({ path: out + '-typed.png' });
    await pg.keyboard.press('Enter');
    await wait(1500);
    await pg.screenshot({ path: out + '-chat.png' });
  }
  if (process.env.CLICKSLOT) {
    const [x, y] = process.env.CLICKSLOT.split(',').map(Number);
    await pg.mouse.click(x, y);
    await wait(1200);
    await pg.screenshot({ path: out + '-click.png' });
    console.log('equipped', await pg.evaluate(() => { const w = window.solarLua.world; return w.equippedTool ? w.equippedTool.props.get('Name') : null; }));
    await pg.mouse.move(300, 450);
    await wait(500);
    await pg.mouse.move(310, 455);
    await wait(500);
    console.log('mouse', await pg.evaluate(() => { const m = window.solarLua.world.mouse; const t = m.props.get('Target'); return JSON.stringify({ target: t ? t.props.get('Name') : null, hit: m.props.get('Hit').slice(0, 3), x: m.props.get('X'), y: m.props.get('Y') }); }));
  }
  if (process.env.EQUIP) {
    await pg.keyboard.press(process.env.EQUIP);
    await wait(1200);
    await pg.screenshot({ path: out + '-equip.png' });
  }
  console.log('assetfail', JSON.stringify(await pg.evaluate(() => window.solarAssetFailures || [])));
  console.log('sent', JSON.stringify(await pg.evaluate(() => window.__sent)));
  console.log(JSON.stringify(await info(), null, 1).slice(0, 1500));
  await b.close();
  srv.close();
  process.exit(0);
})().catch((e) => {
  console.log('FAIL', e.message);
  process.exit(1);
});

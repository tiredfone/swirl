const p = require('/tmp/claude-1000/pp/node_modules/puppeteer-core');
const http = require('http');
const fs = require('fs');
const path = require('path');
const dist = process.env.DIST || '/var/www/swirl/dist/client';
const [, , wsPort, mode] = process.argv;
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
  await pg.setViewport({ width: 1100, height: 700 });
  const fails = [];
  await pg.goto('http://127.0.0.1:' + srv.address().port + '/?swirlws=' + encodeURIComponent('ws://127.0.0.1:' + wsPort + '/ws') + '&swirlapp=' + (mode === 'js' ? 0 : 1), { waitUntil: 'domcontentloaded' });
  await pg.waitForFunction('window.__ready === true || window.__failed', { timeout: 30000 });
  await pg.waitForFunction('window.solarApp && window.solarApp.vm', { timeout: 60000 });
  await wait(9000);
  const leg = async () => {
    await pg.evaluate(() => window.solarApp.vm.run_script('local c = workspace:FindFirstChildOfClass("Model") for _, m in ipairs(workspace:GetChildren()) do if m:FindFirstChild("LeftUpperLeg") then c = m end end local l = c.LeftUpperLeg.CFrame.p - c.LowerTorso.CFrame.p local r = c.LowerTorso.CFrame:inverse() * c.LeftUpperLeg.CFrame.p print("LEGPOS", r.x, r.y, r.z)'));
    await wait(400);
    return pg.evaluate(() => { const l = (window.solarApp.logs || []).map((x) => String(x.msg)).filter((m) => m.startsWith('LEGPOS')).pop(); return l ? l.split(/\s+/).slice(1).map(Number) : null; });
  };
  const idle = await leg();
  await pg.evaluate(() => window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyW', key: 'w', bubbles: true })));
  await wait(1300);
  const walking = await leg();
  await pg.evaluate(() => window.dispatchEvent(new KeyboardEvent('keyup', { code: 'KeyW', key: 'w', bubbles: true })));
  const legShot = process.env.SHOT_WALK;
  if (legShot) await pg.screenshot({ path: legShot });
  const diff = idle && walking ? idle.reduce((a, v, i) => a + Math.abs(v - walking[i]), 0) : 0;
  if (diff < 0.05) fails.push('walk animation did not move the rendered left leg (' + diff.toFixed(3) + ')');
  await wait(500);
  await pg.keyboard.press('Escape');
  await pg.waitForFunction(() => [...document.querySelectorAll('*')].some((e) => !e.children.length && e.textContent === 'Player1' && e.getBoundingClientRect().width > 0 && e.getBoundingClientRect().y > 100), { timeout: 8000 }).catch(() => {});
  await wait(500);
  if (process.env.SHOT_ESC) await pg.screenshot({ path: process.env.SHOT_ESC });
  const info = await pg.evaluate(() => {
    const out = { tab: null, leave: null, row: null };
    document.querySelectorAll('*').forEach((e) => {
      if (e.children.length) return;
      const t = e.textContent;
      const r = e.getBoundingClientRect();
      const cs = getComputedStyle(e);
      if (t === 'Players' && r.width > 0) out.tab = { px: parseFloat(cs.fontSize), h: r.height, w: r.width };
      if (t === 'Leave Game' && r.width > 0) out.leave = { px: parseFloat(cs.fontSize), h: r.height, w: r.width };
      if (t === 'Player1' && r.width > 0 && r.y > 100) out.row = { w: r.width, h: r.height };
    });
    return out;
  });
  if (!info.tab || info.tab.px < 20) fails.push('Players tab label size ' + JSON.stringify(info.tab));
  if (!info.leave || info.leave.h > 30) fails.push('Leave Game wraps or is missing ' + JSON.stringify(info.leave));
  if (!info.row) fails.push('Players tab has no player row');
  console.log(JSON.stringify({ info, diff, fails }));
  await b.close();
  srv.close();
  process.exit(fails.length ? 1 : 0);
})();

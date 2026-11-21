const p = require('/tmp/claude-1000/pp/node_modules/puppeteer-core');
const http = require('http');
const fs = require('fs');
const path = require('path');
const dist = process.env.DIST || '/var/www/swirl/dist/client';
const wsPort = process.argv[2];
const out = process.argv[3] || '/tmp/swirl/anim';
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const page = `<!doctype html><meta charset=utf-8><body style="margin:0;background:#000"><canvas id=swirl-canvas style="width:100vw;height:100vh;display:block" tabindex=0></canvas><div id=swirl-status style="position:fixed;top:0;left:0;color:#fff"></div><script>window.solarClientConfig={};window.solarClientReady=function(){window.__ready=true};window.solarClientFailed=function(r){window.__failed=r}</script><script type=module src="/dist/entry.js"></script>`;
const bad = [];
const srv = http.createServer((req, res) => {
  if (req.url.startsWith('/dist/')) {
    const f = path.join(dist, decodeURIComponent(req.url.slice(6).split('?')[0]));
    const types = { '.js': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css', '.png': 'image/png', '.jpg': 'image/jpeg' };
    try {
      res.writeHead(200, { 'Content-Type': types[path.extname(f)] || 'application/octet-stream' });
      res.end(fs.readFileSync(f));
    } catch (e) {
      res.writeHead(404);
      res.end();
    }
    return;
  }
  if (req.url.startsWith('/asset')) {
    http.get({ host: '127.0.0.1', port: 80, path: req.url, headers: { Host: 'localhost' } }, (r) => {
      res.writeHead(r.statusCode, { 'Content-Type': r.headers['content-type'] || 'application/octet-stream' });
      r.pipe(res);
    }).on('error', () => { res.writeHead(502); res.end(); });
    return;
  }
  if (req.url.startsWith('/chocolate')) bad.push(req.url);
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(page);
});
(async () => {
  await new Promise((r) => srv.listen(0, '127.0.0.1', r));
  const port = srv.address().port;
  const b = await p.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const pg = await b.newPage();
  await pg.setViewport({ width: 1100, height: 700 });
  pg.on('pageerror', (e) => console.log('pageerror', String(e).slice(0, 400)));
  pg.on('requestfailed', (r) => console.log('reqfailed', r.url().slice(0, 150)));
  pg.on('response', (r) => { if (r.status() >= 400) console.log('http', r.status(), r.url().slice(0, 150)); });
  await pg.goto('http://127.0.0.1:' + port + '/?swirlws=' + encodeURIComponent('ws://127.0.0.1:' + wsPort + '/ws'), { waitUntil: 'domcontentloaded' });
  await pg.waitForFunction('window.__ready === true || window.__failed', { timeout: 30000 });
  await pg.waitForFunction('window.solarLua && window.solarLua.ready()', { timeout: 60000 });
  await pg.waitForFunction('window.solarLua.world.own', { timeout: 30000 });
  await wait(+(process.env.SETTLE || 5000));
  await pg.focus('#swirl-canvas');
  const sample = () => pg.evaluate(() => {
    const w = window.solarLua.world;
    const m = w.own.model;
    const t = m.findChild('Torso') || m.findChild('HumanoidRootPart');
    const tc = t.props.get('CFrame');
    const rel = (n) => {
      const q = m.findChild(n);
      if (!q) return null;
      const c = q.props.get('CFrame');
      const d = [c[0] - tc[0], c[1] - tc[1], c[2] - tc[2]];
      return [tc[3] * d[0] + tc[6] * d[1] + tc[9] * d[2], tc[4] * d[0] + tc[7] * d[1] + tc[10] * d[2], tc[5] * d[0] + tc[8] * d[1] + tc[11] * d[2]].map((v) => +v.toFixed(3));
    };
    const hum = w.own.hum;
    let tracks = 0;
    try { tracks = (w.anims.playing(hum) || []).length; } catch (e) {}
    return { pos: window.solarLua.ownPos().map((v) => +v.toFixed(2)), la: rel('Left Arm') || rel('LeftUpperArm'), ra: rel('Right Arm') || rel('RightUpperArm'), ll: rel('Left Leg') || rel('LeftUpperLeg'), rl: rel('Right Leg') || rel('RightUpperLeg'), state: w.own.rec.state, tracks, animate: !!m.findChild('Animate'), running: m.children.filter((c) => c.ClassName === 'LocalScript').map((c) => c.props.get('Name') + ':' + w.host.running.has(c.id)) };
  });
  const result = { idle: await sample() };
  await pg.screenshot({ path: out + '-idle.png' });
  await pg.keyboard.down('w');
  const walk = [];
  for (let i = 0; i < 10; i++) { await wait(150); walk.push(await sample()); }
  await pg.screenshot({ path: out + '-walk.png' });
  result.walk = walk;
  await pg.keyboard.up('w');
  await pg.keyboard.down(' ');
  await wait(120);
  await pg.keyboard.up(' ');
  const jump = [];
  for (let i = 0; i < 8; i++) { await wait(120); jump.push(await sample()); }
  result.jump = jump;
  await wait(1500);
  result.after = await sample();
  result.errors = await pg.evaluate(() => window.solarLua.errors.concat(window.solarLua.gameErrors).slice(0, 10));
  result.chocolateRequests = bad;
  fs.writeFileSync(out + '.json', JSON.stringify(result));
  const range = (k, a) => { const v = walk.map((s) => s[k] && s[k][a]); return Math.max(...v) - Math.min(...v); };
  const swing = Math.max(range('la', 2), range('ra', 2), range('ll', 2), range('rl', 2), range('la', 1), range('ll', 1));
  const dist2 = Math.hypot(walk[walk.length - 1].pos[0] - result.idle.pos[0], walk[walk.length - 1].pos[2] - result.idle.pos[2]);
  const rise = Math.max(...jump.map((s) => s.pos[1])) - result.after.pos[1];
  console.log(JSON.stringify({ swing: +swing.toFixed(3), walked: +dist2.toFixed(2), jumpRise: +rise.toFixed(2), tracks: result.idle.tracks, animate: result.idle.animate, running: result.idle.running, errors: result.errors, chocolateRequests: bad }));
  await b.close();
  srv.close();
  process.exit(swing > 0.3 && dist2 > 3 && rise > 1 && bad.length === 0 ? 0 : 1);
})().catch((e) => { console.log('FAIL', e.message); process.exit(1); });

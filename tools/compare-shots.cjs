const p = require('/tmp/claude-1000/pp/node_modules/puppeteer-core');
const http = require('http');
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');
const dist = process.env.DIST || '/var/www/swirl/dist/client';
const outDir = process.env.OUT || '/tmp/swirl/compare';
const places = (process.env.PLACES || '564:2012,972:2012,3:2018,1669:2012').split(',').map((s) => s.split(':').map(Number));
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
fs.mkdirSync(outDir, { recursive: true });
const page = `<!doctype html><meta charset=utf-8><body style="margin:0;background:#000"><canvas id=swirl-canvas style="width:100vw;height:100vh;display:block" tabindex=0></canvas><div id=swirl-status style="position:fixed;top:0;left:0;color:#fff"></div><script>window.solarClientConfig={};window.solarClientReady=function(){window.__ready=true};window.solarClientFailed=function(r){window.__failed=r}</script><script type=module src="/dist/entry.js"></script>`;
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
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(page);
});
const shoot = async (b, port, wsPort, ref, poses) => {
  const pg = await b.newPage();
  await pg.setViewport({ width: 960, height: 600 });
  pg.on('console', (m) => { if (m.type() === 'error') console.log('console', m.text().slice(0, 200)); });
  pg.on('pageerror', (e) => console.log('pageerror', String(e).slice(0, 200)));
  await pg.goto('http://127.0.0.1:' + port + '/?swirlws=' + encodeURIComponent('ws://127.0.0.1:' + wsPort + '/ws') + (ref ? '&swirlref=three' : ''), { waitUntil: 'domcontentloaded' });
  await pg.waitForFunction('window.__ready === true || window.__failed', { timeout: 40000 });
  await pg.waitForFunction('window.solarLua && window.solarLua.ready()', { timeout: 60000 });
  await pg.waitForFunction('window.solarLua.world.own', { timeout: 30000 });
  await wait(+(process.env.SETTLE || 9000));
  const info = await pg.evaluate(() => { document.getElementById('chocolate').style.display = 'none'; const L = window.solarLua; return { pos: L.ownPos(), era: L.world.era }; });
  if (process.env.EVAL) { const r = await pg.evaluate(process.env.EVAL); if (process.env.EVALOUT) require('fs').writeFileSync(process.env.EVALOUT, String(r).replace(/^data:image\/png;base64,/, ''), 'base64'); else console.log('eval', JSON.stringify(r)); }
  const shots = [];
  for (let i = 0; i < poses.length; i++) {
    const [dx, dy, dz] = poses[i];
    await pg.evaluate(async (pos, dx, dy, dz) => {
      const L = window.solarLua;
      const C = await import(new URL('./chocolate/shared/cframe.js', document.querySelector('script[src$="entry.js"]').src).href);
      L.world.camera.props.set('CameraType', 6);
      L.world.camera.props.set('CFrame', C.lookAt(pos[0] + dx, pos[1] + dy, pos[2] + dz, pos[0], pos[1] + 2, pos[2]));
    }, info.pos, dx, dy, dz);
    await wait(2500);
    const f = outDir + '/' + (ref ? 'chocolate' : 'swirl') + '-' + i + '.png';
    await pg.screenshot({ path: f });
    shots.push(f);
  }
  if (process.env.FAILS) console.log('failures', JSON.stringify(await pg.evaluate(() => window.solarAssetFailures || [])));
  await pg.close();
  return info;
};
(async () => {
  await new Promise((r) => srv.listen(0, '127.0.0.1', r));
  const port = srv.address().port;
  const b = await p.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const poses = [[10, 8, 16], [-18, 12, -10], [0, 30, 40]];
  for (const [place, era] of places) {
    const wsPort = 4500 + (place % 400);
    const args = ['--place=' + place, '--port=' + wsPort, '--site=http://127.0.0.1'].concat(era === 2018 ? ['--era=2018'] : []);
    let sv = spawn('/var/www/swirl/target/release/swirl-server', args, { stdio: 'ignore' });
    await wait(2500);
    const dir = outDir + '/' + place;
    fs.mkdirSync(dir, { recursive: true });
    const save = outDir;
    process.env.OUT = dir;
    try {
      const sw = await shoot(b, port, wsPort, false, poses);
      sv.kill(); await wait(800);
      sv = spawn('/var/www/swirl/target/release/swirl-server', args, { stdio: 'ignore' }); await wait(2500);
      const ch = await shoot(b, port, wsPort, true, poses);
      console.log(place, JSON.stringify({ swirl: sw, chocolate: ch }));
    } catch (e) {
      console.log(place, 'FAIL', e.message);
    }
    for (const f of fs.readdirSync(outDir)) if (/^(swirl|chocolate)-\d\.png$/.test(f)) fs.renameSync(outDir + '/' + f, dir + '/' + f);
    sv.kill();
    await wait(500);
    void save;
  }
  await b.close();
  srv.close();
  process.exit(0);
})();

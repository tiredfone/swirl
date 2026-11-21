const p = require('/tmp/claude-1000/pp/node_modules/puppeteer-core');
const http = require('http');
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');
const dist = process.env.DIST || '/var/www/swirl/dist/client';
const outFile = process.argv[2] || '/tmp/swirl/mv/parity.json';
const tol = Number(process.env.TOL || 0.01);
fs.mkdirSync('/tmp/swirl/mv', { recursive: true });
execFileSync('node', ['/var/www/swirl/tools/movement-run.mjs', '/var/www/html/chocolate', '/tmp/swirl/mv/ref.json']);
const ref = JSON.parse(fs.readFileSync('/tmp/swirl/mv/ref.json'));
const html = `<!doctype html><meta charset=utf-8><script type=importmap>{"imports":{"three":"/dist/chocolate/client/vendor/three.module.js"}}</script><body>ok`;
const srv = http.createServer((req, res) => {
  const u = decodeURIComponent(req.url.split('?')[0]);
  let f = null;
  if (u.startsWith('/dist/')) f = path.join(dist, u.slice(6));
  else if (u === '/scenarios.mjs') f = '/var/www/swirl/tools/movement-scenarios.mjs';
  if (!f) {
    res.writeHead(200, { 'Content-Type': 'text/html' });
    res.end(html);
    return;
  }
  try {
    const types = { '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json' };
    res.writeHead(200, { 'Content-Type': types[path.extname(f)] || 'application/octet-stream' });
    res.end(fs.readFileSync(f));
  } catch (e) {
    res.writeHead(404);
    res.end();
  }
});
(async () => {
  await new Promise((r) => srv.listen(0, '127.0.0.1', r));
  const b = await p.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox'] });
  const pg = await b.newPage();
  pg.on('pageerror', (e) => console.log('pageerror', String(e).slice(0, 300)));
  await pg.goto('http://127.0.0.1:' + srv.address().port + '/');
  const traces = await pg.evaluate(async () => {
    const { runAll } = await import('/scenarios.mjs');
    const R = (await import('/dist/chocolate/client/vendor/rapier.js')).default;
    return await runAll((q) => import('/dist/chocolate/' + q), R);
  });
  await b.close();
  srv.close();
  const rows = [];
  let worst = 0, mism = 0, frames = 0;
  for (const [name, t] of Object.entries(traces)) {
    const r = ref[name];
    let md = 0, sm = 0;
    for (let i = 0; i < t.length; i++) {
      md = Math.max(md, Math.hypot(t[i][0] - r[i][0], t[i][1] - r[i][1], t[i][2] - r[i][2]));
      if (t[i][3] !== r[i][3]) sm++;
    }
    const last = t[t.length - 1];
    rows.push({ scenario: name, frames: t.length, maxPosDiff: +md.toFixed(5), stateMismatches: sm, final: [last[0], last[1], last[2], last[3]] });
    worst = Math.max(worst, md);
    mism += sm;
    frames += t.length;
  }
  fs.writeFileSync(outFile, JSON.stringify({ worst, mism, frames, rows }, null, 1));
  console.log(JSON.stringify({ scenarios: rows.length, frames, worstPosDiff: +worst.toFixed(5), stateMismatches: mism, tolerance: tol }));
  if (process.env.VERBOSE) for (const r of rows) console.log(JSON.stringify(r));
  process.exit(worst <= tol && mism === 0 ? 0 : 1);
})().catch((e) => { console.log('FAIL', e.message); process.exit(1); });

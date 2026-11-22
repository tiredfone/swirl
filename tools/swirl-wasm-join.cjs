const p = require('/tmp/claude-1000/pp/node_modules/puppeteer-core');
const fs = require('fs');
const [, , place, version, port, secs] = process.argv;
if (!place || !version || !port || port === '4201' || port === '4202') {
  console.log('usage: node tools/swirl-wasm-join.cjs <place> <client version> <spare port> [seconds]');
  process.exit(1);
}
const acct = JSON.parse(fs.readFileSync('/home/fone/.config/solar/testing-account.json'));
(async () => {
  const b = await p.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const pg = await b.newPage();
  await pg.setViewport({ width: 1100, height: 700 });
  const log = { ws: [], console: [], pageErrors: [] };
  pg.on('console', (m) => { const t = m.type(); if (t === 'error' || t === 'warning') log.console.push(t + ': ' + m.text().slice(0, 240)); });
  pg.on('pageerror', (e) => log.pageErrors.push(String(e).slice(0, 300)));
  const cdp = await pg.createCDPSession();
  await cdp.send('Network.enable');
  cdp.on('Network.webSocketCreated', (e) => log.ws.push('created ' + e.url.replace(/ticket=[^&]*/, 'ticket=...')));
  cdp.on('Network.webSocketClosed', () => log.ws.push('closed'));
  cdp.on('Network.webSocketFrameReceived', (e) => { if (log.ws.length < 8) log.ws.push('frame ' + String(e.response.payloadData).slice(0, 50)); });
  await pg.goto('http://127.0.0.1/login', { waitUntil: 'networkidle2' });
  await pg.evaluate(async (u, pw) => {
    const t = document.querySelector('input[name=_csrf]').value;
    await fetch('/api/login', { method: 'POST', body: new URLSearchParams({ _csrf: t, Username: u, Password: pw }), credentials: 'same-origin' });
  }, acct.username, acct.password);
  await pg.goto('http://127.0.0.1/play/' + place + '?swirl=' + version + '&swirlws=' + encodeURIComponent('ws://127.0.0.1:' + port + '/ws'), { waitUntil: 'domcontentloaded' });
  await new Promise((r) => setTimeout(r, (+secs || 20) * 1000));
  const st = await pg.evaluate(() => ({
    swirlPage: !!document.getElementById('swirl-canvas'),
    fatal: document.getElementById('swirl-fatal') ? !document.getElementById('swirl-fatal').hidden : null,
    status: document.getElementById('swirl-status') ? document.getElementById('swirl-status').textContent.slice(0, 120) : null,
    lua: !!window.solarLua,
    luaReady: window.solarLua && window.solarLua.ready ? window.solarLua.ready() : null,
    parts: window.solarClient && window.solarClient.debug_parts ? window.solarClient.debug_parts() : null,
    own: window.solarLua && window.solarLua.ownPos ? window.solarLua.ownPos() : null,
    errors: window.solarLua && window.solarLua.errors ? window.solarLua.errors.slice(0, 5) : null,
  }));
  if (process.env.SHOT) fs.writeFileSync(process.env.SHOT, await pg.screenshot());
  console.log(JSON.stringify({ st, ...log }, null, 1));
  await b.close();
  process.exit(0);
})();

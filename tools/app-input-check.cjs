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
const pageHtml = /const page = `([\s\S]*?)`;/.exec(gui)[1].replace('window.solarClientConfig={}', 'window.solarClientConfig={endpoints:{thumbnail:"/image/headshot?userId={id}",avatarThumbnail:"/thumbnail/user/{id}",asset:"/asset/?id={id}",itemThumbnail:"/Image/Item.ashx?ID={id}"}}');
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

const mkPage = async (b, url, w, h) => {
  const pg = await b.newPage();
  await pg.setViewport({ width: w, height: h });
  await pg.goto(url, { waitUntil: 'domcontentloaded' });
  await pg.waitForFunction('window.__ready === true || window.__failed', { timeout: 30000 });
  await pg.waitForFunction('window.solarApp && window.solarApp.vm', { timeout: 60000 });
  return pg;
};
(async () => {
  await new Promise((r) => srv.listen(0, '127.0.0.1', r));
  const era = Number(process.argv[3] || 2018);
  const b = await p.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--disable-background-timer-throttling', '--disable-renderer-backgrounding', '--disable-backgrounding-occluded-windows'] });
  const url = 'http://127.0.0.1:' + srv.address().port + '/?swirlws=' + encodeURIComponent('ws://127.0.0.1:' + wsPort + '/ws') + '&swirlapp=1';
  const fails = [];
  const out = {};
  const a = await mkPage(b, url, 1100, 700);
  const b2 = await p.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const c2 = await mkPage(b2, url, 1100, 700);
  await a.bringToFront();
  await wait(9000);
  const run = (pg, code) => pg.evaluate((c) => window.solarApp.vm.run_script(c), code);
  const logs = (pg) => pg.evaluate(() => (window.solarApp.logs || []).map((x) => String(x.msg)));
  const look = async () => {
    const n = (await logs(a)).filter((m) => m.startsWith('CAMLV')).length;
    await run(a, 'local l = workspace.CurrentCamera.CFrame.lookVector print("CAMLV", l.x, l.y, l.z)');
    for (let i = 0; i < 20; i++) {
      await wait(250);
      const all = (await logs(a)).filter((m) => m.startsWith('CAMLV'));
      if (all.length > n) return all.pop().split(/\s+/).slice(1).map(Number);
    }
    return [0, 0, 0];
  };
  const canvasBox = await a.evaluate(() => { const r = document.getElementById('swirl-canvas').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; });
  const before = await look();
  await a.mouse.move(canvasBox[0], canvasBox[1]);
  await a.mouse.down({ button: 'right' });
  await wait(300);
  let lockedDuring = false;
  for (let i = 0; i < 10 && !lockedDuring; i++) {
    lockedDuring = await a.evaluate(() => !!document.pointerLockElement);
    if (!lockedDuring) await wait(200);
  }
  const overlayDuring = await a.evaluate(() => [...document.querySelectorAll('img')].some((i) => i.style.position === 'fixed' && i.style.display === 'block'));
  let after = before;
  for (let attempt = 0; attempt < 4; attempt++) {
    for (let i = 1; i <= 12; i++) {
      await a.evaluate(() => { const c = document.getElementById('swirl-canvas'); c.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, movementX: 6, movementY: 5, clientX: 550, clientY: 350, pointerType: 'mouse' })); });
      await wait(80);
    }
    await wait(300);
    after = await look();
    if (Math.abs(after[1] - before[1]) > 0.03 && Math.abs(after[0] - before[0]) + Math.abs(after[2] - before[2]) > 0.05) break;
  }
  await a.mouse.up({ button: 'right' });
  let lockedAfter = true;
  for (let i = 0; i < 20 && lockedAfter; i++) {
    await wait(250);
    lockedAfter = await a.evaluate(() => !!document.pointerLockElement);
  }
  await wait(300);
  const overlayAfter = await a.evaluate(() => [...document.querySelectorAll('img')].some((i) => i.style.position === 'fixed' && i.style.display === 'block'));
  out.cam = { before, after, lockedDuring, overlayDuring, lockedAfter, overlayAfter };
  const yawMoved = Math.abs(after[0] - before[0]) + Math.abs(after[2] - before[2]);
  const pitchMoved = Math.abs(after[1] - before[1]);
  if (yawMoved < 0.05) fails.push('right-drag did not change yaw ' + JSON.stringify(out.cam));
  if (pitchMoved < 0.03) fails.push('right-drag did not change pitch ' + JSON.stringify(out.cam));
  if (lockedAfter) fails.push('pointer lock stayed on after releasing right-click');
  if (overlayAfter) fails.push('cursor overlay stayed visible after releasing right-click');
  if (lockedDuring && !overlayDuring) fails.push('cursor hidden while right-dragging');
  await a.mouse.down({ button: 'right' });
  await wait(300);
  await a.evaluate(() => window.dispatchEvent(new Event('blur')));
  await wait(400);
  if (await a.evaluate(() => !!document.pointerLockElement)) fails.push('pointer lock stayed on after blur');
  await a.mouse.up({ button: 'right' });
  await wait(300);
  const who = async (pg) => {
    await run(pg, 'print("ME", game.Players.LocalPlayer.Name)');
    for (let i = 0; i < 20; i++) {
      await wait(300);
      const m = (await logs(pg)).filter((x) => x.startsWith('ME ')).pop();
      if (m) return m.slice(3);
    }
    return '';
  };
  const nameA = await who(a);
  const nameB = await who(c2);
  if (!nameA || !nameB) fails.push('could not read player names');
  await run(c2, 'for _, pl in ipairs(game.Players:GetPlayers()) do pl.Chatted:Connect(function(m) print("GOTCHAT", m) end) end');
  const msg = 'hello from ' + (nameA || 'x').toLowerCase();
  if (era === 2018) {
    await a.mouse.click(270, 374);
    await wait(1500);
    await a.keyboard.type(msg, { delay: 90 });
    await wait(500);
    await a.keyboard.press('Enter');
    await wait(800);
  } else {
    await run(a, 'game.Players:Chat("' + msg + '")');
  }
  const seen = (pg) => pg.evaluate((t) => [...document.querySelectorAll('*')].some((e) => !e.children.length && e.textContent.includes(t) && e.getBoundingClientRect().width > 0), msg);
  out.chatLogs = (await logs(a)).filter((m) => /rror/.test(m)).slice(0, 3).map((m) => m.slice(0, 200));
  if (process.env.SHOT) { await a.screenshot({ path: process.env.SHOT + '-a.png' }); await c2.screenshot({ path: process.env.SHOT + '-b.png' }); }
  out.chat = { sender: false, receiver: false };
  for (let i = 0; i < 12 && !out.chat.receiver; i++) {
    out.chat.sender = out.chat.sender || (await seen(a));
    out.chat.receiver = (await seen(c2)) || (await logs(c2)).some((m) => m.startsWith('GOTCHAT') && m.includes(msg));
    if (!out.chat.receiver) await wait(400);
  }
  if (!out.chat.receiver) fails.push('chat message from one player did not appear for the other');
  const listText = (pg) => pg.evaluate(() => [...document.querySelectorAll('*')].filter((e) => !e.children.length && e.getBoundingClientRect().width > 0 && e.getBoundingClientRect().y < 400 && e.getBoundingClientRect().x > 700).map((e) => e.textContent.trim()).join('|'));
  const lt = await listText(c2);
  out.list = lt;
  if (!lt.includes(nameA) || !lt.includes(nameB)) fails.push('player list does not show both players: ' + lt);
  if (process.env.BADGE) {
    out.badge = await c2.evaluate(() => [...document.querySelectorAll('[data-ipath*="MembershipIcon"], img')].filter((e) => e.getBoundingClientRect().x > 700).map((e) => (e.tagName === 'IMG' ? e.src : (e.querySelector('img') || {}).src || getComputedStyle(e).backgroundImage)).filter(Boolean));
    if (!out.badge.some((u) => /asset/.test(u))) fails.push('player list shows no SolarPerks badge: ' + JSON.stringify(out.badge));
  }
  console.log(JSON.stringify({ out, fails }));
  await b.close();
  await b2.close();
  srv.close();
  process.exit(fails.length ? 1 : 0);
})();

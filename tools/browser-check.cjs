const p = require('/tmp/claude-1000/pp/node_modules/puppeteer-core');
const fs = require('fs');
const [, , place, version, shot] = process.argv;
const acct = JSON.parse(fs.readFileSync('/home/fone/.config/solar/testing-account.json'));
const user = acct.username || acct.Username || acct.user || acct.name;
const pass = acct.password || acct.Password;
(async () => {
  const stamp = '/tmp/swirl/last-join';
  try {
    const last = Number(fs.readFileSync(stamp, 'utf8'));
    const wait = last + 80000 - Date.now();
    if (wait > 0) await new Promise((r) => setTimeout(r, wait));
  } catch (e) {}
  fs.writeFileSync(stamp, String(Date.now()));
  const b = await p.launch({ executablePath: '/usr/bin/chromium', headless: 'new', args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const pg = await b.newPage();
  await pg.setViewport({ width: 1100, height: 700 });
  const log = { console: [], pageErrors: [], failed: [], ws: [], assets: { ok: 0, bad: 0 }, chocolateRequests: [] };
  pg.on('request', (r) => { try { if (new URL(r.url()).pathname.startsWith('/chocolate/')) log.chocolateRequests.push(r.url().slice(0, 160)); } catch (e) {} });
  pg.on('console', (m) => log.console.push(m.type() + ': ' + m.text().slice(0, 300)));
  pg.on('pageerror', (e) => log.pageErrors.push(String(e).slice(0, 400)));
  pg.on('requestfailed', (r) => log.failed.push(r.url().slice(0, 200) + ' ' + (r.failure() && r.failure().errorText)));
  pg.on('response', (r) => { if (r.url().includes('/asset/?id=')) { if (r.status() < 400) log.assets.ok++; else log.assets.bad++; } if (r.status() >= 400) log.failed.push(r.status() + ' ' + r.url().slice(0, 200)); });
  const cdp = await pg.createCDPSession();
  await cdp.send('Network.enable');
  cdp.on('Network.webSocketCreated', (e) => log.ws.push('created ' + e.url.replace(/ticket=[^&]*/, 'ticket=...')));
  cdp.on('Network.webSocketClosed', () => log.ws.push('closed'));
  cdp.on('Network.webSocketFrameReceived', (e) => { if (log.ws.length < 6) log.ws.push('frame ' + String(e.response.payloadData).slice(0, 40)); });
  await pg.goto('http://127.0.0.1/login', { waitUntil: 'networkidle2' });
  const ok = await pg.evaluate(async (u, pw) => {
    const t = document.querySelector('input[name=_csrf]').value;
    const r = await fetch('/api/login', { method: 'POST', body: new URLSearchParams({ _csrf: t, Username: u, Password: pw }), credentials: 'same-origin' });
    return r.status;
  }, user, pass);
  const url = 'http://127.0.0.1/play/' + place + (version ? '?swirl=' + version : '') + (process.env.SWIRLWS ? '&swirlws=' + encodeURIComponent(process.env.SWIRLWS) : '');
  await pg.evaluateOnNewDocument(() => { window.__ready = false; });
  await pg.goto(url, { waitUntil: 'domcontentloaded' });
  await new Promise((r) => setTimeout(r, 10000));
  const state = await pg.evaluate(() => ({
    statusHidden: document.getElementById('swirl-status') ? document.getElementById('swirl-status').hidden : null,
    fatalHidden: document.getElementById('swirl-fatal') ? document.getElementById('swirl-fatal').hidden : null,
    hasSwirlPage: !!document.getElementById('swirl-canvas'),
    config: !!window.solarClientConfig,
    luaFailed: window.solarLuaFailed || null,
  }));
  await pg.waitForFunction('(window.solarLua && window.solarLua.ready()) || (window.solarApp && window.solarApp.vm && document.querySelector("[data-ipath$=PlayerListScreen]") && document.querySelector("[data-ipath$=CurrentLoadout]"))', { timeout: 40000 }).catch(() => {});
  await new Promise((r) => setTimeout(r, 4000));
  const appMode = await pg.evaluate(() => !!window.solarApp);
  const lua = appMode ? await pg.evaluate(() => {
    if (!window.solarApp.vm) return { present: false, failed: 'no vm', app: true, errors: window.solarApp.errors.slice(0, 5) };
    const paths = [...document.querySelectorAll('[data-ipath]')].map((e) => e.getAttribute('data-ipath'));
    const has = (n) => paths.some((x) => x.endsWith('.' + n));
    return { present: true, ready: !!window.solarApp.vm, era: window.solarApp.vm.client_era(), playerList: has('PlayerListScreen') || has('PlayerListContainer'), hotbarOrTopbar: has('CurrentLoadout') || has('TopBarFrame') || has('TopBarContainer'), errors: window.solarApp.errors.slice(0, 5), nodes: paths.length, app: true };
  }) : await pg.evaluate(() => {
    const L = window.solarLua;
    if (!L) return { present: false, failed: window.solarLuaFailed || null };
    const names = L.world.robloxGui.children.map((c) => c.props.get('Name'));
    const pl = L.world.player;
    const pgui = pl && pl.findChildOfClass('PlayerGui');
    return { present: true, ready: L.ready(), era: L.world.era, playerList: names.includes('PlayerListScreen') || names.includes('PlayerListContainer'), hotbarOrTopbar: names.includes('CurrentLoadout') || names.includes('TopBarContainer'), coreChildren: names.length, playerGui: pgui ? pgui.children.length : -1, errors: L.errors.slice(0, 10) };
  });
  let focus = null;
  if (process.env.FOCUS) {
    const [x, y, z, d] = process.env.FOCUS.split(',').map(Number);
    await pg.evaluate((x, y, z, d) => window.solarClient.focus(x, y, z, d), x, y, z, d);
    await new Promise((r) => setTimeout(r, 6000));
    focus = await pg.evaluate(() => ({ parts: window.solarClient.debug_parts(), dbg: window.solarClient.debug(), pos: window.solarClient.own_pos(), meshes: window.solarClient.loaded_meshes(), textures: window.solarClient.loaded_textures() }));
  }
  let walk = null;
  let animSwing = 0;
  let animInfo = null;
  if (process.env.WALK) {
    const pos = () => pg.evaluate(() => (window.solarApp && window.solarApp.vm ? (() => { const o = JSON.parse(window.solarApp.vm.own_state() || 'null'); return o ? [o.cf[0], o.cf[1], o.cf[2]] : null; })() : window.solarLua && window.solarLua.ownPos ? window.solarLua.ownPos() : null));
    const limbs = () => pg.evaluate(() => { if (window.solarApp && window.solarApp.vm) { const m = JSON.parse(window.solarApp.vm.motor_angles()); const ks = Object.keys(m); const l = ks.find((k) => k.endsWith(':Left Hip')); const r = ks.find((k) => k.endsWith(':Right Hip')); return l && r ? { ll: [0, 0, m[l]], rl: [0, 0, 0], animate: ks.length > 0 } : null; } if (!window.solarLua) return null; const w = window.solarLua.world; const m = w.own && w.own.model; if (!m) return null; const t = m.findChild('Torso') || m.findChild('HumanoidRootPart'); const tc = t.props.get('CFrame'); const rel = (n) => { const q = m.findChild(n); if (!q) return null; const c = q.props.get('CFrame'); const d = [c[0] - tc[0], c[1] - tc[1], c[2] - tc[2]]; return [tc[3] * d[0] + tc[6] * d[1] + tc[9] * d[2], tc[4] * d[0] + tc[7] * d[1] + tc[10] * d[2], tc[5] * d[0] + tc[8] * d[1] + tc[11] * d[2]]; }; let tracks = 0; try { tracks = (w.anims.playing(w.own.hum) || []).length; } catch (e) {} return { ll: rel('Left Leg') || rel('LeftUpperLeg'), rl: rel('Right Leg') || rel('RightUpperLeg'), animate: !!m.findChild('Animate'), tracks }; });
    await pg.focus('#swirl-canvas').catch(() => {});
    const a = await pos();
    await pg.keyboard.down('w');
    const swing = [];
    for (let i = 0; i < 4; i++) { await new Promise((r) => setTimeout(r, 100)); const l = await limbs(); if (l && l.ll && l.rl) swing.push(l.ll[2] - l.rl[2]); animInfo = l; }
    await pg.keyboard.up('w');
    animSwing = swing.length ? Math.max(...swing) - Math.min(...swing) : 0;
    const b2 = await pos();
    await pg.keyboard.down(' ');
    await new Promise((r) => setTimeout(r, 300));
    await pg.keyboard.up(' ');
    const c2 = await pos();
    if (shot) {
      await pg.keyboard.down('s');
      await new Promise((r) => setTimeout(r, 120));
      fs.writeFileSync(shot.replace(/\.png$/, '') + '.walk.jpg', await pg.screenshot({ type: 'jpeg', quality: 40 }));
      await pg.keyboard.up('s');
    }
    walk = { animSwing, animate: animInfo && animInfo.animate, start: a, afterW: b2, afterJump: c2, moved: a && b2 ? Math.hypot(b2[0] - a[0], b2[2] - a[2]) : 0, jumped: a && c2 ? c2[1] - b2[1] : 0 };
  }
  const b64 = (await pg.screenshot({ encoding: 'base64' }));
  if (shot) fs.writeFileSync(shot, Buffer.from(b64, 'base64'));
  const drawn = await pg.evaluate(async (data) => {
    const img = new Image();
    img.src = 'data:image/png;base64,' + data;
    await img.decode();
    const t = document.createElement('canvas');
    t.width = 220; t.height = 140;
    const x = t.getContext('2d');
    x.drawImage(img, 0, 0, 220, 140);
    const d = x.getImageData(0, 0, 220, 140).data;
    const set = new Set();
    for (let i = 0; i < d.length; i += 4) set.add((d[i] >> 4) + ',' + (d[i + 1] >> 4) + ',' + (d[i + 2] >> 4));
    return set.size;
  }, b64);
  console.log(JSON.stringify({ lua, focus, walk, login: ok, url: url.replace(/\?.*/, '?swirl=' + version), state, distinctColors: drawn, ...log }, null, 1));
  await b.close();
  const meshOk = !process.env.MIN_MESHES || (focus && focus.meshes >= Number(process.env.MIN_MESHES) && focus.textures >= 1);
  const walkOk = !process.env.WALK || (walk && walk.moved > 3 && walk.afterW[1] > -5 && walk.jumped > 1 && walk.animSwing > 0.2 && walk.animate);
  const noChoc = log.chocolateRequests.length === 0;
  if (!noChoc) console.log('requests to /chocolate/ seen');
  const luaOk = !!(lua && lua.present && lua.ready && lua.playerList && lua.hotbarOrTopbar && lua.errors.length === 0);
  if (!luaOk) console.log('lua gate failed');
  process.exit(luaOk && state.hasSwirlPage && state.statusHidden !== false && state.fatalHidden === true && drawn > 3 && walkOk && meshOk && noChoc ? 0 : 1);
})();

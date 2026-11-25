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
  let legCount = 0;
  const leg = async () => {
    await pg.evaluate(() => window.solarApp.vm.run_script('local c = workspace:FindFirstChildOfClass("Model") for _, m in ipairs(workspace:GetChildren()) do if m:FindFirstChild("LeftUpperLeg") then c = m end end local l = c.LeftUpperLeg.CFrame.p - c.LowerTorso.CFrame.p local r = c.LowerTorso.CFrame:inverse() * c.LeftUpperLeg.CFrame.p print("LEGPOS", r.x, r.y, r.z)'));
    for (let i = 0; i < 20; i++) {
      await wait(250);
      const l = await pg.evaluate(() => { const x = (window.solarApp.logs || []).map((m) => String(m.msg)).filter((m) => m.startsWith('LEGPOS')); return x.length; });
      if (l > legCount) {
        legCount = l;
        break;
      }
    }
    return pg.evaluate(() => { const l = (window.solarApp.logs || []).map((x) => String(x.msg)).filter((m) => m.startsWith('LEGPOS')).pop(); return l ? l.split(/\s+/).slice(1).map(Number) : null; });
  };
  const idle = await leg();
  await pg.evaluate(() => window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyW', key: 'w', bubbles: true })));
  await wait(700);
  const walkSamples = [];
  for (let i = 0; i < 5; i++) {
    walkSamples.push(await leg());
    await wait(150);
  }
  await pg.evaluate(() => window.dispatchEvent(new KeyboardEvent('keyup', { code: 'KeyW', key: 'w', bubbles: true })));
  const legShot = process.env.SHOT_WALK;
  if (legShot) await pg.screenshot({ path: legShot });
  const diff = idle ? Math.max(0, ...walkSamples.filter(Boolean).map((w) => idle.reduce((a, v, i) => a + Math.abs(v - w[i]), 0))) : 0;
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
  if (mode !== 'js') {
    const logsOf = () => pg.evaluate(() => (window.solarApp.logs || []).map((x) => String(x.msg)));
    const run = (code) => pg.evaluate((c) => window.solarApp.vm.run_script(c), code);
    const centre = (re) => pg.evaluate((re) => { const rx = new RegExp(re); for (const e of document.querySelectorAll('[data-ipath]')) { if (rx.test(e.getAttribute('data-ipath'))) { const r = e.getBoundingClientRect(); if (r.width > 0 && r.height > 0) return [r.x + r.width / 2, r.y + r.height / 2]; } } return null; }, re);
    const press = async (pt) => {
      await pg.mouse.move(pt[0] - 8, pt[1] - 3, { steps: 3 });
      await pg.mouse.move(pt[0], pt[1], { steps: 2 });
      await wait(250);
      await pg.mouse.down();
      await wait(250);
      await pg.mouse.up();
      await wait(1200);
    };
    const cursor = await pg.evaluate(() => document.documentElement.style.getPropertyValue('--ch-cursor'));
    if (!/url\(/.test(cursor)) fails.push('custom cursor not set for GUI elements: ' + cursor);
    await run('local p = game.Players.LocalPlayer print("THUMB", pcall(function() return game.Players:GetUserThumbnailAsync(5, Enum.ThumbnailType.HeadShot, Enum.ThumbnailSize.Size48x48) end))');
    await wait(1200);
    if (!(await logsOf()).some((m) => m.startsWith('THUMB') && m.includes('/image/headshot?userId=5'))) fails.push('GetUserThumbnailAsync gave no headshot URL: ' + (await logsOf()).filter((m) => m.startsWith('THUMB')).join(','));
    await run('local g = Instance.new("ScreenGui") g.Parent = game.Players.LocalPlayer:WaitForChild("PlayerGui") local f = Instance.new("Frame") f.Active = true f.Size = UDim2.new(0, 200, 0, 100) f.Position = UDim2.new(0, 700, 0, 450) f.Parent = g local b = Instance.new("ImageButton") b.Size = UDim2.new(0, 50, 0, 50) b.Position = UDim2.new(0, 10, 0, 10) b.Parent = f f.InputChanged:Connect(function() print("GUI-C") end) b.InputBegan:Connect(function(io) print("GUI-B", io.UserInputType == Enum.UserInputType.MouseButton1) end) b.InputEnded:Connect(function() print("GUI-E") end)');
    await wait(1500);
    await pg.keyboard.press('Escape');
    await wait(800);
    await pg.mouse.move(690, 480);
    await pg.mouse.move(725, 520, { steps: 3 });
    await pg.mouse.down();
    await wait(200);
    await pg.mouse.up();
    await wait(1000);
    const gl = await logsOf();
    if (!gl.includes('GUI-C') || !gl.includes('GUI-B true') || !gl.includes('GUI-E')) fails.push('GuiObject InputBegan/Changed/Ended not delivered: ' + gl.filter((m) => m.startsWith('GUI')).join(','));
    await pg.keyboard.press('Escape');
    await wait(1200);
    const tabShown = (marker) => pg.evaluate((t) => [...document.querySelectorAll('*')].some((e) => !e.children.length && e.textContent.trim() === t && e.getBoundingClientRect().width > 0 && e.getBoundingClientRect().y > 100 && e.getBoundingClientRect().y < 650), marker);
    const openTab = async (name, marker) => {
      for (let i = 0; i < 4 && !(await tabShown(marker)); i++) {
        const pt = await pg.evaluate((t) => { for (const e of document.querySelectorAll('*')) if (!e.children.length && e.textContent.trim() === t) { const r = e.getBoundingClientRect(); if (r.width > 0 && r.height > 0) return [r.x + r.width / 2, r.y + r.height / 2]; } return null; }, name);
        if (pt) await press(pt);
      }
      return tabShown(marker);
    };
    if (!(await openTab('Settings', 'Shift Lock Switch'))) fails.push('Settings tab did not open');
    else {
      const st = await centre('VolumeFrame.*\\.Step1$');
      if (!st) fails.push('volume slider missing');
      else {
        await press(st);
        const sel = await pg.evaluate(() => { let step1 = '', step2 = ''; for (const e of document.querySelectorAll('[data-ipath]')) { const p = e.getAttribute('data-ipath'); if (/VolumeFrame.*\.Step2$/.test(p) && e.getBoundingClientRect().width > 0) step2 = getComputedStyle(e).backgroundColor; if (/VolumeFrame.*\.Step1$/.test(p) && e.getBoundingClientRect().width > 0) step1 = (e.querySelector('img') || {}).src || ''; } return [step1, step2]; });
        if (!/SelectedBarLeft/.test(sel[0]) || /0, 162, 255/.test(sel[1])) fails.push('volume slider did not follow a click: ' + JSON.stringify(sel));
      }
    }
    if (!(await openTab('Report', 'Type Of Abuse'))) fails.push('Report tab did not open');
    else {
      const tb = await centre('Short Description \\(Optional\\)Frame\\.TextBox$');
      if (tb) {
        await press(tb);
        await pg.keyboard.type('hello');
        await wait(600);
        const val = await pg.evaluate(() => [...document.querySelectorAll('textarea,input')].filter((e) => e.getBoundingClientRect().width > 0).map((e) => e.value).join('|'));
        if (!val.includes('hello')) fails.push('report text box did not take typing: ' + val);
      } else fails.push('report text box missing');
    }
  }
  if (mode !== 'js') {
    const logsOf2 = () => pg.evaluate(() => (window.solarApp.logs || []).map((x) => String(x.msg)));
    const run2 = (code) => pg.evaluate((c) => window.solarApp.vm.run_script(c), code);
    await pg.evaluate(() => window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyW', key: 'w', bubbles: true })));
    await wait(300);
    await pg.evaluate(() => { window.dispatchEvent(new Event('blur')); });
    await wait(300);
    const pollLog = async (prefix) => {
      for (let i = 0; i < 24; i++) {
        await wait(250);
        if ((await logsOf2()).some((m) => m.startsWith(prefix))) return;
      }
    };
    await run2('print("KEYW", game:GetService("UserInputService"):IsKeyDown(Enum.KeyCode.W))');
    await pollLog('KEYW');
    if (!(await logsOf2()).some((m) => m === 'KEYW false')) fails.push('a key stayed down after the window lost focus');
    await run2('local h = game.Players.LocalPlayer.Character.Humanoid h.Died:Connect(function() print("DIED-EV") end) h.Health = 0');
    await wait(3500);
    await run2('local c = game.Players.LocalPlayer.Character local r = c and c:FindFirstChild("UpperTorso") local h = c and c:FindFirstChild("Head") print("DEATHDIST", (r and h) and (r.Position - h.Position).Magnitude or -1)');
    await pollLog('DEATHDIST');
    const dl = await logsOf2();
    if (!dl.includes('DIED-EV')) fails.push('Humanoid.Died did not fire on the client');
    const dd = dl.filter((m) => m.startsWith('DEATHDIST')).pop();
    const dv = dd ? Number(dd.split(' ')[1]) : -1;
    if (!(dv > 1.45)) fails.push('character did not fall apart on death (head to torso ' + dv + ')');
    await wait(7500);
    const menuUp = await pg.evaluate(() => [...document.querySelectorAll('[data-ipath]')].some((e) => /LeaveGameButtonButton$/.test(e.getAttribute('data-ipath')) && e.getBoundingClientRect().width > 0));
    if (!menuUp) {
      await pg.keyboard.press('Escape');
      await wait(1200);
    }
    await pg.mouse.move(540, 640, { steps: 3 });
    await pg.mouse.move(550, 645, { steps: 2 });
    await wait(300);
    await pg.mouse.down();
    await wait(120);
    await pg.mouse.up();
    await wait(1500);
    let sel = null;
    for (let i = 0; i < 8; i++) {
      sel = await pg.evaluate(() => { for (const e of document.querySelectorAll('[data-ipath]')) if (/LeaveButtonContainer\.LeaveGameButton$/.test(e.getAttribute('data-ipath')) && e.getBoundingClientRect().width > 0) return (e.querySelector('img') || {}).src || ''; return null; });
      if (sel && /MenuButtonSelected/.test(sel)) break;
      await wait(500);
    }
    if (sel === null) fails.push('leave prompt did not open');
    else if (!/MenuButtonSelected/.test(sel)) fails.push('leave confirm button is not highlighted: ' + sel);
    await pg.evaluate(() => { window.__marker = 1; });
    await pg.keyboard.press('Enter');
    await wait(2500);
    const stayed = await pg.evaluate(() => window.__marker === 1).catch(() => false);
    if (stayed) fails.push('Enter did not confirm leaving the game');
  }
  console.log(JSON.stringify({ info, diff, fails }));
  await b.close();
  srv.close();
  process.exit(fails.length ? 1 : 0);
})();

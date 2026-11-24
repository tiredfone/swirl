import { LuaVm } from './swirl_client.js';

const STARTER = 37801172;
const FONT_SIZES = [8, 9, 10, 11, 12, 14, 18, 24, 36, 48, 28, 32, 42, 60, 96];
void FONT_SIZES;

const fetchCoreSource = async (id) => {
  try {
    const r = await fetch('/asset/?id=' + id, { credentials: 'same-origin' });
    if (!r.ok) return null;
    let t = await r.text();
    if (t.charCodeAt(0) === 0xfeff) t = t.slice(1);
    if (t.startsWith('%')) t = t.slice(t.indexOf('\n') + 1);
    if (t.startsWith('<') || /^Failed to fetch asset/.test(t)) return null;
    return t;
  } catch (e) {
    return null;
  }
};

const fetchCoreTree = async (root) => {
  const out = new Map();
  let frontier = [root];
  for (let depth = 0; depth < 4 && frontier.length; depth++) {
    const next = [];
    const got = await Promise.all(frontier.filter((id) => !out.has(id)).map(async (id) => [id, await fetchCoreSource(id)]));
    for (const [id, src] of got) {
      out.set(id, src);
      if (!src) continue;
      for (const re of [/AddCoreScript\(\s*(\d+)/g, /AddStarterScript\(\s*(\d+)/g, /RegisterLibrary\([^,]+,\s*["']?(\d+)/g]) for (const m of src.matchAll(re)) next.push(+m[1]);
    }
    frontier = next;
  }
  return out;
};

export async function startApp({ client, cfg, canvas }) {
  const here = new URL('./', import.meta.url).href;
  const json = async (u) => {
    try {
      const r = await fetch(u);
      return r.ok ? await r.json() : {};
    } catch (e) {
      return {};
    }
  };
  const [index, rewrite, index2018] = await Promise.all([json(here + 'content-index.json'), json(here + 'content/rewrite.json'), json(here + 'content2018-index.json')]);
  const contentUrl = (path) => {
    let p = String(path).replace(/\\/g, '/').replace(/^\/+/, '').replace(/\/{2,}/g, '/');
    if (window.solarEra === 2018) {
      const key = p.toLowerCase();
      const hit = index2018[key] || index2018[key.replace(/\.(dds|tex)$/, '.png')] || index2018['platform/' + key] || index2018['platform/' + key.replace(/\.(dds|tex)$/, '.png')];
      if (hit) return here + 'content2018/' + hit.split('/').map(encodeURIComponent).join('/');
    }
    p = index[p.toLowerCase()] || p;
    p = rewrite[p] || p;
    return here + 'content/' + p.split('/').map(encodeURIComponent).join('/');
  };
  const assetUrl = (id) => ((cfg.endpoints && cfg.endpoints.asset) || '/asset/?id={id}').replace('{id}', id);
  const imageUrl = (url) => {
    const t = String(url || '').trim();
    if (!t) return null;
    const m = /^rbxasset:\/\/(.+)$/i.exec(t);
    if (m) return contentUrl(m[1]);
    const id = /[?&]id=(\d+)/i.exec(t) || /^rbxassetid:\/\/(\d+)/i.exec(t) || /^(\d+)$/.exec(t);
    return id ? assetUrl(id[1]) : null;
  };
  window.solarImageUrl = imageUrl;
  window.solarContentUrl = contentUrl;
  if (window.__resolveContent) window.__resolveContent();
  const loadImage = async (url) => {
    const u = imageUrl(url);
    if (!u) return null;
    try {
      const r = await fetch(u, { credentials: 'same-origin' });
      if (!r.ok) return null;
      let buf = new Uint8Array(await r.arrayBuffer());
      if (buf[0] === 0x25) buf = buf.subarray(buf.indexOf(10) + 1);
      return await createImageBitmap(new Blob([buf]), { imageOrientation: 'flipY', premultiplyAlpha: 'none', colorSpaceConversion: 'none' });
    } catch (e) {
      return null;
    }
  };
  window.solarLoadImage = loadImage;
  window.solarWorldReady = Promise.resolve();

  const link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = here + 'gui.css';
  document.head.appendChild(link);
  const root = document.createElement('div');
  root.id = 'chocolate';
  root.className = 'swirl-gui-root';
  root.style.cssText = 'position:fixed;inset:0;pointer-events:none;z-index:5';
  document.body.appendChild(root);

  const errors = [];
  window.solarApp = { errors, vm: null };
  let vm = null;
  client.set_driven(true);

  const held = new Set();
  const codes = new Set();
  let jumpQueued = false;
  let keyYaw = 0;
  const ms = { x: 0, y: 0, over: false };
  const locked = () => document.pointerLockElement === canvas;
  const typing = () => !!vm && vm.is_typing();
  const send = (o) => client.send(typeof o === 'string' ? o : JSON.stringify(o));
  let dragging = false;

  window.addEventListener('keydown', (e) => {
    if (!vm) return;
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key && e.key.length === 1 && !typing() && !'cvxaz'.includes(e.key.toLowerCase())) e.preventDefault();
    if (typing()) return;
    const k = e.key.toLowerCase();
    if (k === '/' && vm.gui_key_claimed('special:5')) {
      e.preventDefault();
      vm.fire_service('GuiService', 'SpecialKeyPressed', JSON.stringify([['e', 'SpecialKey', 5]]));
      return;
    }
    if (k === 'escape') vm.fire_service('GuiService', 'EscapeKeyPressed', '[]');
    const special = { insert: 0, home: 1, end: 2, pageup: 3, pagedown: 4 }[k];
    if (special !== undefined && vm.gui_key_claimed('special:' + special)) vm.fire_service('GuiService', 'SpecialKeyPressed', JSON.stringify([['e', 'SpecialKey', special]]));
    const claimed = k.length === 1 && vm.gui_key_claimed(k);
    if (claimed && !e.repeat) vm.fire_service('GuiService', 'KeyPressed', JSON.stringify([k]));
    if (k === 'tab') e.preventDefault();
    if (e.repeat) return;
    held.add(k);
    if (e.code) codes.add(e.code);
    if (k === ' ') {
      jumpQueued = true;
      e.preventDefault();
    }
    if (k === 'shift') vm.cam_toggle_shift_lock();
    if (k === 'arrowleft') keyYaw = 1;
    if (k === 'arrowright') keyYaw = -1;
    if (k === 'i') vm.cam_wheel(1);
    if (k === 'o') vm.cam_wheel(-1);
    if (k === 'backspace') {
      e.preventDefault();
      vm.drop_tool();
    }
    if (/^[0-9]$/.test(k) && !claimed) {
      const items = JSON.parse(vm.backpack_items());
      const idx = k === '0' ? 9 : +k - 1;
      if (items[idx]) vm.toggle_slot(items[idx]);
    }
    const lua = k === ' ' ? ' ' : k === 'enter' ? '\r' : k === 'backspace' ? '\b' : k === 'tab' ? '\t' : k === 'shift' ? '0' : k.length === 1 ? k : null;
    if (lua) vm.key_event(lua, true);
  });
  window.addEventListener('keyup', (e) => {
    if (!vm) return;
    const k = e.key.toLowerCase();
    held.delete(k);
    if (e.code) codes.delete(e.code);
    if (k === 'arrowleft' || k === 'arrowright') keyYaw = 0;
    const lua = k === ' ' ? ' ' : k === 'enter' ? '\r' : k === 'shift' ? '0' : k.length === 1 ? k : null;
    if (lua) vm.key_event(lua, false);
  });
  window.addEventListener('blur', () => {
    held.clear();
    codes.clear();
    keyYaw = 0;
  });
  canvas.addEventListener('pointermove', (e) => {
    if (e.pointerType === 'touch') return;
    const r = canvas.getBoundingClientRect();
    ms.x = e.clientX - r.left;
    ms.y = e.clientY - r.top;
    if (vm) vm.mouse_event('Move');
  });
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());
  canvas.addEventListener('pointerdown', (e) => {
    if (e.pointerType === 'touch' || !vm) return;
    if (typing() && document.activeElement) document.activeElement.blur();
    if (e.button === 2) {
      dragging = true;
      try {
        canvas.setPointerCapture(e.pointerId);
        if (canvas.requestPointerLock) {
          const r = canvas.requestPointerLock();
          if (r && r.catch) r.catch(() => {});
        }
      } catch (err) {}
    }
    if (e.button === 0) vm.mouse_button(0, true);
  });
  window.addEventListener('pointerup', (e) => {
    if (e.pointerType === 'touch' || !vm) return;
    if (e.button === 2 && dragging) {
      dragging = false;
      if (locked() && !vm.cam_first_person()) document.exitPointerLock();
    }
    if (e.button === 0) vm.mouse_button(0, false);
  }, true);
  window.addEventListener('pointermove', (e) => {
    if (!vm) return;
    if (dragging || (locked() && JSON.parse(vm.cam_state()).fp)) vm.cam_drag(e.movementX, e.movementY);
  });
  let wheelAcc = 0;
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    if (!vm) return;
    const unit = e.deltaMode === 1 ? 3 : e.deltaMode === 2 ? 1 : 100;
    if (Math.sign(e.deltaY) !== Math.sign(wheelAcc)) wheelAcc = 0;
    wheelAcc += e.deltaY / unit;
    while (Math.abs(wheelAcc) >= 1) {
      const dir = wheelAcc > 0 ? -1 : 1;
      wheelAcc -= Math.sign(wheelAcc);
      vm.cam_wheel(dir);
      vm.mouse_event(dir < 0 ? 'WheelBackward' : 'WheelForward');
    }
    if (Math.abs(e.deltaY) >= unit * 0.99) wheelAcc = 0;
  }, { passive: false });

  let actx = null;
  let master = null;
  const sounds = new Map();
  const ensureAudio = () => {
    if (!actx) {
      try {
        actx = new (window.AudioContext || window.webkitAudioContext)();
        master = actx.createGain();
        master.gain.value = 1;
        master.connect(actx.destination);
      } catch (e) {
        return null;
      }
    }
    if (actx.state === 'suspended') actx.resume().catch(() => {});
    return actx;
  };
  const resume = () => {
    if (actx && actx.state === 'suspended') actx.resume().catch(() => {});
  };
  window.addEventListener('pointerdown', resume, true);
  window.addEventListener('keydown', resume, true);
  const soundBytes = async (url) => {
    const u = imageUrl(url);
    if (!u) return null;
    try {
      const r = await fetch(u, { credentials: 'same-origin' });
      if (!r.ok) return null;
      let buf = new Uint8Array(await r.arrayBuffer());
      if (buf[0] === 0x25) buf = buf.subarray(buf.indexOf(10) + 1);
      return buf;
    } catch (e) {
      return null;
    }
  };
  const stopSound = (id) => {
    const cur = sounds.get(id);
    if (!cur) return;
    sounds.delete(id);
    try {
      if (cur.src) cur.src.stop();
    } catch (e) {}
  };
  const audio = async (o) => {
    if (o.op === 'stop' || o.op === 'pause') return stopSound(o.id);
    if (o.op === 'update') {
      const cur = sounds.get(o.id);
      if (cur && cur.src) {
        cur.gain.gain.value = Math.max(0, Math.min(1, o.volume));
        cur.src.playbackRate.value = Math.max(0.05, o.pitch || 1);
      }
      return;
    }
    const ctx = ensureAudio();
    if (!ctx || !o.url) {
      if (!o.looped) vm.sound_ended(o.id);
      return;
    }
    stopSound(o.id);
    const token = {};
    const cur = { token };
    sounds.set(o.id, cur);
    const bytes = await soundBytes(o.url);
    let buf = null;
    if (bytes) {
      try {
        buf = await ctx.decodeAudioData(bytes.slice().buffer);
      } catch (e) {
        buf = null;
      }
    }
    if (sounds.get(o.id) !== cur) return;
    if (!buf) {
      sounds.delete(o.id);
      if (!o.looped) vm.sound_ended(o.id);
      return;
    }
    const src = ctx.createBufferSource();
    src.buffer = buf;
    src.loop = !!o.looped;
    src.playbackRate.value = Math.max(0.05, o.pitch || 1);
    const gain = ctx.createGain();
    gain.gain.value = Math.max(0, Math.min(1, o.volume));
    src.connect(gain);
    let node = gain;
    if (o.pos) {
      const pan = ctx.createPanner();
      pan.panningModel = 'HRTF';
      pan.distanceModel = 'inverse';
      pan.refDistance = 10;
      pan.rolloffFactor = 1;
      pan.positionX.value = o.pos[0];
      pan.positionY.value = o.pos[1];
      pan.positionZ.value = o.pos[2];
      gain.connect(pan);
      node = pan;
    }
    node.connect(master);
    src.onended = () => {
      if (sounds.get(o.id) === cur) {
        sounds.delete(o.id);
        vm.sound_ended(o.id);
      }
    };
    src.start();
    cur.src = src;
    cur.gain = gain;
  };
  const listener = (cam) => {
    if (!actx) return;
    const l = actx.listener;
    const fwd = [-cam[5], -cam[8], -cam[11]];
    const up = [cam[4], cam[7], cam[10]];
    if (l.positionX) {
      l.positionX.value = cam[0];
      l.positionY.value = cam[1];
      l.positionZ.value = cam[2];
      l.forwardX.value = fwd[0];
      l.forwardY.value = fwd[1];
      l.forwardZ.value = fwd[2];
      l.upX.value = up[0];
      l.upY.value = up[1];
      l.upZ.value = up[2];
    }
  };

  const CELL_COLORS = [null, [91, 154, 76], [214, 194, 141], [160, 82, 64], [126, 125, 122], [70, 70, 70], [122, 120, 117], [192, 193, 195], [213, 181, 54], [158, 116, 72], [110, 78, 48], [138, 132, 120], [150, 146, 140], [96, 116, 82], [180, 178, 170], [196, 40, 28], [13, 105, 172], [45, 110, 170]];
  const CELL_KIND = [null, 'grass', 'sand', 'brick', 'granite', 'concrete', 'corrodedmetal', 'metal', 'foil', 'woodplanks', 'wood', 'pebble', 'concrete', 'slate', 'concrete', 'plastic', 'plastic', 'plastic'];
  const SMOOTH_MATERIALS = ['Air', 'Water', 'Grass', 'Slate', 'Concrete', 'Brick', 'Sand', 'WoodPlanks', 'Rock', 'Glacier', 'Snow', 'Sandstone', 'Mud', 'Basalt', 'Ground', 'CrackedLava', 'Asphalt', 'Cobblestone', 'Ice', 'LeafyGrass', 'Salt', 'Limestone', 'Pavement'];
  const SMOOTH_COLORS = { Grass: [106, 127, 63], Slate: [63, 127, 107], Concrete: [127, 102, 63], Brick: [138, 86, 62], Sand: [143, 126, 95], WoodPlanks: [139, 109, 79], Rock: [102, 108, 111], Glacier: [101, 176, 234], Snow: [195, 199, 218], Sandstone: [137, 90, 71], Mud: [58, 46, 36], Basalt: [30, 30, 37], Ground: [102, 92, 59], CrackedLava: [232, 156, 74], Asphalt: [115, 123, 107], Cobblestone: [132, 123, 90], Ice: [129, 194, 224], LeafyGrass: [115, 132, 74], Salt: [198, 189, 181], Limestone: [206, 173, 148], Pavement: [148, 148, 140], Water: [12, 84, 92] };
  const SMOOTH_KIND = { Grass: 'grass', Slate: 'slate', Concrete: 'concrete', Brick: 'brick', Sand: 'sand', WoodPlanks: 'woodplanks', Rock: 'granite', Glacier: 'ice', Snow: 'plastic', Sandstone: 'sand', Mud: 'pebble', Basalt: 'granite', Ground: 'pebble', CrackedLava: 'granite', Asphalt: 'concrete', Cobblestone: 'cobblestone', Ice: 'ice', LeafyGrass: 'grass', Salt: 'sand', Limestone: 'concrete', Pavement: 'concrete' };
  const terrainSync = () => {
    const smooth = vm.client_era() === 2018;
    for (const k of JSON.parse(vm.take_terrain_keys())) {
      client.clear_terrain_chunk(k);
      for (const g of vm.terrain_chunk(k)) {
        const [m, pos, nrm, uv] = g;
        if (!pos.length) continue;
        if (smooth) {
          const name = SMOOTH_MATERIALS[m] || 'Grass';
          const water = m === 1;
          const c = water ? [12, 84, 92] : SMOOTH_COLORS[name] || [127, 127, 127];
          client.set_terrain_chunk(k, pos, nrm, uv, Float32Array.of(c[0] / 255, c[1] / 255, c[2] / 255, water ? 0.7 : 1, water ? 1 : 0), water ? 'plastic' : SMOOTH_KIND[name] || 'plastic');
        } else {
          const c = CELL_COLORS[m] || [127, 127, 127];
          const water = m === 17;
          client.set_terrain_chunk(k, pos, nrm, uv, Float32Array.of(c[0] / 255, c[1] / 255, c[2] / 255, water ? 0.55 : 1, water ? 1 : 0), CELL_KIND[m] || 'plastic');
        }
      }
    }
  };

  const CURSOR_URLS = { arrow: 'rbxasset://textures/ArrowCursor.png', far: 'rbxasset://textures/ArrowFarCursor.png', hand: 'rbxasset://textures/DragCursor.png', lock: 'rbxasset://textures/MouseLockedCursor.png' };
  let cursorKey = null;
  const setCursor = (key) => {
    if (key === cursorKey) return;
    cursorKey = key;
    const u = imageUrl(CURSOR_URLS[key] || key);
    canvas.style.cursor = u ? `url("${u}") 0 0, auto` : '';
  };

  const applySky = async () => {
    if (!vm) return;
    const sk = JSON.parse(vm.sky_urls() || 'null');
    if (!sk) return;
    const DEF = ['rt', 'lf', 'up', 'dn', 'bk', 'ft'].map((f) => `rbxasset://sky/null_plainsky512_${f}.jpg`);
    const urls = sk.urls.map((u, i) => u || DEF[i]);
    const key = urls.join('|');
    if (key === applySky.key || applySky.busy) return;
    applySky.key = key;
    applySky.busy = true;
    try {
      const load = (list) => Promise.all(list.map((u) => loadImage(u)));
      let imgs = await load(urls);
      if (!imgs.every((i) => i && i.width) && !urls.every((u, i) => u === DEF[i])) imgs = await load(DEF);
      if (applySky.key !== key) return;
      if (imgs.some((i) => !i || !i.width)) {
        client.set_sky_cube([]);
        return;
      }
      const big = Math.max(...imgs.map((im) => Math.max(im.width, im.height)));
      let size = 64;
      while (size < big && size < 1024) size *= 2;
      const maps = await Promise.all(imgs.map((im, i) => {
        const c = document.createElement('canvas');
        c.width = size;
        c.height = size;
        const g = c.getContext('2d');
        if (i === 2) g.setTransform(0, -1, 1, 0, 0, size);
        else if (i === 3) g.setTransform(0, 1, -1, 0, size, 0);
        else g.setTransform(1, 0, 0, -1, 0, size);
        g.drawImage(im, 0, 0, size, size);
        return createImageBitmap(c, { premultiplyAlpha: 'none', colorSpaceConversion: 'none' });
      }));
      client.set_sky_cube(maps);
    } finally {
      applySky.busy = false;
    }
  };

  const fetched = new Set();
  const pump = () => {
    for (const [tok, id] of JSON.parse(vm.take_wanted_models())) {
      fetch(assetUrl(id), { credentials: 'same-origin' }).then((r) => (r.ok ? r.arrayBuffer() : null)).then((b) => {
        let u = b ? new Uint8Array(b) : null;
        if (u && u[0] === 0x25) u = u.subarray(u.indexOf(10) + 1);
        vm.provide_model(tok, u);
      }, () => vm.provide_model(tok, null));
    }
    for (const id of JSON.parse(vm.take_wanted_anims())) {
      fetch(assetUrl(id), { credentials: 'same-origin' }).then((r) => (r.ok ? r.arrayBuffer() : null)).then((b) => {
        let u = b ? new Uint8Array(b) : null;
        if (u && u[0] === 0x25) u = u.subarray(u.indexOf(10) + 1);
        vm.provide_anim(id, u);
      }, () => vm.provide_anim(id, null));
    }
    for (const id of JSON.parse(vm.take_wanted())) {
      if (fetched.has(id)) continue;
      fetched.add(id);
      fetchCoreTree(id).then((tree) => {
        for (const [k, v] of [...tree].reverse()) vm.provide_core_source(k, v);
      });
    }
  };

  let last = 0;
  let due = 0;
  let pingT = 0;
  let booted = false;
  const frameMs = 1000 / 60;
  const frame = (now) => {
    for (const raw of JSON.parse(client.take_tap())) {
      let m;
      try {
        m = JSON.parse(raw);
      } catch (e) {
        continue;
      }
      if (m.t === 'init') {
        vm = new LuaVm(Date.now() / 1000);
        vm.set_run(true);
        vm.set_screen(canvas.clientWidth || 800, canvas.clientHeight || 600);
        window.solarApp.vm = vm;
        booted = false;
      }
      if (!vm) continue;
      try {
        vm.apply(raw);
        if (m.t === 'init') window.solarEra = vm.client_era();
      } catch (e) {
        errors.push('apply: ' + e);
      }
      if (m.t === 'kick') client.close_connection && client.close_connection();
      if (m.t === 'teleport' && (m.place | 0) > 0) {
        window.chocolateLeaving = true;
        location.href = '/game/' + m.place;
      }
    }
    if (!vm) return;
    if (!booted) {
      booted = true;
      vm.attach_gui(root);
    }
    if (now < due - 1) return;
    due = due + frameMs < now ? now + frameMs : due + frameMs;
    const dt = last ? Math.min(0.1, (now - last) / 1000) : 0;
    last = now;
    const key = (code, letter) => (codes.size || !held.has(letter) ? codes.has(code) : true);
    const fw = (key('KeyW', 'w') || held.has('arrowup') ? 1 : 0) - (key('KeyS', 's') || held.has('arrowdown') ? 1 : 0);
    const sd = (key('KeyD', 'd') ? 1 : 0) - (key('KeyA', 'a') ? 1 : 0);
    const jump = held.has(' ') || jumpQueued;
    jumpQueued = false;
    try {
      for (const l of JSON.parse(vm.logs())) {
        (window.solarApp.logs = window.solarApp.logs || []).push(l);
        if (window.solarApp.logs.length > 400) window.solarApp.logs.shift();
        if (l.kind === 'error') console.error(l.msg.split('\n')[0]);
      }
      vm.cam_key_yaw(keyYaw);
      vm.frame(dt, fw, sd, jump, held.has('backspace'), locked());
      const cs = JSON.parse(vm.cam_state());
      const centered = locked() && (cs.shift || cs.fp);
      const k = 1;
      vm.mouse_frame(ms.x, ms.y, canvas.clientWidth || 1, canvas.clientHeight || 1, k, false, centered);
      vm.gui_frame(canvas.clientWidth || 1, canvas.clientHeight || 1, 1, 0, 0, 0, 0, true, now);
      pump();
      const ops = vm.render_ops();
      if (ops !== '[]') client.apply_local(ops);
      const cfs = vm.render_cfs();
      if (cfs.length) client.set_cfs(cfs);
      for (const [id, t] of JSON.parse(vm.render_alpha())) client.set_local_alpha(id, t == null ? NaN : t);
      const cam = vm.render_camera();
      client.set_camera(cam.subarray(0, 12), cam[12]);
      for (const o of JSON.parse(vm.take_sound_ops())) audio(o);
      listener(cam);
      for (const o of JSON.parse(vm.take_out())) send(o);
      const kicked = vm.take_kicked();
      if (kicked) client.close_connection && client.close_connection();
      terrainSync();
      applySky();
      setCursor(vm.mouse_cursor());
    } catch (e) {
      errors.push('frame: ' + (e && e.stack ? e.stack : e));
      console.error(e);
    }
    pingT += dt;
    if (pingT > 5) {
      pingT = 0;
      send({ t: 'ping', x: Date.now() });
    }
  };
  return { frame };
}

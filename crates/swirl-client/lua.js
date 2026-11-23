import { LuaVm } from './swirl_client.js';

const addImportMap = (pub) => {
  const map = {
    imports: {
      three: pub + '/client/vendor/three.module.js',
      '@dimforge/rapier3d-compat': pub + '/client/vendor/rapier.js',
      'wasmoon-lua5.1': pub + '/client/vendor/wasmoon.js',
      lodash: pub + '/client/vendor/lodash.js',
    },
  };
  const s = document.createElement('script');
  s.type = 'importmap';
  s.textContent = JSON.stringify(map);
  document.head.appendChild(s);
};

const addStyle = (pub) => {
  if (document.getElementById('swirl-gui-style')) return;
  const l = document.createElement('link');
  l.id = 'swirl-gui-style';
  l.rel = 'stylesheet';
  l.href = pub + '/client/style.css';
  document.head.appendChild(l);
};

const CELL_KIND = [null, 'grass', 'sand', 'brick', 'granite', 'concrete', 'corrodedmetal', 'metal', 'foil', 'woodplanks', 'wood', 'pebble', 'concrete', 'slate', 'concrete', 'plastic', 'plastic', 'plastic'];
const SMOOTH_KIND = { Grass: 'grass', Slate: 'slate', Concrete: 'concrete', Brick: 'brick', Sand: 'sand', WoodPlanks: 'woodplanks', Rock: 'granite', Glacier: 'ice', Snow: 'plastic', Sandstone: 'sand', Mud: 'pebble', Basalt: 'granite', Ground: 'pebble', CrackedLava: 'granite', Asphalt: 'concrete', Cobblestone: 'cobblestone', Ice: 'ice', LeafyGrass: 'grass', Salt: 'sand', Limestone: 'concrete', Pavement: 'concrete' };
const wid = (id) => (id == null ? null : id > 0 ? id : 0x40000000 + -id);

export async function startLua(opts) {
  const { client, canvas } = opts;
  let resolveWorld;
  window.solarWorldReady = new Promise((r) => { resolveWorld = r; });
  const pub = new URL('./chocolate', import.meta.url).href;
  const cfg = Object.assign({}, opts.cfg, { publicPath: pub });
  addImportMap(pub);
  addStyle(pub);
  const terrainMod = await import(pub + '/shared/terrain.js');
  const [THREE, assets, worldMod, guiMod, inputMod, luahost, C, wasmoon, rapierMod, cameraMod, codec, cursorMod, renderMod] = await Promise.all([
    import('three'),
    import(pub + '/client/assets.js'),
    import(pub + '/client/world.js'),
    import(pub + '/client/gui.js'),
    import(pub + '/client/input2018.js'),
    import(pub + '/shared/luahost.js'),
    import(pub + '/shared/cframe.js'),
    import('wasmoon-lua5.1'),
    import('@dimforge/rapier3d-compat'),
    import(pub + '/client/camera.js'),
    import(pub + '/shared/codec.js'),
    import(pub + '/client/cursor.js'),
    import(pub + '/client/render.js'),
  ]);
  void C;
  const { ClientWorld } = worldMod;
  const { Gui } = guiMod;
  const { Input2018 } = inputMod;
  const { T } = luahost;
  const { Lua } = wasmoon;
  const RAPIER = rapierMod.default || rapierMod;
  const { CameraController } = cameraMod;
  const { CursorLayer, CURSORS } = cursorMod;
  assets.configure(cfg);
  const indexReady = assets.loadContentIndex();
  indexReady.then(() => { window.solarLoadImage = assets.loadImage; });
  
  const runtimeSrc = fetch(pub + '/shared/runtime.lua?v=' + (cfg.version || '')).then((r) => r.text());
  const rapierReady = RAPIER.init();

  const root = document.createElement('div');
  root.id = 'chocolate';
  root.className = 'swirl-gui-root';
  root.style.cssText = 'position:fixed;inset:0;pointer-events:none;z-index:5';
  document.body.appendChild(root);
  const cursor = new CursorLayer(canvas);

  const errors = [];
  const gameErrors = [];
  const logs = [];
  const send = (o) => client.send(JSON.stringify(o));
  const net = { send, bytesIn: 0, bytesOut: 0, close() {} };

  class RustRenderer {
    constructor(world) {
      this.world = world;
      this.camera = new THREE.PerspectiveCamera(70, 1, 0.5, 20000);
      this.touch = false;
      this.targetMs = 1000 / 60;
      this.ops = [];
      this.cfs = new Map();
      this.known = new Set();
      this.dm = world.dm;
      this.ws = this.dm.services.get('Workspace');
      this.lt = this.dm.services.get('Lighting');
      this.dm.observe({
        changed: (inst, name, old, o) => this.changed(inst, name, o),
        reparented: (inst, old, o) => this.reparented(inst, o),
        destroyed: (inst) => this.destroyed(inst),
      });
    }

    inWorld(inst) {
      let p = inst;
      while (p) {
        if (p === this.ws || p === this.lt) return true;
        p = p.parent;
      }
      return false;
    }

    live(inst) {
      return inst.id > 0 || this.known.has(inst.id);
    }

    encode(inst) {
      const rec = codec.encodeInst(inst);
      rec[0] = wid(rec[0]);
      rec[2] = wid(rec[2]);
      const props = rec[3];
      for (let i = 0; i < props.length; i += 2) {
        const p = inst.info.all.get(props[i]);
        if (p && p.type === 'Ref' && props[i + 1] != null) props[i + 1] = wid(props[i + 1]);
      }
      return rec;
    }

    changed(inst, name, o) {
      if (!this.live(inst)) return;
      if (name === 'CFrame') {
        const cf = inst.props.get('CFrame');
        if (cf && cf.length >= 12) this.cfs.set(wid(inst.id), cf);
        return;
      }
      if (o && o.remote) return;
      const p = inst.info.all.get(name);
      if (!p || p.type === 'any') return;
      if (!this.inWorld(inst)) return;
      let v = codec.encodeValue(p.type, inst.props.get(name));
      if (p.type === 'Ref' && v != null) v = wid(v);
      this.ops.push(['p', wid(inst.id), name, v]);
    }

    mark(inst, on) {
      for (const d of [inst, ...inst.descendants()]) if (d.id <= 0) on ? this.known.add(d.id) : this.known.delete(d.id);
    }

    reparented(inst, o) {
      if (o && o.remote) return;
      if (inst.id > 0) {
        const par = inst.parent;
        this.ops.push(['m', inst.id, par && (par.id > 0 || this.known.has(par.id)) ? wid(par.id) : null]);
        return;
      }
      const inW = inst.parent && this.inWorld(inst.parent) && (inst.parent.id > 0 || this.known.has(inst.parent.id));
      const known = this.known.has(inst.id);
      if (inW && !known) {
        const recs = [];
        const walk = (x) => {
          recs.push(this.encode(x));
          for (const c of x.children) walk(c);
        };
        walk(inst);
        this.mark(inst, true);
        this.ops.push(['n', recs]);
      } else if (!inW && known) {
        this.ops.push(['d', wid(inst.id)]);
        this.mark(inst, false);
      } else if (known) this.ops.push(['m', wid(inst.id), wid(inst.parent.id)]);
    }

    destroyed(inst) {
      if (inst.id > 0) this.ops.push(['d', inst.id]);
      else if (this.known.has(inst.id)) {
        this.ops.push(['d', wid(inst.id)]);
        this.known.delete(inst.id);
      }
    }

    setQuality() {}

    setAssemblies() {}

    attach() {}

    setLocalTransparency(part, t) {
      client.set_local_alpha(wid(part.id), t == null ? NaN : t);
    }

    pick(ndcX, ndcY) {
      const ray = new THREE.Raycaster();
      ray.setFromCamera(new THREE.Vector2(ndcX, ndcY), this.camera);
      return { origin: ray.ray.origin.toArray(), dir: ray.ray.direction.toArray() };
    }

    async applySky() {
      const L = this.lt;
      if (!L || this.skyBusy) return;
      const era = this.world.era === 2018;
      const DEF = (era ? ['rt', 'lf', 'up', 'dn', 'bk', 'ft'].map((f) => `rbxasset://textures/sky/sky512_${f}.tex`) : ['rt', 'lf', 'up', 'dn', 'bk', 'ft'].map((f) => `rbxasset://sky/null_plainsky512_${f}.jpg`));
      const sky = L.findChildOfClass('Sky');
      const urls = ['SkyboxRt', 'SkyboxLf', 'SkyboxUp', 'SkyboxDn', 'SkyboxBk', 'SkyboxFt'].map((k, i) => (sky && sky.props.get(k)) || DEF[i]);
      const key = urls.join('|');
      if (key === this.skyKey) return;
      this.skyKey = key;
      this.skyBusy = true;
      try {
        const load = (list) => Promise.all(list.map((u) => Promise.resolve(assets.loadImage(u)).catch(() => null)));
        let imgs = await load(urls);
        if (!imgs.every((i) => i && i.width && i.height) && !urls.every((u, i) => u === DEF[i])) imgs = await load(DEF);
        if (this.skyKey !== key) return;
        if (imgs.some((i) => !i || !i.width || !i.height)) {
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
        this.skyBusy = false;
      }
    }

    syncTerrain() {
      const t = this.world.terrain;
      if (!t) return;
      if (!this.terrainHooked) {
        this.terrainHooked = true;
        this.terrainInst = this.ws.findChildOfClass('Terrain');
        t.listeners.add((keys) => this.terrainUpdate(keys));
        this.terrainUpdate(t.allKeys());
      }
      t.flush();
    }

    terrainUpdate(keys) {
      const t = this.world.terrain;
      const era = this.world.era === 2018 ? 2018 : 2012;
      for (const k of keys) {
        client.clear_terrain_chunk(String(k));
        const { groups } = terrainMod.meshChunk(t, k);
        for (const [m, d] of groups) {
          if (!d.pos.length) continue;
          let color, kind, water, opacity = 1;
          if (era === 2018) {
            const name = terrainMod.SMOOTH_MATERIALS[m] || 'Grass';
            water = m === terrainMod.SMOOTH_WATER;
            const own = this.terrainInst && this.terrainInst.materialColors && this.terrainInst.materialColors.get(name);
            color = water ? [12 / 255, 84 / 255, 92 / 255] : own || (terrainMod.SMOOTH_COLORS[name] || [127, 127, 127]).map((v) => v / 255);
            kind = water ? 'plastic' : SMOOTH_KIND[name] || 'plastic';
            if (water) opacity = 0.7;
          } else {
            const c = terrainMod.CELL_COLORS[m] || [127, 127, 127];
            color = [c[0] / 255, c[1] / 255, c[2] / 255];
            kind = CELL_KIND[m] || 'plastic';
            water = m === terrainMod.CELL_WATER;
            if (water) opacity = 0.55;
          }
          client.set_terrain_chunk(String(k), Float32Array.from(d.pos), Float32Array.from(d.nrm), Float32Array.from(d.uv), Float32Array.of(color[0], color[1], color[2], opacity, water ? 1 : 0), kind);
        }
      }
    }

    frame() {
      this.syncTerrain();
      this.applySky();
      if (this.ops.length) {
        client.apply_local(JSON.stringify(this.ops));
        this.ops.length = 0;
      }
      if (this.cfs.size) {
        const a = new Float64Array(this.cfs.size * 13);
        let i = 0;
        for (const [id, cf] of this.cfs) {
          a[i++] = id;
          for (let k = 0; k < 12; k++) a[i++] = cf[k];
        }
        this.cfs.clear();
        client.set_cfs(a);
      }
      const cam = this.camera;
      cam.updateMatrixWorld(true);
      const e = cam.matrixWorld.elements;
      client.set_camera(Float32Array.of(e[12], e[13], e[14], e[0], e[4], e[8], e[1], e[5], e[9], e[2], e[6], e[10]), cam.fov);
    }
  }

  const state = { world: null, gui: null, input: null, renderer: null, camera: null, pending: [], ready: false, last: 0, due: 0, held: new Set(), codes: new Set(), jumpQueued: false };
  const hooks = {
    touch: false,
    version: cfg.version,
    placeInfo: () => ({ name: cfg.placeName || '', creator: cfg.creatorName || '' }),
    mouseBehavior: (v) => state.input && state.input.setBehavior(v),
    mouseDelta: () => (state.input ? state.input.takeDelta() : [0, 0]),
    mouseButtonDown: (t) => !!(state.input && state.input.buttons.has(t)),
    isKeyCodeDown: (k) => !!(state.input && state.input.isKeyDown(k)),
    mouseIconEnabled: (on) => {
      canvas.style.cursor = on ? '' : 'none';
    },
    screenRay: (x, y) => {
      const k = state.world.guiScale();
      const rc = new THREE.Raycaster();
      rc.setFromCamera(new THREE.Vector2((x / k / root.clientWidth) * 2 - 1, -(y / k / root.clientHeight) * 2 + 1), state.renderer.camera);
      return { o: rc.ray.origin.toArray(), d: rc.ray.direction.toArray() };
    },
    worldToScreen: (p) => {
      const cam = state.renderer.camera;
      const v = new THREE.Vector3(p[0], p[1], p[2]);
      const depth = v.clone().applyMatrix4(cam.matrixWorldInverse).z;
      v.project(cam);
      const k = state.world.guiScale();
      return [((v.x + 1) / 2) * root.clientWidth * k, ((1 - v.y) / 2) * root.clientHeight * k, -depth, depth < 0 && Math.abs(v.x) <= 1 && Math.abs(v.y) <= 1];
    },
    measureText: (text, size, font, frame) => (state.gui && state.gui.measureText ? state.gui.measureText(text, size, font, frame) : null),
    screenSize: () => [root.clientWidth || window.innerWidth, root.clientHeight || window.innerHeight],
    rotateCamera: (dx, dy) => state.camera && state.camera.rotateBy(dx, dy),
    zoomCamera: (d) => state.camera && state.camera.zoomBy(Math.exp(-d * 0.15)),
    cameraTilt: (dir) => state.camera && state.camera.rotateBy(0, dir * 0.1745),
    cameraZoom: (dir) => state.camera && state.camera.stepZoom(dir > 0 ? 1 : -1),
    toggleFullscreen: () => {
      if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
      else document.documentElement.requestFullscreen().catch(() => {});
    },
    inFullscreen: () => !!document.fullscreenElement,
    message: () => '',
    log: (k, msg, where) => {
      const line = where ? where + ': ' + msg : String(msg);
      logs.push(k + ': ' + line);
      if (logs.length > 400) logs.shift();
      if (k === 'error') {
        if (/CoreGui|ScriptContext|RobloxGui|CoreScripts|PlayerScripts/.test(where || '')) errors.push(line);
        else gameErrors.push(line);
        console.error(line);
      } else if (k !== 'print') console.log(line);
    },
    kicked: () => {
      client.close_connection && client.close_connection();
    },
    focusTextBox: (inst, on) => state.gui && state.gui.focusTextBox(inst, on),
    focusedTextBox: () => (state.gui && state.gui.focused) || null,
    setMessage: (msg) => state.gui && state.gui.setHint(msg),
    bubble: (part, msg) => state.gui && state.gui.bubble(part, msg),
    ownCharacter: (model) => {
      const hum = model.findChildOfClass('Humanoid');
      state.world.dm.setProp(state.world.camera, 'CameraSubject', hum);
    },
    died: () => {},
    leave: () => {
      window.chocolateLeaving = true;
      const t = cfg.endpoints && cfg.endpoints.gamePage;
      location.href = t ? String(t).replace('{id}', String(cfg.placeId)) : '/game/' + cfg.placeId;
    },
  };

  const world = new ClientWorld(net, hooks);
  state.world = world;

  const applyFs = (m) => {
    if (!m.s) return;
    for (const [id, st] of Object.entries(m.s)) {
      const other = world.dm.byId.get(+id);
      const prev = world.friendStatus.get(+id) || 1;
      world.friendStatus.set(+id, st | 0);
      if (other && world.player && prev !== (st | 0)) world.dm.emit(world.player, 'FriendStatusChanged', other, T.e('FriendStatus', st | 0));
    }
  };
  const applyFre = (m) => {
    const a = world.dm.byId.get(m.a | 0), b = world.dm.byId.get(m.b | 0);
    const players = world.dm.services.get('Players');
    if (a && b && players) world.dm.emit(players, 'FriendRequestEvent', a, b, T.e('FriendRequestEvent', m.e | 0));
  };

  const route = (m) => {
    const dm = world.dm;
    switch (m.t) {
      case 'rep':
        world.applyRep(m);
        break;
      case 'anim':
        world.remoteAnim(m);
        break;
      case 'terrain':
        world.applyTerrain(m);
        break;
      case 'fs':
        applyFs(m);
        break;
      case 'fre':
        applyFre(m);
        break;
      case 're':
      case 'rfc':
      case 'rfr':
        world.onRemote(m);
        break;
      case 'chat': {
        const players = dm.services.get('Players');
        const speaker = m.p ? dm.byId.get(m.p) : players.children.find((c) => c.ClassName === 'Player' && c.props.get('userId') === m.u);
        const kind = m.k === 'team' ? 1 : m.k === 'whisper' ? 2 : 0;
        const to = m.to ? dm.byId.get(m.to) || null : null;
        if (speaker) {
          dm.emit(players, 'PlayerChatted', T.e('PlayerChatType', kind), speaker, m.m, to);
          dm.emit(speaker, 'Chatted', m.m, to);
        }
        const head = m.h && dm.byId.get(m.h);
        if (head && state.gui && kind !== 2 && world.era !== 2018) state.gui.bubble(head, m.m);
        world.runSoon();
        break;
      }
      case 'bubble': {
        const p = dm.byId.get(m.id);
        if (!p) break;
        if (world.era === 2018) dm.emit(dm.services.get('Chat'), 'Chatted', p, m.m, T.e('ChatColor', (m.c | 0) || 0));
        else if (state.gui) state.gui.bubble(p, m.m);
        break;
      }
      case 'msg':
        if (state.gui) state.gui.setHint(m.m);
        break;
      case 'kick':
        if (typeof window.solarClientFailed === 'function') window.solarClientFailed(m.m || 'You were disconnected');
        break;
      default:
        break;
    }
  };

  let booting = false;
  const boot = async (m) => {
    booting = true;
    client.set_driven(true);
    const text = await runtimeSrc;
    await Promise.all([rapierReady, indexReady]);
    await world.init(m, RAPIER, () => Lua.create({ customWasmUri: pub + '/client/vendor/liblua5.1.wasm' }), text);
    resolveWorld();
    let renderer;
    if (opts.reference) {
      const c2 = document.createElement('canvas');
      c2.id = 'swirl-reference-canvas';
      c2.style.cssText = 'position:fixed;inset:0;width:100%;height:100%;z-index:1';
      canvas.style.visibility = 'hidden';
      canvas.parentNode.insertBefore(c2, canvas.nextSibling);
      renderer = new renderMod.Renderer(c2, world);
      renderer.setQuality(0);
      renderer.attach();
      state.reference = true;
    } else renderer = new RustRenderer(world);
    state.renderer = renderer;
    world.renderer = renderer;
    const camera = new CameraController(world, renderer, canvas);
    state.camera = camera;
    const gui = new Gui(root, world, renderer, {
      leave: () => hooks.leave(),
      sendChat: (msg) => send({ t: 'chat', m: msg }),
      send,
      flush: () => world.host.tick(world.t),
    });
    state.gui = gui;
    world.host.env.textBounds = (inst) => gui.textBounds(inst);
    world.host.env.absoluteSize = (inst) => gui.absoluteSize(inst);
    if (world.era === 2018) {
      state.input = new Input2018(world, gui, canvas, root);
      world.input2018 = state.input;
      camera.luaDriven = true;
    }
    world.bindSettings(renderer);
    wire(gui, camera);
    world.ready = true;
    for (const p of state.pending.splice(0)) route(p);
    world.markLoaded();
    state.ready = true;
  };

  const ms = world.mouseState;
  const wire = (gui, camera) => {
    const held = state.held;
    const codes = state.codes;
    const relock = () => {
      const in18 = world.input2018;
      if (in18 && in18.behavior !== 0 && document.pointerLockElement !== canvas && !gui.isTyping()) in18.setBehavior(in18.behavior);
    };
    const overGuiTarget = (e) => e.target !== canvas && root.contains(e.target);
    window.addEventListener('keydown', (e) => {
      if (e.key !== 'Escape') relock();
      const in18 = world.input2018;
      if (in18) {
        in18.keyDown(e, gui.isTyping());
        if (e.key === 'Tab' || e.key === ' ' || e.key.startsWith('Arrow')) e.preventDefault();
        if (gui.isTyping() || e.repeat) return;
        if (e.key === 'Enter') {
          const gs = world.dm.services.get('GuiService');
          const pg = world.player && world.player.findChildOfClass('PlayerGui');
          const sel = (gs && gs.props.get('SelectedCoreObject')) || (pg && pg.props.get('SelectedObject'));
          if (sel && !sel.destroyed && sel.isA('GuiButton') && sel.props.get('Visible') !== false) {
            e.preventDefault();
            world.dm.emit(sel, 'MouseButton1Click');
            world.dm.emit(sel, 'Activated');
            return;
          }
        }
        if (e.key === '/' && world.host.guiKeys && world.host.guiKeys.has('special:5')) {
          e.preventDefault();
          world.dm.emit(world.dm.services.get('GuiService'), 'SpecialKeyPressed', T.e('SpecialKey', 5));
          return;
        }
        world.ensureAudio();
        held.add(e.key.toLowerCase());
        if (e.code) codes.add(e.code);
        const lk = e.key.length === 1 ? e.key.toLowerCase() : e.key === 'Enter' ? '\r' : null;
        if (lk) world.keyDown(lk);
        return;
      }
      if (gui.isTyping()) return;
      const k = e.key.toLowerCase();
      const dm = world.dm;
      const gs = dm.services.get('GuiService');
      const keys = world.host.guiKeys;
      if (k === '/' && keys.has('special:5')) {
        e.preventDefault();
        dm.emit(gs, 'SpecialKeyPressed', T.e('SpecialKey', 5));
        return;
      }
      if (k === 'escape') dm.emit(gs, 'EscapeKeyPressed');
      const special = { insert: 0, home: 1, end: 2, pageup: 3, pagedown: 4 }[k];
      if (special !== undefined && keys.has('special:' + special)) dm.emit(gs, 'SpecialKeyPressed', T.e('SpecialKey', special));
      const claimed = k.length === 1 && keys.has(k);
      if (claimed && !e.repeat) dm.emit(gs, 'KeyPressed', k);
      if (k === 'tab') e.preventDefault();
      if (e.repeat) return;
      world.ensureAudio();
      held.add(k);
      if (e.code) codes.add(e.code);
      if (k === ' ') {
        state.jumpQueued = true;
        e.preventDefault();
      }
      if (k === 'shift' && world.era !== 2018) camera.toggleShiftLock();
      if (k === 'arrowleft') camera.keyYaw = 1;
      if (k === 'arrowright') camera.keyYaw = -1;
      if (k === 'i') camera.stepZoom(1);
      if (k === 'o') camera.stepZoom(-1);
      if (k === 'backspace') {
        e.preventDefault();
        world.dropTool();
      }
      if (/^[0-9]$/.test(k) && !claimed) {
        const items = world.backpackItems();
        const idx = k === '0' ? 9 : +k - 1;
        if (items[idx]) world.toggleSlot(items[idx]);
      }
      const lua = k === ' ' ? ' ' : k === 'enter' ? '\r' : k === 'backspace' ? '\b' : k === 'tab' ? '\t' : k === 'shift' ? String.fromCharCode(48) : k.length === 1 ? k : null;
      if (lua) world.keyDown(lua);
    });
    window.addEventListener('keyup', (e) => {
      if (world.input2018) world.input2018.keyUp(e, gui.isTyping());
      const k = e.key.toLowerCase();
      held.delete(k);
      if (e.code) codes.delete(e.code);
      if (k === 'arrowleft' || k === 'arrowright') camera.keyYaw = 0;
      const lua = k === ' ' ? ' ' : k === 'enter' ? '\r' : k === 'shift' ? String.fromCharCode(48) : k.length === 1 ? k : null;
      if (lua) world.keyUp(lua);
    });
    window.addEventListener('blur', () => {
      if (world.input2018) world.input2018.blur();
      held.clear();
      codes.clear();
      camera.keyYaw = 0;
    });
    canvas.addEventListener('pointermove', (e) => {
      const r = canvas.getBoundingClientRect();
      const centered = camera.aimCentered();
      ms.x = centered ? r.width / 2 : e.clientX - r.left;
      ms.y = centered ? r.height / 2 : e.clientY - r.top;
      ms.ndc = [(ms.x / r.width) * 2 - 1, -(ms.y / r.height) * 2 + 1];
      world.emitMouse('Move');
    });
    window.addEventListener('pointermove', (e) => {
      if (!world.input2018) return;
      const locked = document.pointerLockElement === canvas;
      if (!locked && overGuiTarget(e)) {
        const r = canvas.getBoundingClientRect();
        ms.x = e.clientX - r.left;
        ms.y = e.clientY - r.top;
      }
      const at = locked ? null : [e.clientX, e.clientY];
      if (locked || e.target === canvas) world.input2018.mouseMove(e.movementX || 0, e.movementY || 0, false, at);
      else if (overGuiTarget(e)) world.input2018.mouseMove(e.movementX || 0, e.movementY || 0, true, at);
    });
    canvas.addEventListener('pointerdown', (e) => {
      world.ensureAudio();
      if (gui.isTyping()) document.activeElement.blur();
      relock();
      if (world.input2018) world.input2018.buttonDown(e.button, false, document.pointerLockElement === canvas ? null : [e.clientX, e.clientY]);
      if (e.button === 0) world.mouseDown(0);
    });
    root.addEventListener('pointerdown', (e) => {
      if (!world.input2018 || !overGuiTarget(e)) return;
      world.ensureAudio();
      const r = canvas.getBoundingClientRect();
      ms.x = e.clientX - r.left;
      ms.y = e.clientY - r.top;
      world.input2018.buttonDown(e.button, true, [e.clientX, e.clientY]);
    }, true);
    window.addEventListener('pointerup', (e) => {
      if (world.input2018) world.input2018.buttonUp(e.button, overGuiTarget(e), document.pointerLockElement === canvas ? null : [e.clientX, e.clientY]);
      if (e.button === 0 && ms.down) world.mouseUp(0);
    }, true);
    let wheelAcc = 0;
    canvas.addEventListener('wheel', (e) => {
      const unit = e.deltaMode === 1 ? 3 : e.deltaMode === 2 ? 1 : 100;
      if (Math.sign(e.deltaY) !== Math.sign(wheelAcc)) wheelAcc = 0;
      wheelAcc += e.deltaY / unit;
      while (Math.abs(wheelAcc) >= 1) {
        wheelAcc -= Math.sign(wheelAcc);
        world.emitMouse(e.deltaY > 0 ? 'WheelBackward' : 'WheelForward');
        if (world.input2018) world.input2018.wheel(e.deltaY > 0 ? -1 : 1, false);
      }
      if (world.input2018) e.preventDefault();
      if (Math.abs(e.deltaY) >= unit * 0.99) wheelAcc = 0;
    }, { passive: false });
  };

  const frame = (now) => {
    if (window.solarVm && /[?&]swirlhost=1/.test(location.search)) {
      try {
        window.solarVm.tick(1 / 60);
        for (const l of JSON.parse(window.solarVm.logs())) (window.solarRustLogs = window.solarRustLogs || []).push(l);
      } catch (e) {
        errors.push('vmtick: ' + e);
      }
    }
    for (const raw of JSON.parse(client.take_tap())) {
      let m;
      try {
        m = JSON.parse(raw);
      } catch (e) {
        continue;
      }
      if (m.t === 'init' || m.t === 'rep') {
        try {
          if (!window.solarVm || m.t === 'init') {
            window.solarVm = new LuaVm(Date.now() / 1000);
            window.solarVm.set_run(/[?&]swirlhost=1/.test(location.search));
          }
          window.solarVm.apply(raw);
        } catch (e) {
          errors.push('vm: ' + (e && e.stack ? e.stack : e));
        }
      }
      if (m.t === 'init') {
        if (!booting) boot(m).catch((e) => {
          errors.push('boot: ' + (e && e.stack ? e.stack : e));
          console.error(e);
        });
      } else if (world.ready) {
        try {
          route(m);
        } catch (e) {
          errors.push('route: ' + (e && e.stack ? e.stack : e));
        }
      } else if (booting) state.pending.push(m);
    }
    if (!state.ready) return;
    const frameMs = 1000 / 60;
    if (now < state.due - 1) return;
    state.due = state.due + frameMs < now ? now + frameMs : state.due + frameMs;
    const dt = state.last ? Math.min(0.1, (now - state.last) / 1000) : 0;
    state.last = now;
    const camera = state.camera;
    const f = camera.forwardFlat(), rt = camera.rightFlat();
    const held = state.held, codes = state.codes;
    const key = (code, letter) => (codes.size || !held.has(letter) ? codes.has(code) : true);
    const fw = (key('KeyW', 'w') || held.has('arrowup') ? 1 : 0) - (key('KeyS', 's') || held.has('arrowdown') ? 1 : 0);
    const sd = (key('KeyD', 'd') ? 1 : 0) - (key('KeyA', 'a') ? 1 : 0);
    const move = [f[0] * fw + rt[0] * sd, 0, f[2] * fw + rt[2] * sd];
    const input = { move, face: camera.faceDir(), jump: held.has(' ') || state.jumpQueued, dismount: held.has('backspace'), forward: fw > 0.3, back: fw < -0.3, left: sd < -0.3, right: sd > 0.3 };
    state.jumpQueued = false;
    try {
      const w = canvas.clientWidth || 1;
      const h = canvas.clientHeight || 1;
      state.renderer.camera.aspect = w / h;
      state.renderer.camera.updateProjectionMatrix();
      world.camera.props.set('ViewportSize', [w, h]);
      world.update(dt, input);
      camera.update(dt);
      if (camera.aimCentered()) {
        const cr = canvas.getBoundingClientRect();
        ms.x = cr.width / 2;
        ms.y = cr.height / 2;
        ms.ndc = [0, 0];
      }
      const ray = state.renderer.pick(ms.ndc[0], ms.ndc[1]);
      ms.overGui = false;
      const cd = world.updateMouse(ray, state.renderer);
      const icon = (world.toolMouse && world.toolMouse.props.get('Icon')) || world.mouse.props.get('Icon');
      cursor.set(camera.shiftLock && camera.mouseLocked() && !camera.firstPerson ? CURSORS.mouseLock : icon || (cd ? CURSORS.hand : world.mouseHit && world.mouseHit.part ? CURSORS.arrow : CURSORS.far));
      if (document.pointerLockElement === canvas || camera.aimCentered()) cursor.place(camera.aimCentered(), true);
      state.renderer.frame(dt);
      state.gui.frame();
    } catch (e) {
      errors.push('frame: ' + (e && e.stack ? e.stack : e));
      console.error(e);
    }
  };

  const ownPos = () => {
    const o = world.own;
    if (!o) return null;
    const cf = o.rec.cf;
    return [cf[0], cf[1], cf[2]];
  };

  const api = { world, state, errors, gameErrors, logs, frame, root, ownPos, ready: () => state.ready };
  window.solarLua = api;
  return api;
}

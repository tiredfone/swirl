const trim = (p) => String(p || '/chocolate').replace(/\/+$/, '');

function addImportMap(pub) {
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
}

function addStyle(pub) {
  if (document.getElementById('swirl-gui-style')) return;
  const l = document.createElement('link');
  l.id = 'swirl-gui-style';
  l.rel = 'stylesheet';
  l.href = pub + '/client/style.css';
  document.head.appendChild(l);
}

export async function startLua(opts) {
  const { client, cfg, canvas } = opts;
  const pub = trim(cfg.publicPath);
  addImportMap(pub);
  addStyle(pub);
  const [THREE, assets, worldMod, guiMod, inputMod, luahost, C, wasmoon] = await Promise.all([
    import('three'),
    import(pub + '/client/assets.js'),
    import(pub + '/client/world.js'),
    import(pub + '/client/gui.js'),
    import(pub + '/client/input2018.js'),
    import(pub + '/shared/luahost.js'),
    import(pub + '/shared/cframe.js'),
    import('wasmoon-lua5.1'),
  ]);
  const { ClientWorld, prefetch2018 } = worldMod;
  const { Gui } = guiMod;
  const { Input2018 } = inputMod;
  const { T } = luahost;
  const { Lua } = wasmoon;
  assets.configure(cfg);
  assets.loadContentIndex();
  const runtimeSrc = fetch(pub + '/shared/runtime.lua?v=' + (cfg.version || '')).then((r) => r.text());

  const root = document.createElement('div');
  root.id = 'chocolate';
  root.className = 'swirl-gui-root';
  root.style.cssText = 'position:fixed;inset:0;pointer-events:none;z-index:5';
  document.body.appendChild(root);

  const errors = [];
  const logs = [];
  const send = (o) => client.send(JSON.stringify(o));
  const net = { send, bytesIn: 0, bytesOut: 0, close() {} };

  const cam3 = new THREE.PerspectiveCamera(57.3, 1, 0.5, 20000);
  const rendererStub = { camera: cam3 };

  const noHit = (o, d) => ({ part: null, point: [o[0] + d[0], o[1] + d[1], o[2] + d[2]], normal: [0, 1, 0] });

  class SwirlWorld extends ClientWorld {
    constructor(n, hooks) {
      super(n, hooks);
      this.partCache = null;
      this.partDirty = true;
      this.physics = {
        attachWorkspace() {},
        setTerrain() {},
        removePart() {},
        enter() {},
        raycast: (o, d, ig) => this.rayCast(o, d, ig),
        touchingParts: () => [],
      };
    }

    trackChar() {
      return null;
    }

    dropChar() {}

    charDied() {}

    takeOwnership(m) {
      const model = this.dm.byId.get(m.id);
      if (!model) {
        this.pendingOwn = m;
        return;
      }
      this.pendingOwn = null;
      const hum = model.findChildOfClass('Humanoid');
      const cf = Array.isArray(m.c) ? m.c.slice() : [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1];
      this.own = { model, hum, ctrl: { teleport() {}, vel: [0, 0, 0] }, rec: { cf, model, hum }, moveTo: null };
      this.ownModel = model;
      this.tpSeq = m.s;
      const held = model.children.find((c) => c.ClassName === 'Tool');
      if (held && this.equippedTool !== held) {
        this.equippedTool = held;
        this.dm.emit(held, 'Equipped', this.openToolMouse());
      }
      if (this.hooks.ownCharacter) this.hooks.ownCharacter(model);
    }

    applyOp(op) {
      super.applyOp(op);
      if (op[0] === 'n' || op[0] === 'm' || op[0] === 'd') {
        this.partDirty = true;
        if (this.pendingOwn) this.takeOwnership(this.pendingOwn);
      }
    }

    parts() {
      if (this.partDirty || !this.partCache) {
        this.partDirty = false;
        const ws = this.dm.services.get('Workspace');
        this.partCache = ws ? ws.descendants().filter((d) => d.isA('BasePart') && d.ClassName !== 'Terrain') : [];
      }
      return this.partCache;
    }

    rayCast(o, d, ignore) {
      const ig = (ignore || []).filter(Boolean);
      let best = Infinity;
      let hitPart = null;
      let hitN = [0, 1, 0];
      for (const p of this.parts()) {
        if (p.destroyed) continue;
        if (ig.length && ig.some((x) => p === x || p.isDescendantOf(x))) continue;
        const cf = p.props.get('CFrame');
        const sz = p.props.get('Size');
        if (!cf || !sz) continue;
        const rx = o[0] - cf[0], ry = o[1] - cf[1], rz = o[2] - cf[2];
        const lo = [cf[3] * rx + cf[6] * ry + cf[9] * rz, cf[4] * rx + cf[7] * ry + cf[10] * rz, cf[5] * rx + cf[8] * ry + cf[11] * rz];
        const ld = [cf[3] * d[0] + cf[6] * d[1] + cf[9] * d[2], cf[4] * d[0] + cf[7] * d[1] + cf[10] * d[2], cf[5] * d[0] + cf[8] * d[1] + cf[11] * d[2]];
        let t0 = 0, t1 = best, axis = -1, sign = 1;
        let ok = true;
        for (let i = 0; i < 3; i++) {
          const h = sz[i] / 2;
          if (Math.abs(ld[i]) < 1e-9) {
            if (lo[i] < -h || lo[i] > h) {
              ok = false;
              break;
            }
          } else {
            let a = (-h - lo[i]) / ld[i], b = (h - lo[i]) / ld[i];
            let s = -1;
            if (a > b) {
              const tmp = a;
              a = b;
              b = tmp;
              s = 1;
            }
            if (a > t0) {
              t0 = a;
              axis = i;
              sign = s;
            }
            if (b < t1) t1 = b;
            if (t0 > t1) {
              ok = false;
              break;
            }
          }
        }
        if (!ok || t0 >= best || t0 <= 0) continue;
        best = t0;
        hitPart = p;
        const nl = [0, 0, 0];
        if (axis >= 0) nl[axis] = sign;
        hitN = [cf[3] * nl[0] + cf[4] * nl[1] + cf[5] * nl[2], cf[6] * nl[0] + cf[7] * nl[1] + cf[8] * nl[2], cf[9] * nl[0] + cf[10] * nl[1] + cf[11] * nl[2]];
      }
      if (!hitPart) return noHit(o, d);
      return { part: hitPart, point: [o[0] + d[0] * best, o[1] + d[1] * best, o[2] + d[2] * best], normal: hitN };
    }

    async initSwirl(msg, luaFactory, runtimeText) {
      const { applyRecords } = await import(pub + '/shared/codec.js');
      const { ScriptHost } = await import(pub + '/shared/luahost.js');
      const { Core2018 } = await import(pub + '/shared/core2018.js');
      const { FLAGS_2018 } = await import(pub + '/shared/api2018.js');
      const { Animations } = await import(pub + '/shared/animator.js');
      const { parsePlace } = await import(pub + '/shared/rbxl.js');
      const dm = this.dm;
      this.era = msg.era === 2018 ? 2018 : 2012;
      dm.era = this.era;
      const pack = this.era === 2018 ? await prefetch2018(cfg.version) : null;
      applyRecords(dm, msg.snap);
      for (const s of ['Workspace', 'Players', 'Lighting', 'StarterGui', 'StarterPack', 'Teams', 'SoundService', 'Debris', 'ReplicatedStorage', 'ReplicatedFirst', 'RunService', 'UserInputService', 'ContentProvider', 'GuiService', 'Chat']) dm.service(s);
      this.player = dm.byId.get(msg.you);
      this.user = msg.user;
      const players = dm.services.get('Players');
      players.props.set('LocalPlayer', this.player);
      this.mouse = dm.create('PlayerMouse', null);
      this.mouse.props.set('Name', 'Mouse');
      const ws = dm.services.get('Workspace');
      this.camera = dm.create('Camera', null);
      this.camera.props.set('Name', 'Camera');
      this.camera.props.set('CameraType', 5);
      this.camera.props.set('FieldOfView', 57.3);
      for (const o of dm.observers) if (o.created) o.created(this.camera);
      dm.setParent(this.camera, ws);
      ws.props.set('CurrentCamera', this.camera);
      dm.observe({
        reparented: (inst, old, o) => this.onReparent(inst, old, o),
        destroyed: (inst) => {
          if (this.sounds.has(inst)) this.stopSound(inst);
        },
        changed: (inst, name, old, o) => this.onChange(inst, name, old, o),
      });
      this.coreSources = pack ? new Map() : await assets.loadCoreSources([worldMod.STARTER_SCRIPT]).catch(() => new Map());
      const coreGui = dm.service('CoreGui');
      this.robloxGui = dm.create('ScreenGui', null);
      this.robloxGui.props.set('Name', 'RobloxGui');
      this.robloxGui.props.set('AbsoluteSize', this.guiArea().size);
      for (const o of dm.observers) if (o.created) o.created(this.robloxGui);
      dm.setParent(this.robloxGui, coreGui);
      if (pack) {
        for (const s of ['StarterPlayer', 'TweenService', 'TextService', 'LocalizationService', 'VRService', 'RobloxReplicatedStorage']) dm.service(s);
        this.core = new Core2018(dm, pack.bundle);
        this.core.buildModules(this.robloxGui);
        const cf = this.gui('Frame', 'ControlFrame', this.robloxGui, { BackgroundTransparency: 1, Size: [1, 0, 1, 0] });
        this.gui('Frame', 'BottomLeftControl', cf, { BackgroundTransparency: 1, Position: [0, 0, 1, -50], Size: [0, 130, 0, 50] });
      } else this.buildControlFrame();
      for (const s of ['ScriptContext', 'GuiService', 'LogService', 'MarketplaceService', 'TeleportService', 'InsertService', 'ChangeHistoryService', 'PersonalServerService', 'PointsService', 'HttpService', 'ContextActionService', 'NetworkClient', 'CollectionService', 'LocalBackpack']) dm.service(s);
      const uis = dm.services.get('UserInputService');
      uis.props.set('TouchEnabled', false);
      uis.props.set('MouseEnabled', true);
      uis.props.set('KeyboardEnabled', true);
      this.host = new ScriptHost(dm, this.env());
      this.anims = new Animations(dm, async (id) => {
        const bytes = await assets.animationBytes(id);
        if (!new TextDecoder().decode(bytes.subarray(0, 8)).startsWith('<roblox')) throw new Error('asset ' + id + ' is not a model');
        const model = dm.create('Model', null);
        model.props.set('Name', 'Model');
        dm.load(parsePlace(bytes), model);
        return model;
      });
      if (pack) this.host.flags = FLAGS_2018;
      await this.host.init(luaFactory, runtimeText, pack && pack.runtime ? [{ source: pack.runtime, name: 'runtime2018' }] : []);
      if (pack) {
        dm.root.props.set('PlaceId', msg.place | 0);
        this.core.coreScript('LoadingScript', this.robloxGui);
        this.core.starterScript(dm.services.get('ScriptContext'), false);
      } else if (this.coreSources.get(worldMod.STARTER_SCRIPT)) this.addCoreScript(worldMod.STARTER_SCRIPT, dm.services.get('ScriptContext'), 'StarterScript');
    }
  }

  const state = { world: null, gui: null, input: null, pending: [], ready: false, last: 0, mouseAcc: 0 };
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
      rc.setFromCamera(new THREE.Vector2((x / k / root.clientWidth) * 2 - 1, -(y / k / root.clientHeight) * 2 + 1), cam3);
      return { o: rc.ray.origin.toArray(), d: rc.ray.direction.toArray() };
    },
    worldToScreen: (p) => {
      const v = new THREE.Vector3(p[0], p[1], p[2]);
      const depth = v.clone().applyMatrix4(cam3.matrixWorldInverse).z;
      v.project(cam3);
      const k = state.world.guiScale();
      return [((v.x + 1) / 2) * root.clientWidth * k, ((1 - v.y) / 2) * root.clientHeight * k, -depth, depth < 0 && Math.abs(v.x) <= 1 && Math.abs(v.y) <= 1];
    },
    measureText: (text, size, font, frame) => (state.gui && state.gui.measureText ? state.gui.measureText(text, size, font, frame) : null),
    screenSize: () => [root.clientWidth || window.innerWidth, root.clientHeight || window.innerHeight],
    rotateCamera: () => {},
    zoomCamera: () => {},
    cameraTilt: () => {},
    cameraZoom: () => {},
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
        errors.push(line);
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

  const world = new SwirlWorld(net, hooks);
  state.world = world;

  const route = (m) => {
    const dm = world.dm;
    switch (m.t) {
      case 'rep':
        world.applyRep(m);
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
        world.runSoon();
        break;
      }
      case 'bubble': {
        const p = dm.byId.get(m.id);
        if (p && world.era === 2018) dm.emit(dm.services.get('Chat'), 'Chatted', p, m.m, T.e('ChatColor', (m.c | 0) || 0));
        break;
      }
      case 'msg':
        if (state.gui) state.gui.setHint(m.m);
        break;
      default:
        break;
    }
  };

  let booting = false;
  const boot = async (m) => {
    booting = true;
    const text = await runtimeSrc;
    await world.initSwirl(m, () => Lua.create({ customWasmUri: pub + '/client/vendor/liblua5.1.wasm' }), text);
    const gui = new Gui(root, world, rendererStub, {
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
    }
    wire(gui);
    world.ready = true;
    for (const p of state.pending.splice(0)) route(p);
    world.markLoaded();
    state.ready = true;
  };

  const ms = world.mouseState;
  const wire = (gui) => {
    const rectOf = () => canvas.getBoundingClientRect();
    canvas.addEventListener('pointermove', (e) => {
      const r = rectOf();
      ms.x = e.clientX - r.left;
      ms.y = e.clientY - r.top;
      ms.ndc = [(ms.x / r.width) * 2 - 1, -(ms.y / r.height) * 2 + 1];
      state.moved = true;
      world.emitMouse('Move');
    });
    window.addEventListener('pointermove', (e) => {
      if (!state.input) return;
      const over = e.target !== canvas && root.contains(e.target);
      if (over) {
        const r = rectOf();
        ms.x = e.clientX - r.left;
        ms.y = e.clientY - r.top;
      }
      const locked = document.pointerLockElement === canvas;
      if (locked || e.target === canvas) state.input.mouseMove(e.movementX || 0, e.movementY || 0, false, locked ? null : [e.clientX, e.clientY]);
      else if (over) state.input.mouseMove(e.movementX || 0, e.movementY || 0, true, [e.clientX, e.clientY]);
    });
    canvas.addEventListener('pointerdown', (e) => {
      if (gui.isTyping()) document.activeElement.blur();
      if (state.input) state.input.buttonDown(e.button, false, [e.clientX, e.clientY]);
      if (e.button === 0) world.mouseDown(0);
    });
    root.addEventListener('pointerdown', (e) => {
      if (!state.input || e.target === canvas) return;
      state.input.buttonDown(e.button, true, [e.clientX, e.clientY]);
    }, true);
    window.addEventListener('pointerup', (e) => {
      const over = e.target !== canvas && root.contains(e.target);
      if (state.input) state.input.buttonUp(e.button, over, [e.clientX, e.clientY]);
      if (e.button === 0 && ms.down) world.mouseUp(0);
    }, true);
    canvas.addEventListener('wheel', (e) => {
      world.emitMouse(e.deltaY > 0 ? 'WheelBackward' : 'WheelForward');
      if (state.input) state.input.wheel(e.deltaY > 0 ? -1 : 1, false);
    }, { passive: true });
    window.addEventListener('keydown', (e) => {
      if (state.input) state.input.keyDown(e, gui.isTyping());
      if (gui.isTyping() || e.repeat) return;
      const k = e.key.length === 1 ? e.key.toLowerCase() : e.key.toLowerCase();
      const dm = world.dm;
      const gs = dm.services.get('GuiService');
      const keys = world.host.guiKeys;
      if (k === 'escape') dm.emit(gs, 'EscapeKeyPressed');
      const claimed = k.length === 1 && keys && keys.has(k);
      if (k === '/' && keys && keys.has('special:5')) {
        e.preventDefault();
        dm.emit(gs, 'SpecialKeyPressed', T.e('SpecialKey', 5));
        return;
      }
      if (claimed) dm.emit(gs, 'KeyPressed', k);
      if (world.era !== 2018 && /^[0-9]$/.test(k) && !claimed) {
        const items = world.backpackItems();
        const idx = k === '0' ? 9 : +k - 1;
        if (items[idx]) world.toggleSlot(items[idx]);
      }
      if (world.era !== 2018 && k === 'backspace') world.dropTool();
      const lua = k === ' ' ? ' ' : k === 'enter' ? '\r' : k === 'backspace' ? '\b' : k === 'tab' ? '\t' : k.length === 1 ? k : null;
      if (lua) world.keyDown(lua);
    });
    window.addEventListener('keyup', (e) => {
      if (state.input) state.input.keyUp(e, gui.isTyping());
      const k = e.key.length === 1 ? e.key.toLowerCase() : e.key.toLowerCase();
      const lua = k === ' ' ? ' ' : k === 'enter' ? '\r' : k.length === 1 ? k : null;
      if (lua) world.keyUp(lua);
    });
    window.addEventListener('blur', () => {
      if (state.input) state.input.blur();
    });
  };

  const syncCamera = () => {
    let pose;
    try {
      pose = JSON.parse(client.camera_pose());
    } catch (e) {
      return;
    }
    const cf = C.lookAt(pose[0], pose[1], pose[2], pose[3], pose[4], pose[5]);
    world.camera.props.set('CFrame', cf);
    const w = canvas.clientWidth || 1;
    const h = canvas.clientHeight || 1;
    cam3.aspect = w / h;
    cam3.fov = (pose[6] * 180) / Math.PI;
    cam3.updateProjectionMatrix();
    cam3.position.set(cf[0], cf[1], cf[2]);
    const m = new THREE.Matrix4().set(cf[3], cf[4], cf[5], cf[0], cf[6], cf[7], cf[8], cf[1], cf[9], cf[10], cf[11], cf[2], 0, 0, 0, 1);
    m.decompose(cam3.position, cam3.quaternion, cam3.scale);
    cam3.updateMatrixWorld(true);
    cam3.matrixWorldInverse.copy(cam3.matrixWorld).invert();
    world.camera.props.set('ViewportSize', [w, h]);
  };

  const syncOwn = () => {
    if (!world.own) return;
    try {
      const p = JSON.parse(client.own_pos());
      if (!p) return;
      const cf = world.own.rec.cf;
      cf[0] = p[0];
      cf[1] = p[1];
      cf[2] = p[2];
    } catch (e) {}
  };

  const frame = (now) => {
    for (const raw of JSON.parse(client.take_tap())) {
      let m;
      try {
        m = JSON.parse(raw);
      } catch (e) {
        continue;
      }
      if (m.t === 'init') {
        if (!booting) boot(m).catch((e) => {
          errors.push('boot: ' + (e && e.stack ? e.stack : e));
          console.error(e);
        });
      } else if (world.ready) route(m);
      else if (booting) state.pending.push(m);
    }
    if (!state.ready) return;
    const dt = state.last ? Math.min(0.1, (now - state.last) / 1000) : 0;
    state.last = now;
    state.mouseAcc += dt;
    world.t += dt;
    world.mouseSendAcc += dt;
    syncCamera();
    syncOwn();
    const dm = world.dm;
    try {
      world.host.tick(world.t);
      const rs = dm.services.get('RunService');
      if (rs && dm.hasListener(rs, 'Stepped')) dm.emit(rs, 'Stepped', world.t, dt);
      if (rs && dm.hasListener(rs, 'Heartbeat')) dm.emit(rs, 'Heartbeat', dt);
      if (rs && dm.hasListener(rs, 'RenderStepped')) dm.emit(rs, 'RenderStepped', dt);
      world.host.renderStep(dt);
      if (state.moved || state.mouseAcc > 0.15) {
        state.moved = false;
        state.mouseAcc = 0;
        const k = world.guiScale();
        const rc = new THREE.Raycaster();
        rc.setFromCamera(new THREE.Vector2(ms.ndc ? ms.ndc[0] : 0, ms.ndc ? ms.ndc[1] : 0), cam3);
        world.updateMouse({ origin: rc.ray.origin.toArray(), dir: rc.ray.direction.toArray() }, rendererStub);
        void k;
      }
      world.sendMouse(false);
      state.gui.frame();
    } catch (e) {
      errors.push('frame: ' + (e && e.stack ? e.stack : e));
      console.error(e);
    }
  };

  const api = { world, state, errors, logs, frame, root, ready: () => state.ready };
  window.solarLua = api;
  return api;
}

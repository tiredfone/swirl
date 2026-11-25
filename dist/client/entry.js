import { useApp as appMode } from './pack.js';
import init, { Client, Boot, client_version } from './swirl_client.js';

const byId = (id) => document.getElementById(id);

function failed(reason) {
  if (window.solarBoot) window.solarBoot.fail();
  if (typeof window.solarClientFailed === 'function') window.solarClientFailed(reason);
}

export async function start(opts = {}) {
  const cfg = opts.config || window.solarClientConfig || {};
  const status = byId('swirl-status');
  let boot = null;
  const setStatus = (s) => {
    if (boot && boot.present()) boot.set_status(s);
    else if (status) status.textContent = s;
  };
  try {
    setStatus('Loading Swirl');
    await init({ module_or_path: new URL('./swirl_client_bg.wasm', import.meta.url) });
    boot = new Boot();
    window.solarBoot = boot;
    window.solarSetStatus = setStatus;
    setStatus('Requesting a server');
    const override = new URLSearchParams(location.search).get('swirlws');
    let t = opts.ticket;
    if (override && /^wss?:\/\/(127\.0\.0\.1|localhost)(:\d+)?\//.test(override)) t = { ws: override, ticket: 'dev' };
    if (!t) {
      const endpoint = cfg.endpoints && cfg.endpoints.ticket;
      const r = await fetch(endpoint, {
        method: 'POST',
        credentials: 'same-origin',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': cfg.csrf },
        body: JSON.stringify(cfg.server ? { placeId: cfg.placeId, server: cfg.server } : { placeId: cfg.placeId }),
      });
      t = await r.json().catch(() => null);
      if (!r.ok || !t || !t.ticket) throw new Error((t && t.error) || 'Could not get a join ticket');
    }
    const path = t.ws || cfg.websocketPath || (cfg.publicPath || '') + '/ws';
    const base = /^wss?:\/\//.test(path) ? path : (location.protocol === 'https:' ? 'wss:' : 'ws:') + '//' + location.host + path;
    const canvas = opts.canvas || byId('swirl-canvas');
    const client = new Client(canvas);
    window.solarClient = client;
    setStatus('Connecting');
    client.connect(base + '?ticket=' + encodeURIComponent(t.ticket));
    setInterval(() => client.ping(), 5000);
    const useAppEarly = appMode;
    if (useAppEarly) window.solarContentReady = new Promise((res) => { window.__resolveContent = res; });
    const assetUrl = (id) => (id.startsWith('c:') ? (window.solarContentUrl ? window.solarContentUrl(id.slice(2)) : new URL('./chocolate/content/' + id.slice(2), import.meta.url).href) : ((cfg.endpoints && cfg.endpoints.asset) || '/asset/?id={id}').replace('{id}', id));
    let inflight = 0;
    const fetchAsset = async (r) => {
      inflight++;
      try {
        if (useAppEarly && r.id.startsWith('c:')) await window.solarContentReady;
        for (let n = 0; r.id.startsWith('c:') && !window.solarContentUrl && n < 600; n++) await new Promise((res) => setTimeout(res, 50));
        for (let n = 0; r.k === 'tex' && !(window.solarLoadImage && window.solarWorldReady) && n < 600; n++) await new Promise((res) => setTimeout(res, 50));
        if (r.k === 'tex' && window.solarWorldReady) await window.solarWorldReady;
        if (r.k === 'tex' && window.solarLoadImage) {
          const img = await window.solarLoadImage(r.id.startsWith('c:') ? 'rbxasset://' + r.id.slice(2) : r.id);
          if (!img) throw new Error('no image');
          client.provide_texture(r.id, img);
          return;
        }
        const resp = await fetch(assetUrl(r.id), { credentials: 'same-origin' });
        if (!resp.ok) throw new Error('status ' + resp.status);
        let buf = new Uint8Array(await resp.arrayBuffer());
        if (buf[0] === 0x25) buf = buf.subarray(buf.indexOf(10) + 1);
        if (r.k === 'mesh') client.provide_mesh(r.id, buf);
        else client.provide_texture(r.id, await createImageBitmap(new Blob([buf]), { imageOrientation: 'flipY', premultiplyAlpha: 'none', colorSpaceConversion: 'none' }));
      } catch (e) {
        (window.solarAssetFailures || (window.solarAssetFailures = [])).push(r.k + ':' + r.id + ':' + (e && e.message));
        client.provide_failed(r.k, r.id);
      } finally {
        inflight--;
      }
    };
    const pending = [];
    setInterval(() => {
      for (const r of JSON.parse(client.take_requests())) pending.push(r);
      while (inflight < 6 && pending.length) fetchAsset(pending.shift());
    }, 80);
    let lua = null;
    let luaDone = false;
    const useApp = appMode;
    const luaStart = (useApp ? import('./app.js').then((m) => m.startApp({ client, cfg, canvas })) : import('./lua.js').then((m) => m.startLua({ client, cfg, canvas, reference: new URLSearchParams(location.search).get('swirlref') === 'three' }))).then((l) => { lua = l; luaDone = true; }).catch((e) => {
      luaDone = true;
      window.solarLuaFailed = String(e && e.stack ? e.stack : e);
      console.error(e);
    });
    void luaStart;
    let shown = false;
    let loaded = false;
    let loadedAt = 0;
    let ownSince = 0;
    const loop = () => {
      const now = performance.now();
      if (lua) lua.frame(now);
      client.frame(now);
      if (client.ready() && !loaded) {
        loaded = true;
        loadedAt = now;
        setStatus('Loading the world');
      }
      let ownOk = true;
      if (useApp && loaded && !shown) {
        const v = window.solarApp && window.solarApp.vm;
        if (v && v.own_ready()) ownSince = ownSince || now;
        ownOk = (ownSince && now - ownSince > 600) || now - loadedAt > 30000;
      }
      if (loaded && !shown && luaDone && ownOk) {
        shown = true;
        setStatus('Starting');
        const c = client.boot_counts();
        boot.finish(client.era(), c[0], c[1], now);
        if (!boot.present() && status) status.hidden = true;
        if (typeof window.solarClientReady === 'function') window.solarClientReady();
      }
      boot.step(now);
      if (client.status().startsWith('Disconnected')) {
        failed('disconnected');
        return;
      }
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
    return client;
  } catch (e) {
    failed(e && e.message ? e.message : String(e));
    throw e;
  }
}

if (window.solarClientConfig && !window.solarClientNoAutostart) start();

export default start;

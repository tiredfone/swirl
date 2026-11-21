import init, { Client, client_version } from './swirl_client.js';

const byId = (id) => document.getElementById(id);

function failed(reason) {
  if (typeof window.solarClientFailed === 'function') window.solarClientFailed(reason);
}

export async function start(opts = {}) {
  const cfg = opts.config || window.solarClientConfig || {};
  const status = byId('swirl-status');
  const setStatus = (s) => {
    if (status) status.textContent = s;
  };
  try {
    setStatus('Loading Swirl ' + client_version());
    await init({ module_or_path: new URL('./swirl_client_bg.wasm', import.meta.url) });
    setStatus('Requesting a server');
    let t = opts.ticket;
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
    setStatus('Joining');
    client.connect(base + '?ticket=' + encodeURIComponent(t.ticket));
    setInterval(() => client.ping(), 5000);
    let shown = false;
    const loop = () => {
      client.frame();
      if (client.ready() && !shown) {
        shown = true;
        if (typeof window.solarClientReady === 'function') window.solarClientReady();
      }
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

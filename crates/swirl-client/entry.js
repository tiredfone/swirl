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
    setStatus('Loading Swirl');
    await init({ module_or_path: new URL('./swirl_client_bg.wasm', import.meta.url) });
    setStatus('Swirl ' + client_version());
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
    setStatus('Joining');
    client.connect(base + '?ticket=' + encodeURIComponent(t.ticket));
    setInterval(() => client.ping(), 5000);
    const assetUrl = (id) => (id.startsWith('c:') ? (cfg.publicPath || '/chocolate') + '/content/' + id.slice(2) : ((cfg.endpoints && cfg.endpoints.asset) || '/asset/?id={id}').replace('{id}', id));
    let inflight = 0;
    const fetchAsset = async (r) => {
      inflight++;
      try {
        const resp = await fetch(assetUrl(r.id), { credentials: 'same-origin' });
        if (!resp.ok) throw new Error('status ' + resp.status);
        let buf = new Uint8Array(await resp.arrayBuffer());
        if (buf[0] === 0x25) buf = buf.subarray(buf.indexOf(10) + 1);
        if (r.k === 'mesh') client.provide_mesh(r.id, buf);
        else client.provide_texture(r.id, await createImageBitmap(new Blob([buf]), { premultiplyAlpha: 'none', colorSpaceConversion: 'none' }));
      } catch (e) {
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
    let shown = false;
    const loop = () => {
      client.frame(performance.now());
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

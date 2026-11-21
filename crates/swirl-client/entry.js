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
    const bar = document.createElement('div');
    bar.id = 'swirl-hotbar';
    bar.style.cssText = 'position:fixed;left:50%;bottom:10px;transform:translateX(-50%);display:flex;gap:4px;z-index:20;font:12px sans-serif;user-select:none';
    document.body.appendChild(bar);
    let invJson = '';
    let inv = [];
    const send = (o) => client.send(JSON.stringify(o));
    const toggle = (i) => {
      const t = inv[i];
      if (!t) return;
      send({ t: t.equipped ? 'unequip' : 'equip', id: t.id });
    };
    const drawBar = () => {
      bar.innerHTML = '';
      inv.slice(0, 9).forEach((t, i) => {
        const d = document.createElement('div');
        d.style.cssText = 'width:64px;height:48px;box-sizing:border-box;border:2px solid ' + (t.equipped ? '#4fc3f7' : '#555') + ';background:' + (t.equipped ? 'rgba(40,90,120,.85)' : 'rgba(20,20,20,.7)') + ';color:#fff;display:flex;flex-direction:column;justify-content:space-between;padding:2px 3px;cursor:pointer;overflow:hidden;border-radius:3px';
            const n = document.createElement('span');
            n.textContent = String(i + 1);
            n.style.cssText = 'opacity:.7';
            const l = document.createElement('span');
            l.textContent = t.name;
            l.style.cssText = 'text-align:center;white-space:nowrap;overflow:hidden;text-overflow:ellipsis';
            d.append(n, l);
        d.dataset.slot = String(i);
        d.addEventListener('mousedown', (e) => {
          e.stopPropagation();
          toggle(i);
        });
        bar.appendChild(d);
      });
    };
    setInterval(() => {
      if (!client.ready()) return;
      const j = client.inventory();
      if (j !== invJson) {
        invJson = j;
        inv = JSON.parse(j);
        drawBar();
      }
    }, 150);
    window.addEventListener('keydown', (e) => {
      const tg = e.target;
      if (tg && (tg.tagName === 'INPUT' || tg.tagName === 'TEXTAREA' || tg.isContentEditable)) return;
      if (e.key >= '1' && e.key <= '9' && !e.ctrlKey && !e.altKey && !e.metaKey) toggle(+e.key - 1);
      if (e.key === 'Backspace' || e.key === 'Delete') {
        const t = inv.find((x) => x.equipped);
        if (t) send({ t: 'drop', id: t.id });
      }
    });
    const heldTool = () => inv.find((x) => x.equipped);
    let acting = null;
    const cv = opts.canvas || byId('swirl-canvas');
    cv.addEventListener('mousedown', (e) => {
      if (e.button !== 0) return;
      const t = heldTool();
      if (t) {
        acting = t.id;
        send({ t: 'act', id: t.id, down: true });
      }
    });
    window.addEventListener('mouseup', (e) => {
      if (e.button !== 0 || acting === null) return;
      send({ t: 'act', id: acting, down: false });
      acting = null;
    });
    const chat = document.createElement('div');
    chat.id = 'swirl-chat';
    chat.style.cssText = 'position:fixed;left:8px;top:40px;width:340px;max-width:60vw;z-index:20;font:13px sans-serif;color:#fff';
    const log = document.createElement('div');
    log.style.cssText = 'max-height:180px;overflow:hidden;display:flex;flex-direction:column;justify-content:flex-end;background:rgba(0,0,0,.35);padding:4px 6px;text-shadow:1px 1px 1px #000;border-radius:3px';
    const box = document.createElement('input');
    box.type = 'text';
    box.maxLength = 200;
    box.placeholder = 'Press / to chat';
    box.style.cssText = 'width:100%;box-sizing:border-box;margin-top:3px;padding:3px 6px;font:16px sans-serif;background:rgba(0,0,0,.5);color:#fff;border:1px solid #555;border-radius:3px';
    chat.append(log, box);
    document.body.appendChild(chat);
    const line = (who, text, color) => {
      const d = document.createElement('div');
      d.style.cssText = 'word-break:break-word;color:' + (color || '#fff');
      if (who) {
        const b = document.createElement('b');
        b.textContent = '[' + who + ']: ';
        d.appendChild(b);
      }
      d.appendChild(document.createTextNode(text));
      log.appendChild(d);
      while (log.childNodes.length > 40) log.removeChild(log.firstChild);
    };
    box.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Enter') {
        const t = box.value.trim();
        if (t) send({ t: 'chat', m: t });
        box.value = '';
        box.blur();
      } else if (e.key === 'Escape') {
        box.blur();
      }
    });
    box.addEventListener('keyup', (e) => e.stopPropagation());
    window.addEventListener('keydown', (e) => {
      const tg = e.target;
      if (e.key === '/' && !(tg && (tg.tagName === 'INPUT' || tg.tagName === 'TEXTAREA'))) {
        e.preventDefault();
        box.focus();
      }
    });
    const pump = (m) => {
      if (m.t === 'chat') line(m.n, m.m);
      else if (m.t === 'sys') line('', m.m, '#bbb');
    };
    window.solarRemote = {
      send: (id, args) => send({ t: 're', id, a: args }),
      take: () => remoteQueue.splice(0),
    };
    const remoteQueue = [];
    setInterval(() => {
      for (const raw of JSON.parse(client.take_remote())) {
        const m = JSON.parse(raw);
        if (m.t === 'chat' || m.t === 'sys') pump(m);
        else remoteQueue.push(m);
      }
    }, 100);
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

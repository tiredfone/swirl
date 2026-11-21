const WS = (await import('/var/www/swirl/vendor/chocolate/node_modules/ws/wrapper.mjs')).default;
const port = +process.argv[2];
const mk = (name) => new Promise((resolve) => {
  const ws = new WS('ws://127.0.0.1:' + port + '/ws?ticket=dev');
  const st = { name, ws, own: null, ch: [], models: [] };
  ws.on('message', (d) => {
    const m = JSON.parse(d.toString());
    if (m.t === 'init') { st.init = m; resolve(st); }
    if (m.t === 'rep') {
      for (const a of m.after || []) if (a.t === 'own') st.own = a;
      for (const r of m.ch || []) st.ch.push(r);
      for (const op of m.o || []) if (op[0] === 'n') for (const rec of op[1]) if (rec[1] === 'Model') st.models.push(rec[0]);
    }
  });
});
const a = await mk('A');
await new Promise((r) => setTimeout(r, 300));
const b = await mk('B');
await new Promise((r) => setTimeout(r, 600));
a.ws.send(JSON.stringify({ t: 'char', s: 1, c: [12, 6, 3, 1, 0, 0, 0, 1, 0, 0, 0, 1], v: [16, 0, 0], st: 1 }));
await new Promise((r) => setTimeout(r, 600));
console.log(JSON.stringify({ aOwn: a.own && a.own.id, bOwn: b.own && b.own.id, bSawModels: b.models, bCh: b.ch.slice(-1) }));
process.exit(0);

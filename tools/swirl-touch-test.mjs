const WS = (await import('/var/www/html/chocolate/node_modules/ws/wrapper.mjs')).default;
const port = +process.argv[2];
const ws = new WS('ws://127.0.0.1:' + port + '/ws?ticket=dev');
const st = { owns: [], hp: [], models: [] };
ws.on('message', (d) => {
  const m = JSON.parse(d.toString());
  if (m.t !== 'rep') return;
  for (const a of m.after || []) if (a.t === 'own') st.owns.push(a.id);
  for (const op of m.o || []) {
    if (op[0] === 'p' && op[2] === 'Health') st.hp.push(op[3]);
    if (op[0] === 'n') for (const r of op[1]) if (r[1] === 'Model') st.models.push(r[0]);
    if (op[0] === 'd') st.models.push('d' + op[1]);
  }
});
await new Promise((r) => setTimeout(r, 800));
ws.send(JSON.stringify({ t: 'char', s: 1, c: [20, 4, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1], v: [0, 0, 0], st: 7 }));
await new Promise((r) => setTimeout(r, 6500));
console.log(JSON.stringify(st));
ws.close();
process.exit(0);

const WS = (await import('/var/www/html/chocolate/node_modules/ws/wrapper.mjs')).default;
const port = +process.argv[2];
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
function client() {
  const ws = new WS('ws://127.0.0.1:' + port + '/ws?ticket=dev');
  const c = { ws, ids: {}, hp: [], re: [], rfr: [] };
  ws.on('message', (d) => {
    const m = JSON.parse(d.toString());
    if (m.t === 'init') {
      c.you = m.you;
      for (const r of m.snap) {
        const i = r[3].indexOf('Name');
        const n = i >= 0 ? r[3][i + 1] : r[1];
        if (r[1] === 'RemoteEvent' || r[1] === 'RemoteFunction') c.ids[n] = r[0];
      }
    }
    if (m.t === 're') c.re.push(m);
    if (m.t === 'rfr') c.rfr.push(m);
    if (m.t === 'rep') for (const op of m.o || []) if (op[0] === 'p' && op[2] === 'Health') c.hp.push([op[1], op[3]]);
  });
  return c;
}
const a = client();
const b = client();
await wait(1200);
console.log('remote ids', JSON.stringify(a.ids));
a.ws.send(JSON.stringify({ t: 're', id: a.ids.Hit, a: ['s', 'Player2', 'n', 30] }));
a.ws.send(JSON.stringify({ t: 'rf', id: a.ids.Ask, k: 7, a: ['n', 40, 'n', 2] }));
await wait(800);
console.log('A re', JSON.stringify(a.re));
console.log('B re', JSON.stringify(b.re));
console.log('A rfr', JSON.stringify(a.rfr));
console.log('hp', JSON.stringify(b.hp));
a.ws.close();
b.ws.close();
process.exit(0);

const WS = (await import('/var/www/swirl/vendor/chocolate/node_modules/ws/wrapper.mjs')).default;
const port = +process.argv[2];
const toolName = process.argv[3] || 'ClassicSword';
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
function client(label) {
  const ws = new WS('ws://127.0.0.1:' + port + '/ws?ticket=dev');
  const c = { label, ws, you: null, tools: [], ops: [], hp: [], own: null, names: new Map(), classes: new Map() };
  ws.on('message', (d) => {
    const m = JSON.parse(d.toString());
    if (m.t === 'init') {
      c.you = m.you;
      for (const r of m.snap) if (r[1] === 'Backpack') c.bp = r[0];
      for (const r of m.snap) {
        c.names.set(r[0], r[1]);
        const i = r[3].indexOf('Name');
        if (r[1] === 'Tool') c.tools.push({ id: r[0], name: i >= 0 ? r[3][i + 1] : '', parent: r[2] });
      }
    }
    if (m.t === 'rep') {
      for (const a of m.after || []) if (a.t === 'own') c.own = a;
      for (const op of m.o || []) {
        c.ops.push(op);
        if (op[0] === 'p' && op[2] === 'Health') c.hp.push([op[1], op[3]]);
        if (op[0] === 'n') for (const r of op[1]) if (r[1] === 'Tool') {
          const i = r[3].indexOf('Name');
          c.tools.push({ id: r[0], name: i >= 0 ? r[3][i + 1] : '', parent: r[2] });
        }
      }
    }
  });
  return c;
}
const a = client('A');
const b = client('B');
await wait(1200);
const bpA = a.bp;
const sw = a.tools.find((t) => t.name === toolName && t.parent === a.bp);
console.log('cands', JSON.stringify(a.tools.filter((t) => t.name === toolName)), 'bp', a.bp);
console.log('A tools', a.tools.length);
console.log('A found', JSON.stringify(sw));
await wait(300);
a.ws.send(JSON.stringify({ t: 'equip', id: sw.id }));
const mark = a.ops.length;
await wait(600);
console.log('after equip A', JSON.stringify(a.ops.slice(mark)).slice(0, 700));
console.log('A ops', JSON.stringify(a.ops.map((o) => o[0] + ':' + (o[0] === 'n' ? o[1][0][1] : o[1]))).slice(0, 400));
console.log('B ops', JSON.stringify(b.ops.map((o) => o[0] + ':' + (o[0] === 'n' ? o[1][0][1] : o[1]))).slice(0, 400));
const welds = (x) => x.ops.filter((o) => o[0] === 'n' && o[1].some((r) => r[1] === 'Weld')).length;
console.log('weld ops A', welds(a), 'B', welds(b));
a.ws.send(JSON.stringify({ t: 'act', id: sw.id, down: true }));
await wait(200);
a.ws.send(JSON.stringify({ t: 'act', id: sw.id, down: false }));
await wait(1500);
console.log('A hp ops', JSON.stringify(a.hp), 'B hp ops', JSON.stringify(b.hp));
let cycles = 0;
for (; cycles < 30 && !b.hp.some((h) => h[1] <= 0); cycles++) {
  a.ws.send(JSON.stringify({ t: 'char', s: 1, c: [40, 3, 40, 1, 0, 0, 0, 1, 0, 0, 0, 1], v: [0, 0, 0], st: 7 }));
  await wait(250);
  a.ws.send(JSON.stringify({ t: 'char', s: 1, c: [0, 3, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1], v: [0, 0, 0], st: 7 }));
  await wait(250);
}
console.log('cycles', cycles, 'B died', b.hp.some((h) => h[1] <= 0), 'B hp last', JSON.stringify(b.hp.slice(-3)));
await wait(5600);
console.log('B own respawn', JSON.stringify(b.own && b.own.id));
a.ws.send(JSON.stringify({ t: 'unequip', id: sw.id }));
await wait(600);
console.log('unequip ops B', JSON.stringify(b.ops.slice(-3)).slice(0, 300));
a.ws.close();
b.ws.close();
process.exit(0);

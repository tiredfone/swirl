const WS = (await import('/var/www/html/chocolate/node_modules/ws/wrapper.mjs')).default;
const port = +process.argv[2];
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const ws = new WS('ws://127.0.0.1:' + port + '/ws?ticket=dev');
const nodes = new Map();
let you = null;
let own = null;
const log = [];
const add = (r) => nodes.set(r[0], { cls: r[1], parent: r[2] });
const kids = (id) => [...nodes].filter(([, n]) => n.parent === id).map(([i]) => i);
const under = (id, root) => {
  for (let n = nodes.get(id); n; n = nodes.get(n.parent)) if (n.parent === root) return true;
  return false;
};
ws.on('message', (d) => {
  const m = JSON.parse(d.toString());
  if (m.t === 'init') {
    you = m.you;
    for (const r of m.snap) add(r);
  }
  if (m.t === 'rep') {
    for (const a of m.after || []) if (a.t === 'own') own = a.id;
    for (const op of m.o || []) {
      if (op[0] === 'n') for (const r of op[1]) add(r);
      if (op[0] === 'm' && nodes.has(op[1])) nodes.get(op[1]).parent = op[2];
      if (op[0] === 'd') {
        log.push(['d', nodes.get(op[1]) && nodes.get(op[1]).cls, op[1]]);
        nodes.delete(op[1]);
      }
    }
  }
});
await wait(1500);
const bp = [...nodes].find(([, n]) => n.cls === 'Backpack');
const tools = () => (bp && nodes.has(bp[0]) ? kids(bp[0]).filter((i) => nodes.get(i).cls === 'Tool' || nodes.get(i).cls === 'HopperBin').length : -1);
const before = tools();
const model = own;
const joints = () => [...nodes].filter(([i, n]) => /^(Motor6D|Weld|Snap|Motor|ManualWeld|Glue)$/.test(n.cls) && model && under(i, model)).length;
const j0 = joints();
const tool = bp && kids(bp[0]).find((i) => nodes.get(i).cls === 'Tool');
if (tool) ws.send(JSON.stringify({ t: 'equip', id: tool }));
await wait(600);
ws.send(JSON.stringify({ t: 'char', s: 1, c: [0, -1000, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1], v: [0, 0, 0], st: 7 }));
await wait(1500);
const j1 = joints();
const destroyedJoints = log.filter((l) => /^(Motor6D|Weld|Snap|Motor|ManualWeld|Glue)$/.test(l[1])).length;
await wait(8000);
const nbp = [...nodes].filter(([, n]) => n.cls === 'Backpack').map(([i]) => i);
const after = nbp.length ? kids(nbp[nbp.length - 1]).filter((i) => nodes.get(i).cls === 'Tool' || nodes.get(i).cls === 'HopperBin').length : -1;
const heldNow = model && own && own !== model ? kids(own).filter((i) => nodes.get(i).cls === 'Tool').length : 0;
const fails = [];
if (before < 1) fails.push('no tools to test');
if (j0 < 1) fails.push('no joints seen on the character');
if (j1 !== 0 && destroyedJoints < j0) fails.push('joints not broken on death: before ' + j0 + ' after ' + j1 + ' destroyed ' + destroyedJoints);
if (own === model) fails.push('character did not respawn');
if (after !== before) fails.push('backpack tools after respawn ' + after + ', expected ' + before);
if (heldNow !== 0) fails.push('tool still equipped on new character');
console.log(JSON.stringify({ before, after, j0, j1, destroyedJoints, heldNow, fails }));
ws.close();
process.exit(fails.length ? 1 : 0);

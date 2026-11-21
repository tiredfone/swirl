const WS = (await import('/var/www/swirl/vendor/chocolate/node_modules/ws/wrapper.mjs')).default;
const { DataModel } = await import('/var/www/swirl/vendor/chocolate/shared/instance.js');
const { applyRecords } = await import('/var/www/swirl/vendor/chocolate/shared/codec.js');
const port = +process.argv[2];
const ws = new WS('ws://127.0.0.1:' + port + '/ws?ticket=dev');
const dm = new DataModel('client');
const seen = [];
ws.on('message', (d) => {
  const m = JSON.parse(d.toString());
  if (m.t === 'init') {
    dm.era = m.era;
    applyRecords(dm, m.snap);
    seen.push('init:' + m.snap.length);
  } else if (m.t === 'rep') {
    for (const op of m.o) {
      if (op[0] === 'n') applyRecords(dm, op[1]);
      else if (op[0] === 'p') { const i = dm.byId.get(op[1]); if (i) dm.setProp(i, op[2], op[3], { remote: true }); }
      else if (op[0] === 'd') { const i = dm.byId.get(op[1]); if (i) dm.destroy(i); }
      seen.push(op[0]);
    }
  }
});
await new Promise((r) => setTimeout(r, 4500));
const w = dm.services.get('Workspace');
const names = w.children.map((c) => c.Name).sort();
console.log(JSON.stringify({ names, ops: seen.join(',') }));
ws.close();
process.exit(0);

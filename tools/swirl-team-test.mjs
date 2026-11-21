const WS = (await import('/var/www/swirl/vendor/chocolate/node_modules/ws/wrapper.mjs')).default;
const port = +process.argv[2];
const ws = new WS('ws://127.0.0.1:' + port + '/ws?ticket=dev');
let init = null;
const changes = [];
ws.on('message', (d) => {
  const m = JSON.parse(d.toString());
  if (m.t === 'init') init = m;
  for (const op of m.o || []) if (op[0] === 'p' && op[2] === 'TeamColor') changes.push(op[3]);
});
await new Promise((r) => setTimeout(r, 800));
const rec = (id) => init.snap.find((r) => r[0] === id);
const props = (r) => Object.fromEntries(r[3].reduce((a, v, i, arr) => (i % 2 ? a : [...a, [v, arr[i + 1]]]), []));
const me = props(rec(init.you));
const spawns = init.snap.filter((r) => r[1] === 'SpawnLocation').map((r) => ({ id: r[0], ...props(r) }));
const mine = spawns.find((s) => s.TeamColor === me.TeamColor && !s.Neutral);
const other = spawns.find((s) => s.TeamColor !== me.TeamColor);
const model = init.snap.find((r) => r[1] === 'Model');
console.log(JSON.stringify({ team: me.TeamColor, neutral: me.Neutral, spawns: spawns.map((s) => [s.TeamColor, s.CFrame && s.CFrame.slice(0, 3)]), hasModelInInit: !!model }));
if (other) {
  const cf = other.CFrame;
  ws.send(JSON.stringify({ t: 'char', s: 1, c: [cf[0], cf[1] + 3.6, cf[2], 1, 0, 0, 0, 1, 0, 0, 0, 1], v: [0, 0, 0], st: 7 }));
}
await new Promise((r) => setTimeout(r, 1200));
console.log(JSON.stringify({ changes }));
ws.close();
process.exit(0);

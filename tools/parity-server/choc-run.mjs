import fs0 from 'fs';
const CH = process.env.CHOCOLATE || '/var/www/html/chocolate';
const placeId = +process.argv[2];
const era = +(process.argv[3] || 2012);
const ms = +(process.argv[4] || 8000);
if (!placeId) {
  console.log('usage: CHOCOLATE_DATA_DIR=/tmp/claude-1000/chocdata node choc-run.mjs <place> [era] [ms]');
  process.exit(1);
}
process.env.CHOCOLATE_DATA_DIR = process.env.CHOCOLATE_DATA_DIR || '/tmp/claude-1000/chocdata';
process.env.CHOCOLATE_KEY = 'parity-dummy';
const { Game } = await import(CH + '/server/game.js');
const sent = [];
const lines = [];
const game = new Game(placeId, {
  jobId: 'parity', kind: 'public', privateId: 0, allowGear: true, era,
  send: (cid, data) => sent.push(data),
  kick: (cid, reason) => lines.push(`[kick] ${reason}`),
  log: (line) => lines.push(line.replace(/^\[place [^\]]+\] /, '')),
});
await game.start();
await game.join(1, { id: +(process.env.UID || 132), name: 'TestingAccount', membership: 0, age: 100, dev: true, perks: null, avatarType: process.env.AVATAR || 'R6', scale: process.env.SCALE ? JSON.parse(process.env.SCALE) : null, animations: [] });
if (process.env.RUNFILE) { await new Promise((r) => setTimeout(r, 2000)); game.runCode(fs0.readFileSync(process.env.RUNFILE, 'utf8'), 'parity'); }
await new Promise((r) => setTimeout(r, ms));
if (process.env.FINDCOIN) { const w = game.dm.root; const cs = w.descendants().filter((d) => d.props.get('Name') === 'Coin'); console.log('coins', cs.length, cs.slice(0,3).map((c) => { const p = []; for (let x = c; x; x = x.parent) p.push(x.props.get('Name')); return c.ClassName + ' ' + p.join('<'); }).join(' ; ')); }
game.stop();
const out = process.argv[5] || `/tmp/swirl/par/${placeId}.choc.log`;
const fs = await import('fs');
fs.writeFileSync(out, lines.join('\n') + '\n');
const kinds = {};
const repKeys = {};
for (const s of sent) {
  const m = JSON.parse(s);
  kinds[m.t] = (kinds[m.t] || 0) + 1;
  if (m.t === 'rep') for (const k of Object.keys(m)) if (k !== 't') repKeys[k] = (repKeys[k] || 0) + 1;
}
if (process.env.REPKEYS) console.log('rep keys', JSON.stringify(repKeys));
if (process.env.REPOPS) { const sig = {}; for (const s of sent) { const m = JSON.parse(s); if (m.t === 'rep' && m.o) for (const o of m.o) { const k = o[0] + ':' + (o[0] === 'p' ? o[2] : ''); sig[k] = (sig[k] || 0) + 1; } } console.log('ops', JSON.stringify(sig)); const ns = []; for (const s of sent) { const m = JSON.parse(s); if (m.t === 'rep' && m.o) for (const o of m.o) if (o[0] === 'n') ns.push(o[1][0].slice(0, 4).join('|')); } console.log('n ops', JSON.stringify(ns.slice(0, 12))); const ids = {}; for (const s of sent) { const m = JSON.parse(s); if (m.t === 'rep' && m.o) for (const o of m.o) if (o[0] === 'p' && o[2] === 'CFrame') ids[o[1]] = (ids[o[1]] || 0) + 1; } const names = Object.entries(ids).map(([id, n]) => [game.dm.byId.get(+id) ? game.dm.byId.get(+id).getFullName ? game.dm.byId.get(+id).getFullName() : game.dm.byId.get(+id).props.get('Name') : id, n]); console.log('cframe ops', JSON.stringify(names).slice(0, 500)); }
console.log(JSON.stringify(kinds));
process.exit(0);

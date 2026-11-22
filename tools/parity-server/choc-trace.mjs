import fs from 'fs';
const CH = process.env.CHOCOLATE || '/var/www/html/chocolate';
const [, , placeId, era, ticks, out, model] = process.argv;
process.env.CHOCOLATE_DATA_DIR = process.env.CHOCOLATE_DATA_DIR || '/tmp/claude-1000/chocdata';
process.env.CHOCOLATE_KEY = 'parity-dummy';
const { Game } = await import(CH + '/server/game.js');
const lines = [];
let simNow = 0;
Date.now = () => simNow;
const origTick = Game.prototype.tick;
Game.prototype.tick = function () {};
const game = new Game(+placeId, { jobId: 'trace', kind: 'public', privateId: 0, allowGear: true, era: +era, send: () => {}, kick: () => {}, log: (l) => lines.push(l.replace(/^\[place [^\]]+\] /, '')) });
await game.start();
const trace = [];
const ws = game.dm.services.get('Workspace');
for (let i = 0; i < +ticks; i++) {
  simNow = (i + 1) * (1000 / 30);
  origTick.call(game);
  const row = [];
  const m = ws.children.find((c) => c.props.get('Name') === (model || 'Dummy'));
  if (m) for (const p of m.children) if (p.isA('BasePart')) row.push([p.props.get('Name'), p.props.get('CFrame'), p.props.get('Velocity')]);
  for (const p of ws.children) if (p.isA('BasePart') && !p.props.get('Anchored')) row.push([p.props.get('Name'), p.props.get('CFrame'), p.props.get('Velocity')]);
  trace.push(row);
}
if (process.env.LEDGE) {
  const rec = [...game.chars.values()][0];
  const c = rec.ctrl;
  const dir = [1, 0, 0];
  const ds = [];
  const foot = c.pos[1] - 2.5;
  for (let k = 0; k <= 22; k++) {
    const y = foot + k * 0.25;
    const r = game.physics.raycast([c.pos[0], y, c.pos[2]], [dir[0] * 1.7, 0, dir[2] * 1.7], [rec.model], (q) => q.ClassName !== 'Terrain' && q.props.get('CanCollide'));
    ds.push(r.part ? [r.part.props.get('Name'), +Math.hypot(r.point[0] - c.pos[0], r.point[2] - c.pos[2]).toFixed(2), +r.normal[0].toFixed(2), +r.normal[1].toFixed(2)] : null);
  }
  console.log('pos', c.pos, 'state', c.state, 'ledgeTop', c.ledgeTop, JSON.stringify(ds));
}
fs.writeFileSync(out, JSON.stringify(trace));
fs.writeFileSync(out + '.log', lines.filter((l) => /\[(print|error|warn)\]/.test(l)).join('\n') + '\n');
process.exit(0);

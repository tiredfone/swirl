import fs from 'fs';
import RAPIER from '/var/www/html/chocolate/node_modules/@dimforge/rapier3d-compat/dist/rapier.mjs';
const CH = process.env.CHOCOLATE || '/var/www/html/chocolate';
const [, , file, ticks, out] = process.argv;
const { DataModel } = await import(CH + '/shared/instance.js');
const { parsePlace } = await import(CH + '/shared/rbxl.js');
const { Physics } = await import(CH + '/shared/physics.js');
await RAPIER.init();
const dm = new DataModel('server');
const nodes = parsePlace(fs.readFileSync(file));
dm.service('Workspace');
dm.load(nodes, dm.root);
const ws = dm.service('Workspace');
const phys = new Physics(dm, RAPIER, { simulate: true });
phys.attachWorkspace(ws);
const trace = [];
for (let i = 0; i < +ticks; i++) {
  phys.step(1 / 60);
  const row = [];
  for (const p of ws.descendants()) {
    if (!p.isA('BasePart') || p.props.get('Anchored')) continue;
    row.push([p.props.get('Name'), p.props.get('CFrame'), p.props.get('Velocity')]);
  }
  trace.push(row);
}
fs.writeFileSync(out, JSON.stringify(trace));
process.exit(0);

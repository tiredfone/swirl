import path from 'path';
import fs from 'fs';
import { scenario, sequenceXml } from './animation-scenario.mjs';
const root = process.argv[2];
const dir = process.argv[3];
const load = (p) => import(path.join(root, p));
const { DataModel } = await load('shared/instance.js');
const { Animations } = await load('shared/animator.js');
const { parsePlace } = await load('shared/rbxl.js');
fs.mkdirSync(dir, { recursive: true });
for (const [id, seq] of Object.entries(scenario.seqs)) fs.writeFileSync(path.join(dir, 'seq-' + id + '.xml'), sequenceXml(seq));
fs.writeFileSync(path.join(dir, 'scenario.json'), JSON.stringify(scenario));
const dm = new DataModel('client');
const ws = dm.service('Workspace');
const model = dm.create('Model', null);
dm.setParent(model, ws);
const hum = dm.create('Humanoid', null);
dm.setParent(hum, model);
const parts = {};
for (const n of scenario.parts) {
  const p = dm.create('Part', null);
  p.props.set('Name', n);
  dm.setParent(p, model);
  parts[n] = p;
}
const motors = [];
for (const [n, a, b] of scenario.motors) {
  const j = dm.create('Motor6D', null);
  j.props.set('Name', n);
  j.props.set('Part0', parts[a]);
  j.props.set('Part1', parts[b]);
  dm.setParent(j, model);
  motors.push([b, j]);
}
const anims = new Animations(dm, async (id) => {
  const m = dm.create('Model', null);
  dm.load(parsePlace(new TextEncoder().encode(fs.readFileSync(path.join(dir, 'seq-' + id + '.xml'), 'utf8'))), m);
  return m;
});
const out = [];
const dt = 1 / 60;
let oi = 0;
for (let f = 0; f < scenario.frames; f++) {
  while (oi < scenario.ops.length && scenario.ops[oi].f <= f) {
    const o = scenario.ops[oi++];
    anims.op(hum, o.key, o.op, o.id, o.a, o.b, o.c);
  }
  await new Promise((r) => setImmediate(r));
  await Promise.resolve();
  anims.step(dt, () => {});
  out.push(motors.map(([n, j]) => [n, j.anim ? j.anim.map((v) => Math.round(v * 1e5) / 1e5) : null]));
}
fs.writeFileSync(path.join(dir, 'js.json'), JSON.stringify(out));
process.exit(0);

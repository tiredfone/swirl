import fs from 'fs';
const rotx = (a) => { const c = Math.cos(a), s = Math.sin(a); return [1, 0, 0, 0, c, -s, 0, s, c]; };
const cfp = (x, y, z, r) => [x, y, z, ...(r || [1, 0, 0, 0, 1, 0, 0, 0, 1])];
const P = (name, pos, size, r) => [name, cfp(pos[0], pos[1], pos[2], r), size];
const floor = P('Floor', [0, -0.5, 0], [400, 1, 400]);
const fwd = [0, 0, -1];
const dg = [0.70710678, 0, -0.70710678];
const idle = [0, 0, 0];
const S = (s, mv, jump) => (jump ? { s, mv, jump } : { s, mv });
const out = {};
out.walk = { parts: [floor], steps: [S(1, idle), S(3, fwd)] };
out.diag = { parts: [floor], steps: [S(1, idle), S(3, dg)] };
out.turn = { parts: [floor], steps: [S(1, idle), S(1, fwd), S(1, dg), S(1, [-1, 0, 0]), S(1, [0, 0, 1])] };
out.stop = { parts: [floor], steps: [S(1, idle), S(1, fwd), S(1, idle)] };
out.jumpstand = { parts: [floor], steps: [S(1, idle), S(0.1, idle, true), S(2, idle)] };
out.jumpwalk = { parts: [floor], steps: [S(1, idle), S(1, fwd), S(0.1, fwd, true), S(2, fwd)] };
out.jumpdiag = { parts: [floor], steps: [S(1, idle), S(1, dg), S(0.1, dg, true), S(2, dg)] };
out.jumphold = { parts: [floor], steps: [S(1, idle), S(3, fwd, true)] };
out.ledgeoff = { parts: [P('Top', [0, -0.5, -4], [20, 1, 20])], steps: [S(1, idle), S(4, fwd)] };
out.ledgediag = { parts: [P('Top', [0, -0.5, -4], [20, 1, 20])], steps: [S(1, idle), S(4, dg)] };
out.drop = { parts: [floor, P('Tower', [0, 5, -6], [20, 10, 8])], steps: [S(1, idle), S(1, [0, 0, 1]), S(1, fwd)] };
for (const deg of [15, 30, 45, 60]) {
  const a = (deg * Math.PI) / 180;
  const len = 40;
  const cy = (len / 2) * Math.sin(a) - 0.5 * Math.cos(a);
  const cz = -6 - (len / 2) * Math.cos(a) - 0.5 * Math.sin(a);
  out['slope' + deg] = { parts: [floor, P('Ramp', [0, cy, cz], [20, 1, len], rotx(a))], steps: [S(1, idle), S(4, fwd)] };
  out['slopediag' + deg] = { parts: [floor, P('Ramp', [0, cy, cz], [20, 1, len], rotx(a))], steps: [S(1, idle), S(4, dg)] };
}
out.wall = { parts: [floor, P('Wall', [0, 5, -8], [20, 10, 2])], steps: [S(1, idle), S(3, fwd)] };
out.walldiag = { parts: [floor, P('Wall', [0, 5, -8], [30, 10, 2])], steps: [S(1, idle), S(3, dg)] };
out.corner = { parts: [floor, P('W1', [0, 5, -8], [20, 10, 2]), P('W2', [-6, 5, -2], [2, 10, 20])], steps: [S(1, idle), S(3, dg), S(2, [-1, 0, 0])] };
const stairs = (h, n, d) => [floor, ...Array.from({ length: n }, (_, k) => P('St' + k, [0, (h * (k + 1)) / 2 - 0.0, -6 - k * d - d / 2], [20, h * (k + 1), d]))];
out.stairs05 = { parts: stairs(0.5, 8, 2), steps: [S(1, idle), S(5, fwd)] };
out.stairs1 = { parts: stairs(1, 6, 2), steps: [S(1, idle), S(5, fwd)] };
out.stairs05diag = { parts: stairs(0.5, 8, 2), steps: [S(1, idle), S(5, dg)] };
out.stairs1diag = { parts: stairs(1, 6, 2), steps: [S(1, idle), S(5, dg)] };
out.step15 = { parts: [floor, P('B', [0, 0.75, -8], [20, 1.5, 4])], steps: [S(1, idle), S(2, fwd)] };
out.step4 = { parts: [floor, P('B', [0, 2, -8], [20, 4, 4])], steps: [S(1, idle), S(2, fwd), S(0.1, fwd, true), S(2, fwd)] };
const dtv = (name, dts, extra) => { const b = JSON.parse(JSON.stringify(out[name])); b.dts = dts; out[name + '_' + extra] = b; };
const j30 = [1 / 30];
const jit = [1 / 144, 1 / 60, 1 / 30, 1 / 75, 1 / 20, 1 / 240];
for (const n of ['walk', 'diag', 'turn', 'jumpwalk', 'jumpdiag', 'ledgeoff', 'slope30', 'stairs05', 'stairs1diag', 'step4', 'corner', 'wall']) {
  dtv(n, j30, 'dt30');
  dtv(n, jit, 'jit');
}
out.facewalk = { parts: [floor], steps: [S(1, idle), { s: 1, mv: fwd, face: [1, 0, 0] }, { s: 1, mv: dg, face: [0, 0, -1] }, { s: 1, mv: [0, 0, 1], face: [-1, 0, -1] }], dts: [1 / 60] };
fs.writeFileSync(new URL('./movement-spec.json', import.meta.url), JSON.stringify(out));
console.log(Object.keys(out).length);

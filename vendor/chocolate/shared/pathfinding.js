const STEP = 4;
const MAX_NODES = 2500;
const DIRS = [[1, 0], [-1, 0], [0, 1], [0, -1], [1, 1], [1, -1], [-1, 1], [-1, -1]];

export function findPath(cast, start, goal, opts = {}) {
  const radius = opts.AgentRadius || 2;
  const height = opts.AgentHeight || 5;
  const canJump = opts.AgentCanJump !== false;
  const ground = (x, y, z, drop) => {
    const r = cast([x, y, z], [0, -drop, 0]);
    if (!r.hit) return null;
    if (r.normal[1] < 0.55) return null;
    return r.point[1];
  };
  const blocked = (ax, ay, az, bx, by, bz) => {
    const dx = bx - ax, dz = bz - az;
    const len = Math.hypot(dx, dz) || 1;
    const px = -dz / len * radius * 0.6, pz = dx / len * radius * 0.6;
    for (const [h, off] of [[1.2, 0], [height - 1, 0], [2.2, 1], [2.2, -1]]) {
      const oy = ay + h, ty = by + h;
      const o = [ax + px * off, oy, az + pz * off];
      const d = [dx, ty - oy, dz];
      const r = cast(o, d);
      if (r.hit) return true;
    }
    return false;
  };
  const sg = ground(start[0], start[1] + 2, start[2], 12);
  const sy = sg === null ? start[1] - 3 : sg;
  const sx = start[0], sz = start[2];
  const key = (i, j) => i * 100003 + j;
  const nodes = new Map();
  const open = [];
  const startNode = { i: 0, j: 0, x: sx, y: sy, z: sz, g: 0, f: 0, parent: null, jump: false };
  const hdist = (x, z) => Math.hypot(goal[0] - x, goal[2] - z);
  startNode.f = hdist(sx, sz) * 1.4;
  nodes.set(key(0, 0), startNode);
  open.push(startNode);
  let best = startNode;
  let reached = null;
  let count = 0;
  const push = (n) => {
    open.push(n);
    let k = open.length - 1;
    while (k > 0) {
      const p = (k - 1) >> 1;
      if (open[p].f <= open[k].f) break;
      [open[p], open[k]] = [open[k], open[p]];
      k = p;
    }
  };
  const pop = () => {
    const top = open[0];
    const last = open.pop();
    if (open.length) {
      open[0] = last;
      let k = 0;
      for (;;) {
        const l = 2 * k + 1, r = l + 1;
        let m = k;
        if (l < open.length && open[l].f < open[m].f) m = l;
        if (r < open.length && open[r].f < open[m].f) m = r;
        if (m === k) break;
        [open[m], open[k]] = [open[k], open[m]];
        k = m;
      }
    }
    return top;
  };
  open.length = 0;
  push(startNode);
  while (open.length && count < MAX_NODES) {
    const cur = pop();
    if (cur.closed) continue;
    cur.closed = true;
    count++;
    if (hdist(cur.x, cur.z) < STEP * 0.75 && Math.abs(cur.y - goal[1]) < 12 || hdist(cur.x, cur.z) < 2) {
      reached = cur;
      break;
    }
    if (hdist(cur.x, cur.z) < hdist(best.x, best.z)) best = cur;
    for (const [di, dj] of DIRS) {
      const ni = cur.i + di, nj = cur.j + dj;
      const k = key(ni, nj);
      const old = nodes.get(k);
      if (old && old.closed) continue;
      const nx = sx + ni * STEP, nz = sz + nj * STEP;
      const gy = ground(nx, cur.y + 4, nz, 4 + 8);
      if (gy === null) continue;
      const rise = gy - cur.y;
      if (rise > (canJump ? 4.5 : 1.8) || rise < -12) continue;
      if (blocked(cur.x, cur.y, cur.z, nx, gy, nz)) continue;
      const g = cur.g + Math.hypot(di, dj) * STEP + (rise > 1.8 ? 4 : 0) + Math.abs(rise) * 0.3;
      if (old && old.g <= g) continue;
      const n = { i: ni, j: nj, x: nx, y: gy, z: nz, g, f: g + hdist(nx, nz) * 1.4, parent: cur, jump: rise > 1.8, closed: false };
      nodes.set(k, n);
      push(n);
    }
  }
  const end = reached || best;
  const chain = [];
  for (let n = end; n; n = n.parent) chain.push(n);
  chain.reverse();
  const pts = chain.map((n) => ({ pos: [n.x, n.y, n.z], jump: n.jump }));
  if (reached) {
    const gg = ground(goal[0], goal[1] + 2, goal[2], 14);
    pts.push({ pos: [goal[0], gg === null ? goal[1] : gg, goal[2]], jump: false });
  }
  pts[0].pos = [start[0], start[1] - (sg === null ? 0 : start[1] - sy), start[2]];
  return { status: reached ? 0 : 1, points: pts };
}

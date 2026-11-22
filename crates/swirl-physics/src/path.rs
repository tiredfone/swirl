use std::collections::HashMap;

const STEP: f64 = 4.0;
const MAX_NODES: usize = 2500;
const DIRS: [(i64, i64); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

pub struct CastHit {
    pub hit: bool,
    pub point: [f64; 3],
    pub normal: [f64; 3],
}

pub struct Waypoint {
    pub pos: [f64; 3],
    pub jump: bool,
}

pub struct PathResult {
    pub status: i32,
    pub points: Vec<Waypoint>,
}

struct Node {
    i: i64,
    j: i64,
    x: f64,
    y: f64,
    z: f64,
    g: f64,
    f: f64,
    parent: Option<usize>,
    jump: bool,
    closed: bool,
}

fn hypot(a: f64, b: f64) -> f64 {
    a.hypot(b)
}

pub fn find_path(cast: &dyn Fn([f64; 3], [f64; 3]) -> CastHit, start: [f64; 3], goal: [f64; 3], radius: f64, height: f64, can_jump: bool) -> PathResult {
    let ground = |x: f64, y: f64, z: f64, drop: f64| -> Option<f64> {
        let r = cast([x, y, z], [0.0, -drop, 0.0]);
        if !r.hit {
            return None;
        }
        if r.normal[1] < 0.55 {
            return None;
        }
        Some(r.point[1])
    };
    let blocked = |ax: f64, ay: f64, az: f64, bx: f64, by: f64, bz: f64| -> bool {
        let dx = bx - ax;
        let dz = bz - az;
        let mut len = hypot(dx, dz);
        if len == 0.0 {
            len = 1.0;
        }
        let px = -dz / len * radius * 0.6;
        let pz = dx / len * radius * 0.6;
        for (h, off) in [(1.2, 0.0), (height - 1.0, 0.0), (2.2, 1.0), (2.2, -1.0)] {
            let oy = ay + h;
            let ty = by + h;
            let o = [ax + px * off, oy, az + pz * off];
            let d = [dx, ty - oy, dz];
            if cast(o, d).hit {
                return true;
            }
        }
        false
    };
    let sg = ground(start[0], start[1] + 2.0, start[2], 12.0);
    let sy = sg.unwrap_or(start[1] - 3.0);
    let (sx, sz) = (start[0], start[2]);
    let key = |i: i64, j: i64| i * 100003 + j;
    let mut nodes: Vec<Node> = vec![];
    let mut index: HashMap<i64, usize> = HashMap::new();
    let mut open: Vec<usize> = vec![];
    let hdist = |x: f64, z: f64| hypot(goal[0] - x, goal[2] - z);
    nodes.push(Node { i: 0, j: 0, x: sx, y: sy, z: sz, g: 0.0, f: hdist(sx, sz) * 1.4, parent: None, jump: false, closed: false });
    index.insert(key(0, 0), 0);
    let mut best = 0usize;
    let mut reached: Option<usize> = None;
    let mut count = 0;
    fn push(open: &mut Vec<usize>, nodes: &[Node], n: usize) {
        open.push(n);
        let mut k = open.len() - 1;
        while k > 0 {
            let p = (k - 1) >> 1;
            if nodes[open[p]].f <= nodes[open[k]].f {
                break;
            }
            open.swap(p, k);
            k = p;
        }
    }
    fn pop(open: &mut Vec<usize>, nodes: &[Node]) -> usize {
        let top = open[0];
        let last = open.pop().unwrap();
        if !open.is_empty() {
            open[0] = last;
            let mut k = 0;
            loop {
                let l = 2 * k + 1;
                let r = l + 1;
                let mut m = k;
                if l < open.len() && nodes[open[l]].f < nodes[open[m]].f {
                    m = l;
                }
                if r < open.len() && nodes[open[r]].f < nodes[open[m]].f {
                    m = r;
                }
                if m == k {
                    break;
                }
                open.swap(m, k);
                k = m;
            }
        }
        top
    }
    push(&mut open, &nodes, 0);
    while !open.is_empty() && count < MAX_NODES {
        let cur = pop(&mut open, &nodes);
        if nodes[cur].closed {
            continue;
        }
        nodes[cur].closed = true;
        count += 1;
        let (cx, cy, cz, ci, cj, cg) = (nodes[cur].x, nodes[cur].y, nodes[cur].z, nodes[cur].i, nodes[cur].j, nodes[cur].g);
        if (hdist(cx, cz) < STEP * 0.75 && (cy - goal[1]).abs() < 12.0) || hdist(cx, cz) < 2.0 {
            reached = Some(cur);
            break;
        }
        if hdist(cx, cz) < hdist(nodes[best].x, nodes[best].z) {
            best = cur;
        }
        for (di, dj) in DIRS {
            let ni = ci + di;
            let nj = cj + dj;
            let k = key(ni, nj);
            let old = index.get(&k).copied();
            if let Some(o) = old {
                if nodes[o].closed {
                    continue;
                }
            }
            let nx = sx + ni as f64 * STEP;
            let nz = sz + nj as f64 * STEP;
            let gy = match ground(nx, cy + 4.0, nz, 4.0 + 8.0) {
                Some(g) => g,
                None => continue,
            };
            let rise = gy - cy;
            if rise > (if can_jump { 4.5 } else { 1.8 }) || rise < -12.0 {
                continue;
            }
            if blocked(cx, cy, cz, nx, gy, nz) {
                continue;
            }
            let g = cg + hypot(di as f64, dj as f64) * STEP + (if rise > 1.8 { 4.0 } else { 0.0 }) + rise.abs() * 0.3;
            if let Some(o) = old {
                if nodes[o].g <= g {
                    continue;
                }
            }
            nodes.push(Node { i: ni, j: nj, x: nx, y: gy, z: nz, g, f: g + hdist(nx, nz) * 1.4, parent: Some(cur), jump: rise > 1.8, closed: false });
            let id = nodes.len() - 1;
            index.insert(k, id);
            push(&mut open, &nodes, id);
        }
    }
    let end = reached.unwrap_or(best);
    let mut chain = vec![];
    let mut n = Some(end);
    while let Some(i) = n {
        chain.push(i);
        n = nodes[i].parent;
    }
    chain.reverse();
    let mut pts: Vec<Waypoint> = chain.iter().map(|i| Waypoint { pos: [nodes[*i].x, nodes[*i].y, nodes[*i].z], jump: nodes[*i].jump }).collect();
    if reached.is_some() {
        let gg = ground(goal[0], goal[1] + 2.0, goal[2], 14.0);
        pts.push(Waypoint { pos: [goal[0], gg.unwrap_or(goal[1]), goal[2]], jump: false });
    }
    pts[0].pos = [start[0], start[1] - (if sg.is_none() { 0.0 } else { start[1] - sy }), start[2]];
    PathResult { status: if reached.is_some() { 0 } else { 1 }, points: pts }
}

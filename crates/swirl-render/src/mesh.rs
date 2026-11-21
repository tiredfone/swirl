use swirl_core::math::*;

pub use swirl_core::meshfile::Mesh;

pub const TPL_W: f64 = 585.0;
pub const TPL_H: f64 = 559.0;
pub const BEVEL: f64 = 0.065;
pub use swirl_core::meshfile::{parse_mesh, MAX_MESH_VERTS};

pub type Region = [[f64; 4]; 6];

pub const FACE_ORDER: [char; 6] = ['F', 'B', 'R', 'L', 'U', 'D'];

pub fn torso_region() -> Region {
    [[231.0, 74.0, 128.0, 128.0], [427.0, 74.0, 128.0, 128.0], [165.0, 74.0, 64.0, 128.0], [361.0, 74.0, 64.0, 128.0], [231.0, 8.0, 128.0, 64.0], [231.0, 204.0, 128.0, 64.0]]
}

pub fn right_region() -> Region {
    [[217.0, 355.0, 64.0, 128.0], [85.0, 355.0, 64.0, 128.0], [151.0, 355.0, 64.0, 128.0], [19.0, 355.0, 64.0, 128.0], [217.0, 289.0, 64.0, 64.0], [217.0, 485.0, 64.0, 64.0]]
}

pub fn left_region() -> Region {
    [[308.0, 355.0, 64.0, 128.0], [440.0, 355.0, 64.0, 128.0], [506.0, 355.0, 64.0, 128.0], [374.0, 355.0, 64.0, 128.0], [308.0, 289.0, 64.0, 64.0], [308.0, 485.0, 64.0, 64.0]]
}

pub fn mesh_bounds(m: &Mesh) -> (V3, V3) {
    let mut lo = [1e9; 3];
    let mut hi = [-1e9; 3];
    for p in &m.pos {
        for k in 0..3 {
            if p[k] < lo[k] {
                lo[k] = p[k];
            }
            if p[k] > hi[k] {
                hi[k] = p[k];
            }
        }
    }
    (lo, hi)
}

pub struct FaceDef {
    pub n: V3,
    p00: V3,
    ds: V3,
    dt: V3,
    dd: f64,
    tt: f64,
    r: [f64; 4],
}

impl FaceDef {
    fn uv_of(&self, p: V3) -> [f64; 2] {
        let q = sub(p, self.p00);
        let s = (dot(q, self.ds) / self.dd).clamp(0.0, 1.0);
        let t = (dot(q, self.dt) / self.tt).clamp(0.0, 1.0);
        [(self.r[0] + 0.5 + s * (self.r[2] - 1.0)) / TPL_W, (self.r[1] + 0.5 + t * (self.r[3] - 1.0)) / TPL_H]
    }
}

pub fn face_at(face: usize, h: V3, hi: V3, s: f64, t: f64) -> V3 {
    match face {
        0 => [hi[0] - 2.0 * hi[0] * s, hi[1] - 2.0 * hi[1] * t, -h[2]],
        1 => [-hi[0] + 2.0 * hi[0] * s, hi[1] - 2.0 * hi[1] * t, h[2]],
        2 => [h[0], hi[1] - 2.0 * hi[1] * t, hi[2] - 2.0 * hi[2] * s],
        3 => [-h[0], hi[1] - 2.0 * hi[1] * t, -hi[2] + 2.0 * hi[2] * s],
        4 => [hi[0] - 2.0 * hi[0] * s, h[1], hi[2] - 2.0 * hi[2] * t],
        _ => [hi[0] - 2.0 * hi[0] * s, -h[1], -hi[2] + 2.0 * hi[2] * t],
    }
}

const FACE_N: [V3; 6] = [[0.0, 0.0, -1.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0]];

pub fn box_faces(hx: f64, hy: f64, hz: f64, region: &Region, b: f64) -> Vec<FaceDef> {
    let h = [hx, hy, hz];
    let hi = [hx - b, hy - b, hz - b];
    (0..6)
        .map(|f| {
            let p00 = face_at(f, h, hi, 0.0, 0.0);
            let ds = sub(face_at(f, h, hi, 1.0, 0.0), p00);
            let dt = sub(face_at(f, h, hi, 0.0, 1.0), p00);
            FaceDef { n: FACE_N[f], p00, ds, dt, dd: dot(ds, ds), tt: dot(dt, dt), r: region[f] }
        })
        .collect()
}

pub fn template_mesh(m: &Mesh, hx: f64, hy: f64, hz: f64, region: &Region) -> Mesh {
    let f = box_faces(hx, hy, hz, region, BEVEL);
    let face_of = |n: V3| -> Option<usize> {
        let mut best = None;
        let mut bd = 0.5;
        for (i, fd) in f.iter().enumerate() {
            let d = dot(n, fd.n);
            if d > bd {
                bd = d;
                best = Some(i);
            }
        }
        best
    };
    let mut uv = Vec::with_capacity(m.pos.len());
    let mut i = 0;
    while i + 2 < m.pos.len() {
        let votes = [face_of(m.nrm[i]), face_of(m.nrm[i + 1]), face_of(m.nrm[i + 2])];
        let mut fi = votes[0];
        if votes[1] == votes[2] && votes[1].is_some() {
            fi = votes[1];
        }
        let fi = match fi {
            Some(x) => x,
            None => {
                let s = norm([
                    m.nrm[i][0] + m.nrm[i + 1][0] + m.nrm[i + 2][0],
                    m.nrm[i][1] + m.nrm[i + 1][1] + m.nrm[i + 2][1],
                    m.nrm[i][2] + m.nrm[i + 1][2] + m.nrm[i + 2][2],
                ]);
                face_of(s).unwrap_or(0)
            }
        };
        for k in 0..3 {
            uv.push(f[fi].uv_of(m.pos[i + k]));
        }
        i += 3;
    }
    Mesh { pos: m.pos.clone(), nrm: m.nrm.clone(), uv }
}

pub fn clean_mesh(m: &Mesh) -> Mesh {
    let mut o = Mesh::default();
    let n = m.pos.len();
    let mut t = 0;
    while t + 2 < n {
        let (a, b, c) = (m.pos[t], m.pos[t + 1], m.pos[t + 2]);
        let sum = a[0] + a[1] + a[2] + b[0] + b[1] + b[2] + c[0] + c[1] + c[2];
        if !sum.is_finite() {
            t += 3;
            continue;
        }
        let fnv = cross(sub(b, a), sub(c, a));
        let fl = (fnv[0] * fnv[0] + fnv[1] * fnv[1] + fnv[2] * fnv[2]).sqrt();
        if !(fl > 1e-12) {
            t += 3;
            continue;
        }
        let fnv = [fnv[0] / fl, fnv[1] / fl, fnv[2] / fl];
        let mut ns: [Option<V3>; 3] = [None; 3];
        let mut good = [0.0; 3];
        let mut ng = 0;
        for k in 0..3 {
            let q = m.nrm.get(t + k).copied();
            let l = q.map(|q| (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt()).unwrap_or(0.0);
            if l > 0.1 && l.is_finite() {
                let q = q.unwrap();
                ns[k] = Some([q[0] / l, q[1] / l, q[2] / l]);
                good[0] += q[0] / l;
                good[1] += q[1] / l;
                good[2] += q[2] / l;
                ng += 1;
            }
        }
        let mut fb = fnv;
        if ng > 0 {
            let mut gl = (good[0] * good[0] + good[1] * good[1] + good[2] * good[2]).sqrt();
            if gl == 0.0 {
                gl = 1.0;
            }
            fb = [good[0] / gl, good[1] / gl, good[2] / gl];
        }
        for k in 0..3 {
            o.pos.push(m.pos[t + k]);
            o.nrm.push(ns[k].unwrap_or(fb));
            let u = m.uv.get(t + k).copied().unwrap_or([0.0, 0.0]);
            let ok = |v: f64| if v.is_finite() && v.abs() < 1e4 { v } else { 0.0 };
            o.uv.push([ok(u[0]), ok(u[1])]);
        }
        t += 3;
    }
    o
}

pub fn parse_mesh2016(bytes: &[u8]) -> Result<Mesh, String> {
    let mut m = parse_mesh(bytes)?;
    if !bytes.starts_with(b"version 1.") {
        for uv in m.uv.iter_mut() {
            uv[1] = 1.0 - uv[1];
        }
    }
    Ok(m)
}

pub fn plain_box(hx: f64, hy: f64, hz: f64) -> Mesh {
    let mut out = Mesh::default();
    let region = torso_region();
    let faces = box_faces(hx, hy, hz, &region, 0.0);
    let h = [hx, hy, hz];
    for (fi, f) in faces.iter().enumerate() {
        let c = [face_at(fi, h, h, 0.0, 0.0), face_at(fi, h, h, 1.0, 0.0), face_at(fi, h, h, 1.0, 1.0), face_at(fi, h, h, 0.0, 1.0)];
        for t in [[0usize, 1, 2], [0, 2, 3]] {
            let (a, b, cc) = (c[t[0]], c[t[1]], c[t[2]]);
            let list = if dot(cross(sub(b, a), sub(cc, a)), f.n) >= 0.0 { [a, b, cc] } else { [a, cc, b] };
            for p in list {
                out.pos.push(p);
                out.nrm.push(f.n);
                out.uv.push([0.0, 0.0]);
            }
        }
    }
    out
}

pub fn sphere_mesh(rx: f64, ry: f64, rz: f64) -> Mesh {
    let mut out = Mesh::default();
    let s = 24usize;
    let t = 16usize;
    let pt = |i: usize, j: usize| -> (V3, V3, [f64; 2]) {
        let th = i as f64 / s as f64 * 2.0 * std::f64::consts::PI;
        let ph = j as f64 / t as f64 * std::f64::consts::PI;
        let n = [ph.sin() * th.cos(), ph.cos(), ph.sin() * th.sin()];
        ([n[0] * rx, n[1] * ry, n[2] * rz], n, [i as f64 / s as f64, j as f64 / t as f64])
    };
    for i in 0..s {
        for j in 0..t {
            let a = pt(i, j);
            let b = pt(i + 1, j);
            let c = pt(i + 1, j + 1);
            let d = pt(i, j + 1);
            for tri in [[a, c, b], [a, d, c]] {
                for q in tri {
                    out.pos.push(q.0);
                    out.nrm.push(q.1);
                    out.uv.push(q.2);
                }
            }
        }
    }
    out
}

pub fn scene_faces_pub(faces: &[Vec<V3>]) -> Mesh {
    let mut out = Mesh::default();
    for f in faces {
        let mut f = f.clone();
        let mut n = norm(cross(sub(f[1], f[0]), sub(f[2], f[0])));
        let mut mid = [0.0; 3];
        for q in &f {
            mid = [mid[0] + q[0], mid[1] + q[1], mid[2] + q[2]];
        }
        if dot(n, mid) < 0.0 {
            f.reverse();
            n = [-n[0], -n[1], -n[2]];
        }
        for i in 1..f.len() - 1 {
            for p in [f[0], f[i], f[i + 1]] {
                out.pos.push(p);
                out.nrm.push(n);
                out.uv.push([0.0, 0.0]);
            }
        }
    }
    out
}

pub fn scene_cylinder(x: f64, y: f64, z: f64, along_y: bool) -> Mesh {
    let s = 16usize;
    let at = |u: f64, t: f64| -> V3 {
        if along_y {
            [t.cos() * x, u * y, t.sin() * z]
        } else {
            [u * x, t.cos() * y, t.sin() * z]
        }
    };
    let mut faces: Vec<Vec<V3>> = vec![];
    let mut top = vec![];
    let mut bot = vec![];
    for i in 0..s {
        let t0 = i as f64 / s as f64 * 2.0 * std::f64::consts::PI;
        let t1 = (i + 1) as f64 / s as f64 * 2.0 * std::f64::consts::PI;
        let mut q = vec![at(-1.0, t0), at(1.0, t0), at(1.0, t1), at(-1.0, t1)];
        if along_y {
            q.reverse();
        }
        faces.push(q);
        top.push(at(1.0, t0));
        bot.push(at(-1.0, t0));
    }
    if along_y {
        faces.push(top);
        bot.reverse();
        faces.push(bot);
    } else {
        top.reverse();
        faces.push(top);
        faces.push(bot);
    }
    scene_faces_pub(&faces)
}

pub fn scene_decal(size: V3, face: i64) -> Mesh {
    let x = size[0] / 2.0;
    let y = size[1] / 2.0;
    let z = size[2] / 2.0;
    let g = 0.01;
    let (tl, tr, br, bl, n): (V3, V3, V3, V3, V3) = match face {
        0 => ([x + g, y, z], [x + g, y, -z], [x + g, -y, -z], [x + g, -y, z], [1.0, 0.0, 0.0]),
        1 => ([x, y + g, z], [-x, y + g, z], [-x, y + g, -z], [x, y + g, -z], [0.0, 1.0, 0.0]),
        2 => ([-x, y, z + g], [x, y, z + g], [x, -y, z + g], [-x, -y, z + g], [0.0, 0.0, 1.0]),
        3 => ([-x - g, y, -z], [-x - g, y, z], [-x - g, -y, z], [-x - g, -y, -z], [-1.0, 0.0, 0.0]),
        4 => ([-x, -y - g, z], [x, -y - g, z], [x, -y - g, -z], [-x, -y - g, -z], [0.0, -1.0, 0.0]),
        _ => ([x, y, -z - g], [-x, y, -z - g], [-x, -y, -z - g], [x, -y, -z - g], [0.0, 0.0, -1.0]),
    };
    Mesh {
        pos: vec![tl, bl, br, tl, br, tr],
        nrm: vec![n; 6],
        uv: vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 1.0], [1.0, 0.0]],
    }
}

pub fn alpha_cuts_out(img: &crate::img::Img, uv: &[[f64; 2]]) -> bool {
    let w = img.w;
    let h = img.h;
    let at = |u: f64, v: f64| -> u32 {
        let u = u - u.floor();
        let v = v - v.floor();
        (img.px[((v * h as f64) as usize).min(h - 1) * w + ((u * w as f64) as usize).min(w - 1)] >> 24) & 255
    };
    let mut tot = 0usize;
    let mut clear = 0usize;
    let mut i = 0;
    while i + 2 < uv.len() {
        let (a, b, e) = (uv[i], uv[i + 1], uv[i + 2]);
        for q in [a, b, e, [(a[0] + b[0] + e[0]) / 3.0, (a[1] + b[1] + e[1]) / 3.0]] {
            tot += 1;
            if at(q[0], q[1]) < 128 {
                clear += 1;
            }
        }
        i += 3;
    }
    tot > 0 && clear as f64 / tot as f64 <= 0.9
}


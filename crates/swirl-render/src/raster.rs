use crate::mesh::Mesh;
use crate::texture::Texture;
use std::rc::Rc;
use swirl_core::math::*;

pub const AMBIENT: f64 = 128.0 / 255.0;
pub const KEY: f64 = 0.75 * 0.9;
pub const FILL: f64 = 0.75 * 0.9 * 0.4 * 0.9;
pub const KS: f64 = 1.0;
pub const SPEC_EXP: f64 = 45.0;
pub const LAMP_DIR: V3 = [0.47490, 0.82250, -0.31280];

pub struct Call<'a> {
    pub geom: &'a Mesh,
    pub cf: CF,
    pub mode: u8,
    pub color: Option<[f64; 3]>,
    pub tex: Option<Rc<Texture>>,
    pub studs: bool,
    pub half: Option<[f64; 3]>,
    pub tint: [f64; 3],
    pub alpha: f64,
    pub blend: bool,
}

pub struct Tri {
    pub bx0: i64,
    pub bx1: i64,
    pub by0: i64,
    pub by1: i64,
    pub e: [f64; 6],
    pub p: Vec<[f64; 3]>,
    pub mode: u8,
    pub color: [f64; 3],
    pub tex: Option<Rc<Texture>>,
    pub studs: bool,
    pub half: [f64; 3],
    pub tint: [f64; 3],
    pub specular: f64,
    pub alpha: f64,
    pub blend: bool,
    pub depth: f64,
}

pub fn setup(tris: &mut Vec<Tri>, c: &Call, vp: &M4, eye: V3, wss: usize, hss: usize) {
    let r = c.cf.r;
    let t = c.cf.p;
    let l = LAMP_DIR;
    let n = c.geom.pos.len();
    let mut vs: Vec<([f64; 4], Vec<f64>)> = Vec::with_capacity(n);
    for i in 0..n {
        let p = c.geom.pos[i];
        let nn = c.geom.nrm[i];
        let wx = r[0] * p[0] + r[1] * p[1] + r[2] * p[2] + t[0];
        let wy = r[3] * p[0] + r[4] * p[1] + r[5] * p[2] + t[1];
        let wz = r[6] * p[0] + r[7] * p[1] + r[8] * p[2] + t[2];
        let nv = norm([
            r[0] * nn[0] + r[1] * nn[1] + r[2] * nn[2],
            r[3] * nn[0] + r[4] * nn[1] + r[5] * nn[2],
            r[6] * nn[0] + r[7] * nn[1] + r[8] * nn[2],
        ]);
        let ndl = nv[0] * l[0] + nv[1] * l[1] + nv[2] * l[2];
        let vd = norm([eye[0] - wx, eye[1] - wy, eye[2] - wz]);
        let hh = norm([vd[0] + l[0], vd[1] + l[1], vd[2] + l[2]]);
        let nh = nv[0] * hh[0] + nv[1] * hh[1] + nv[2] * hh[2];
        let spec = (if nh > 0.0 { nh.powf(SPEC_EXP) } else { 0.0 }) * (if ndl >= 0.0 { 1.0 } else { 0.0 });
        let cx = vp[0] * wx + vp[4] * wy + vp[8] * wz + vp[12];
        let cy = vp[1] * wx + vp[5] * wy + vp[9] * wz + vp[13];
        let cz = vp[2] * wx + vp[6] * wy + vp[10] * wz + vp[14];
        let cw = vp[3] * wx + vp[7] * wy + vp[11] * wz + vp[15];
        let mut vy = vec![ndl.max(0.0), (-ndl).max(0.0), spec];
        if c.mode == 1 || c.mode == 3 {
            vy.push(c.geom.uv[i][0]);
            vy.push(c.geom.uv[i][1]);
        }
        if c.mode == 1 && c.studs {
            vy.push(p[0]);
            vy.push(p[1]);
            vy.push(p[2]);
            vy.push(nn[1]);
        }
        if c.mode == 2 {
            vy.push(p[0]);
            vy.push(p[1]);
            vy.push(nn[2]);
        }
        vs.push(([cx, cy, cz, cw], vy));
    }
    if vs.is_empty() {
        return;
    }
    let nv = vs[0].1.len();
    let snap = (1u32 << 8) as f64;
    let mut i = 0;
    while i + 2 < n {
        let tri = [&vs[i], &vs[i + 1], &vs[i + 2]];
        i += 3;
        if tri.iter().any(|v| v.0[3] <= 1e-6) {
            continue;
        }
        let mut s: Vec<(f64, f64, f64, f64, &Vec<f64>)> = Vec::with_capacity(3);
        for v in tri {
            let iw = 1.0 / v.0[3];
            let sx = (v.0[0] * iw + 1.0) * 0.5 * wss as f64;
            let sy = (1.0 - (v.0[1] * iw + 1.0) * 0.5) * hss as f64;
            let sx = (sx * snap).round() / snap;
            let sy = (sy * snap).round() / snap;
            s.push((sx, sy, (v.0[2] * iw + 1.0) * 0.5, iw, &v.1));
        }
        let (x0, y0) = (s[0].0, s[0].1);
        let (x1, y1) = (s[1].0, s[1].1);
        let (x2, y2) = (s[2].0, s[2].1);
        let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
        if !(area < 0.0) {
            continue;
        }
        let d0 = (x2 - x1) * (y0 - y1) - (y2 - y1) * (x0 - x1);
        let d1 = (x0 - x2) * (y1 - y2) - (y0 - y2) * (x1 - x2);
        if d0 == 0.0 || d1 == 0.0 {
            continue;
        }
        let a0 = -(y2 - y1) / d0;
        let b0 = (x2 - x1) / d0;
        let c0 = ((y2 - y1) * x1 - (x2 - x1) * y1) / d0;
        let a1 = -(y0 - y2) / d1;
        let b1 = (x0 - x2) / d1;
        let c1 = ((y0 - y2) * x2 - (x0 - x2) * y2) / d1;
        let plane = |f0: f64, f1: f64, f2: f64| -> [f64; 3] {
            let e0 = f0 - f2;
            let e1 = f1 - f2;
            [a0 * e0 + a1 * e1, b0 * e0 + b1 * e1, f2 + c0 * e0 + c1 * e1]
        };
        let mut p = vec![plane(s[0].3, s[1].3, s[2].3), plane(s[0].2, s[1].2, s[2].2)];
        for k in 0..nv {
            p.push(plane(s[0].4[k] * s[0].3, s[1].4[k] * s[1].3, s[2].4[k] * s[2].3));
        }
        let minx = x0.min(x1).min(x2);
        let maxx = x0.max(x1).max(x2);
        let miny = y0.min(y1).min(y2);
        let maxy = y0.max(y1).max(y2);
        tris.push(Tri {
            bx0: ((minx - 0.5).ceil() as i64).max(0),
            bx1: ((maxx - 0.5).floor() as i64).min(wss as i64 - 1),
            by0: ((miny - 0.5).ceil() as i64).max(0),
            by1: ((maxy - 0.5).floor() as i64).min(hss as i64 - 1),
            e: [a0, b0, c0, a1, b1, c1],
            p,
            mode: c.mode,
            color: c.color.unwrap_or([0.0; 3]),
            tex: c.tex.clone(),
            studs: c.studs,
            half: c.half.unwrap_or([0.0; 3]),
            tint: c.tint,
            specular: KS,
            alpha: c.alpha,
            blend: false,
            depth: 0.0,
        });
    }
}

pub fn rasterise(tris: &[Tri], w: usize, h: usize, ss: usize, stud: &Texture, face_box: [f64; 4]) -> Vec<u8> {
    let wss = w * ss;
    let hss = h * ss;
    let mut col = vec![0u32; wss * hss];
    let mut zb = vec![1.0f64; wss * hss];
    let (fx0, fx1, fy0, fy1) = (face_box[0], face_box[1], face_box[2], face_box[3]);
    let fdx = fx1 - fx0;
    let fdy = fy1 - fy0;
    let eps = -1e-9;
    for t in tris {
        if t.bx1 < t.bx0 {
            continue;
        }
        let [a0, b0, c0, a1, b1, c1] = t.e;
        let p = &t.p;
        let (iwa, iwb, iwc) = (p[0][0], p[0][1], p[0][2]);
        let (za, zb_, zc) = (p[1][0], p[1][1], p[1][2]);
        for py in t.by0..=t.by1 {
            let y = py as f64 + 0.5;
            let row = py as usize * wss;
            for px in t.bx0..=t.bx1 {
                let x = px as f64 + 0.5;
                let l0 = a0 * x + b0 * y + c0;
                if l0 < eps {
                    continue;
                }
                let l1 = a1 * x + b1 * y + c1;
                if l1 < eps {
                    continue;
                }
                if 1.0 - l0 - l1 < eps {
                    continue;
                }
                let z = za * x + zb_ * y + zc;
                let idx = row + px as usize;
                if z > zb[idx] || z < 0.0 || z > 1.0 {
                    continue;
                }
                let iw = iwa * x + iwb * y + iwc;
                let w_ = 1.0 / iw;
                let pl = |k: usize| (p[k][0] * x + p[k][1] * y + p[k][2]) * w_;
                let g0 = pl(2);
                let g1 = pl(3);
                let g2 = pl(4);
                let (ar, ag, ab);
                match t.mode {
                    1 => {
                        let u = pl(5);
                        let v = pl(6);
                        let tx = t.tex.as_ref().unwrap().sample0(u, v);
                        ar = tx[0];
                        ag = tx[1];
                        ab = tx[2];
                    }
                    3 => {
                        let u = pl(5);
                        let v = pl(6);
                        let tex = t.tex.as_ref().unwrap();
                        let tx = if tex.mip {
                            tex.sample(u, v, (p[5][0] - u * iwa) * w_, (p[6][0] - v * iwa) * w_, (p[5][1] - u * iwb) * w_, (p[6][1] - v * iwb) * w_)
                        } else {
                            tex.sample0(u, v)
                        };
                        if !t.blend && tx[3] < 0.5 && tex.cut {
                            continue;
                        }
                        ar = tx[0] * t.tint[0];
                        ag = tx[1] * t.tint[1];
                        ab = tx[2] * t.tint[2];
                    }
                    2 => {
                        let lx = pl(5);
                        let ly = pl(6);
                        let lnz = pl(7);
                        let fu = (fx1 - lx) / fdx;
                        let fv = (fy1 - ly) / fdy;
                        let dlxdx = (p[5][0] - lx * iwa) * w_;
                        let dlxdy = (p[5][1] - lx * iwb) * w_;
                        let dlydx = (p[6][0] - ly * iwa) * w_;
                        let dlydy = (p[6][1] - ly * iwb) * w_;
                        let cu = fu < 0.0 || fu > 1.0;
                        let cv = fv < 0.0 || fv > 1.0;
                        let tx = t.tex.as_ref().unwrap().sample(
                            fu.clamp(0.0, 1.0),
                            fv.clamp(0.0, 1.0),
                            if cu { 0.0 } else { -dlxdx / fdx },
                            if cv { 0.0 } else { -dlydx / fdy },
                            if cu { 0.0 } else { -dlxdy / fdx },
                            if cv { 0.0 } else { -dlydy / fdy },
                        );
                        let k = if lnz <= -0.05 { tx[3] } else { 0.0 };
                        ar = t.color[0] + (tx[0] - t.color[0]) * k;
                        ag = t.color[1] + (tx[1] - t.color[1]) * k;
                        ab = t.color[2] + (tx[2] - t.color[2]) * k;
                    }
                    _ => {
                        ar = t.color[0];
                        ag = t.color[1];
                        ab = t.color[2];
                    }
                }
                let lit = AMBIENT + KEY * g0 + FILL * g1;
                let mut r = ar * lit;
                let mut gg = ag * lit;
                let mut bb = ab * lit;
                if t.studs {
                    let lny = pl(10);
                    if lny > 0.99 {
                        let ly = pl(8);
                        if ly > t.half[1] - 0.01 {
                            let lx = pl(7);
                            let lz = pl(9);
                            let st = stud.sample(
                                t.half[0] - lx,
                                t.half[2] - lz,
                                -(p[7][0] - lx * iwa) * w_,
                                -(p[9][0] - lz * iwa) * w_,
                                -(p[7][1] - lx * iwb) * w_,
                                -(p[9][1] - lz * iwb) * w_,
                            );
                            let sa = st[3];
                            r += (st[0] - r) * sa;
                            gg += (st[1] - gg) * sa;
                            bb += (st[2] - bb) * sa;
                        }
                    }
                }
                let sp = KEY * t.specular * g2;
                r += sp;
                gg += sp;
                bb += sp;
                let q = |v: f64| -> u32 {
                    if v >= 1.0 {
                        255
                    } else if v <= 0.0 {
                        0
                    } else {
                        (v * 255.0 + 0.5) as u32
                    }
                };
                let (r8, g8, b8) = (q(r), q(gg), q(bb));
                if t.blend {
                    let mut al = (1.0f64).min(t.alpha);
                    al *= al;
                    if al <= 0.003 {
                        continue;
                    }
                    let ia = 1.0 - al;
                    let dc = col[idx];
                    let nr = (r8 as f64 * al + ((dc >> 16) & 255) as f64 * ia).round() as u32;
                    let ng = (g8 as f64 * al + ((dc >> 8) & 255) as f64 * ia).round() as u32;
                    let nb = (b8 as f64 * al + (dc & 255) as f64 * ia).round() as u32;
                    let na = (al * 255.0 + ((dc >> 24) & 255) as f64 * ia).round() as u32;
                    col[idx] = (na.min(255) << 24) | (nr.min(255) << 16) | (ng.min(255) << 8) | nb.min(255);
                    continue;
                }
                zb[idx] = z;
                col[idx] = (255 << 24) | (r8 << 16) | (g8 << 8) | b8;
            }
        }
    }
    let mut buf: Vec<[u32; 4]> = col.iter().map(|&c| [(c >> 16) & 255, (c >> 8) & 255, c & 255, (c >> 24) & 255]).collect();
    let (mut cw, mut ch) = (wss, hss);
    let mut s = ss;
    while s > 1 {
        let nw = cw >> 1;
        let nh = ch >> 1;
        let mut nb = Vec::with_capacity(nw * nh);
        for y in 0..nh {
            for x in 0..nw {
                let i = 2 * y * cw + 2 * x;
                let (p, q, r, t) = (buf[i], buf[i + 1], buf[i + cw], buf[i + cw + 1]);
                nb.push([(p[0] + q[0] + r[0] + t[0] + 2) >> 2, (p[1] + q[1] + r[1] + t[1] + 2) >> 2, (p[2] + q[2] + r[2] + t[2] + 2) >> 2, (p[3] + q[3] + r[3] + t[3] + 2) >> 2]);
            }
        }
        buf = nb;
        cw = nw;
        ch = nh;
        s >>= 1;
    }
    let mut out = Vec::with_capacity(buf.len() * 4);
    for p in buf {
        let a = p[3];
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else if a == 255 {
            out.extend_from_slice(&[p[0] as u8, p[1] as u8, p[2] as u8, 255]);
        } else {
            let f = |c: u32| ((c * 255 + (a >> 1)) / a).min(255) as u8;
            out.extend_from_slice(&[f(p[0]), f(p[1]), f(p[2]), a as u8]);
        }
    }
    out
}

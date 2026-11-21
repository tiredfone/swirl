use crate::raster::*;
use crate::texture::Texture;

const LOD_K: f64 = 0.5;

fn soft_white(v: f64) -> f64 {
    if v <= 0.75 {
        v
    } else {
        0.75 + 0.25 * (1.0 - ((v - 0.75) * -4.0).exp())
    }
}

fn avg4(c1: u32, c2: u32, c3: u32, c4: u32) -> u32 {
    let e = ((c1 & 0x00FF00FF) + (c2 & 0x00FF00FF) + (c3 & 0x00FF00FF) + (c4 & 0x00FF00FF) + 0x00020002) >> 2 & 0x00FF00FF;
    let o = (((c1 >> 8) & 0x00FF00FF) + ((c2 >> 8) & 0x00FF00FF) + ((c3 >> 8) & 0x00FF00FF) + ((c4 >> 8) & 0x00FF00FF) + 0x00020002) >> 2 & 0x00FF00FF;
    e | (o << 8)
}

struct Ta {
    a0: f64,
    b0: f64,
    c0: f64,
    a1: f64,
    b1: f64,
    c1: f64,
    a2: f64,
    b2: f64,
    c2: f64,
    n0: f64,
    n1: f64,
    n2: f64,
    za: f64,
    zb: f64,
    zc: f64,
    x0: i64,
    x1: i64,
    y0: i64,
    y1: i64,
}

fn shade(t: &Tri, x: f64, y: f64, dk: f64, stud: &Texture, fx1: f64, fy1: f64, fdx: f64, fdy: f64) -> u32 {
    let p = &t.p;
    let iwa = p[0][0];
    let iwb = p[0][1];
    let w = 1.0 / (iwa * x + iwb * y + p[0][2]);
    let pl = |k: usize| (p[k][0] * x + p[k][1] * y + p[k][2]) * w;
    let g0 = pl(2);
    let g1 = pl(3);
    let g2 = pl(4);
    let (ar, ag, ab);
    match t.mode {
        1 | 3 => {
            let u = pl(5);
            let v = pl(6);
            let tx = t.tex.as_ref().unwrap().sample(u, v, (p[5][0] - u * iwa) * w * dk, (p[6][0] - v * iwa) * w * dk, (p[5][1] - u * iwb) * w * dk, (p[6][1] - v * iwb) * w * dk);
            if t.mode == 1 {
                ar = tx[0];
                ag = tx[1];
                ab = tx[2];
            } else {
                ar = tx[0] * t.tint[0];
                ag = tx[1] * t.tint[1];
                ab = tx[2] * t.tint[2];
            }
        }
        2 => {
            let lx = pl(5);
            let ly = pl(6);
            let lnz = pl(7);
            let fu = (fx1 - lx) / fdx;
            let fv = (fy1 - ly) / fdy;
            let dlxdx = (p[5][0] - lx * iwa) * w * dk;
            let dlxdy = (p[5][1] - lx * iwb) * w * dk;
            let dlydx = (p[6][0] - ly * iwa) * w * dk;
            let dlydy = (p[6][1] - ly * iwb) * w * dk;
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
                    -(p[7][0] - lx * iwa) * w * dk,
                    -(p[9][0] - lz * iwa) * w * dk,
                    -(p[7][1] - lx * iwb) * w * dk,
                    -(p[9][1] - lz * iwb) * w * dk,
                );
                let sa = st[3];
                r += (st[0] - r) * sa;
                gg += (st[1] - gg) * sa;
                bb += (st[2] - bb) * sa;
            }
        }
    }
    let sp = KEY * t.specular * g2;
    r = soft_white(r + sp);
    gg = soft_white(gg + sp);
    bb = soft_white(bb + sp);
    let q = |v: f64| -> u32 {
        if v >= 1.0 {
            255
        } else if v <= 0.0 {
            0
        } else {
            (v * 255.0 + 0.5) as u32
        }
    };
    (255 << 24) | (q(r) << 16) | (q(gg) << 8) | q(bb)
}

fn alpha_ok(t: &Tri, x: f64, y: f64) -> bool {
    let p = &t.p;
    let iwa = p[0][0];
    let iwb = p[0][1];
    let w = 1.0 / (iwa * x + iwb * y + p[0][2]);
    let u = (p[5][0] * x + p[5][1] * y + p[5][2]) * w;
    let v = (p[6][0] * x + p[6][1] * y + p[6][2]) * w;
    let tx = t.tex.as_ref().unwrap().sample(u, v, (p[5][0] - u * iwa) * w, (p[6][0] - v * iwa) * w, (p[5][1] - u * iwb) * w, (p[6][1] - v * iwb) * w);
    tx[3] >= 0.5
}

pub fn rasterise_fast(tris: &[Tri], w: usize, h: usize, ss: usize, stud: &Texture, face_box: [f64; 4]) -> Vec<u8> {
    let s2 = ss * ss;
    let eps = -1e-9;
    let do_edges = ss > 1;
    let (fx0, fx1, fy0, fy1) = (face_box[0], face_box[1], face_box[2], face_box[3]);
    let fdx = fx1 - fx0;
    let fdy = fy1 - fy0;
    let dk = if ss > 1 { ss as f64 * LOD_K } else { 1.0 };
    stud.lod.set(true);
    for t in tris {
        if let Some(tx) = &t.tex {
            if tx.mip {
                tx.lod.set(true);
            }
        }
    }
    let ssf = ss as f64;
    let ssi = ss as i64;
    let mut ta: Vec<Ta> = Vec::with_capacity(tris.len());
    let mut is_cut = vec![false; tris.len()];
    let mut is_bl = vec![false; tris.len()];
    for (ti, t) in tris.iter().enumerate() {
        let [a0, b0, c0, a1, b1, c1] = t.e;
        let a2 = -(a0 + a1);
        let b2 = -(b0 + b1);
        let c2 = 1.0 - c0 - c1;
        let h0 = (a0 * a0 + b0 * b0).sqrt();
        let h1 = (a1 * a1 + b1 * b1).sqrt();
        let h2 = (a2 * a2 + b2 * b2).sqrt();
        ta.push(Ta {
            a0,
            b0,
            c0,
            a1,
            b1,
            c1,
            a2,
            b2,
            c2,
            n0: if h0 > 0.0 { 1.0 / h0 } else { 0.0 },
            n1: if h1 > 0.0 { 1.0 / h1 } else { 0.0 },
            n2: if h2 > 0.0 { 1.0 / h2 } else { 0.0 },
            za: t.p[1][0],
            zb: t.p[1][1],
            zc: t.p[1][2],
            x0: (t.bx0.div_euclid(ssi)).max(0),
            x1: (t.bx1.div_euclid(ssi)).min(w as i64 - 1),
            y0: (t.by0.div_euclid(ssi)).max(0),
            y1: (t.by1.div_euclid(ssi)).min(h as i64 - 1),
        });
        is_bl[ti] = t.blend;
        is_cut[ti] = t.mode == 3 && t.tex.as_ref().map(|x| x.cut && x.has_alpha).unwrap_or(false);
    }
    let r_ = ssf * 0.7072 + 1e-6;
    let n = w * h;
    let mut id_a = vec![-1i32; n];
    let mut zb_a = vec![1.0f64; n];
    let mut edge = vec![false; n];
    let mut elist: Vec<usize> = vec![];
    let mut rmin = vec![1i64 << 30; h];
    let mut rmax = vec![-1i64; h];
    let sh = |t: &Tri, x: f64, y: f64| shade(t, x, y, dk, stud, fx1, fy1, fdx, fdy);
    for (ti, t) in tris.iter().enumerate() {
        if t.bx1 < t.bx0 {
            continue;
        }
        let a = &ta[ti];
        if a.y1 < a.y0 {
            continue;
        }
        let cut = is_cut[ti];
        for yy in a.y0..=a.y1 {
            let y = (yy as f64 + 0.5) * ssf;
            let mut xl = -1e18;
            let mut xh = 1e18;
            let mut skip = false;
            for (aa, bb, cc) in [(a.a0, a.b0, a.c0), (a.a1, a.b1, a.c1), (a.a2, a.b2, a.c2)] {
                let tt = bb * y + cc;
                if aa > 1e-12 {
                    let v = (eps - tt) / aa;
                    if v > xl {
                        xl = v;
                    }
                } else if aa < -1e-12 {
                    let v = (eps - tt) / aa;
                    if v < xh {
                        xh = v;
                    }
                } else if tt < eps {
                    skip = true;
                    break;
                }
            }
            if skip || xl > xh {
                continue;
            }
            let mut xa = (xl / ssf - 0.5).ceil() as i64;
            if xa < a.x0 {
                xa = a.x0;
            }
            let mut xb = (xh / ssf - 0.5).floor() as i64;
            if xb > a.x1 {
                xb = a.x1;
            }
            if xb < xa {
                continue;
            }
            let row = yy as usize * w;
            let zy = a.zb * y + a.zc;
            let mut any = false;
            for xx in xa..=xb {
                let x = (xx as f64 + 0.5) * ssf;
                let z = a.za * x + zy;
                let p = row + xx as usize;
                if z > zb_a[p] || z < 0.0 || z > 1.0 {
                    continue;
                }
                if cut {
                    let ok = alpha_ok(t, x, y);
                    if do_edges && !edge[p] {
                        let m = ssf * 0.35;
                        if alpha_ok(t, x - m, y - m) != ok || alpha_ok(t, x + m, y - m) != ok || alpha_ok(t, x - m, y + m) != ok || alpha_ok(t, x + m, y + m) != ok {
                            edge[p] = true;
                            elist.push(p);
                        }
                    }
                    if !ok {
                        continue;
                    }
                }
                zb_a[p] = z;
                id_a[p] = ti as i32;
                any = true;
            }
            if any || cut {
                if xa < rmin[yy as usize] {
                    rmin[yy as usize] = xa;
                }
                if xb > rmax[yy as usize] {
                    rmax[yy as usize] = xb;
                }
            }
        }
    }
    if do_edges {
        for lr in 0..h {
            let lo = rmin[lr].min(if lr > 0 { rmin[lr - 1] } else { 1 << 30 }).min(if lr + 1 < h { rmin[lr + 1] } else { 1 << 30 }) - 1;
            let hi = rmax[lr].max(if lr > 0 { rmax[lr - 1] } else { -1 }).max(if lr + 1 < h { rmax[lr + 1] } else { -1 }) + 1;
            if hi < 0 {
                continue;
            }
            let lo = lo.max(0);
            let hi = hi.min(w as i64 - 1);
            let row = lr * w;
            let down = lr + 1 < h;
            for xx in lo..=hi {
                let p = row + xx as usize;
                let a = id_a[p];
                if (xx as usize) + 1 < w && id_a[p + 1] != a {
                    if !edge[p] {
                        edge[p] = true;
                        elist.push(p);
                    }
                    if !edge[p + 1] {
                        edge[p + 1] = true;
                        elist.push(p + 1);
                    }
                }
                if down {
                    let q = p + w;
                    if id_a[q] != a {
                        if !edge[p] {
                            edge[p] = true;
                            elist.push(p);
                        }
                        if !edge[q] {
                            edge[q] = true;
                            elist.push(q);
                        }
                    }
                    if (xx as usize) + 1 < w && id_a[q + 1] != a {
                        if !edge[p] {
                            edge[p] = true;
                            elist.push(p);
                        }
                        if !edge[q + 1] {
                            edge[q + 1] = true;
                            elist.push(q + 1);
                        }
                    }
                    if xx > 0 && id_a[q - 1] != a {
                        if !edge[p] {
                            edge[p] = true;
                            elist.push(p);
                        }
                        if !edge[q - 1] {
                            edge[q - 1] = true;
                            elist.push(q - 1);
                        }
                    }
                }
            }
        }
    }
    let mut ord = vec![-1i64; n];
    let ne = elist.len();
    for (k, &p) in elist.iter().enumerate() {
        ord[p] = k as i64;
    }
    let mut sz = vec![1.0f64; ne * s2];
    let mut sid = vec![-1i32; ne * s2];
    if ne > 0 {
        for (ti, t) in tris.iter().enumerate() {
            if t.bx1 < t.bx0 {
                continue;
            }
            let a = &ta[ti];
            if a.y1 < a.y0 {
                continue;
            }
            let cut = is_cut[ti];
            for yy in a.y0..=a.y1 {
                let row = yy as usize * w;
                let cy = (yy as f64 + 0.5) * ssf;
                for xx in a.x0..=a.x1 {
                    let p = row + xx as usize;
                    if !edge[p] {
                        continue;
                    }
                    let cx = (xx as f64 + 0.5) * ssf;
                    let d0 = (a.a0 * cx + a.b0 * cy + a.c0) * a.n0;
                    if d0 < -r_ {
                        continue;
                    }
                    let d1 = (a.a1 * cx + a.b1 * cy + a.c1) * a.n1;
                    if d1 < -r_ {
                        continue;
                    }
                    let d2 = (a.a2 * cx + a.b2 * cy + a.c2) * a.n2;
                    if d2 < -r_ {
                        continue;
                    }
                    let all = d0 > r_ && d1 > r_ && d2 > r_;
                    let k0 = ord[p] as usize * s2;
                    let bx = xx * ssi;
                    let by = yy * ssi;
                    for j in 0..ss {
                        let y = (by + j as i64) as f64 + 0.5;
                        let zy = a.zb * y + a.zc;
                        let ty0 = a.b0 * y + a.c0;
                        let ty1 = a.b1 * y + a.c1;
                        for i in 0..ss {
                            let x = (bx + i as i64) as f64 + 0.5;
                            if !all {
                                if a.a0 * x + ty0 < eps {
                                    continue;
                                }
                                if a.a1 * x + ty1 < eps {
                                    continue;
                                }
                                if 1.0 - (a.a0 * x + ty0) - (a.a1 * x + ty1) < eps {
                                    continue;
                                }
                            }
                            let z = a.za * x + zy;
                            let k = k0 + j * ss + i;
                            if z > sz[k] || z < 0.0 || z > 1.0 {
                                continue;
                            }
                            if cut && !alpha_ok(t, x, y) {
                                continue;
                            }
                            sz[k] = z;
                            sid[k] = ti as i32;
                        }
                    }
                }
            }
        }
    }
    let mut out = Vec::with_capacity(n * 4);
    for yy in 0..h {
        let row = yy * w;
        for xx in 0..w {
            let p = row + xx;
            if !edge[p] {
                let id = id_a[p];
                if id < 0 {
                    out.extend_from_slice(&[0, 0, 0, 0]);
                    continue;
                }
                let c = sh(&tris[id as usize], (xx as f64 + 0.5) * ssf, (yy as f64 + 0.5) * ssf);
                out.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]);
                continue;
            }
            let k0 = ord[p] as usize * s2;
            let bx = (xx * ss) as f64;
            let by = (yy * ss) as f64;
            let mut cnt: Vec<(i32, u32, f64, f64)> = vec![];
            for k in 0..s2 {
                let id = sid[k0 + k];
                if id < 0 {
                    continue;
                }
                let sx = bx + (k % ss) as f64 + 0.5;
                let sy = by + (k / ss) as f64 + 0.5;
                match cnt.iter_mut().find(|e| e.0 == id) {
                    Some(e) => {
                        e.1 += 1;
                        e.2 += sx;
                        e.3 += sy;
                    }
                    None => cnt.push((id, 1, sx, sy)),
                }
            }
            if cnt.is_empty() {
                out.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let cc: Vec<(i32, u32)> = cnt.iter().map(|e| (e.0, sh(&tris[e.0 as usize], e.2 / e.1 as f64, e.3 / e.1 as f64))).collect();
            let mut s = vec![0u32; s2];
            for k in 0..s2 {
                let id = sid[k0 + k];
                s[k] = if id < 0 { 0 } else { cc.iter().find(|e| e.0 == id).unwrap().1 };
            }
            let c = if ss == 4 {
                avg4(avg4(s[0], s[1], s[4], s[5]), avg4(s[2], s[3], s[6], s[7]), avg4(s[8], s[9], s[12], s[13]), avg4(s[10], s[11], s[14], s[15]))
            } else if ss == 2 {
                avg4(s[0], s[1], s[2], s[3])
            } else {
                s[0]
            };
            let a = (c >> 24) & 255;
            if a == 0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let mut r = (c >> 16) & 255;
            let mut g = (c >> 8) & 255;
            let mut b = c & 255;
            if a < 255 {
                let hh = a >> 1;
                r = ((r * 255 + hh) / a).min(255);
                g = ((g * 255 + hh) / a).min(255);
                b = ((b * 255 + hh) / a).min(255);
            }
            out.extend_from_slice(&[r as u8, g as u8, b as u8, a as u8]);
        }
    }
    let _ = (is_bl, fx0, fy0);
    out
}

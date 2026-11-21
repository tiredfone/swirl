use crate::img::{encode_png, Img};
use crate::mesh::*;
use crate::raster::*;
use crate::texture::Texture;
use std::rc::Rc;
use swirl_core::brickcolors;
use swirl_core::math::*;

pub const CAM_DIR: V3 = [-0.40142, 0.39402, -0.82681];
pub const CAM_FOV: f64 = 70.86;
pub const FACE_BOX: [f64; 4] = [0.0343, -0.0233, 1.0806, 1.0544];
pub const HEAD_DIR: V3 = [0.0, 0.0, -1.0];
pub const HEAD_FOV: f64 = 42.84;
pub const HEAD_DIST: f64 = 3.405;
pub const HEAD_AIM: f64 = 0.0;
pub const HEAD_MAXOUT: f64 = 1.6;
pub const HEAD_NEAR: f64 = 2.6;
pub const TPL_WI: usize = 585;
pub const TPL_HI: usize = 559;

pub struct Base {
    pub head: Mesh,
    pub head_raw: Mesh,
    pub head_box: (V3, V3),
    pub torso: Mesh,
    pub right_arm: Mesh,
    pub left_arm: Mesh,
    pub right_leg: Mesh,
    pub left_leg: Mesh,
    pub stud: Rc<Texture>,
    pub inlet: Img,
    pub face: Img,
}

fn load(dir: &str, f: &str) -> Result<Vec<u8>, String> {
    std::fs::read(format!("{}/{}", dir, f)).map_err(|e| format!("{}: {}", f, e))
}

impl Base {
    pub fn load(data: &str) -> Result<Base, String> {
        let head_raw = parse_mesh(&load(data, "head.mesh")?)?;
        let head = head_raw.clone();
        let part = |f: &str, hx: f64, hy: f64, hz: f64, r: &Region| -> Result<Mesh, String> {
            Ok(template_mesh(&parse_mesh(&load(data, f)?)?, hx, hy, hz, r))
        };
        let dec = |f: &str| -> Result<Img, String> { Img::decode(&load(data, f)?, 2048).ok_or_else(|| format!("{} did not decode", f)) };
        Ok(Base {
            head_box: mesh_bounds(&head),
            head,
            head_raw,
            torso: part("torso.mesh", 1.0, 1.0, 0.5, &torso_region())?,
            right_arm: part("rightarm.mesh", 0.5, 1.0, 0.5, &right_region())?,
            left_arm: part("leftarm.mesh", 0.5, 1.0, 0.5, &left_region())?,
            right_leg: part("rightleg.mesh", 0.5, 1.0, 0.5, &right_region())?,
            left_leg: part("leftleg.mesh", 0.5, 1.0, 0.5, &left_region())?,
            stud: Rc::new(Texture::new(dec("stud.png")?, false)),
            inlet: dec("inlet.png")?,
            face: dec("face.png")?,
        })
    }
}

#[derive(Clone)]
pub struct Item {
    pub geom: Mesh,
    pub cf: CF,
    pub size: V3,
    pub tex: Option<Rc<Texture>>,
    pub color: [f64; 3],
    pub tint: [f64; 3],
    pub alpha: f64,
}

pub struct Look {
    pub colors: [[f64; 3]; 6],
    pub shirt: Option<Img>,
    pub pants: Option<Img>,
    pub tshirt: Option<Img>,
    pub face: Option<Img>,
    pub items: Vec<Item>,
    pub n_hats: usize,
    pub tool: bool,
    pub notes: Vec<String>,
}

pub const HEAD: usize = 0;
pub const TORSO: usize = 1;
pub const LARM: usize = 2;
pub const RARM: usize = 3;
pub const LLEG: usize = 4;
pub const RLEG: usize = 5;

pub fn brick(n: i64) -> [f64; 3] {
    let c = brickcolors::rgb(n);
    [c[0] as f64 / 255.0, c[1] as f64 / 255.0, c[2] as f64 / 255.0]
}

impl Look {
    pub fn empty() -> Look {
        Look { colors: [brick(194); 6], shirt: None, pants: None, tshirt: None, face: None, items: vec![], n_hats: 0, tool: false, notes: vec![] }
    }
}

pub struct Cfs {
    pub torso: CF,
    pub head: CF,
    pub larm: CF,
    pub rarm: CF,
    pub lleg: CF,
    pub rleg: CF,
}

pub fn shoulder_r() -> CF {
    CF::new([1.0, 0.5, 0.0], [0.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0])
}

pub fn shoulder_r1() -> CF {
    CF::new([-0.5, 0.5, 0.0], [0.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0])
}

pub fn part_cfs(raised: bool, shoulder_angle: f64) -> Cfs {
    let torso = CF::at([0.0, 3.0, 0.0]);
    let mut c = Cfs {
        torso,
        head: CF::at([0.0, 4.5, 0.0]),
        larm: CF::at([-1.5, 3.0, 0.0]),
        rarm: CF::at([1.5, 3.0, 0.0]),
        lleg: CF::at([-0.5, 1.0, 0.0]),
        rleg: CF::at([0.5, 1.0, 0.0]),
    };
    if raised {
        c.rarm = torso.mul(&shoulder_r()).mul(&CF::new([0.0; 3], rot_z(shoulder_angle))).mul(&shoulder_r1().inv());
    }
    c
}

fn css(c: [f64; 3]) -> [u32; 3] {
    [(c[0] * 255.0).round() as u32, (c[1] * 255.0).round() as u32, (c[2] * 255.0).round() as u32]
}

fn paint_sheets(look: &Look, inlet: &Img) -> (Rc<Texture>, Rc<Texture>) {
    let fill_region = |g: &mut Img, regions: &Region, col: [f64; 3]| {
        for r in regions {
            g.fill_rect(r[0] as i64 - 1, r[1] as i64 - 1, r[2] as i64 + 2, r[3] as i64 + 2, css(col));
        }
    };
    let torso = torso_region();
    let right = right_region();
    let left = left_region();
    let c = &look.colors;
    let mut up = Img::new(TPL_WI, TPL_HI);
    let mut lo = Img::new(TPL_WI, TPL_HI);
    up.fill_rect(0, 0, TPL_WI as i64, TPL_HI as i64, css(c[TORSO]));
    fill_region(&mut up, &right, c[RARM]);
    fill_region(&mut up, &left, c[LARM]);
    fill_region(&mut up, &torso, c[TORSO]);
    let k = 1.0 / (1.0 - 2.0 * BEVEL);
    let mut flip = Img::new(inlet.w, inlet.h);
    for y in 0..inlet.h {
        for x in 0..inlet.w {
            flip.px[y * inlet.w + x] = inlet.px[y * inlet.w + (inlet.w - 1 - x)];
        }
    }
    if look.shirt.is_none() {
        for side in [&right, &left] {
            let d = side[5];
            let iw = (d[2] * k).round();
            let ih = (d[3] * k).round();
            up.draw_scaled(
                &flip,
                (d[0] + d[2] / 2.0 - iw / 2.0).round() as i64,
                (d[1] + d[3] / 2.0 - ih / 2.0).round() as i64,
                iw as i64,
                ih as i64,
                Some([d[0] as i64, d[1] as i64, d[2] as i64, d[3] as i64]),
            );
        }
    }
    lo.fill_rect(0, 0, TPL_WI as i64, TPL_HI as i64, css(c[RLEG]));
    fill_region(&mut lo, &right, c[RLEG]);
    fill_region(&mut lo, &left, c[LLEG]);
    if let Some(p) = &look.pants {
        let clip: Vec<[i64; 4]> = torso.iter().map(|r| [r[0] as i64 - 1, r[1] as i64 - 1, r[2] as i64 + 2, r[3] as i64 + 2]).collect();
        up.draw_over(p, Some(&clip));
        lo.draw_over(p, None);
    }
    if let Some(s) = &look.shirt {
        up.draw_over(s, None);
    }
    if let Some(t) = &look.tshirt {
        let f = torso[0];
        up.draw_scaled(t, f[0] as i64, f[1] as i64, f[2] as i64, f[3] as i64, None);
    }
    (Rc::new(Texture::new(up, false)), Rc::new(Texture::new(lo, false)))
}

fn ndc_of(vp: &M4, p: V3) -> Option<[f64; 2]> {
    let w = vp[3] * p[0] + vp[7] * p[1] + vp[11] * p[2] + vp[15];
    if w <= 1e-6 {
        return None;
    }
    Some([(vp[0] * p[0] + vp[4] * p[1] + vp[8] * p[2] + vp[12]) / w, (vp[1] * p[0] + vp[5] * p[1] + vp[9] * p[2] + vp[13]) / w])
}

fn scale_translate(s: f64, tx: f64, ty: f64) -> M4 {
    [s, 0.0, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, ty, 0.0, 1.0]
}

pub fn draw(base: &Base, look: &Look, cfs: &Cfs, w: usize, h: usize, ss: usize, view: &str) -> Vec<u8> {
    let head_view = view == "head";
    let items: Vec<&Item> = look.items.iter().collect();
    let n_hats = look.n_hats.min(items.len());
    let (vp, eye, center);
    if head_view {
        let hp = cfs.head.p;
        center = [hp[0], hp[1] + HEAD_AIM, hp[2]];
        let mut fov = HEAD_FOV * std::f64::consts::PI / 180.0;
        let mut dist = HEAD_DIST;
        let cd = norm(HEAD_DIR);
        let mut near = f64::INFINITY;
        for it in &items[..n_hats] {
            for p in &it.geom.pos {
                let q = it.cf.apply(*p);
                near = near.min(dist - ((q[0] - center[0]) * cd[0] + (q[1] - center[1]) * cd[1] + (q[2] - center[2]) * cd[2]));
            }
        }
        if near < HEAD_NEAR {
            let nd = dist + (HEAD_NEAR - near);
            fov = 2.0 * ((fov / 2.0).tan() * dist / nd).atan();
            dist = nd;
        }
        eye = [center[0] + cd[0] * dist, center[1] + cd[1] * dist, center[2] + cd[2] * dist];
        let mut v = mul4(&perspective(fov, w as f64 / h as f64, 0.05, 200.0), &look_at(eye, center, [0.0, 1.0, 0.0]));
        let mut m = 0.0f64;
        for it in &items[..n_hats] {
            for p in &it.geom.pos {
                if let Some(q) = ndc_of(&v, it.cf.apply(*p)) {
                    m = m.max(q[0].abs()).max(q[1].abs());
                }
            }
        }
        if m > 0.96 {
            let s = (1.0 / HEAD_MAXOUT).max(0.96 / m);
            v = mul4(&scale_translate(s, 0.0, 0.0), &v);
        }
        vp = v;
    } else {
        let mut lo = [1e9; 3];
        let mut hi = [-1e9; 3];
        let mut add_box = |cf: &CF, size: V3| {
            for sx in [-1.0, 1.0] {
                for sy in [-1.0, 1.0] {
                    for sz in [-1.0, 1.0] {
                        let q = cf.apply([sx * size[0] / 2.0, sy * size[1] / 2.0, sz * size[2] / 2.0]);
                        for k in 0..3 {
                            lo[k] = f64::min(lo[k], q[k]);
                            hi[k] = f64::max(hi[k], q[k]);
                        }
                    }
                }
            }
        };
        add_box(&cfs.torso, [2.0, 2.0, 1.0]);
        add_box(&cfs.head, [2.0, 1.0, 1.0]);
        for cf in [&cfs.larm, &cfs.rarm, &cfs.lleg, &cfs.rleg] {
            add_box(cf, [1.0, 2.0, 1.0]);
        }
        for it in &items {
            add_box(&it.cf, it.size);
        }
        center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let radius = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt() / 2.0;
        let fov = CAM_FOV * std::f64::consts::PI / 180.0;
        let dist = radius / (fov / 2.0).tan();
        let cd = norm(CAM_DIR);
        eye = [center[0] + cd[0] * dist, center[1] + cd[1] * dist, center[2] + cd[2] * dist];
        let v = mul4(&perspective(fov, w as f64 / h as f64, 0.1, 200.0), &look_at(eye, center, [0.0, 1.0, 0.0]));
        let (mut nx0, mut nx1, mut ny0, mut ny1) = (1e9f64, -1e9f64, 1e9f64, -1e9f64);
        let mut ndc = |p: V3| {
            let x = v[0] * p[0] + v[4] * p[1] + v[8] * p[2] + v[12];
            let y = v[1] * p[0] + v[5] * p[1] + v[9] * p[2] + v[13];
            let ww = v[3] * p[0] + v[7] * p[1] + v[11] * p[2] + v[15];
            if ww <= 1e-6 {
                return;
            }
            let (x, y) = (x / ww, y / ww);
            nx0 = nx0.min(x);
            nx1 = nx1.max(x);
            ny0 = ny0.min(y);
            ny1 = ny1.max(y);
        };
        let mut box_pts = |cf: &CF, size: V3| {
            for sx in [-1.0, 1.0] {
                for sy in [-1.0, 1.0] {
                    for sz in [-1.0, 1.0] {
                        ndc(cf.apply([sx * size[0] / 2.0, sy * size[1] / 2.0, sz * size[2] / 2.0]));
                    }
                }
            }
        };
        box_pts(&cfs.torso, [2.0, 2.0, 1.0]);
        for cf in [&cfs.larm, &cfs.rarm, &cfs.lleg, &cfs.rleg] {
            box_pts(cf, [1.0, 2.0, 1.0]);
        }
        for p in &base.head.pos {
            ndc(cfs.head.apply(*p));
        }
        for it in &items {
            for p in &it.geom.pos {
                ndc(it.cf.apply(*p));
            }
        }
        let mut vp_ = v;
        if nx1 > nx0 {
            let mut s = 0.94 / ((nx1 - nx0) / 2.0).max((ny1 - ny0) / 2.0);
            s = s.min(1.0);
            let tx = -s * (nx0 + nx1) / 2.0;
            let ty = -s * (ny0 + ny1) / 2.0;
            vp_ = mul4(&scale_translate(s, tx, ty), &v);
        }
        vp = vp_;
    }

    let (up_tex, lo_tex) = paint_sheets(look, &base.inlet);
    let studs = look.shirt.is_none();
    let (hb_lo, hb_hi) = base.head_box;
    let fs = (hb_hi[0] - hb_lo[0]) / 2.0;
    let fb = FACE_BOX;
    let face_box = [(fb[0] - fb[2]) * fs, (fb[0] + fb[2]) * fs, (fb[1] - fb[3]) * fs, (fb[1] + fb[3]) * fs];
    let face_img = look.face.clone().unwrap_or_else(|| base.face.clone());
    let face_tex = Rc::new(Texture::new(face_img, false));
    let wss = w * ss;
    let hss = h * ss;
    let mut calls: Vec<Call> = vec![
        mk(&base.torso, cfs.torso, 1, None, Some(up_tex.clone()), studs, Some([1.0, 1.0, 0.5])),
        mk(&base.right_arm, cfs.rarm, 1, None, Some(up_tex.clone()), studs, Some([0.5, 1.0, 0.5])),
        mk(&base.left_arm, cfs.larm, 1, None, Some(up_tex.clone()), studs, Some([0.5, 1.0, 0.5])),
        mk(&base.right_leg, cfs.rleg, 1, None, Some(lo_tex.clone()), false, None),
        mk(&base.left_leg, cfs.lleg, 1, None, Some(lo_tex.clone()), false, None),
        mk(&base.head, cfs.head, 2, Some(look.colors[HEAD]), Some(face_tex), false, None),
    ];
    if head_view {
        let hd = calls.pop().unwrap();
        calls = vec![hd];
    }
    for it in &items {
        let blend = it.alpha < 0.999;
        if let Some(t) = &it.tex {
            calls.push(Call { geom: &it.geom, cf: it.cf, mode: 3, color: None, tex: Some(t.clone()), studs: false, half: None, tint: it.tint, alpha: it.alpha, blend });
        } else {
            let c = [it.color[0] * it.tint[0], it.color[1] * it.tint[1], it.color[2] * it.tint[2]];
            calls.push(Call { geom: &it.geom, cf: it.cf, mode: 0, color: Some(c), tex: None, studs: false, half: None, tint: [1.0; 3], alpha: it.alpha, blend });
        }
    }
    let mut solid = Vec::new();
    let mut glass = Vec::new();
    for c in &calls {
        let mut tt = Vec::new();
        setup(&mut tt, c, &vp, eye, wss, hss);
        if c.blend {
            glass.append(&mut tt);
        } else {
            solid.append(&mut tt);
        }
    }
    for q in glass.iter_mut() {
        q.blend = true;
        q.depth = q.p[1][0] * ((q.bx0 + q.bx1) as f64 / 2.0 + 0.5) + q.p[1][1] * ((q.by0 + q.by1) as f64 / 2.0 + 0.5) + q.p[1][2];
    }
    glass.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal));
    let mut tris = solid;
    tris.append(&mut glass);
    let rgba = rasterise(&tris, w, h, ss, &base.stud, face_box);
    encode_png(w, h, &rgba)
}

fn mk<'a>(geom: &'a Mesh, cf: CF, mode: u8, color: Option<[f64; 3]>, tex: Option<Rc<Texture>>, studs: bool, half: Option<[f64; 3]>) -> Call<'a> {
    Call { geom, cf, mode, color, tex, studs, half, tint: [1.0; 3], alpha: 1.0, blend: false }
}

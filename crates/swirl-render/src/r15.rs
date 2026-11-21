use crate::body::{composite_layer_with, layer_over};
use crate::img::Img;
use crate::items::{cf_from, mesh_bytes_pub};
use crate::look::{asset_id, Ctx};
use crate::mesh::*;
use crate::scene::*;
use crate::texture::Texture;
use std::collections::HashMap;
use std::rc::Rc;
use swirl_core::math::*;
use swirl_core::rbx::{Inst, Value};

pub const GROUPS: [(&str, &str); 14] = [
    ("UpperTorso", "torso"),
    ("LowerTorso", "torso"),
    ("LeftUpperArm", "leftArm"),
    ("LeftLowerArm", "leftArm"),
    ("LeftHand", "leftArm"),
    ("RightUpperArm", "rightArm"),
    ("RightLowerArm", "rightArm"),
    ("RightHand", "rightArm"),
    ("LeftUpperLeg", "leftLeg"),
    ("LeftLowerLeg", "leftLeg"),
    ("LeftFoot", "leftLeg"),
    ("RightUpperLeg", "rightLeg"),
    ("RightLowerLeg", "rightLeg"),
    ("RightFoot", "rightLeg"),
];

const JOINTS: [(&str, Option<&str>, [f64; 3]); 15] = [
    ("LowerTorso", None, [0.0, 2.0, 0.0]),
    ("UpperTorso", Some("LowerTorso"), [0.0, 2.4, 0.0]),
    ("Head", Some("UpperTorso"), [0.0, 4.0, 0.0]),
    ("RightUpperArm", Some("UpperTorso"), [1.0, 3.763, 0.0]),
    ("RightLowerArm", Some("RightUpperArm"), [1.5, 3.035, 0.0]),
    ("RightHand", Some("RightLowerArm"), [1.5, 2.275, 0.0]),
    ("LeftUpperArm", Some("UpperTorso"), [-1.0, 3.763, 0.0]),
    ("LeftLowerArm", Some("LeftUpperArm"), [-1.5, 3.035, 0.0]),
    ("LeftHand", Some("LeftLowerArm"), [-1.5, 2.275, 0.0]),
    ("RightUpperLeg", Some("LowerTorso"), [0.5, 2.0, 0.0]),
    ("RightLowerLeg", Some("RightUpperLeg"), [0.5, 1.178, 0.0]),
    ("RightFoot", Some("RightLowerLeg"), [0.5, 0.252, 0.0]),
    ("LeftUpperLeg", Some("LowerTorso"), [-0.5, 2.0, 0.0]),
    ("LeftLowerLeg", Some("LeftUpperLeg"), [-0.5, 1.178, 0.0]),
    ("LeftFoot", Some("LeftLowerLeg"), [-0.5, 0.252, 0.0]),
];

const ATT_PARTS: [(&str, &str); 18] = [
    ("HatAttachment", "Head"),
    ("HairAttachment", "Head"),
    ("FaceFrontAttachment", "Head"),
    ("FaceCenterAttachment", "Head"),
    ("NeckAttachment", "UpperTorso"),
    ("BodyFrontAttachment", "UpperTorso"),
    ("BodyBackAttachment", "UpperTorso"),
    ("LeftCollarAttachment", "UpperTorso"),
    ("RightCollarAttachment", "UpperTorso"),
    ("WaistCenterAttachment", "LowerTorso"),
    ("WaistFrontAttachment", "LowerTorso"),
    ("WaistBackAttachment", "LowerTorso"),
    ("LeftShoulderAttachment", "LeftUpperArm"),
    ("RightShoulderAttachment", "RightUpperArm"),
    ("LeftGripAttachment", "LeftHand"),
    ("RightGripAttachment", "RightHand"),
    ("LeftFootAttachment", "LeftFoot"),
    ("RightFootAttachment", "RightFoot"),
];

fn joint_name(n: &str) -> &'static str {
    match n {
        "LowerTorso" => "Root",
        "UpperTorso" => "Waist",
        "Head" => "Neck",
        "RightUpperArm" => "RightShoulder",
        "RightLowerArm" => "RightElbow",
        "RightHand" => "RightWrist",
        "LeftUpperArm" => "LeftShoulder",
        "LeftLowerArm" => "LeftElbow",
        "LeftHand" => "LeftWrist",
        "RightUpperLeg" => "RightHip",
        "RightLowerLeg" => "RightKnee",
        "RightFoot" => "RightAnkle",
        "LeftUpperLeg" => "LeftHip",
        "LeftLowerLeg" => "LeftKnee",
        _ => "LeftAnkle",
    }
}

fn stance_part(p: &str) -> &'static str {
    match p {
        "Right Arm" => "RightUpperArm",
        "Left Arm" => "LeftUpperArm",
        "Right Leg" => "RightUpperLeg",
        _ => "LeftUpperLeg",
    }
}

pub struct RigPart {
    pub name: String,
    pub cf: CF,
    pub size: V3,
    pub mesh: Option<Mesh>,
}

pub struct Rig {
    pub parts: Vec<RigPart>,
    pub atts: HashMap<String, CF>,
}

fn fit_mesh(m: &Mesh, size: V3) -> Mesh {
    let (lo, hi) = mesh_bounds(m);
    let mut s = [1.0; 3];
    let mut mid = [0.0; 3];
    for k in 0..3 {
        let ext = hi[k] - lo[k];
        s[k] = if ext > 1e-6 { size[k] / ext } else { 1.0 };
        mid[k] = (hi[k] + lo[k]) / 2.0;
    }
    let pos = m.pos.iter().map(|p| [(p[0] - mid[0]) * s[0], (p[1] - mid[1]) * s[1], (p[2] - mid[2]) * s[2]]).collect();
    let nrm = m
        .nrm
        .iter()
        .map(|n| {
            let n = [n[0] / s[0], n[1] / s[1], n[2] / s[2]];
            let mut l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if l == 0.0 {
                l = 1.0;
            }
            [n[0] / l, n[1] / l, n[2] / l]
        })
        .collect();
    Mesh { pos, nrm, uv: m.uv.clone() }
}

fn scale_mesh(m: &Mesh, k: V3) -> Mesh {
    let pos = m.pos.iter().map(|p| [p[0] * k[0], p[1] * k[1], p[2] * k[2]]).collect();
    let mut nrm = m.nrm.clone();
    if (k[0] - k[1]).abs() > 1e-6 || (k[1] - k[2]).abs() > 1e-6 {
        nrm = m
            .nrm
            .iter()
            .map(|n| {
                let n = [n[0] / k[0], n[1] / k[1], n[2] / k[2]];
                let mut l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                if l == 0.0 {
                    l = 1.0;
                }
                [n[0] / l, n[1] / l, n[2] / l]
            })
            .collect();
    }
    Mesh { pos, nrm, uv: m.uv.clone() }
}

pub fn load_rig(data: &str) -> Result<Rig, String> {
    let dir = format!("{}/r15", data);
    let txt = std::fs::read_to_string(format!("{}/rig.json", dir)).map_err(|e| format!("rig.json: {}", e))?;
    let j: serde_json::Value = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
    let cf_of = |c: &serde_json::Value| -> CF {
        let a: Vec<f64> = c.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let mut r = [0.0; 9];
        r.copy_from_slice(&a[3..12]);
        CF::new([a[0], a[1], a[2]], r)
    };
    let mut parts = vec![];
    for p in j["parts"].as_array().ok_or("no parts")? {
        let size_v: Vec<f64> = p["size"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let size = [size_v[0], size_v[1], size_v[2]];
        let mesh = match p["mesh"].as_str() {
            Some(f) if !f.is_empty() => {
                let b = std::fs::read(format!("{}/{}", dir, f)).map_err(|e| e.to_string())?;
                Some(fit_mesh(&parse_mesh(&b)?, size))
            }
            _ => None,
        };
        parts.push(RigPart { name: p["name"].as_str().unwrap().to_string(), cf: cf_of(&p["cf"]), size, mesh });
    }
    let mut atts = HashMap::new();
    if let Some(o) = j["attachments"].as_object() {
        for (k, v) in o {
            atts.insert(k.clone(), cf_of(v));
        }
    }
    Ok(Rig { parts, atts })
}

pub struct PkgPart {
    pub size: V3,
    pub att: HashMap<String, CF>,
    pub mesh: Option<String>,
    pub tex: Option<String>,
}

pub struct R15<'a> {
    pub rig: &'a Rig,
    pub scale: [f64; 4],
    pub pkg: HashMap<String, PkgPart>,
    pub world: HashMap<String, CF>,
    pub stance: String,
    pub pose: Option<String>,
    pub straight_legs: bool,
}

pub fn parse_scale(spec: &str) -> [f64; 4] {
    let mut s = [1.0; 4];
    let mut depth = false;
    for kv in spec.split(',') {
        if let Some((k, v)) = kv.split_once(':') {
            if let Ok(f) = v.trim().parse::<f64>() {
                match k.trim() {
                    "height" => s[0] = f,
                    "width" => s[1] = f,
                    "depth" => {
                        s[2] = f;
                        depth = true;
                    }
                    "head" => s[3] = f,
                    _ => {}
                }
            }
        }
    }
    if !depth {
        s[2] = s[1];
    }
    s
}

impl<'a> R15<'a> {
    pub fn new(rig: &'a Rig, scale: [f64; 4], stance: &str, pose: Option<String>) -> R15<'a> {
        R15 { rig, scale, pkg: HashMap::new(), world: HashMap::new(), stance: stance.to_string(), pose, straight_legs: false }
    }

    pub fn set_straight_legs(&mut self, on: bool) {
        self.straight_legs = on;
    }

    pub fn use_package(&mut self, parts: &[(String, Inst)]) {
        self.pkg.clear();
        for (name, mp) in parts {
            let size = mp.props.get("size").or(mp.props.get("Size")).and_then(|v| v.as_v3()).unwrap_or([0.0; 3]);
            if size[0] <= 0.0 || size[1] <= 0.0 || size[2] <= 0.0 {
                continue;
            }
            let mut att = HashMap::new();
            for a in &mp.children {
                if a.cls == "Attachment" {
                    if let Some(Value::CFrame(p, m)) = a.props.get("CFrame") {
                        att.insert(a.name.clone(), CF::new(*p, *m));
                    }
                }
            }
            let url = |k: &str| mp.props.get(k).and_then(|v| v.as_url()).map(|s| s.to_string());
            self.pkg.insert(name.clone(), PkgPart { size, att, mesh: url("MeshId"), tex: url("TextureID") });
        }
    }

    fn part_scale(&self, name: &str) -> V3 {
        let s = self.scale;
        if name == "Head" {
            [s[3]; 3]
        } else {
            [s[1], s[0], s[2]]
        }
    }

    fn rig_part(&self, name: &str) -> &RigPart {
        self.rig.parts.iter().find(|p| p.name == name).unwrap()
    }

    fn pkg_att(&self, part: &str, att: &str) -> Option<CF> {
        let a = self.pkg.get(part)?.att.get(att)?;
        let k = self.part_scale(part);
        Some(CF::new([a.p[0] * k[0], a.p[1] * k[1], a.p[2] * k[2]], a.r))
    }

    fn size_of(&self, name: &str) -> V3 {
        let k = self.part_scale(name);
        let sz = self.pkg.get(name).map(|p| p.size).unwrap_or(self.rig_part(name).size);
        [sz[0] * k[0], sz[1] * k[1], sz[2] * k[2]]
    }

    fn rest(&self, name: &str) -> CF {
        let s = self.scale;
        if name == "Head" {
            let h = self.rig_part("Head").cf;
            return CF::new([0.0, 4.0 * s[0] + (h.p[1] - 4.0) * s[3], 0.0], h.r);
        }
        let c = self.rig_part(name).cf;
        CF::new([c.p[0] * s[1], c.p[1] * s[0], c.p[2] * s[2]], c.r)
    }

    fn pose_from_asset(&self, ctx: &Ctx, notes: &mut Vec<String>) -> HashMap<String, CF> {
        let id = match &self.pose {
            Some(i) => i.clone(),
            None => return HashMap::new(),
        };
        let mut out = HashMap::new();
        if let Some(bytes) = ctx.fetch(&id) {
            if let Ok(tree) = swirl_core::rbx::parse(&bytes) {
                let mut best: Option<(f64, &Inst)> = None;
                let mut stack: Vec<&Inst> = tree.roots.iter().collect();
                while let Some(n) = stack.pop() {
                    if n.cls == "Keyframe" {
                        let t = n.props.get("Time").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        if best.is_none() || t < best.unwrap().0 {
                            best = Some((t, n));
                        }
                        continue;
                    }
                    for c in &n.children {
                        stack.push(c);
                    }
                }
                if let Some((_, kf)) = best {
                    let mut stack: Vec<&Inst> = kf.children.iter().collect();
                    while let Some(n) = stack.pop() {
                        if n.cls == "Pose" {
                            if JOINTS.iter().any(|j| j.0 == n.name) {
                                out.insert(n.name.clone(), cf_from(n.props.get("CFrame")));
                            }
                            for c in &n.children {
                                stack.push(c);
                            }
                        }
                    }
                }
            }
        }
        if out.is_empty() {
            notes.push("the pose animation could not be loaded, so the character stands straight".into());
        }
        out
    }

    pub fn compute_world(&mut self, ctx: &Ctx, tool: bool, notes: &mut Vec<String>) {
        let s = self.scale;
        let pi = std::f64::consts::PI;
        let angles: Option<Vec<(&str, f64)>> = match self.stance.as_str() {
            "walking" => Some(vec![("Right Arm", 0.6), ("Left Arm", -0.6), ("Right Leg", -0.6), ("Left Leg", 0.6)]),
            "sitting" => Some(vec![("Right Arm", 0.25), ("Left Arm", 0.25), ("Right Leg", pi / 2.0), ("Left Leg", pi / 2.0)]),
            "overlord" => Some(vec![("Right Arm", pi / 2.0)]),
            _ => None,
        };
        let mut pose: HashMap<String, CF> = HashMap::new();
        if let Some(a) = angles {
            for (part, ang) in a {
                pose.insert(stance_part(part).to_string(), CF::new([0.0; 3], rot_x(ang)));
            }
        } else {
            pose = self.pose_from_asset(ctx, notes);
            if self.straight_legs {
                for j in ["LeftUpperLeg", "RightUpperLeg", "LeftLowerLeg", "RightLowerLeg", "LeftFoot", "RightFoot"] {
                    pose.remove(j);
                }
            }
        }
        if tool {
            pose.insert("RightUpperArm".into(), CF::new([0.0; 3], [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0]));
            pose.insert("RightLowerArm".into(), CF::identity());
            pose.insert("RightHand".into(), CF::identity());
        }
        let mut motion: HashMap<String, CF> = HashMap::new();
        let mut world: HashMap<String, CF> = HashMap::new();
        for (name, parent, pivot) in JOINTS {
            let j = CF::at([pivot[0] * s[1], pivot[1] * s[0], pivot[2] * s[2]]);
            let t0 = pose.get(name).copied().unwrap_or(CF::identity());
            let t = CF::new([t0.p[0] * s[1], t0.p[1] * s[0], t0.p[2] * s[2]], t0.r);
            if self.pkg.is_empty() {
                let m = j.mul(&t).mul(&j.inv());
                let mo = match parent {
                    None => m,
                    Some(p) => motion[p].mul(&m),
                };
                motion.insert(name.to_string(), mo);
                world.insert(name.to_string(), mo.mul(&self.rest(name)));
                continue;
            }
            let jn = format!("{}RigAttachment", joint_name(name));
            let c1 = self.pkg_att(name, &jn).unwrap_or_else(|| self.rest(name).inv().mul(&j));
            match parent {
                None => {
                    world.insert(name.to_string(), j.mul(&t).mul(&c1.inv()));
                }
                Some(p) => {
                    let c0 = self.pkg_att(p, &jn).unwrap_or_else(|| self.rest(p).inv().mul(&j));
                    let w = world[p].mul(&c0).mul(&t).mul(&c1.inv());
                    world.insert(name.to_string(), w);
                }
            }
        }
        if !self.pkg.is_empty() {
            let mut want = f64::INFINITY;
            let mut got = f64::INFINITY;
            for f in ["LeftFoot", "RightFoot"] {
                want = want.min(self.rest(f).p[1] - self.rig_part(f).size[1] * self.part_scale(f)[1] / 2.0);
                got = got.min(world[f].p[1] - self.size_of(f)[1] / 2.0);
            }
            if want.is_finite() && got.is_finite() {
                let shift = CF::at([0.0, want - got, 0.0]);
                for (_, w) in world.iter_mut() {
                    *w = shift.mul(w);
                }
            }
        }
        self.world = world;
    }

    fn attachment(&self, att: &str) -> Option<CF> {
        let part = ATT_PARTS.iter().find(|a| a.0 == att).map(|a| a.1);
        if let Some(p) = part {
            if let Some(w) = self.world.get(p) {
                if let Some(own) = self.pkg_att(p, att) {
                    return Some(w.mul(&own));
                }
            }
        }
        let a = self.rig.atts.get(att)?;
        let part = part?;
        let pc = self.rig_part(part).cf;
        let k = self.part_scale(part);
        let o = [a.p[0] - pc.p[0], a.p[1] - pc.p[1], a.p[2] - pc.p[2]];
        let local = CF::new(rmul(&tr(&pc.r), [o[0] * k[0], o[1] * k[1], o[2] * k[2]]), mm(&tr(&pc.r), &a.r));
        Some(self.world.get(part)?.mul(&local))
    }

    pub fn accessory_scale(&self, h: &Inst) -> f64 {
        for a in &h.children {
            if a.cls == "Attachment" {
                if let Some(p) = ATT_PARTS.iter().find(|x| x.0 == a.name) {
                    let k = self.part_scale(p.1);
                    return (k[0] + k[1] + k[2]) / 3.0;
                }
            }
        }
        self.scale[3]
    }

    pub fn accessory_frame(&self, h: &Inst) -> Option<CF> {
        let k = self.accessory_scale(h);
        for a in &h.children {
            if a.cls == "Attachment" && ATT_PARTS.iter().any(|x| x.0 == a.name) {
                let f = match self.attachment(&a.name) {
                    Some(f) => f,
                    None => continue,
                };
                let l = cf_from(a.props.get("CFrame"));
                let l = CF::new([l.p[0] * k, l.p[1] * k, l.p[2] * k], l.r);
                return Some(f.mul(&l.inv()));
            }
        }
        None
    }

    pub fn tool_frame(&self, tool: &Inst) -> CF {
        let g = self
            .attachment("RightGripAttachment")
            .unwrap_or_else(|| self.world["RightHand"].mul(&CF::new([0.0, -0.15, 0.0], [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0])));
        g.mul(&crate::items::grip_cf(tool).inv())
    }

    pub fn apply_cfs(&self, cfs: &mut Cfs) {
        let w = &self.world;
        let s = self.scale;
        cfs.head = w["Head"];
        cfs.torso = w["UpperTorso"].mul(&CF::at([0.0, -0.2 * s[0], 0.0]));
        cfs.larm = w["LeftLowerArm"];
        cfs.rarm = w["RightLowerArm"];
        cfs.lleg = w["LeftLowerLeg"];
        cfs.rleg = w["RightLowerLeg"];
    }

    pub fn scale_item(&self, it: &mut Item, k: f64) {
        if (k - 1.0).abs() < 1e-6 {
            return;
        }
        for p in it.geom.pos.iter_mut() {
            *p = [p[0] * k, p[1] * k, p[2] * k];
        }
        it.size = [it.size[0] * k, it.size[1] * k, it.size[2] * k];
    }

    pub fn mesh_scaled(&self, name: &str, mesh: &Mesh) -> Mesh {
        let k = self.part_scale(name);
        if (k[0] - 1.0).abs() + (k[1] - 1.0).abs() + (k[2] - 1.0).abs() < 1e-6 {
            return mesh.clone();
        }
        scale_mesh(mesh, k)
    }

    fn package_mesh(&self, ctx: &Ctx, name: &str) -> Option<(Mesh, Option<Rc<Texture>>, Option<Img>)> {
        let pk = self.pkg.get(name)?;
        let mid = pk.mesh.as_ref().and_then(|u| asset_id(u))?;
        let tid = pk.tex.as_ref().and_then(|u| asset_id(u));
        let bytes = ctx.fetch(&mid)?;
        let parsed = parse_mesh(&bytes);
        if let Err(e) = &parsed {
            if e.contains("not ported") {
                ctx.unsupported.set(true);
            }
        }
        let mesh = fit_mesh(&parsed.ok()?, self.size_of(name));
        let mut texture = None;
        let img = tid.and_then(|t| ctx.image(&format!("rbxassetid://{}", t), 0));
        if let Some(i) = &img {
            let mut t = Texture::new(i.clone(), false);
            t.repeat = false;
            texture = Some(Rc::new(t));
        }
        Some((mesh, texture, img))
    }

    pub fn body_calls(&self, ctx: &Ctx, look: &Look) -> Vec<(Mesh, CF, Rc<Texture>)> {
        let groups: [(&str, &str, usize, usize, [bool; 2]); 5] = [
            ("torso", "R15CompositTorsoBase", 388, 272, [true, true]),
            ("leftArm", "R15CompositLeftArmBase", 264, 284, [false, true]),
            ("rightArm", "R15CompositRightArmBase", 264, 284, [false, true]),
            ("leftLeg", "R15CompositLeftArmBase", 264, 284, [true, false]),
            ("rightLeg", "R15CompositRightArmBase", 264, 284, [true, false]),
        ];
        let slot = |g: &str| match g {
            "torso" => TORSO,
            "leftArm" => LARM,
            "rightArm" => RARM,
            "leftLeg" => LLEG,
            _ => RLEG,
        };
        let mut tex: HashMap<&str, Rc<Texture>> = HashMap::new();
        let mut imgs: HashMap<&str, Img> = HashMap::new();
        for (g, mesh, w, h, layers) in groups {
            let c = look.colors[slot(g)];
            let mut im = Img::new(w, h);
            im.fill_rect(0, 0, w as i64, h as i64, [(c[0] * 255.0).round() as u32, (c[1] * 255.0).round() as u32, (c[2] * 255.0).round() as u32]);
            if layers[0] {
                if let Some(p) = &look.pants {
                    composite_layer_with(ctx, &mut im, mesh, p, [1.0; 3], 1.0, h as f64);
                }
            }
            if layers[1] {
                if let Some(s) = &look.shirt {
                    composite_layer_with(ctx, &mut im, mesh, s, [1.0; 3], 1.0, h as f64);
                }
            }
            let mut t = Texture::new(im.clone(), false);
            t.repeat = false;
            tex.insert(g, Rc::new(t));
            imgs.insert(g, im);
        }
        let mut calls = vec![];
        for p in &self.rig.parts {
            let g = match GROUPS.iter().find(|x| x.0 == p.name) {
                Some(g) => g.1,
                None => continue,
            };
            if let Ok(o) = std::env::var("SWIRL_ONLY") {
                if o != p.name {
                    continue;
                }
            }
            let cf = self.world.get(&p.name).copied().unwrap_or(p.cf);
            if self.pkg.contains_key(&p.name) {
                let pm = self.package_mesh(ctx, &p.name);
                if std::env::var("SWIRL_DEBUG").is_ok() {
                    eprintln!("{} g={} pkgmesh={} tex={}", p.name, g, pm.is_some(), pm.as_ref().map(|x| x.1.is_some()).unwrap_or(false));
                }
                if let Some((mesh, ptex, pimg)) = pm {
                    let mut skin = ptex.clone().unwrap_or_else(|| tex[g].clone());
                    if ptex.is_some() {
                        if let Some(pi) = &pimg {
                            if let Some(l) = layer_over(&imgs[g], pi) {
                                skin = Rc::new(l);
                            }
                        }
                    }
                    calls.push((mesh, cf, skin));
                    continue;
                }
            }
            if let Some(m) = &p.mesh {
                calls.push((self.mesh_scaled(&p.name, m), cf, tex[g].clone()));
            }
        }
        calls
    }
}

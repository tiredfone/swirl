use crate::img::Img;
use crate::look::Ctx;
use crate::mesh::*;
use crate::scene::*;
use crate::texture::Texture;
use std::rc::Rc;
use swirl_core::math::*;
use swirl_core::rbx::{Inst, Value};

const PART_CLASSES: [&str; 4] = ["Part", "MeshPart", "UnionOperation", "WedgePart"];

pub fn cf_from(v: Option<&Value>) -> CF {
    match v {
        Some(Value::CFrame(p, m)) => CF::new(*p, *m),
        _ => CF::identity(),
    }
}

fn v3_or(v: Option<&Value>, d: V3) -> V3 {
    v.and_then(|v| v.as_v3()).unwrap_or(d)
}

pub fn handle_of(x: &Inst) -> Option<&Inst> {
    let parts: Vec<&Inst> = x.children.iter().filter(|c| PART_CLASSES.contains(&c.cls.as_str())).collect();
    for c in &parts {
        if c.name == "Handle" {
            return Some(c);
        }
    }
    parts.first().copied()
}

pub fn part_size(h: &Inst) -> V3 {
    let p = &h.props;
    v3_or(p.get("size").or(p.get("Size")).or(p.get("size3")).or(p.get("Size3")), [1.0, 1.0, 1.0])
}

pub fn part_color(h: &Inst) -> [f64; 3] {
    let p = &h.props;
    if let Some(Value::Color3(r, g, b)) = p.get("Color3uint8").or(p.get("Color")) {
        return [*r, *g, *b];
    }
    if let Some(b) = p.get("BrickColor") {
        return match b {
            Value::Brick(n) => brick(*n),
            Value::Num(n) => brick(*n as i64),
            _ => brick(194),
        };
    }
    brick(194)
}

fn body_att(part: &str, key: &str) -> Option<V3> {
    Some(match (part, key) {
        ("Head", "hat") | ("Head", "hair") => [0.0, 0.6, 0.0],
        ("Head", "facefront") => [0.0, 0.0, -0.6],
        ("Head", "facecenter") => [0.0, 0.0, 0.0],
        ("Torso", "neck") => [0.0, 1.0, 0.0],
        ("Torso", "bodyfront") => [0.0, 0.0, -0.5],
        ("Torso", "bodyback") => [0.0, 0.0, 0.5],
        ("Torso", "leftcollar") => [-1.0, 1.0, 0.0],
        ("Torso", "rightcollar") => [1.0, 1.0, 0.0],
        ("Torso", "waistfront") => [0.0, -1.0, -0.5],
        ("Torso", "waistcenter") => [0.0, -1.0, 0.0],
        ("Torso", "waistback") => [0.0, -1.0, 0.5],
        ("Left Arm", "leftshoulder") | ("Right Arm", "rightshoulder") => [0.0, 1.0, 0.0],
        ("Left Arm", "leftgrip") | ("Right Arm", "rightgrip") => [0.0, -1.0, 0.0],
        ("Left Leg", "leftfoot") | ("Right Leg", "rightfoot") => [0.0, -1.0, 0.0],
        _ => return None,
    })
}

const PARTS: [&str; 6] = ["Head", "Torso", "Left Arm", "Right Arm", "Left Leg", "Right Leg"];

fn attachment_for(name: &str) -> Option<(&'static str, String)> {
    let mut n = name.to_lowercase();
    if n.ends_with("attachment") {
        n.truncate(n.len() - 10);
    }
    let n: String = n.chars().filter(|c| c.is_ascii_lowercase()).collect();
    for p in PARTS {
        if body_att(p, &n).is_some() {
            return Some((p, n));
        }
    }
    None
}

fn acc_type_att(t: i64) -> Option<(&'static str, &'static str)> {
    Some(match t {
        1 => ("Head", "hat"),
        2 => ("Head", "hair"),
        3 => ("Head", "facefront"),
        4 => ("Torso", "neck"),
        5 => ("Left Arm", "leftshoulder"),
        6 => ("Torso", "bodyfront"),
        7 => ("Torso", "bodyback"),
        8 => ("Torso", "waistcenter"),
        15 => ("Left Leg", "leftfoot"),
        16 => ("Right Leg", "rightfoot"),
        18 | 19 => ("Head", "facefront"),
        _ => return None,
    })
}

fn part_cf<'a>(cfs: &'a Cfs, name: &str) -> &'a CF {
    match name {
        "Head" => &cfs.head,
        "Torso" => &cfs.torso,
        "Left Arm" => &cfs.larm,
        "Right Arm" => &cfs.rarm,
        "Left Leg" => &cfs.lleg,
        _ => &cfs.rleg,
    }
}

pub fn accessory_cf(acc: &Inst, h: &Inst, cfs: &Cfs) -> CF {
    let atts: Vec<&Inst> = h.children.iter().filter(|c| c.cls == "Attachment").collect();
    let mut hit: Option<(&Inst, String, String)> = None;
    for a in &atts {
        if let Some((p, k)) = attachment_for(&a.name) {
            hit = Some((a, p.to_string(), k));
            break;
        }
    }
    let at = acc.props.get("AccessoryType").and_then(|v| v.as_f64()).unwrap_or(0.0) as i64;
    if hit.is_none() && !atts.is_empty() {
        if let Some((p, k)) = acc_type_att(at) {
            hit = Some((atts[0], p.to_string(), k.to_string()));
        }
    }
    if let Some((a, p, k)) = hit {
        let body = part_cf(cfs, &p).mul(&CF::at(body_att(&p, &k).unwrap()));
        return body.mul(&cf_from(a.props.get("CFrame")).inv());
    }
    let ap = cf_from(acc.props.get("AttachmentPoint"));
    cfs.head.mul(&CF::at([0.0, 0.5, 0.0])).mul(&ap.inv())
}

pub fn grip_cf(tool: &Inst) -> CF {
    let p = &tool.props;
    if let Some(Value::CFrame(..)) = p.get("Grip") {
        return cf_from(p.get("Grip"));
    }
    let r = v3_or(p.get("GripRight"), [1.0, 0.0, 0.0]);
    let u = v3_or(p.get("GripUp"), [0.0, 1.0, 0.0]);
    let f = v3_or(p.get("GripForward"), [0.0, 0.0, -1.0]);
    let pos = v3_or(p.get("GripPos"), [0.0, 0.0, 0.0]);
    CF::new(pos, [r[0], u[0], -f[0], r[1], u[1], -f[1], r[2], u[2], -f[2]])
}

pub fn right_grip_c0() -> CF {
    CF::new([0.0, -1.0, 0.0], [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0])
}

pub fn welded_parts<'a>(root: &'a Inst, handle: &'a Inst, cf: CF) -> Vec<(&'a Inst, CF)> {
    let mut parts: Vec<(String, &Inst)> = vec![];
    let mut joints: Vec<&Inst> = vec![];
    fn walk<'b>(x: &'b Inst, parts: &mut Vec<(String, &'b Inst)>, joints: &mut Vec<&'b Inst>) {
        for c in &x.children {
            if PART_CLASSES.contains(&c.cls.as_str()) {
                parts.push((c.referent.clone().unwrap_or_default(), c));
            } else if matches!(c.cls.as_str(), "Weld" | "ManualWeld" | "Snap" | "Glue" | "Motor" | "Motor6D") {
                joints.push(c);
            }
            walk(c, parts, joints);
        }
    }
    walk(root, &mut parts, &mut joints);
    let find = |r: &str| parts.iter().find(|(k, _)| k == r).map(|(_, i)| *i);
    let mut placed: Vec<(String, CF)> = vec![(handle.referent.clone().unwrap_or_default(), cf)];
    let mut out: Vec<(&Inst, CF)> = vec![];
    let refof = |j: &Inst, k: &str| -> String {
        match j.props.get(k) {
            Some(Value::Ref(r)) => r.clone(),
            _ => String::new(),
        }
    };
    let mut grew = true;
    while grew && out.len() < 8 {
        grew = false;
        for j in &joints {
            let a = refof(j, "Part0");
            let b = refof(j, "Part1");
            let (pa, pb) = (find(&a), find(&b));
            if pa.is_none() || pb.is_none() {
                continue;
            }
            let ia = placed.iter().position(|(k, _)| *k == a);
            let ib = placed.iter().position(|(k, _)| *k == b);
            if ia.is_some() == ib.is_some() {
                continue;
            }
            let c0 = cf_from(j.props.get("C0"));
            let c1 = cf_from(j.props.get("C1"));
            if let Some(ia) = ia {
                let n = placed[ia].1.mul(&c0).mul(&c1.inv());
                placed.push((b, n));
                out.push((pb.unwrap(), n));
            } else {
                let n = placed[ib.unwrap()].1.mul(&c1).mul(&c0.inv());
                placed.push((a, n));
                out.push((pa.unwrap(), n));
            }
            grew = true;
            if out.len() >= 8 {
                break;
            }
        }
    }
    out
}

fn str_eq_ci(v: &Value, s: &str) -> bool {
    matches!(v, Value::Str(x) if x.eq_ignore_ascii_case(s))
}

fn php_cast_int(v: Option<&Value>, d: i64) -> i64 {
    v.and_then(|v| v.as_f64()).map(|f| f as i64).unwrap_or(d)
}

pub fn mesh_bytes(ctx: &Ctx, r: &str) -> Option<Vec<u8>> {
    if r.to_ascii_lowercase().starts_with("rbxasset://") {
        let rel = &r[11..];
        if !rel.is_empty() && rel.bytes().all(|b| b.is_ascii_digit()) {
            return ctx.fetch(rel);
        }
        let map = [
            ("fonts/head.mesh", "head.mesh"),
            ("fonts/torso.mesh", "torso.mesh"),
            ("fonts/leftarm.mesh", "leftarm.mesh"),
            ("fonts/rightarm.mesh", "rightarm.mesh"),
            ("fonts/leftleg.mesh", "leftleg.mesh"),
            ("fonts/rightleg.mesh", "rightleg.mesh"),
            ("textures/face.png", "face.png"),
        ];
        let low = rel.to_lowercase();
        for (k, f) in map {
            if k == low {
                return std::fs::read(format!("{}/{}", ctx.data, f)).ok();
            }
        }
        return None;
    }
    let id = crate::look::asset_id(r)?;
    ctx.fetch(&id)
}

pub fn build_handle(ctx: &Ctx, base: &Base, h: &Inst, cf: CF, notes: &mut Vec<String>) -> Option<Item> {
    let sm = h.children.iter().find(|c| matches!(c.cls.as_str(), "SpecialMesh" | "FileMesh" | "BlockMesh"));
    let size = part_size(h);
    let color = part_color(h);
    let tr = h.props.get("Transparency").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let alpha = (1.0 - tr).clamp(0.0, 1.0);
    let mut scale = [1.0; 3];
    let mut off = [0.0; 3];
    let mut tint = [1.0; 3];
    let mut fit_to: Option<V3> = None;
    let mut mesh_type: Option<Value> = None;
    if let Some(s) = sm {
        scale = v3_or(s.props.get("Scale"), [1.0; 3]);
        off = v3_or(s.props.get("Offset"), [0.0; 3]);
        if let Some(v) = s.props.get("VertexColor").and_then(|v| v.as_v3()) {
            tint = v;
        }
        mesh_type = s.props.get("MeshType").cloned();
        if s.cls == "BlockMesh" {
            mesh_type = Some(Value::Str("block".into()));
        }
    }
    let mut mesh_url: Option<String> = sm.and_then(|s| s.url("MeshId")).map(|s| s.to_string());
    let mut tex_url: Option<String> = sm.and_then(|s| s.url("TextureId")).map(|s| s.to_string());
    if h.cls == "MeshPart" {
        mesh_url = h.url("MeshId").or(h.url("MeshID")).map(|s| s.to_string());
        tex_url = h.url("TextureID").or(h.url("TextureId")).map(|s| s.to_string());
        fit_to = Some(size);
        for c in &h.children {
            if c.cls == "SurfaceAppearance" {
                if let Some(u) = c.url("ColorMap") {
                    if !u.is_empty() {
                        tex_url = Some(u.to_string());
                    }
                }
                break;
            }
        }
    }
    let mesh_url = mesh_url.filter(|s| !s.is_empty());
    let tex_url = tex_url.filter(|s| !s.is_empty());
    let type_num = mesh_type.as_ref().and_then(|v| match v {
        Value::Num(n) => Some(*n as i64),
        _ => None,
    });
    let type_is = |s: &str| mesh_type.as_ref().map(|v| str_eq_ci(v, s)).unwrap_or(false);
    let shape = php_cast_int(h.props.get("shape").or(h.props.get("Shape")), 1);
    let is_head_type = type_num == Some(0) || type_is("Head");
    let fail = |notes: &mut Vec<String>| {
        notes.push(format!("an item mesh could not be loaded{}", mesh_url.as_ref().map(|u| format!(" ({})", &u[..u.len().min(60)])).unwrap_or_default()));
    };
    let m: Mesh;
    if mesh_url.is_some() && (mesh_type.is_none() || type_num == Some(5) || type_is("FileMesh") || h.cls == "MeshPart") {
        let b = mesh_bytes(ctx, mesh_url.as_ref().unwrap());
        let parsed = b.ok_or_else(|| "a mesh could not be fetched".to_string()).and_then(|b| parse_mesh2016(&b));
        match parsed {
            Ok(p) => m = clean_mesh(&p),
            Err(_) => {
                fail(notes);
                return None;
            }
        }
    } else if is_head_type {
        m = base.head_raw.clone();
        let k = (size[0] / 2.0).min(size[1]).min(size[2]) / 1.25;
        scale = [scale[0] * k, scale[1] * k, scale[2] * k];
    } else if type_num == Some(3) || type_is("Sphere") {
        m = sphere_mesh(size[0] / 2.0, size[1] / 2.0, size[2] / 2.0);
    } else if sm.is_none() && h.cls == "Part" && shape == 0 {
        let r = size[0].min(size[1]).min(size[2]) / 2.0;
        m = sphere_mesh(r, r, r);
    } else if sm.is_none() && h.cls == "Part" && shape == 2 {
        let r = size[1].min(size[2]) / 2.0;
        m = scene_cylinder(size[0] / 2.0, r, r, false);
    } else {
        m = plain_box(size[0] / 2.0, size[1] / 2.0, size[2] / 2.0);
    }
    let tex: Option<Img> = tex_url.as_ref().and_then(|u| ctx.image_big(u, 0, true));
    if let Some(f) = fit_to {
        let (lo, hi) = mesh_bounds(&m);
        let mut sc = [1.0; 3];
        let mut of = [0.0; 3];
        for k in 0..3 {
            let e = hi[k] - lo[k];
            sc[k] = if e > 1e-6 { f[k] / e } else { 1.0 };
            of[k] = -(hi[k] + lo[k]) / 2.0 * sc[k];
        }
        scale = sc;
        off = of;
    }
    let s = scale;
    let o = off;
    let mut pos: Vec<V3> = m.pos.iter().map(|p| [p[0] * s[0] + o[0], p[1] * s[1] + o[1], p[2] * s[2] + o[2]]).collect();
    let mut nrm: Vec<V3> = m
        .nrm
        .iter()
        .map(|n| norm([if s[0] != 0.0 { n[0] / s[0] } else { 0.0 }, if s[1] != 0.0 { n[1] / s[1] } else { 0.0 }, if s[2] != 0.0 { n[2] / s[2] } else { 0.0 }]))
        .collect();
    let mut uv = m.uv.clone();
    if s[0] * s[1] * s[2] < 0.0 {
        let mut t = 0;
        while t + 2 < pos.len() {
            pos.swap(t + 1, t + 2);
            nrm.swap(t + 1, t + 2);
            uv.swap(t + 1, t + 2);
            t += 3;
        }
    }
    let mut item_tex: Option<Rc<Texture>> = None;
    if let Some(t) = tex {
        let cut = alpha_cuts_out(&t, &uv);
        let mut tx = Texture::new(t, true);
        tx.cut = cut;
        item_tex = Some(Rc::new(tx));
    }
    Some(Item { thumb_cam: None, geom: Mesh { pos, nrm, uv }, cf, size, tex: item_tex, color, tint, alpha })
}

pub fn build_decals(ctx: &Ctx, h: &Inst, cf: CF) -> Vec<Item> {
    let mut out = vec![];
    let mut size = part_size(h);
    let mut off = [0.0; 3];
    for c in &h.children {
        if !matches!(c.cls.as_str(), "SpecialMesh" | "FileMesh" | "BlockMesh" | "CylinderMesh") {
            continue;
        }
        let mt = c.props.get("MeshType");
        let brick = c.cls == "BlockMesh"
            || (c.cls == "SpecialMesh" && (matches!(mt, Some(Value::Num(n)) if *n as i64 == 6) || matches!(mt, Some(Value::Str(s)) if s == "Brick")));
        if !brick {
            return vec![];
        }
        let sc = v3_or(c.props.get("Scale"), [1.0; 3]);
        off = v3_or(c.props.get("Offset"), [0.0; 3]);
        size = [size[0] * sc[0], size[1] * sc[1], size[2] * sc[2]];
        break;
    }
    if h.cls != "Part" || php_cast_int(h.props.get("shape").or(h.props.get("Shape")), 1) != 1 {
        return vec![];
    }
    for c in &h.children {
        if c.cls != "Decal" {
            continue;
        }
        let url = c.url("Texture").unwrap_or("");
        let dt = c.props.get("Transparency").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let alpha = (1.0 - dt).clamp(0.0, 1.0);
        if url.is_empty() || alpha < 0.05 {
            continue;
        }
        let img = match ctx.image_big(url, 0, true) {
            Some(i) => i,
            None => continue,
        };
        let face = match c.props.get("Face") {
            Some(Value::Str(s)) => match s.to_lowercase().as_str() {
                "right" => 0,
                "top" => 1,
                "back" => 2,
                "left" => 3,
                "bottom" => 4,
                _ => 5,
            },
            Some(v) => v.as_f64().map(|f| f as i64).unwrap_or(5),
            None => 5,
        };
        let mut geom = scene_decal(size, face);
        if off != [0.0; 3] {
            for gp in geom.pos.iter_mut() {
                *gp = [gp[0] + off[0], gp[1] + off[1], gp[2] + off[2]];
            }
        }
        let cut = alpha_cuts_out(&img, &geom.uv);
        let mut tx = Texture::new(img, true);
        tx.cut = cut;
        out.push(Item { thumb_cam: None, geom, cf, size, tex: Some(Rc::new(tx)), color: [1.0; 3], tint: [1.0; 3], alpha });
    }
    out
}

pub struct BuildOut {
    pub cfs: Cfs,
    pub base: Base,
    pub tex: crate::body::BodyTex,
    pub r15: Option<R15Draw>,
}

pub struct BuildOpts<'a> {
    pub view: &'a str,
    pub stance: &'a str,
    pub rig: Option<&'a crate::r15::Rig>,
    pub scale: [f64; 4],
    pub pose: Option<String>,
    pub item: bool,
    pub keep_pkg_colors: bool,
}

pub fn build_look(ctx: &Ctx, base: &Base, look: &mut Look, ids: &[String], o: &BuildOpts) -> Result<BuildOut, String> {
    let (view, stance) = (o.view, o.stance);
    let mut face_set = false;
    let mut accs: Vec<Inst> = vec![];
    let mut tool: Option<Inst> = None;
    for id in ids {
        crate::look::add_asset(ctx, look, id, &mut face_set, &mut accs, &mut tool);
    }
    look.face_set = face_set;
    accs.truncate(8);
    if o.item {
        if let Some(c) = look.pkg_colors {
            look.colors = c;
        }
        let body = look.body_meshes.iter().any(|b| b.is_some()) || look.head_mesh.is_some();
        let clothes = look.shirt.is_some() || look.pants.is_some() || look.tshirt.is_some();
        let worn = !accs.is_empty() || tool.is_some();
        if worn && !body && !clothes && !face_set {
            look.only = "items".into();
        } else if !worn && !clothes && (look.head_mesh.is_some() || face_set) && !look.body_meshes.iter().any(|b| b.is_some()) {
            look.only = "head".into();
        } else if clothes && !body && !worn && !face_set {
            for k in 0..6 {
                look.colors[k] = brick(1);
            }
        } else if !worn && !clothes && !body && !face_set {
            return Err(format!("item is not a hat, gear, face, head, shirt, pants, t-shirt or package{}", look.notes.first().map(|n| format!(" ({})", n)).unwrap_or_default()));
        }
        if look.only.is_empty() && body && !clothes {
            look.pkgview = true;
        }
        if !o.keep_pkg_colors {
            look.pkg_colors = None;
        }
    }
    if view == "head" {
        tool = None;
    }
    look.tool = tool.is_some();
    let mut cfs = part_cfs(look.tool, std::f64::consts::PI / 2.0);
    stance_r6(&mut cfs, look.tool, stance);
    let mut notes = std::mem::take(&mut look.notes);
    let mut r15 = o.rig.map(|rig| crate::r15::R15::new(rig, o.scale, stance, o.pose.clone()));
    if let Some(r) = r15.as_mut() {
        r.use_package(&look.r15_parts);
        r.compute_world(ctx, look.tool, &mut notes);
        r.apply_cfs(&mut cfs);
        if std::env::var("SWIRL_DEBUG").is_ok() {
            for n in r.world.keys().cloned().collect::<Vec<_>>() {
                eprintln!("{} {:?}", n, r.world[&n].p);
            }
        }
    }
    let mut items: Vec<Item> = vec![];
    for acc in &accs {
        match handle_of(acc) {
            None => notes.push("a hat has no Handle part so it was skipped".into()),
            Some(h) => {
                let cf = r15.as_ref().and_then(|r| r.accessory_frame(h)).unwrap_or_else(|| accessory_cf(acc, h, &cfs));
                if let Some(mut it) = build_handle(ctx, base, h, cf, &mut notes) {
                    it.thumb_cam = thumb_cam_of(acc, h);
                    let k = r15.as_ref().map(|r| r.accessory_scale(h));
                    if let (Some(r), Some(k)) = (r15.as_ref(), k) {
                        r.scale_item(&mut it, k);
                    }
                    items.push(it);
                    for mut d in build_decals(ctx, h, cf) {
                        if let (Some(r), Some(k)) = (r15.as_ref(), k) {
                            r.scale_item(&mut d, k);
                        }
                        items.push(d);
                    }
                }
            }
        }
    }
    look.n_hats = items.len();
    if let Some(t) = &tool {
        match handle_of(t) {
            None => notes.push("the gear has no Handle part so it was skipped".into()),
            Some(th) => {
                let cf = match r15.as_ref() {
                    Some(r) => r.tool_frame(t),
                    None => cfs.rarm.mul(&right_grip_c0()).mul(&grip_cf(t).inv()),
                };
                if let Some(it) = build_handle(ctx, base, th, cf, &mut notes) {
                    let icf = it.cf;
                    items.push(it);
                    items.extend(build_decals(ctx, th, icf));
                    for (wp, wcf) in welded_parts(t, th, icf) {
                        if let Some(wi) = build_handle(ctx, base, wp, wcf, &mut notes) {
                            items.push(wi);
                            items.extend(build_decals(ctx, wp, wcf));
                        }
                    }
                }
            }
        }
    }
    look.items = items;
    look.notes = notes;
    let (nb, bt) = crate::body::character_base_2012(ctx, base, look);
    let r15d = r15.as_ref().map(|r| R15Draw { calls: r.body_calls(ctx, look), head: r.mesh_scaled("Head", &nb.head) });
    if look.only == "items" && look.items.is_empty() {
        return Err(format!("the item could not be drawn{}", look.notes.first().map(|n| format!(" ({})", n)).unwrap_or_default()));
    }
    Ok(BuildOut { cfs, base: nb, tex: bt, r15: r15d })
}

pub fn stance_r6(cfs: &mut Cfs, tool: bool, stance: &str) {
    let pi = std::f64::consts::PI;
    let angles: Vec<(&str, f64)> = match stance {
        "walking" => vec![("Right Arm", 0.6), ("Left Arm", -0.6), ("Right Leg", -0.6), ("Left Leg", 0.6)],
        "sitting" => vec![("Right Arm", 0.25), ("Left Arm", 0.25), ("Right Leg", pi / 2.0), ("Left Leg", pi / 2.0)],
        "overlord" => vec![("Right Arm", pi / 2.0)],
        _ => return,
    };
    for (part, a) in angles {
        if tool && part == "Right Arm" {
            continue;
        }
        let p = if part.ends_with("Arm") { [0.0, 3.5, 0.0] } else { [0.0, 2.0, 0.0] };
        let cur = *part_cf(cfs, part);
        let m = CF::new(p, rot_x(a)).mul(&CF::at([-p[0], -p[1], -p[2]])).mul(&cur);
        match part {
            "Right Arm" => cfs.rarm = m,
            "Left Arm" => cfs.larm = m,
            "Right Leg" => cfs.rleg = m,
            _ => cfs.lleg = m,
        }
    }
}

pub fn mesh_bytes_pub(ctx: &Ctx, r: &str) -> Option<Vec<u8>> {
    mesh_bytes(ctx, r)
}

pub fn thumb_cam_of(acc: &Inst, h: &Inst) -> Option<ThumbCam> {
    h.props.get("CFrame")?;
    for c in &acc.children {
        if c.cls != "Camera" || !c.props.contains_key("CFrame") {
            continue;
        }
        let inv = cf_from(h.props.get("CFrame")).inv();
        let cam = inv.mul(&cf_from(c.props.get("CFrame")));
        let fp = inv.apply(cf_from(c.props.get("Focus").or(c.props.get("CFrame"))).p);
        let d = sub(cam.p, fp);
        if dot(d, d) < 1e-6 {
            return None;
        }
        let fov = c.props.get("FieldOfView").and_then(|v| v.as_f64()).unwrap_or(70.0);
        let fov = if fov == 0.0 { 70.0 } else { fov };
        return Some(ThumbCam { eye: cam.p, up: [cam.r[1], cam.r[4], cam.r[7]], focus: fp, fov: fov.clamp(15.0, 90.0) });
    }
    None
}

pub fn package_ids(body: &[u8]) -> Vec<String> {
    let s = String::from_utf8_lossy(body);
    let t = s.trim();
    if t.is_empty() || t.len() > 4096 || t.starts_with('<') || !t.chars().all(|c| (' '..='~').contains(&c) || c.is_whitespace()) {
        return vec![];
    }
    let mut out: Vec<String> = vec![];
    for piece in t.split(|c: char| c == ';' || c == ',' || c == '|' || c.is_whitespace()) {
        if let Some(id) = crate::look::asset_id(piece) {
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out.truncate(16);
    out
}

use crate::img::Img;
use crate::items::mesh_bytes;
use crate::look::{asset_id, Ctx};
use crate::mesh::*;
use crate::scene::*;
use crate::texture::Texture;
use std::rc::Rc;
use swirl_core::math::*;
use swirl_core::rbx::{Inst, Value};

pub struct BodyTex {
    pub parts: [Option<Rc<Texture>>; 5],
    pub head: Option<Rc<Texture>>,
}

pub fn mesh_content(v: Option<&Value>) -> Option<String> {
    let s = match v? {
        Value::Url(u) => u.clone(),
        Value::Str(u) => u.clone(),
        Value::Num(n) if n.is_finite() && *n == n.floor() => format!("{:.0}", n),
        _ => return None,
    };
    if s.is_empty() || s == "0" {
        None
    } else {
        Some(s)
    }
}

pub fn character_mesh(ctx: &Ctx, name: &str) -> Result<Mesh, String> {
    let b = std::fs::read(format!("{}/2016/character/{}.mesh", ctx.data, name)).map_err(|e| e.to_string())?;
    parse_mesh2016(&b)
}

pub fn head_variant(shape: &Inst) -> Option<String> {
    if shape.cls != "CylinderMesh" && shape.cls != "BlockMesh" {
        return None;
    }
    let g = |k: &str| shape.props.get(k).and_then(|v| v.as_f64());
    let b = g("Bevel").unwrap_or(0.0);
    let r = g("Bevel Roundness").or(g("Roundness")).unwrap_or(0.0);
    let u = g("Bulge").unwrap_or(0.0);
    let variants: &[(&str, [f64; 3])] = if shape.cls == "CylinderMesh" {
        &[("A", [0.0, 0.0, 0.5]), ("C", [0.1, 0.0, 0.5]), ("D", [0.4, 0.0, 0.0]), ("E", [0.66, 0.0, 0.5]), ("F", [0.0, 0.0, 0.0]), ("G", [0.0, 0.0, 1.0]), ("H", [0.2, 0.0, 1.0]), ("N", [0.4, 1.0, 0.0]), ("O", [0.2, 0.0, 0.5]), ("P", [0.1, 0.0, 0.0])]
    } else {
        &[("B", [0.0, 0.0, 0.0]), ("I", [0.5, 0.0, 0.0]), ("J", [0.3, 0.0, 0.0]), ("M", [0.05, 0.0, 0.0])]
    };
    for (s, v) in variants {
        if (b - v[0]).abs().max((r - v[1]).abs()).max((u - v[2]).abs()) < 1e-5 {
            return Some(format!("head{}", s));
        }
    }
    None
}

fn neck_rig(mesh: &Inst) -> Option<V3> {
    for c in &mesh.children {
        if c.name != "NeckRigAttachment" {
            continue;
        }
        let v = c.props.get("Value").or(c.props.get("CFrame")).or(c.props.get("Position"));
        return match v {
            Some(Value::V3(x, y, z)) => Some([*x, *y, *z]),
            Some(Value::CFrame(p, _)) => Some(*p),
            _ => None,
        };
    }
    None
}

fn mesh_from(ctx: &Ctx, v: Option<&Value>) -> Option<Mesh> {
    let c = mesh_content(v)?;
    let b = mesh_bytes(ctx, &c)?;
    parse_mesh2016(&b).ok().map(|m| clean_mesh(&m))
}

fn composite_layer(ctx: &Ctx, out: &mut Img, name: &str, image: &Img, color: [f64; 3]) {
    let mesh = match character_mesh(ctx, name) {
        Ok(m) => m,
        Err(_) => return,
    };
    let mut tex = Texture::new(image.clone(), false);
    tex.repeat = false;
    let sx = 0.75;
    let flip_h = 512.0;
    let mut visited = vec![false; out.w * out.h];
    let n = mesh.pos.len();
    let mut i = 0;
    while i + 2 < n {
        let p: Vec<[f64; 2]> = (0..3).map(|j| [mesh.pos[i + j][0] * sx, flip_h - mesh.pos[i + j][1]]).collect();
        let (ax, ay, bx, by, cx, cy) = (p[0][0], p[0][1], p[1][0], p[1][1], p[2][0], p[2][1]);
        let den = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
        if den.abs() < 1e-10 {
            i += 3;
            continue;
        }
        let x0 = ((ax.min(bx).min(cx) - 0.5).ceil() as i64).max(0);
        let x1 = ((ax.max(bx).max(cx) - 0.5).floor() as i64).min(out.w as i64 - 1);
        let y0 = ((ay.min(by).min(cy) - 0.5).ceil() as i64).max(0);
        let y1 = ((ay.max(by).max(cy) - 0.5).floor() as i64).min(out.h as i64 - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let index = y as usize * out.w + x as usize;
                if visited[index] {
                    continue;
                }
                let xf = x as f64 + 0.5;
                let yf = y as f64 + 0.5;
                let a = ((by - cy) * (xf - cx) + (cx - bx) * (yf - cy)) / den;
                let b = ((cy - ay) * (xf - cx) + (ax - cx) * (yf - cy)) / den;
                let c = 1.0 - a - b;
                if a.min(b).min(c) < -1e-7 {
                    continue;
                }
                visited[index] = true;
                let (u0, u1, u2) = (mesh.uv[i], mesh.uv[i + 1], mesh.uv[i + 2]);
                let t = tex.sample0(a * u0[0] + b * u1[0] + c * u2[0], a * u0[1] + b * u1[1] + c * u2[1]);
                if t[3] <= 0.0 {
                    continue;
                }
                let dst = out.px[index];
                let da = ((dst >> 24) & 255) as f64 / 255.0;
                let alpha = t[3] + da * (1.0 - t[3]);
                let mut value = ((alpha * 255.0).round() as u32) << 24;
                for (ch, sh) in [16u32, 8, 0].iter().enumerate() {
                    let v = (t[ch] * color[ch] * t[3] + ((dst >> sh) & 255) as f64 / 255.0 * da * (1.0 - t[3])) / alpha;
                    value |= ((v * 255.0).round().clamp(0.0, 255.0) as u32) << sh;
                }
                out.px[index] = value;
            }
        }
        i += 3;
    }
}

fn clothes_atlas(ctx: &Ctx, look: &Look) -> Option<Img> {
    if look.pants.is_none() && look.shirt.is_none() && look.tshirt.is_none() {
        return None;
    }
    let mut atlas = Img::new(1024, 512);
    for (im, name) in [(&look.pants, "CompositPantsTemplate"), (&look.shirt, "CompositShirtTemplate"), (&look.tshirt, "CompositTShirt")] {
        if let Some(i) = im {
            composite_layer(ctx, &mut atlas, name, i, [1.0; 3]);
        }
    }
    Some(atlas)
}

fn part_image(ctx: &Ctx, props: &std::collections::HashMap<String, Value>, color: [f64; 3], clothes: Option<&Img>) -> Option<Img> {
    let bid = mesh_content(props.get("BaseTextureId"));
    let oid = mesh_content(props.get("OverlayTextureId"));
    if bid.is_none() && oid.is_none() {
        return None;
    }
    let get = |v: &Option<String>| -> Option<Img> {
        let v = v.as_ref()?;
        if let Some(id) = asset_id(v) {
            if let Some(b) = ctx.fetch(&id) {
                if Img::is_image_bytes(&b) {
                    return Img::decode_cap(&b, 512, 2048);
                }
            }
        }
        ctx.image(v, 0)
    };
    let base = get(&bid);
    let over = get(&oid);
    let first = base.as_ref().or(over.as_ref())?;
    let mut img = Img::new(first.w, first.h);
    img.fill_rect(0, 0, img.w as i64, img.h as i64, [(color[0] * 255.0).round() as u32, (color[1] * 255.0).round() as u32, (color[2] * 255.0).round() as u32]);
    if let Some(b) = &base {
        if b.w == img.w && b.h == img.h {
            img.draw_over(b, None);
        } else {
            let (w, h) = (img.w as i64, img.h as i64);
            img.draw_scaled(b, 0, 0, w, h, None);
        }
    }
    if let Some(atlas) = clothes {
        let (w, h) = (img.w, img.h);
        for y in 0..h {
            let row = (((y as f64 + 0.5) / h as f64 * 512.0) as usize) * 1024;
            for x in 0..w {
                let t = atlas.px[row + ((x as f64 + 0.5) / w as f64 * 768.0) as usize];
                if t >> 24 != 0 {
                    img.px[y * w + x] = t;
                }
            }
        }
    }
    if let Some(o) = &over {
        if o.w == img.w && o.h == img.h {
            img.draw_over(o, None);
        } else {
            let (w, h) = (img.w as i64, img.h as i64);
            img.draw_scaled(o, 0, 0, w, h, None);
        }
    }
    Some(img)
}

fn has_tag(p: &std::collections::HashMap<String, Value>, tag: &str) -> bool {
    match p.get("Tags") {
        Some(Value::Str(s)) => s.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| w == tag),
        _ => false,
    }
}

fn flat_with(color: [f64; 3], w: usize, h: usize) -> Img {
    let mut f = Img::new(w, h);
    f.fill_rect(0, 0, w as i64, h as i64, [(color[0] * 255.0).round() as u32, (color[1] * 255.0).round() as u32, (color[2] * 255.0).round() as u32]);
    f
}

pub fn character_base_2012(ctx: &Ctx, base: &Base, look: &mut Look) -> (Base, BodyTex) {
    let mut base = base.clone();
    let mut tex = BodyTex { parts: Default::default(), head: None };
    let names = [TORSO, RARM, LARM, RLEG, LLEG];
    let dims = [[1.0, 1.0, 0.5], [0.5, 1.0, 0.5], [0.5, 1.0, 0.5], [0.5, 1.0, 0.5], [0.5, 1.0, 0.5]];
    let regions = [torso_region(), right_region(), left_region(), right_region(), left_region()];
    let atlas = if look.body_meshes.iter().any(|b| b.is_some()) { clothes_atlas(ctx, look) } else { None };
    for (pi, &slot) in names.iter().enumerate() {
        let cm = match &look.body_meshes[slot] {
            Some(c) => c.clone(),
            None => continue,
        };
        let name = ["Head", "Torso", "LeftArm", "RightArm", "LeftLeg", "RightLeg"][slot];
        let mesh = match mesh_from(ctx, cm.props.get("MeshId")) {
            Some(m) => m,
            None => {
                look.notes.push(format!("{}: the package mesh could not be loaded", name));
                continue;
            }
        };
        let target = match pi {
            0 => &mut base.torso,
            1 => &mut base.right_arm,
            2 => &mut base.left_arm,
            3 => &mut base.right_leg,
            _ => &mut base.left_leg,
        };
        *target = mesh.clone();
        match part_image(ctx, &cm.props, look.colors[slot], atlas.as_ref()) {
            Some(img) => {
                let mut t = Texture::new(img, false);
                t.repeat = false;
                tex.parts[pi] = Some(Rc::new(t));
            }
            None => {
                let d = dims[pi];
                *target = template_mesh(&mesh, d[0], d[1], d[2], &regions[pi]);
            }
        }
    }
    let shape = look.head_mesh.clone();
    let cm = look.body_meshes[HEAD].clone();
    if shape.is_some() || cm.is_some() {
        let props = match (&shape, &cm) {
            (Some(s), _) => s.props.clone(),
            (None, Some(c)) => c.props.clone(),
            _ => unreachable!(),
        };
        let g3 = |k: &str, d: V3| props.get(k).and_then(|v| v.as_v3()).unwrap_or(d);
        let mut scale = if shape.is_some() { g3("Scale", [1.25; 3]) } else { [1.25; 3] };
        let mut off = if shape.is_some() { g3("Offset", [0.0; 3]) } else { [0.0; 3] };
        if let Some(s) = &shape {
            if let Some(n) = neck_rig(s) {
                off = [off[0] - n[0], off[1] - (n[1] + 0.5), off[2] - n[2]];
            }
        }
        let variant = shape.as_ref().and_then(head_variant);
        let mut mesh: Option<Mesh> = match &variant {
            Some(v) => character_mesh(ctx, v).ok().map(|m| clean_mesh(&m)),
            None => mesh_from(ctx, props.get("MeshId")),
        };
        let own_mesh = mesh.is_some();
        let k: V3;
        if variant.is_some() && own_mesh {
            k = [1.0; 3];
        } else if own_mesh {
            k = scale;
        } else {
            mesh = Some(base.head_raw.clone());
            scale = [scale[0] / 1.25, scale[1] / 1.25, scale[2] / 1.25];
            k = scale;
        }
        let mut mesh = mesh.unwrap();
        mesh.pos = mesh.pos.iter().map(|p| [p[0] * k[0] + off[0], p[1] * k[1] + off[1], p[2] * k[2] + off[2]]).collect();
        base.head_box = mesh_bounds(&mesh);
        base.head = mesh;
        let img = if shape.is_some() {
            mesh_content(props.get("TextureId")).and_then(|c| ctx.image(&c, 0))
        } else {
            part_image(ctx, &props, [1.0; 3], None)
        };
        let color = look.colors[HEAD];
        let no_face = has_tag(&props, "NoFace");
        let face_img: Option<Img> = if look.face_set { look.face.clone() } else if no_face { None } else { Some(base.face.clone()) };
        if let Some(img) = img {
            let mut flat = flat_with(color, img.w, img.h);
            flat.draw_over(&img, None);
            if let (Some(f), true) = (&face_img, shape.is_some()) {
                let (w, h) = (flat.w as i64, flat.h as i64);
                flat.draw_scaled(f, 0, 0, w, h, None);
            }
            tex.head = Some(Rc::new(Texture::new(flat, true)));
        } else if shape.is_some() && mesh_content(props.get("TextureId")).is_some() {
            look.notes.push(format!("head: the texture {} could not be loaded, so the head has no picture on it", mesh_content(props.get("TextureId")).unwrap()));
        }
        if tex.head.is_none() && shape.is_some() && own_mesh && !no_face {
            let face = if look.face_set { look.face.clone() } else { Some(base.face.clone()) };
            if let Some(f) = face {
                let mut flat = flat_with(color, f.w, f.h);
                let (w, h) = (flat.w as i64, flat.h as i64);
                flat.draw_scaled(&f, 0, 0, w, h, None);
                tex.head = Some(Rc::new(Texture::new(flat, true)));
            }
        }
        if tex.head.is_none() && shape.is_some() && no_face {
            tex.head = Some(Rc::new(Texture::new(flat_with(color, 4, 4), true)));
        }
    }
    (base, tex)
}

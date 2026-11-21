use crate::assets::AssetSource;
use crate::img::Img;
use crate::scene::*;
use std::cell::RefCell;
use std::collections::HashMap;
use swirl_core::rbx::{self, Inst, Value};

pub struct Ctx<'a> {
    pub src: &'a dyn AssetSource,
    pub data: String,
    pub images: RefCell<HashMap<String, Option<Img>>>,
    pub unsupported: std::cell::Cell<bool>,
    pub missing: std::cell::Cell<bool>,
    pub content_dir: RefCell<Option<String>>,
}

fn digits_at(s: &str, from: usize) -> Option<String> {
    let d: String = s[from..].chars().take_while(|c| c.is_ascii_digit()).collect();
    if d.is_empty() || d.len() > 18 || d.starts_with('0') {
        None
    } else {
        Some(d)
    }
}

pub fn asset_id(r: &str) -> Option<String> {
    if r.is_empty() {
        return None;
    }
    let low = r.to_ascii_lowercase();
    let mut found = None;
    for pat in ["?id=", "&id="] {
        if let Some(i) = low.find(pat) {
            found = digits_at(&low, i + pat.len());
            break;
        }
    }
    if found.is_none() && low.starts_with("rbxassetid://") {
        found = digits_at(&low, 13).filter(|d| d.len() == low.len() - 13);
    }
    if found.is_none() {
        let t = low.trim();
        if t.bytes().all(|b| b.is_ascii_digit()) {
            found = digits_at(t, 0).filter(|d| d.len() == t.len());
        }
    }
    found
}

impl<'a> Ctx<'a> {
    pub fn new(src: &'a dyn AssetSource, data: &str) -> Ctx<'a> {
        Ctx { src, data: data.to_string(), images: RefCell::new(HashMap::new()), unsupported: std::cell::Cell::new(false), missing: std::cell::Cell::new(false), content_dir: RefCell::new(None) }
    }

    pub fn fetch(&self, id: &str) -> Option<Vec<u8>> {
        let r = self.src.fetch(id);
        if r.is_none() && !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
            self.missing.set(true);
        }
        r.filter(|b| b.len() <= 8 * 1024 * 1024)
    }

    pub fn image(&self, r: &str, depth: usize) -> Option<Img> {
        self.image_big(r, depth, false)
    }

    pub fn image_big(&self, r: &str, depth: usize, big: bool) -> Option<Img> {
        if r.to_ascii_lowercase().starts_with("rbxasset://") {
            let b = crate::items::mesh_bytes_pub(self, r)?;
            if !Img::is_image_bytes(&b) {
                return None;
            }
            return if big { Img::decode_hat(&b, 512) } else { Img::decode(&b, 2048) };
        }
        let id = asset_id(r)?;
        let key = format!("{}{}", id, if big { "h" } else { "" });
        if let Some(c) = self.images.borrow().get(&key) {
            return c.clone();
        }
        let b = self.fetch(&id)?;
        let res = if Img::is_image_bytes(&b) {
            let r = if big { Img::decode_hat(&b, 512) } else { Img::decode(&b, 2048) };
            if r.is_none() && !b.starts_with(b"\x89PNG") {
                self.unsupported.set(true);
            }
            r
        } else if depth > 2 {
            None
        } else {
            let tree = rbx::parse(&b).ok()?;
            let mut found: Option<String> = None;
            fn walk(x: &Inst, id: &str, found: &mut Option<String>) {
                for k in ["Texture", "ShirtTemplate", "PantsTemplate", "Graphic", "TextureId", "Image"] {
                    if let Some(Value::Url(u)) = x.props.get(k) {
                        if found.is_none() {
                            if let Some(a) = asset_id(u) {
                                if a != id {
                                    *found = Some(u.clone());
                                }
                            }
                        }
                    }
                }
                for c in &x.children {
                    walk(c, id, found);
                }
            }
            for r in &tree.roots {
                walk(r, &id, &mut found);
            }
            match found {
                Some(u) => self.image_big(&u, depth + 1, big),
                None => None,
            }
        };
        self.images.borrow_mut().insert(key, res.clone());
        res
    }
}

pub fn parse_color_int(n: i64) -> [f64; 3] {
    brick(n)
}

fn template(look: &mut Look, img: Option<Img>, what: &str, tag: &str) -> Option<Img> {
    match img {
        None => {
            look.notes.push(format!("{}: the {} image did not load", tag, what));
            None
        }
        Some(i) if i.w != TPL_WI || i.h != TPL_HI => {
            look.notes.push(format!("{}: the {} is not a 585x559 template, so it was ignored", tag, what));
            None
        }
        Some(i) => Some(i),
    }
}

fn holds_wearable(list: &[Inst], depth: usize) -> bool {
    for x in list {
        if matches!(x.cls.as_str(), "Tool" | "Hat" | "Accessory" | "Accoutrement") {
            return true;
        }
        if depth < 8 && holds_wearable(&x.children, depth + 1) {
            return true;
        }
    }
    false
}

pub struct Accessory<'t> {
    pub inst: &'t Inst,
}

pub fn absorb(ctx: &Ctx, look: &mut Look, roots: &[Inst], tag: &str, face_set: &mut bool, accs: &mut Vec<Inst>, tool: &mut Option<Inst>) {
    fn take(ctx: &Ctx, look: &mut Look, list: &[Inst], depth: usize, tag: &str, face_set: &mut bool, accs: &mut Vec<Inst>, tool: &mut Option<Inst>) {
        if depth > 8 {
            return;
        }
        let mut has_worn = false;
        if depth == 0 {
            has_worn = list.iter().any(|y| matches!(y.cls.as_str(), "Hat" | "Accessory" | "Accoutrement" | "Tool" | "HopperBin"));
        }
        for x in list {
            if has_worn && matches!(x.cls.as_str(), "Part" | "MeshPart") && x.name.to_lowercase() == "handle" {
                continue;
            }
            if x.cls == "Folder" && x.name == "R15ArtistIntent" {
                for mp in &x.children {
                    if matches!(mp.cls.as_str(), "MeshPart" | "Part") && crate::r15::GROUPS.iter().any(|g| g.0 == mp.name) {
                        look.r15_parts.retain(|(n, _)| *n != mp.name);
                        look.r15_parts.push((mp.name.clone(), mp.clone()));
                    }
                }
                continue;
            }
            if x.name.to_lowercase().starts_with("r15") && !matches!(x.cls.as_str(), "Hat" | "Accessory" | "Accoutrement" | "Tool") {
                continue;
            }
            match x.cls.as_str() {
                "Shirt" => {
                    let im = ctx.image(x.url("ShirtTemplate").unwrap_or(""), 0);
                    if let Some(i) = template(look, im, "shirt", tag) {
                        look.shirt = Some(i);
                    }
                }
                "Pants" => {
                    let im = ctx.image(x.url("PantsTemplate").unwrap_or(""), 0);
                    if let Some(i) = template(look, im, "pants", tag) {
                        look.pants = Some(i);
                    }
                }
                "ShirtGraphic" => match ctx.image(x.url("Graphic").unwrap_or(""), 0) {
                    Some(i) => look.tshirt = Some(i),
                    None => look.notes.push(format!("{}: the t-shirt image did not load", tag)),
                },
                "Decal" => {
                    if !*face_set {
                        *face_set = true;
                        match ctx.image(x.url("Texture").unwrap_or(""), 0) {
                            Some(i) => look.face = Some(i),
                            None => look.notes.push(format!("{}: the face image did not load", tag)),
                        }
                    }
                }
                "Hat" | "Accessory" | "Accoutrement" => accs.push(x.clone()),
                "Tool" => {
                    if tool.is_none() {
                        *tool = Some(x.clone());
                    }
                }
                "CharacterMesh" => {
                    let slot = match x.props.get("BodyPart") {
                        Some(Value::Num(n)) => Some(*n as i64),
                        Some(Value::Str(s)) => match s.as_str() {
                            "Head" => Some(0),
                            "Torso" => Some(1),
                            "LeftArm" => Some(2),
                            "RightArm" => Some(3),
                            "LeftLeg" => Some(4),
                            "RightLeg" => Some(5),
                            _ => None,
                        },
                        None => Some(0),
                        _ => None,
                    };
                    if let Some(s) = slot {
                        if (0..6).contains(&s) {
                            look.body_meshes[s as usize] = Some(x.clone());
                        }
                    }
                }
                "SpecialMesh" | "CylinderMesh" | "BlockMesh" | "FileMesh" => look.head_mesh = Some(x.clone()),
                _ => take(ctx, look, &x.children, depth + 1, tag, face_set, accs, tool),
            }
        }
    }
    take(ctx, look, roots, 0, tag, face_set, accs, tool);
}

pub fn add_asset(ctx: &Ctx, look: &mut Look, id: &str, face_set: &mut bool, accs: &mut Vec<Inst>, tool: &mut Option<Inst>) {
    let b = match ctx.fetch(id) {
        Some(b) => b,
        None => {
            look.notes.push(format!("{}: could not be fetched", id));
            return;
        }
    };
    if Img::is_image_bytes(&b) {
        look.notes.push(format!("{}: this is a bare image, not a shirt, pants, face, hat or gear asset, so it was refused", id));
        return;
    }
    let tree = match rbx::parse(&b) {
        Ok(t) => t,
        Err(_) => {
            look.notes.push(format!("{}: not a shirt, pants, face, hat or gear asset", id));
            return;
        }
    };
    if !holds_wearable(&tree.roots, 0) && String::from_utf8_lossy(&b).contains("BodyColors") {
        if let Some(c) = body_colors(&tree.roots) {
            look.pkg_colors = Some(c);
        }
        return;
    }
    absorb(ctx, look, &tree.roots, id, face_set, accs, tool);
}

fn find_body_colors(list: &[Inst]) -> Option<&Inst> {
    for i in list {
        if i.cls == "BodyColors" {
            return Some(i);
        }
        if let Some(r) = find_body_colors(&i.children) {
            return Some(r);
        }
    }
    None
}

const DEFAULT_HEX: [[f64; 3]; 6] = [
    [245.0 / 255.0, 205.0 / 255.0, 48.0 / 255.0],
    [13.0 / 255.0, 105.0 / 255.0, 172.0 / 255.0],
    [245.0 / 255.0, 205.0 / 255.0, 48.0 / 255.0],
    [245.0 / 255.0, 205.0 / 255.0, 48.0 / 255.0],
    [164.0 / 255.0, 189.0 / 255.0, 71.0 / 255.0],
    [164.0 / 255.0, 189.0 / 255.0, 71.0 / 255.0],
];

pub fn body_colors(roots: &[Inst]) -> Option<[[f64; 3]; 6]> {
    let bc = find_body_colors(roots)?;
    let names = ["HeadColor", "TorsoColor", "LeftArmColor", "RightArmColor", "LeftLegColor", "RightLegColor"];
    let mut out = DEFAULT_HEX;
    for (j, n) in names.iter().enumerate() {
        let v3 = bc.props.get(&format!("{}3", n));
        let v = bc.props.get(*n);
        if let Some(Value::Color3(r, g, b)) = v3 {
            let q = |x: f64| (x * 255.0).round() / 255.0;
            out[j] = [q(*r), q(*g), q(*b)];
        } else if let Some(Value::Brick(n)) = v {
            if swirl_core::brickcolors::has(*n) {
                out[j] = brick(*n);
            }
        } else if let Some(Value::Num(n)) = v {
            if swirl_core::brickcolors::has(*n as i64) {
                out[j] = brick(*n as i64);
            }
        }
    }
    Some(out)
}

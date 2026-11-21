use serde_json::Value as J;
use std::collections::HashMap;
use swirl_core::classes;

pub struct Inst {
    pub id: u32,
    pub class: String,
    pub parent: Option<u32>,
    pub props: HashMap<String, J>,
    pub children: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct Part {
    pub id: u32,
    pub cf: [f32; 12],
    pub size: [f32; 3],
    pub color: [f32; 3],
    pub alpha: f32,
    pub shape: u8,
    pub mesh: Option<MeshRef>,
    pub decals: Vec<(u8, String, f32, [f32; 4])>,
    pub top: u8,
    pub bottom: u8,
}

#[derive(Clone, Debug)]
pub struct MeshRef {
    pub mesh: String,
    pub tex: Option<String>,
    pub scale: [f32; 3],
    pub off: [f32; 3],
    pub fit: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub amb: f32,
    pub key: f32,
    pub fill: f32,
    pub tint: [f32; 3],
}

pub fn asset_id(s: &str) -> Option<String> {
    let low = s.to_ascii_lowercase();
    let digits = |x: &str| -> Option<String> {
        let d: String = x.chars().take_while(|c| c.is_ascii_digit()).collect();
        if d.is_empty() || d.starts_with('0') || d.len() > 18 {
            None
        } else {
            Some(d)
        }
    };
    if let Some(i) = low.find("id=") {
        return digits(&low[i + 3..]);
    }
    if let Some(r) = low.strip_prefix("rbxassetid://") {
        return digits(r);
    }
    if low.bytes().all(|b| b.is_ascii_digit()) {
        return digits(&low);
    }
    None
}

#[derive(Default)]
pub struct World {
    pub insts: HashMap<u32, Inst>,
    pub you: Option<u32>,
    pub place: u64,
    pub era: i32,
    pub ready: bool,
    pub own: Option<Own>,
    pub chars: HashMap<u32, CharRec>,
    pub tp_seq: i64,
    pub teleport: Option<[f32; 12]>,
    pub t: f32,
    pub pending_own: Option<(u32, [f32; 12], i64)>,
    pub driven: bool,
    pub local_t: HashMap<u32, f32>,
}

pub struct Own {
    pub model: u32,
    pub torso: u32,
    pub offsets: Vec<(u32, [f32; 12], u8)>,
    pub start: [f32; 12],
}

pub struct CharRec {
    pub torso: u32,
    pub offsets: Vec<(u32, [f32; 12], u8)>,
}

fn arr(v: Option<&J>, n: usize) -> Option<Vec<f32>> {
    let a = v?.as_array()?;
    if a.len() < n {
        return None;
    }
    Some(a.iter().take(n).map(|x| x.as_f64().unwrap_or(0.0) as f32).collect())
}

impl World {
    pub fn new() -> World {
        World::default()
    }

    fn add_record(&mut self, r: &J) {
        let a = match r.as_array() {
            Some(a) if a.len() >= 4 => a,
            _ => return,
        };
        let id = a[0].as_u64().unwrap_or(0) as u32;
        let class = a[1].as_str().unwrap_or("").to_string();
        let parent = a[2].as_u64().map(|p| p as u32);
        let mut props = HashMap::new();
        if let Some(p) = a[3].as_array() {
            let mut i = 0;
            while i + 1 < p.len() {
                if let Some(k) = p[i].as_str() {
                    props.insert(k.to_string(), p[i + 1].clone());
                }
                i += 2;
            }
        }
        if let Some(old) = self.insts.get(&id) {
            if let Some(op) = old.parent {
                if let Some(pi) = self.insts.get_mut(&op) {
                    pi.children.retain(|c| *c != id);
                }
            }
        }
        self.insts.insert(id, Inst { id, class, parent, props, children: vec![] });
        if let Some(p) = parent {
            if let Some(pi) = self.insts.get_mut(&p) {
                pi.children.push(id);
            }
        }
    }

    fn link(&mut self) {
        let ids: Vec<u32> = self.insts.keys().copied().collect();
        for id in ids {
            let p = self.insts[&id].parent;
            if let Some(p) = p {
                if let Some(pi) = self.insts.get_mut(&p) {
                    if !pi.children.contains(&id) {
                        pi.children.push(id);
                    }
                }
            }
        }
    }

    fn remove(&mut self, id: u32) {
        let kids = self.insts.get(&id).map(|i| i.children.clone()).unwrap_or_default();
        for k in kids {
            self.remove(k);
        }
        if let Some(i) = self.insts.remove(&id) {
            if let Some(p) = i.parent {
                if let Some(pi) = self.insts.get_mut(&p) {
                    pi.children.retain(|c| *c != id);
                }
            }
        }
    }

    pub fn apply(&mut self, text: &str) -> Option<String> {
        let m: J = serde_json::from_str(text).ok()?;
        let t = m["t"].as_str()?.to_string();
        match t.as_str() {
            "init" => {
                self.insts.clear();
                self.place = m["place"].as_u64().unwrap_or(0);
                self.era = m["era"].as_i64().unwrap_or(2012) as i32;
                self.you = m["you"].as_u64().map(|v| v as u32);
                if let Some(s) = m["snap"].as_array() {
                    let recs: Vec<&J> = s.iter().collect();
                    for r in recs {
                        self.add_record(r);
                    }
                }
                self.link_all();
                self.ready = true;
            }
            "rep" => {
                if let Some(ops) = m["o"].as_array() {
                    for op in ops {
                        self.apply_op(op);
                    }
                }
                if let Some(ph) = m["ph"].as_array().filter(|_| !self.driven) {
                    for r in ph {
                        if let Some(a) = r.as_array() {
                            if a.len() >= 13 {
                                let id = a[0].as_u64().unwrap_or(0) as u32;
                                if self.is_own_part(id) {
                                    continue;
                                }
                                let cf: Vec<J> = a[1..13].to_vec();
                                if let Some(i) = self.insts.get_mut(&id) {
                                    i.props.insert("CFrame".into(), J::Array(cf));
                                }
                            }
                        }
                    }
                }
                if let Some(ch) = m["ch"].as_array().filter(|_| !self.driven) {
                    for r in ch {
                        if let Some(a) = r.as_array() {
                            if a.len() >= 13 {
                                let model = a[0].as_u64().unwrap_or(0) as u32;
                                if self.own.as_ref().map(|o| o.model == model).unwrap_or(false) {
                                    continue;
                                }
                                let mut cf = [0.0f32; 12];
                                for k in 0..12 {
                                    cf[k] = a[1 + k].as_f64().unwrap_or(0.0) as f32;
                                }
                                let vx = a.get(13).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                let vz = a.get(15).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                let st = a.get(16).and_then(|v| v.as_u64()).unwrap_or(7) as u8;
                                self.move_char(model, cf, (vx * vx + vz * vz).sqrt(), st);
                            }
                        }
                    }
                }
                if let Some(after) = m["after"].as_array().filter(|_| !self.driven) {
                    for a in after {
                        match a["t"].as_str() {
                            Some("own") => {
                                let model = a["id"].as_u64().unwrap_or(0) as u32;
                                if let Some(c) = arr(Some(&a["c"]), 12) {
                                    let mut cf = [0.0f32; 12];
                                    cf.copy_from_slice(&c);
                                    self.take_own(model, cf, a["s"].as_i64().unwrap_or(1));
                                }
                            }
                            Some("tp") => {
                                self.tp_seq = a["s"].as_i64().unwrap_or(self.tp_seq);
                                if let Some(c) = arr(Some(&a["c"]), 12) {
                                    let mut cf = [0.0f32; 12];
                                    cf.copy_from_slice(&c);
                                    self.teleport = Some(cf);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        if let Some((model, cf, seq)) = self.pending_own {
            self.take_own(model, cf, seq);
        }
        Some(t)
    }

    fn link_all(&mut self) {
        for i in self.insts.values_mut() {
            i.children.clear();
        }
        self.link();
    }

    fn apply_op(&mut self, op: &J) {
        let a = match op.as_array() {
            Some(a) if !a.is_empty() => a,
            _ => return,
        };
        match a[0].as_str().unwrap_or("") {
            "p" if a.len() >= 4 => {
                let id = a[1].as_u64().unwrap_or(0) as u32;
                if self.driven && a[2].as_str() == Some("CFrame") {
                    return;
                }
                if let (Some(i), Some(k)) = (self.insts.get_mut(&id), a[2].as_str()) {
                    i.props.insert(k.to_string(), a[3].clone());
                }
            }
            "m" if a.len() >= 3 => {
                let id = a[1].as_u64().unwrap_or(0) as u32;
                let np = a[2].as_u64().map(|v| v as u32);
                let old = self.insts.get(&id).and_then(|i| i.parent);
                if let Some(o) = old {
                    if let Some(pi) = self.insts.get_mut(&o) {
                        pi.children.retain(|c| *c != id);
                    }
                }
                if let Some(i) = self.insts.get_mut(&id) {
                    i.parent = np;
                }
                if let Some(p) = np {
                    if let Some(pi) = self.insts.get_mut(&p) {
                        pi.children.push(id);
                    }
                }
            }
            "n" if a.len() >= 2 => {
                if let Some(recs) = a[1].as_array() {
                    for r in recs {
                        self.add_record(r);
                    }
                    self.link();
                }
            }
            "d" if a.len() >= 2 => {
                let id = a[1].as_u64().unwrap_or(0) as u32;
                self.remove(id);
            }
            _ => {}
        }
    }

    pub fn apply_local(&mut self, text: &str) {
        let ops: J = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return,
        };
        if let Some(a) = ops.as_array() {
            for op in a {
                self.apply_op(op);
            }
        }
    }

    pub fn is_own_part(&self, id: u32) -> bool {
        match &self.own {
            Some(o) => o.torso == id || o.offsets.iter().any(|(p, _, _)| *p == id),
            None => false,
        }
    }

    fn model_parts(&self, model: u32) -> Vec<u32> {
        let mut out = vec![];
        let mut stack = vec![model];
        while let Some(id) = stack.pop() {
            if let Some(i) = self.insts.get(&id) {
                if id != model && classes::is_a(&i.class, "BasePart") {
                    out.push(id);
                }
                for &c in &i.children {
                    stack.push(c);
                }
            }
        }
        out.sort();
        out
    }

    fn cf_of(&self, id: u32) -> [f32; 12] {
        let mut cf = crate::mat::CF_ID;
        if let Some(v) = self.insts.get(&id).and_then(|i| arr(i.props.get("CFrame"), 12)) {
            cf.copy_from_slice(&v);
        }
        cf
    }

    pub fn set_cf(&mut self, id: u32, cf: &[f32; 12]) {
        if let Some(i) = self.insts.get_mut(&id) {
            i.props.insert("CFrame".into(), J::Array(cf.iter().map(|v| J::from(*v as f64)).collect()));
        }
    }

    fn torso_of(&self, model: u32) -> Option<u32> {
        let parts = self.model_parts(model);
        for n in ["Torso", "HumanoidRootPart", "UpperTorso"] {
            if let Some(p) = parts.iter().find(|p| self.insts.get(p).map(|i| i.props.get("Name").and_then(|v| v.as_str()) == Some(n)).unwrap_or(false)) {
                return Some(*p);
            }
        }
        parts.first().copied()
    }

    fn offsets_for(&self, model: u32, torso: u32) -> Vec<(u32, [f32; 12], u8)> {
        let inv = crate::mat::cf_inv(&self.cf_of(torso));
        self.model_parts(model)
            .into_iter()
            .filter(|p| *p != torso)
            .map(|p| {
                let name = self.insts.get(&p).and_then(|i| i.props.get("Name")).and_then(|v| v.as_str()).unwrap_or("");
                let kind = match name {
                    "Left Arm" => 1,
                    "Right Arm" => 2,
                    "Left Leg" => 3,
                    "Right Leg" => 4,
                    _ => 0,
                };
                (p, crate::mat::cf_mul(&inv, &self.cf_of(p)), kind)
            })
            .collect()
    }

    fn apply_welds(&mut self, model: u32) {
        let mut welds: Vec<(u32, u32, [f32; 12], [f32; 12])> = vec![];
        let mut stack = vec![model];
        while let Some(id) = stack.pop() {
            if let Some(i) = self.insts.get(&id) {
                if i.class == "Weld" {
                    let p0 = i.props.get("Part0").and_then(|v| v.as_u64()).map(|v| v as u32);
                    let p1 = i.props.get("Part1").and_then(|v| v.as_u64()).map(|v| v as u32);
                    if let (Some(a), Some(b)) = (p0, p1) {
                        let mut c0 = crate::mat::CF_ID;
                        let mut c1 = crate::mat::CF_ID;
                        if let Some(v) = arr(i.props.get("C0"), 12) {
                            c0.copy_from_slice(&v);
                        }
                        if let Some(v) = arr(i.props.get("C1"), 12) {
                            c1.copy_from_slice(&v);
                        }
                        welds.push((a, b, c0, c1));
                    }
                }
                for &c in &i.children {
                    stack.push(c);
                }
            }
        }
        for _ in 0..2 {
            for (a, b, c0, c1) in &welds {
                if !self.insts.contains_key(a) || !self.insts.contains_key(b) {
                    continue;
                }
                let w = crate::mat::cf_mul(&crate::mat::cf_mul(&self.cf_of(*a), c0), &crate::mat::cf_inv(c1));
                self.set_cf(*b, &w);
            }
        }
    }

    pub fn inventory(&self) -> Vec<(u32, String, bool)> {
        let you = match self.you {
            Some(y) => y,
            None => return vec![],
        };
        let mut out = vec![];
        let mut scan = |container: u32, equipped: bool, out: &mut Vec<(u32, String, bool)>| {
            if let Some(c) = self.insts.get(&container) {
                for &k in &c.children {
                    if let Some(t) = self.insts.get(&k) {
                        if t.class == "Tool" {
                            let name = t.props.get("Name").and_then(|v| v.as_str()).unwrap_or("Tool").to_string();
                            out.push((k, name, equipped));
                        }
                    }
                }
            }
        };
        if let Some(me) = self.insts.get(&you) {
            for &c in &me.children {
                if self.insts.get(&c).map(|i| i.class == "Backpack").unwrap_or(false) {
                    scan(c, false, &mut out);
                }
            }
        }
        if let Some(o) = &self.own {
            scan(o.model, true, &mut out);
        }
        out.sort_by_key(|t| t.0);
        out
    }

    fn pose(&mut self, model: u32, torso_cf: [f32; 12], offs: &[(u32, [f32; 12], u8)], speed: f32, state: u8) {
        let amount = (speed / 6.0).clamp(0.0, 1.0);
        let rate = (speed / 16.0).clamp(0.5, 1.6);
        let swing = (self.t * 10.0 * rate).sin() * 0.9 * amount;
        for (p, o, kind) in offs {
            let a = match kind {
                1 => swing,
                2 => -swing,
                3 => -swing,
                4 => swing,
                _ => 0.0,
            };
            let a = if (*kind == 1 || *kind == 2) && (state == 2 || state == 3) { 2.8 } else { a };
            let rel = if *kind == 0 || a == 0.0 {
                *o
            } else {
                let pivot = [o[0], o[1] + 1.0, o[2]];
                crate::mat::pivot_rotate(o, pivot, a)
            };
            let w = crate::mat::cf_mul(&torso_cf, &rel);
            self.set_cf(*p, &w);
        }
        self.apply_welds(model);
    }

    fn move_char(&mut self, model: u32, cf: [f32; 12], speed: f32, state: u8) {
        if !self.chars.contains_key(&model) {
            let torso = match self.torso_of(model) {
                Some(t) => t,
                None => return,
            };
            let offsets = self.offsets_for(model, torso);
            self.chars.insert(model, CharRec { torso, offsets });
        }
        let (torso, offs) = {
            let c = &self.chars[&model];
            (c.torso, c.offsets.clone())
        };
        self.set_cf(torso, &cf);
        self.pose(model, cf, &offs, speed, state);
    }

    pub fn take_own(&mut self, model: u32, cf: [f32; 12], seq: i64) {
        let torso = match self.insts.contains_key(&model).then(|| self.torso_of(model)).flatten() {
            Some(t) => t,
            None => {
                self.pending_own = Some((model, cf, seq));
                return;
            }
        };
        self.pending_own = None;
        let offsets = self.offsets_for(model, torso);
        self.tp_seq = seq;
        self.own = Some(Own { model, torso, offsets, start: cf });
        self.teleport = Some(cf);
    }

    pub fn move_own(&mut self, cf: [f32; 12], speed: f32, state: u8) {
        let (model, torso, offs) = match &self.own {
            Some(o) => (o.model, o.torso, o.offsets.clone()),
            None => return,
        };
        self.set_cf(torso, &cf);
        self.pose(model, cf, &offs, speed, state);
    }

    pub fn colliders(&self) -> Vec<crate::controller::Obb> {
        let mut out = vec![];
        let ws = match self.workspace() {
            Some(w) => w,
            None => return out,
        };
        let mut stack: Vec<(u32, bool)> = vec![(ws, false)];
        while let Some((id, in_char)) = stack.pop() {
            let i = match self.insts.get(&id) {
                Some(i) => i,
                None => continue,
            };
            let has_hum = i.class == "Model" && i.children.iter().any(|c| self.insts.get(c).map(|x| x.class == "Humanoid").unwrap_or(false));
            let ic = in_char || has_hum;
            for &c in &i.children {
                stack.push((c, ic));
            }
            if ic || id == ws || !classes::is_a(&i.class, "BasePart") || i.class == "Terrain" {
                continue;
            }
            if i.props.get("CanCollide").and_then(|v| v.as_bool()) == Some(false) {
                continue;
            }
            let cf = self.cf_of(id);
            let size = arr(i.props.get("Size"), 3).map(|v| [v[0], v[1], v[2]]).unwrap_or([4.0, 1.2, 2.0]);
            out.push(crate::controller::Obb {
                c: [cf[0], cf[1], cf[2]],
                r: [cf[3], cf[4], cf[5], cf[6], cf[7], cf[8], cf[9], cf[10], cf[11]],
                half: [size[0] / 2.0, size[1] / 2.0, size[2] / 2.0],
            });
        }
        out
    }

    pub fn humanoid_of(&self, model: u32) -> (f32, f32) {
        let mut ws = 16.0;
        let mut jp = 50.0;
        if let Some(m) = self.insts.get(&model) {
            for c in &m.children {
                if let Some(h) = self.insts.get(c) {
                    if h.class == "Humanoid" {
                        ws = h.props.get("WalkSpeed").and_then(|v| v.as_f64()).unwrap_or(16.0) as f32;
                        jp = h.props.get("JumpPower").and_then(|v| v.as_f64()).unwrap_or(50.0) as f32;
                    }
                }
            }
        }
        (ws, jp)
    }

    pub fn is_a(&self, id: u32, base: &str) -> bool {
        self.insts.get(&id).map(|i| classes::is_a(&i.class, base)).unwrap_or(false)
    }

    pub fn workspace(&self) -> Option<u32> {
        self.insts.values().find(|i| i.class == "Workspace").map(|i| i.id)
    }

    pub fn camera_cf(&self) -> Option<[f32; 12]> {
        let ws = self.insts.get(&self.workspace()?)?;
        let cam = ws.props.get("CurrentCamera")?.as_u64()? as u32;
        let c = self.insts.get(&cam)?;
        let cf = arr(c.props.get("CFrame"), 12)?;
        let mut o = [0.0; 12];
        o.copy_from_slice(&cf);
        Some(o)
    }

    pub fn parts(&self) -> Vec<Part> {
        let mut out = vec![];
        let ws = match self.workspace() {
            Some(w) => w,
            None => return out,
        };
        let mut stack = vec![ws];
        while let Some(id) = stack.pop() {
            let i = match self.insts.get(&id) {
                Some(i) => i,
                None => continue,
            };
            for &c in i.children.iter().rev() {
                stack.push(c);
            }
            if id == ws || !classes::is_a(&i.class, "BasePart") || i.class == "Terrain" {
                continue;
            }
            let mut cf = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
            if let Some(v) = arr(i.props.get("CFrame"), 12) {
                cf.copy_from_slice(&v);
            }
            let size = arr(i.props.get("Size"), 3).map(|v| [v[0], v[1], v[2]]).unwrap_or([4.0, 1.2, 2.0]);
            let color = match arr(i.props.get("Color"), 3) {
                Some(c) => [c[0], c[1], c[2]],
                None => {
                    let b = i.props.get("BrickColor").and_then(|v| v.as_i64()).unwrap_or(194);
                    let c = swirl_core::rbx::brick_rgb01(b);
                    [c[0] as f32, c[1] as f32, c[2] as f32]
                }
            };
            let mut alpha = 1.0 - i.props.get("Transparency").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            if let Some(t) = self.local_t.get(&id) {
                alpha *= 1.0 - t;
            }
            let mut shape = i.props.get("shape").or(i.props.get("Shape")).and_then(|v| v.as_i64()).unwrap_or(1) as u8;
            if i.class == "WedgePart" {
                shape = 3;
            }
            let mut size = size;
            for &c in &i.children {
                if let Some(ci) = self.insts.get(&c) {
                    if matches!(ci.class.as_str(), "SpecialMesh" | "BlockMesh" | "CylinderMesh") {
                        let ty = match ci.class.as_str() {
                            "BlockMesh" => 6,
                            "CylinderMesh" => 4,
                            _ => ci.props.get("MeshType").and_then(|v| v.as_i64()).unwrap_or(6),
                        };
                        if ty != 5 {
                            if let Some(sc) = arr(ci.props.get("Scale"), 3) {
                                size = [size[0] * sc[0], size[1] * sc[1], size[2] * sc[2]];
                            }
                            shape = match ty {
                                2 => 3,
                                3 => 0,
                                4 => 4,
                                _ => 1,
                            };
                        }
                        break;
                    }
                }
            }
            let mut mesh = None;
            if i.class == "MeshPart" {
                if let Some(m) = i.props.get("MeshId").and_then(|v| v.as_str()).and_then(asset_id) {
                    let tex = i.props.get("TextureID").or(i.props.get("TextureId")).and_then(|v| v.as_str()).and_then(asset_id);
                    mesh = Some(MeshRef { mesh: m, tex, scale: [1.0; 3], off: [0.0; 3], fit: true });
                }
            } else {
                for &c in &i.children {
                    if let Some(ci) = self.insts.get(&c) {
                        if classes::is_a(&ci.class, "FileMesh") {
                            let ty = ci.props.get("MeshType").and_then(|v| v.as_i64()).unwrap_or(6);
                            if ci.class == "FileMesh" || ty == 5 {
                                if let Some(m) = ci.props.get("MeshId").and_then(|v| v.as_str()).and_then(asset_id) {
                                    let tex = ci.props.get("TextureId").and_then(|v| v.as_str()).and_then(asset_id);
                                    let sc = arr(ci.props.get("Scale"), 3).map(|v| [v[0], v[1], v[2]]).unwrap_or([1.0; 3]);
                                    let of = arr(ci.props.get("Offset"), 3).map(|v| [v[0], v[1], v[2]]).unwrap_or([0.0; 3]);
                                    mesh = Some(MeshRef { mesh: m, tex, scale: sc, off: of, fit: false });
                                }
                            }
                            break;
                        }
                    }
                }
            }
            let mut decals = vec![];
            for &c in &i.children {
                if let Some(ci) = self.insts.get(&c) {
                    if ci.class == "Decal" || ci.class == "Texture" {
                        if let Some(tid) = ci.props.get("Texture").and_then(|v| v.as_str()).and_then(asset_id) {
                            let face = ci.props.get("Face").and_then(|v| v.as_i64()).unwrap_or(5) as u8;
                            let da = 1.0 - ci.props.get("Transparency").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            if da >= 0.05 {
                                decals.push((face, tid, da, [0.0, 0.0, 1.0, 1.0]));
                            }
                        }
                    }
                }
            }
            if let Some(par) = i.parent.and_then(|p| self.insts.get(&p)) {
                if par.class == "Model" {
                    let name = i.props.get("Name").and_then(|v| v.as_str()).unwrap_or("");
                    self.clothing_decals(par, name, &mut decals);
                }
            }
            if mesh.is_none() && i.props.get("Name").and_then(|v| v.as_str()) == Some("Head") {
                for &c in &i.children {
                    if let Some(ci) = self.insts.get(&c) {
                        if ci.class == "SpecialMesh" && ci.props.get("MeshType").and_then(|v| v.as_i64()).unwrap_or(0) == 0 {
                            let sc = arr(ci.props.get("Scale"), 3).map(|v| [v[0] / 1.25, v[1] / 1.25, v[2] / 1.25]).unwrap_or([1.0; 3]);
                            mesh = Some(MeshRef { mesh: "c:fonts/head.mesh".into(), tex: None, scale: sc, off: [0.0; 3], fit: false });
                            for d in decals.iter_mut() {
                                if d.0 == 5 {
                                    d.0 = 6;
                                }
                            }
                        }
                    }
                }
            }
            let surf = |k: &str, d: u8| i.props.get(k).and_then(|v| v.as_i64()).map(|v| v as u8).unwrap_or(d);
            let (top, bottom) = (surf("TopSurface", 3), surf("BottomSurface", 4));
            out.push(Part { id, cf, size, color, alpha, shape, mesh, decals, top, bottom });
        }
        out
    }

    fn clothing_decals(&self, model: &Inst, part: &str, out: &mut Vec<(u8, String, f32, [f32; 4])>) {
        let region: Option<[[f32; 4]; 6]> = match part {
            "Torso" => Some([[231.0, 74.0, 128.0, 128.0], [427.0, 74.0, 128.0, 128.0], [165.0, 74.0, 64.0, 128.0], [361.0, 74.0, 64.0, 128.0], [231.0, 8.0, 128.0, 64.0], [231.0, 204.0, 128.0, 64.0]]),
            "Right Arm" | "Right Leg" => Some([[217.0, 355.0, 64.0, 128.0], [85.0, 355.0, 64.0, 128.0], [151.0, 355.0, 64.0, 128.0], [19.0, 355.0, 64.0, 128.0], [217.0, 289.0, 64.0, 64.0], [217.0, 485.0, 64.0, 64.0]]),
            "Left Arm" | "Left Leg" => Some([[308.0, 355.0, 64.0, 128.0], [440.0, 355.0, 64.0, 128.0], [506.0, 355.0, 64.0, 128.0], [374.0, 355.0, 64.0, 128.0], [308.0, 289.0, 64.0, 64.0], [308.0, 485.0, 64.0, 64.0]]),
            _ => None,
        };
        let region = match region {
            Some(r) => r,
            None => return,
        };
        let leg = part.ends_with("Leg");
        let torso = part == "Torso";
        let find = |cls: &str, key: &str| -> Option<String> {
            model.children.iter().filter_map(|c| self.insts.get(c)).find(|i| i.class == cls).and_then(|i| i.props.get(key)).and_then(|v| v.as_str()).and_then(asset_id)
        };
        let faces = [5u8, 2, 0, 3, 1, 4];
        let mut add = |tid: &str, out: &mut Vec<(u8, String, f32, [f32; 4])>| {
            for (k, r) in region.iter().enumerate() {
                out.push((faces[k], tid.to_string(), 1.0, [r[0] / 585.0, r[1] / 559.0, r[2] / 585.0, r[3] / 559.0]));
            }
        };
        if leg || torso {
            if let Some(t) = find("Pants", "PantsTemplate") {
                add(&t, out);
            }
        }
        if !leg {
            if let Some(t) = find("Shirt", "ShirtTemplate") {
                add(&t, out);
            }
        }
        if torso {
            if let Some(t) = find("ShirtGraphic", "Graphic") {
                let r = region[0];
                let _ = r;
                out.push((5, t, 1.0, [0.0, 0.0, 1.0, 1.0]));
            }
        }
    }

    pub fn lighting_inst(&self) -> Option<&Inst> {
        self.insts.values().find(|i| i.class == "Lighting")
    }

    pub fn light(&self) -> Light {
        let era2018 = self.era == 2018;
        let props = self.lighting_inst().map(|i| &i.props);
        let c3 = |k: &str, d: [f32; 3]| props.and_then(|p| arr(p.get(k), 3)).map(|v| [v[0], v[1], v[2]]).unwrap_or(d);
        let mut amb = c3("Ambient", [0.5; 3]);
        let top = c3("ColorShift_Top", [0.0; 3]);
        let bot = c3("ColorShift_Bottom", [0.0; 3]);
        let bright = props.and_then(|p| p.get("Brightness")).and_then(|v| v.as_f64()).map(|b| (b as f32).clamp(0.0, 5.0)).unwrap_or(1.0);
        let out = c3("OutdoorAmbient", [0.5; 3]);
        if era2018 && !props.map(|p| p.contains_key("Ambient")).unwrap_or(false) {
            amb = [0.0; 3];
        }
        let mut a = [0.0f32; 3];
        let mut k = [0.0f32; 3];
        let mut f = [0.0f32; 3];
        for i in 0..3 {
            a[i] = if era2018 { (0.5 * (amb[i] + out[i] * 0.55) / 0.275).clamp(0.0, 1.2) } else { amb[i].clamp(0.0, 1.2) };
            k[i] = 0.55 * bright * (1.0 + top[i] * 0.6 / 0.9);
            f[i] = 0.12 * (0.28 + bot[i] * 0.6) / 0.28;
        }
        let avg = |v: [f32; 3]| (v[0] + v[1] + v[2]) / 3.0;
        let (aa, kk) = (avg(a), avg(k));
        let base = (aa + kk * 0.7).max(1e-3);
        let mix = |i: usize| a[i] + k[i] * 0.7;
        Light { amb: aa, key: kk, fill: avg(f), tint: [mix(0) / base, mix(1) / base, mix(2) / base] }
    }

    pub fn sky_faces(&self) -> [String; 6] {
        let names = ["SkyboxRt", "SkyboxLf", "SkyboxUp", "SkyboxDn", "SkyboxBk", "SkyboxFt"];
        let defs = ["rt", "lf", "up", "dn", "bk", "ft"];
        let sky = self.lighting_inst().and_then(|l| l.children.iter().find_map(|c| self.insts.get(c).filter(|i| i.class == "Sky")));
        let mut out: [String; 6] = Default::default();
        for i in 0..6 {
            let id = sky.and_then(|s| s.props.get(names[i])).and_then(|v| v.as_str()).and_then(asset_id);
            out[i] = match id {
                Some(id) => id,
                None => format!("c:sky/null_plainsky512_{}.jpg", defs[i]),
            };
        }
        out
    }

    pub fn bounds(&self) -> ([f32; 3], f32) {
        let parts = self.parts();
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in &parts {
            if p.size.iter().cloned().fold(0.0, f32::max) > 400.0 {
                continue;
            }
            for k in 0..3 {
                lo[k] = lo[k].min(p.cf[k] - p.size[k] / 2.0);
                hi[k] = hi[k].max(p.cf[k] + p.size[k] / 2.0);
            }
        }
        if lo[0] > hi[0] {
            return ([0.0, 5.0, 0.0], 30.0);
        }
        let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let r = (((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt() / 2.0).max(6.0);
        (c, r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_and_rep() {
        let init = r#"{"t":"init","place":1,"era":2012,"you":3,"snap":[[1,"Workspace",0,["Name","Workspace"]],[2,"Part",1,["Name","Baseplate","Size",[512,1.2,512],"CFrame",[0,-0.6,0,1,0,0,0,1,0,0,0,1],"Color",[0.5,0.5,0.5]]],[3,"Player",0,[]]]}"#;
        let mut w = World::new();
        assert_eq!(w.apply(init).as_deref(), Some("init"));
        assert_eq!(w.parts().len(), 1);
        w.apply(r#"{"t":"rep","o":[["p",2,"Size",[10,1,10]],["n",[[9,"Part",1,["Name","B"]]]]]}"#);
        assert_eq!(w.parts().len(), 2);
        w.apply(r#"{"t":"rep","o":[["d",9]]}"#);
        assert_eq!(w.parts().len(), 1);
        assert_eq!(w.parts()[0].size, [10.0, 1.0, 10.0]);
    }
}

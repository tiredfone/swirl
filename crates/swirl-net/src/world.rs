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
}

#[derive(Default)]
pub struct World {
    pub insts: HashMap<u32, Inst>,
    pub you: Option<u32>,
    pub place: u64,
    pub era: i32,
    pub ready: bool,
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
            }
            _ => {}
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
            let alpha = 1.0 - i.props.get("Transparency").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let shape = i.props.get("shape").or(i.props.get("Shape")).and_then(|v| v.as_i64()).unwrap_or(1) as u8;
            out.push(Part { id, cf, size, color, alpha, shape });
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

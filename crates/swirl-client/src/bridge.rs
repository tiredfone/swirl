use std::collections::{HashMap, HashSet};
use swirl_core::instance::{Change, DataModel, InstId, Val, LOCAL_BASE};
use swirl_core::wire::{encode_inst, encode_value};

#[derive(Default)]
pub struct Bridge {
    pub render_from: usize,
    pub known: HashSet<InstId>,
    pub cfs: indexmap_lite::Map,
    pub ops: Vec<serde_json::Value>,
    pub cf_marks: Vec<InstId>,
    pub alpha: Vec<(InstId, Option<f64>)>,
}

mod indexmap_lite {
    #[derive(Default)]
    pub struct Map {
        pub order: Vec<u32>,
        pub vals: std::collections::HashMap<u32, [f64; 12]>,
    }

    impl Map {
        pub fn insert(&mut self, k: u32, v: [f64; 12]) {
            if self.vals.insert(k, v).is_none() {
                self.order.push(k);
            }
        }

        pub fn clear(&mut self) {
            self.order.clear();
            self.vals.clear();
        }
    }
}

fn in_world(d: &DataModel, id: InstId) -> bool {
    let ws = d.services.get("Workspace").copied();
    let lt = d.services.get("Lighting").copied();
    let mut cur = Some(id);
    let mut guard = 0;
    while let Some(c) = cur {
        if Some(c) == ws || Some(c) == lt {
            return true;
        }
        cur = d.get(c).and_then(|i| i.parent);
        guard += 1;
        if guard > 10000 {
            break;
        }
    }
    false
}

impl Bridge {
    fn live(&self, id: InstId) -> bool {
        id < LOCAL_BASE || self.known.contains(&id)
    }

    fn cf_of(d: &DataModel, id: InstId) -> Option<[f64; 12]> {
        match d.get_prop(id, "CFrame") {
            Some(Val::Arr(a)) if a.len() >= 12 => {
                let mut c = [0.0; 12];
                c.copy_from_slice(&a[..12]);
                Some(c)
            }
            _ => None,
        }
    }

    fn mark(&mut self, d: &DataModel, id: InstId, on: bool) {
        let mut all = vec![id];
        d.descendants(id, &mut all);
        for x in all {
            if x >= LOCAL_BASE {
                if on {
                    self.known.insert(x);
                } else {
                    self.known.remove(&x);
                }
            }
        }
    }

    fn walk(&self, d: &DataModel, id: InstId, recs: &mut Vec<serde_json::Value>) {
        if let Some(r) = encode_inst(d, id) {
            recs.push(r);
        }
        for c in d.get(id).map(|i| i.children.clone()).unwrap_or_default() {
            self.walk(d, c, recs);
        }
    }

    pub fn collect(&mut self, d: &DataModel, remote: bool) {
        let end = d.journal.len();
        let from = self.render_from.min(end);
        let entries: Vec<Change> = d.journal[from..end].to_vec();
        self.render_from = end;
        for ch in entries {
            match ch {
                Change::Prop(id, name) => {
                    if !self.live(id) {
                        continue;
                    }
                    if name == "CFrame" {
                        if let Some(c) = Self::cf_of(d, id) {
                            self.cfs.insert(id, c);
                        }
                        continue;
                    }
                    if remote {
                        continue;
                    }
                    let class = match d.get(id) {
                        Some(i) => i.class.clone(),
                        None => continue,
                    };
                    let p = match swirl_core::classes::prop_info(&class, &name) {
                        Some(p) if p.ty != "any" => p,
                        _ => continue,
                    };
                    if !in_world(d, id) {
                        continue;
                    }
                    if let Some(v) = d.get_prop(id, &name).and_then(|v| encode_value(d, &p.ty, v)) {
                        self.ops.push(serde_json::json!(["p", id, name, v]));
                    }
                }
                Change::Parent { id, new, .. } => {
                    if remote {
                        continue;
                    }
                    if id < LOCAL_BASE {
                        let par = new.filter(|p| self.live(*p));
                        self.ops.push(serde_json::json!(["m", id, par]));
                        continue;
                    }
                    let in_w = new.map(|p| in_world(d, p) && self.live(p)).unwrap_or(false);
                    let known = self.known.contains(&id);
                    if in_w && !known {
                        let mut recs = vec![];
                        self.walk(d, id, &mut recs);
                        self.mark(d, id, true);
                        self.ops.push(serde_json::json!(["n", recs]));
                    } else if !in_w && known {
                        self.ops.push(serde_json::json!(["d", id]));
                        self.mark(d, id, false);
                    } else if known {
                        self.ops.push(serde_json::json!(["m", id, new]));
                    }
                }
                Change::Destroy { id, .. } => {
                    if remote {
                        continue;
                    }
                    if id < LOCAL_BASE {
                        self.ops.push(serde_json::json!(["d", id]));
                    } else if self.known.remove(&id) {
                        self.ops.push(serde_json::json!(["d", id]));
                    }
                }
            }
        }
    }

    pub fn flush_marks(&mut self, d: &DataModel) {
        for id in std::mem::take(&mut self.cf_marks) {
            if let Some(c) = Self::cf_of(d, id) {
                self.cfs.insert(id, c);
            }
        }
    }

    pub fn take_ops(&mut self) -> String {
        let v = std::mem::take(&mut self.ops);
        serde_json::to_string(&v).unwrap_or_default()
    }

    pub fn take_cfs(&mut self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.cfs.order.len() * 13);
        for k in &self.cfs.order {
            out.push(*k as f64);
            out.extend_from_slice(&self.cfs.vals[k]);
        }
        self.cfs.clear();
        out
    }

    pub fn take_alpha(&mut self) -> String {
        let v: Vec<serde_json::Value> = std::mem::take(&mut self.alpha).into_iter().map(|(i, a)| serde_json::json!([i, a])).collect();
        serde_json::to_string(&v).unwrap_or_default()
    }
}

#[allow(dead_code)]
fn _unused(_: HashMap<u32, u32>) {}

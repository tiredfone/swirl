use crate::classes;
use crate::instance::{Change, DataModel, InstId, Val};
use serde_json::Value as J;

pub fn decode_value(ty: &str, v: &J) -> Val {
    match ty {
        "Ref" => Val::Ref(v.as_u64().map(|x| x as InstId)),
        "bool" => Val::Bool(v.as_bool().unwrap_or(false)),
        "string" | "Content" => Val::Str(v.as_str().unwrap_or("").to_string()),
        "int" | "number" | "enum" | "BC" => Val::Num(v.as_f64().unwrap_or(0.0)),
        _ => match v {
            J::Array(a) => Val::Arr(a.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect()),
            J::Null => Val::Nil,
            J::Bool(b) => Val::Bool(*b),
            J::Number(n) => Val::Num(n.as_f64().unwrap_or(0.0)),
            J::String(s) => Val::Str(s.clone()),
            _ => Val::Nil,
        },
    }
}

fn record(dm: &mut DataModel, r: &J) -> Option<(InstId, Option<InstId>)> {
    let a = r.as_array().filter(|a| a.len() >= 4)?;
    let id = a[0].as_u64()? as InstId;
    let class = a[1].as_str()?.to_string();
    let parent = a[2].as_u64().map(|p| p as InstId);
    let mut props = vec![];
    if let Some(p) = a[3].as_array() {
        let mut i = 0;
        while i + 1 < p.len() {
            if let Some(k) = p[i].as_str() {
                let ty = classes::prop_info(&class, k).map(|d| d.ty.clone()).unwrap_or_default();
                props.push((k.to_string(), decode_value(&ty, &p[i + 1])));
            }
            i += 2;
        }
    }
    let tags: Vec<String> = a.get(4).and_then(|t| t.as_array()).map(|t| t.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    if let Some(old) = dm.get(id).map(|i| i.parent) {
        dm.link_quiet(id, None);
        let _ = old;
    }
    dm.adopt(id, &class, props, tags);
    if classes::is_service(&class) && parent.map(|p| p == dm.root).unwrap_or(true) {
        dm.services.insert(class, id);
    }
    Some((id, parent))
}

fn link(dm: &mut DataModel, pending: Vec<(InstId, Option<InstId>)>) {
    let root = dm.root;
    for (id, parent) in pending {
        let p = match parent {
            Some(p) if dm.get(p).is_some() => Some(p),
            _ => {
                let is_service = dm.get(id).map(|i| classes::is_service(&i.class)).unwrap_or(false);
                if is_service {
                    Some(root)
                } else {
                    None
                }
            }
        };
        dm.link_quiet(id, p);
    }
}

pub fn apply_init(dm: &mut DataModel, snap: &[J]) {
    dm.clear_wire();
    let mut pending = vec![];
    for r in snap {
        if let Some(x) = record(dm, r) {
            pending.push(x);
        }
    }
    link(dm, pending);
}

pub fn apply_ops(dm: &mut DataModel, ops: &[J]) -> Vec<(InstId, Option<InstId>, Option<InstId>)> {
    let mut moved = vec![];
    for op in ops {
        let a = match op.as_array() {
            Some(a) if !a.is_empty() => a,
            _ => continue,
        };
        match a[0].as_str().unwrap_or("") {
            "p" if a.len() >= 4 => {
                let id = a[1].as_u64().unwrap_or(0) as InstId;
                let name = a[2].as_str().unwrap_or("");
                let class = match dm.get(id) {
                    Some(i) => i.class.clone(),
                    None => continue,
                };
                let ty = classes::prop_info(&class, name).map(|d| d.ty.clone()).unwrap_or_default();
                let _ = dm.set_prop(id, name, decode_value(&ty, &a[3]));
            }
            "m" if a.len() >= 3 => {
                let id = a[1].as_u64().unwrap_or(0) as InstId;
                let np = a[2].as_u64().map(|v| v as InstId);
                let old = dm.get(id).and_then(|i| i.parent);
                dm.link_quiet(id, np);
                dm.journal.push(Change::Parent { id, old, new: np });
                moved.push((id, old, np));
            }
            "n" if a.len() >= 2 => {
                if let Some(recs) = a[1].as_array() {
                    let mut pending = vec![];
                    for r in recs {
                        if let Some(x) = record(dm, r) {
                            pending.push(x);
                        }
                    }
                    for (id, parent) in &pending {
                        moved.push((*id, None, *parent));
                    }
                    let ids: Vec<(InstId, Option<InstId>)> = pending.clone();
                    link(dm, pending);
                    for (id, parent) in ids {
                        if dm.get(id).and_then(|i| i.parent) == parent && parent.is_some() {
                            dm.journal.push(Change::Parent { id, old: None, new: parent });
                        }
                    }
                }
            }
            "d" if a.len() >= 2 => {
                let id = a[1].as_u64().unwrap_or(0) as InstId;
                let old = dm.get(id).and_then(|i| i.parent);
                dm.destroy_quiet(id);
                dm.journal.push(Change::Destroy { id, old });
                moved.push((id, old, None));
            }
            _ => {}
        }
    }
    moved
}

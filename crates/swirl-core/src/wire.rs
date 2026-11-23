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

pub struct Applied {
    pub moved: Vec<(InstId, Option<InstId>, Option<InstId>)>,
    pub props: Vec<(InstId, String, Option<Val>, Val)>,
    pub sounds: Vec<(InstId, String)>,
}

pub fn apply_ops(dm: &mut DataModel, ops: &[J]) -> Applied {
    let mut moved = vec![];
    let mut props: Vec<(InstId, String, Option<Val>, Val)> = vec![];
    let mut sounds: Vec<(InstId, String)> = vec![];
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
                let v = decode_value(&ty, &a[3]);
                let old = dm.get_prop(id, name).cloned();
                if old.as_ref() != Some(&v) {
                    props.push((id, name.to_string(), old, v.clone()));
                }
                let _ = dm.set_prop(id, name, v);
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
            "s" if a.len() >= 3 => {
                let id = a[1].as_u64().unwrap_or(0) as InstId;
                sounds.push((id, a[2].as_str().unwrap_or("").to_string()));
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
    Applied { moved, props, sounds }
}

fn round4(v: f64) -> f64 {
    (v * 10000.0 + 0.5).floor() / 10000.0
}

fn num_json(v: f64) -> J {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        serde_json::json!(v as i64)
    } else {
        serde_json::json!(v)
    }
}

pub fn encode_value(dm: &DataModel, ty: &str, v: &Val) -> Option<J> {
    match ty {
        "Ref" => Some(match v {
            Val::Ref(Some(id)) if dm.get(*id).map(|i| !i.destroyed).unwrap_or(false) => serde_json::json!(id),
            _ => J::Null,
        }),
        "V3" | "V2" | "C3" | "UD2" | "CF" => match v {
            Val::Arr(a) => Some(J::Array(a.iter().map(|x| num_json(round4(*x))).collect())),
            _ => Some(J::Null),
        },
        "number" => match v {
            Val::Num(n) if n.is_finite() => Some(num_json(*n)),
            _ => Some(serde_json::json!(0)),
        },
        "any" => None,
        _ => Some(match v {
            Val::Nil => J::Null,
            Val::Bool(b) => serde_json::json!(b),
            Val::Num(n) => num_json(*n),
            Val::Str(s) => serde_json::json!(s),
            Val::Arr(a) => J::Array(a.iter().map(|x| num_json(*x)).collect()),
            Val::Ref(_) => J::Null,
        }),
    }
}

fn is_default(p: &classes::PropDef, v: &Val) -> bool {
    let d = Val::from_default(&p.ty, &p.def);
    match (&d, v) {
        (Val::Arr(a), Val::Arr(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y),
        (Val::Arr(_), _) => false,
        (Val::Num(a), Val::Num(b)) => a == b || (p.ty == "number" && (*a as f32) == (*b as f32)),
        (Val::Ref(None), Val::Ref(None)) => true,
        (Val::Ref(None), _) => false,
        (a, b) => a == b,
    }
}

pub fn encode_inst(dm: &DataModel, id: InstId) -> Option<J> {
    let inst = dm.get(id)?;
    let mut keys: Vec<&String> = inst.props.keys().collect();
    keys.sort();
    let mut props: Vec<J> = vec![];
    if let Some(c) = classes::class_info(&inst.class) {
        for k in keys {
            let p = match c.all.get(k) {
                Some(p) => p,
                None => continue,
            };
            if p.ty == "any" {
                continue;
            }
            let v = &inst.props[k];
            if k != "Name" && is_default(p, v) {
                continue;
            }
            if let Some(e) = encode_value(dm, &p.ty, v) {
                props.push(serde_json::json!(k));
                props.push(e);
            }
        }
    }
    let mut rec = vec![serde_json::json!(inst.id), serde_json::json!(inst.class), inst.parent.map(|p| serde_json::json!(p)).unwrap_or(J::Null), J::Array(props)];
    if !inst.tags.is_empty() {
        rec.push(serde_json::json!(inst.tags));
    }
    Some(J::Array(rec))
}

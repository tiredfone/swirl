use serde_json::{json, Value as J};
use swirl_core::classes::{self, PropDef};
use swirl_core::instance::{DataModel, InstId, Val};

fn round4(v: f64) -> f64 {
    (v * 10000.0 + 0.5).floor() / 10000.0
}

fn num_json(v: f64) -> J {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        json!(v as i64)
    } else {
        json!(v)
    }
}

pub fn encode_value(dm: &DataModel, ty: &str, v: &Val) -> Option<J> {
    match ty {
        "Ref" => Some(match v {
            Val::Ref(Some(id)) if dm.get(*id).map(|i| !i.destroyed).unwrap_or(false) => json!(id),
            _ => J::Null,
        }),
        "V3" | "V2" | "C3" | "UD2" | "CF" => match v {
            Val::Arr(a) => Some(J::Array(a.iter().map(|x| num_json(round4(*x))).collect())),
            _ => Some(J::Null),
        },
        "number" => match v {
            Val::Num(n) if n.is_finite() => Some(num_json(*n)),
            _ => Some(json!(0)),
        },
        "any" => None,
        _ => Some(match v {
            Val::Nil => J::Null,
            Val::Bool(b) => json!(b),
            Val::Num(n) => num_json(*n),
            Val::Str(s) => json!(s),
            Val::Arr(a) => J::Array(a.iter().map(|x| num_json(*x)).collect()),
            Val::Ref(_) => J::Null,
        }),
    }
}

fn is_default(p: &PropDef, v: &Val) -> bool {
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

pub fn hidden_prop(dm: &DataModel, id: InstId, name: &str) -> bool {
    if name == "Archivable" {
        return true;
    }
    let class = dm.get(id).map(|i| i.class.as_str()).unwrap_or("");
    if name == "Source" && class == "Script" {
        return true;
    }
    if name == "Source" && class == "ModuleScript" {
        return dm.era != 2018;
    }
    false
}

pub fn encode_inst(dm: &DataModel, id: InstId) -> J {
    let inst = dm.get(id).unwrap();
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
            if hidden_prop(dm, id, k) {
                continue;
            }
            if let Some(e) = encode_value(dm, &p.ty, v) {
                props.push(json!(k));
                props.push(e);
            }
        }
    }
    let mut rec = vec![json!(inst.id), json!(inst.class), inst.parent.map(|p| json!(p)).unwrap_or(J::Null), J::Array(props)];
    if !inst.tags.is_empty() {
        rec.push(json!(inst.tags));
    }
    J::Array(rec)
}

const SCOPED: [&str; 4] = ["PlayerGui", "Backpack", "PlayerScripts", "StarterGear"];

pub fn snapshot_for(dm: &DataModel, roots: &[InstId], player: InstId) -> Vec<J> {
    let mut out = vec![];
    fn walk(dm: &DataModel, x: InstId, scope: Option<InstId>, player: InstId, out: &mut Vec<J>) {
        let inst = dm.get(x).unwrap();
        let mut s = scope;
        if SCOPED.contains(&inst.class.as_str()) {
            if let Some(p) = inst.parent {
                if dm.get(p).map(|i| i.class == "Player").unwrap_or(false) {
                    s = Some(p);
                }
            }
        }
        if let Some(sc) = s {
            if sc != player {
                return;
            }
        }
        out.push(encode_inst(dm, x));
        for &c in &inst.children {
            walk(dm, c, s, player, out);
        }
    }
    for &r in roots {
        walk(dm, r, None, player, &mut out);
    }
    out
}

pub fn replicated(dm: &DataModel, roots: &[InstId], id: InstId, viewer: Option<InstId>) -> bool {
    let mut cur = Some(id);
    let mut guard = 0;
    while let Some(c) = cur {
        let i = match dm.get(c) {
            Some(i) => i,
            None => return false,
        };
        if i.destroyed {
            return false;
        }
        if i.parent == Some(dm.root) {
            return roots.contains(&c);
        }
        if SCOPED.contains(&i.class.as_str()) {
            if let Some(p) = i.parent {
                if dm.get(p).map(|x| x.class == "Player").unwrap_or(false) && viewer != Some(p) {
                    return false;
                }
            }
        }
        cur = i.parent;
        guard += 1;
        if guard > 10000 {
            return false;
        }
    }
    false
}

pub fn subtree(dm: &DataModel, id: InstId, out: &mut Vec<J>, viewer: Option<InstId>) {
    out.push(encode_inst(dm, id));
    if let Some(i) = dm.get(id) {
        let is_player = i.class == "Player";
        for &c in &i.children {
            if is_player && viewer != Some(id) && dm.get(c).map(|k| SCOPED.contains(&k.class.as_str())).unwrap_or(false) {
                continue;
            }
            subtree(dm, c, out, viewer);
        }
    }
}

pub fn ops_for(dm: &DataModel, roots: &[InstId], journal: &[swirl_core::instance::Change], viewer: Option<InstId>) -> Vec<J> {
    use swirl_core::instance::Change;
    let mut ops: Vec<J> = vec![];
    let mut sent_new: std::collections::HashSet<InstId> = std::collections::HashSet::new();
    let mut new_roots: Vec<InstId> = vec![];
    for ch in journal {
        if let Change::Parent { id, old, .. } = ch {
            let was = old.map(|o| replicated(dm, roots, o, viewer)).unwrap_or(false);
            if !was && replicated(dm, roots, *id, viewer) {
                new_roots.push(*id);
            }
        }
    }
    for ch in journal {
        match ch {
            Change::Prop(id, name) => {
                if !replicated(dm, roots, *id, viewer) || hidden_prop(dm, *id, name) || new_roots.iter().any(|r| r == id || dm.is_descendant_of(*id, *r)) {
                    continue;
                }
                let inst = match dm.get(*id) {
                    Some(i) => i,
                    None => continue,
                };
                if let Some(p) = classes::prop_info(&inst.class, name) {
                    if let (Some(v), true) = (inst.props.get(name), p.ty != "any") {
                        if let Some(e) = encode_value(dm, &p.ty, v) {
                            ops.push(json!(["p", id, name, e]));
                        }
                    }
                }
            }
            Change::Parent { id, old, new } => {
                let now = replicated(dm, roots, *id, viewer);
                let was = old.map(|o| replicated(dm, roots, o, viewer)).unwrap_or(false);
                if was && now {
                    ops.push(json!(["m", id, new]));
                } else if !was && now {
                    if sent_new.iter().any(|s| *s == *id || dm.is_descendant_of(*id, *s)) {
                        continue;
                    }
                    if new_roots.iter().any(|r| r != id && dm.is_descendant_of(*id, *r)) {
                        continue;
                    }
                    let mut recs = vec![];
                    subtree(dm, *id, &mut recs, viewer);
                    sent_new.insert(*id);
                    ops.push(json!(["n", recs]));
                } else if was && !now {
                    ops.push(json!(["d", id]));
                }
            }
            Change::Destroy { id, old } => {
                if old.map(|o| replicated(dm, roots, o, viewer)).unwrap_or(false) {
                    ops.push(json!(["d", id]));
                }
            }
        }
    }
    ops
}

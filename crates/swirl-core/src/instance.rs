use crate::classes::{self, PropDef};
use serde_json::Value as J;
use std::collections::HashMap;

pub type InstId = u32;

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Nil,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<f64>),
    Ref(Option<InstId>),
}

impl Val {
    pub fn from_default(ty: &str, def: &J) -> Val {
        match ty {
            "Ref" => Val::Ref(None),
            "bool" => Val::Bool(def.as_bool().unwrap_or(false)),
            "string" | "Content" => Val::Str(def.as_str().unwrap_or("").to_string()),
            "int" | "number" | "enum" | "BC" => Val::Num(def.as_f64().unwrap_or(0.0)),
            _ => match def.as_array() {
                Some(a) => Val::Arr(a.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect()),
                None => Val::Nil,
            },
        }
    }
}

#[derive(Debug)]
pub struct Instance {
    pub id: InstId,
    pub class: String,
    pub name: String,
    pub parent: Option<InstId>,
    pub children: Vec<InstId>,
    pub props: HashMap<String, Val>,
    pub tags: Vec<String>,
    pub destroyed: bool,
}

#[derive(Clone, Debug)]
pub enum Change {
    Prop(InstId, String),
    Parent { id: InstId, old: Option<InstId>, new: Option<InstId> },
    Destroy { id: InstId, old: Option<InstId> },
}

pub struct DataModel {
    pub journal: Vec<Change>,
    pub side: String,
    pub era: i32,
    insts: Vec<Option<Instance>>,
    pub root: InstId,
    pub services: HashMap<String, InstId>,
}

impl DataModel {
    pub fn new(side: &str) -> DataModel {
        let mut dm = DataModel { journal: vec![], side: side.to_string(), era: 2012, insts: vec![], root: 0, services: HashMap::new() };
        let id = dm.alloc("DataModel", "Game");
        dm.root = id;
        dm
    }

    fn alloc(&mut self, class: &str, name: &str) -> InstId {
        let id = self.insts.len() as InstId;
        let mut props = HashMap::new();
        if let Some(c) = classes::class_info(class) {
            for (k, p) in &c.all {
                props.insert(k.clone(), Val::from_default(&p.ty, &p.def));
            }
        }
        if props.contains_key("Name") {
            props.insert("Name".to_string(), Val::Str(name.to_string()));
        }
        self.insts.push(Some(Instance { id, class: class.to_string(), name: name.to_string(), parent: None, children: vec![], props, tags: vec![], destroyed: false }));
        id
    }

    pub fn get(&self, id: InstId) -> Option<&Instance> {
        self.insts.get(id as usize)?.as_ref()
    }

    pub fn get_mut_inst(&mut self, id: InstId) -> Option<&mut Instance> {
        self.get_mut(id)
    }

    fn get_mut(&mut self, id: InstId) -> Option<&mut Instance> {
        self.insts.get_mut(id as usize)?.as_mut()
    }

    pub fn create(&mut self, class: &str, parent: Option<InstId>) -> Result<InstId, String> {
        if !classes::is_creatable(class) {
            return Err(format!("Can not create an Instance of class {}", class));
        }
        let id = self.alloc(class, class);
        if let Some(p) = parent {
            self.set_parent(id, Some(p))?;
        }
        Ok(id)
    }

    pub fn create_any(&mut self, class: &str, parent: Option<InstId>) -> Result<InstId, String> {
        if classes::class_info(class).is_none() {
            return Err(format!("unknown class {}", class));
        }
        let id = self.alloc(class, class);
        if let Some(p) = parent {
            self.set_parent(id, Some(p))?;
        }
        Ok(id)
    }

    pub fn service(&mut self, name: &str) -> Result<InstId, String> {
        if let Some(&id) = self.services.get(name) {
            return Ok(id);
        }
        if !classes::is_service(name) {
            return Err(format!("{} is not a service", name));
        }
        let id = self.alloc(name, name);
        self.services.insert(name.to_string(), id);
        let root = self.root;
        self.set_parent(id, Some(root))?;
        Ok(id)
    }

    pub fn is_descendant_of(&self, id: InstId, other: InstId) -> bool {
        let mut cur = self.get(id).and_then(|i| i.parent);
        let mut guard = 0;
        while let Some(c) = cur {
            if c == other {
                return true;
            }
            cur = self.get(c).and_then(|i| i.parent);
            guard += 1;
            if guard > 100000 {
                break;
            }
        }
        false
    }

    pub fn set_parent(&mut self, id: InstId, parent: Option<InstId>) -> Result<(), String> {
        if let Some(p) = parent {
            if p == id || self.is_descendant_of(p, id) {
                return Err("Attempt to set parent would result in circular reference".into());
            }
            if self.get(p).is_none() {
                return Err("parent does not exist".into());
            }
        }
        let old = self.get(id).ok_or("no such instance")?.parent;
        if let Some(o) = old {
            if let Some(oi) = self.get_mut(o) {
                oi.children.retain(|&c| c != id);
            }
        }
        if let Some(p) = parent {
            self.get_mut(p).unwrap().children.push(id);
        }
        self.get_mut(id).unwrap().parent = parent;
        if old != parent {
            self.journal.push(Change::Parent { id, old, new: parent });
        }
        Ok(())
    }

    pub fn name(&self, id: InstId) -> &str {
        self.get(id).map(|i| i.name.as_str()).unwrap_or("")
    }

    pub fn set_prop(&mut self, id: InstId, name: &str, value: Val) -> Result<(), String> {
        let class = self.get(id).ok_or("no such instance")?.class.clone();
        let canon = classes::canonical_prop(&class, name).ok_or_else(|| format!("{} is not a valid member of {}", name, class))?;
        if canon == "Name" {
            if let Val::Str(s) = &value {
                self.get_mut(id).unwrap().name = s.clone();
            }
        }
        self.journal.push(Change::Prop(id, canon.clone()));
        self.get_mut(id).unwrap().props.insert(canon, value);
        Ok(())
    }

    pub fn get_mut_name(&mut self, id: InstId, name: &str) {
        if let Some(i) = self.get_mut(id) {
            i.name = name.to_string();
        }
    }

    pub fn set_prop_quiet(&mut self, id: InstId, name: &str, value: Val) {
        if let Some(i) = self.get_mut(id) {
            i.props.insert(name.to_string(), value);
        }
    }

    pub fn get_prop(&self, id: InstId, name: &str) -> Option<&Val> {
        let i = self.get(id)?;
        let canon = classes::canonical_prop(&i.class, name)?;
        i.props.get(&canon)
    }

    pub fn find_child(&self, id: InstId, name: &str, recursive: bool) -> Option<InstId> {
        let i = self.get(id)?;
        for &c in &i.children {
            if self.name(c) == name {
                return Some(c);
            }
        }
        if recursive {
            for &c in &i.children {
                if let Some(f) = self.find_child(c, name, true) {
                    return Some(f);
                }
            }
        }
        None
    }

    pub fn find_child_of_class(&self, id: InstId, class: &str) -> Option<InstId> {
        self.get(id)?.children.iter().copied().find(|&c| self.get(c).map(|i| i.class == class).unwrap_or(false))
    }

    pub fn is_a(&self, id: InstId, base: &str) -> bool {
        self.get(id).map(|i| classes::is_a(&i.class, base)).unwrap_or(false)
    }

    pub fn descendants(&self, id: InstId, out: &mut Vec<InstId>) {
        if let Some(i) = self.get(id) {
            for &c in &i.children {
                out.push(c);
                self.descendants(c, out);
            }
        }
    }

    pub fn full_name(&self, id: InstId) -> String {
        let mut parts = vec![];
        let mut cur = Some(id);
        while let Some(c) = cur {
            if c == self.root {
                break;
            }
            parts.push(self.name(c).to_string());
            cur = self.get(c).and_then(|i| i.parent);
        }
        parts.reverse();
        parts.join(".")
    }

    pub fn destroy(&mut self, id: InstId) {
        let kids: Vec<InstId> = self.get(id).map(|i| i.children.clone()).unwrap_or_default();
        for k in kids {
            self.destroy(k);
        }
        let old = self.get(id).and_then(|i| i.parent);
        let _ = self.set_parent(id, None);
        self.journal.retain(|c| !matches!(c, Change::Parent { id: i, new: None, .. } if *i == id));
        self.journal.push(Change::Destroy { id, old });
        if let Some(i) = self.get_mut(id) {
            i.destroyed = true;
        }
    }

    pub fn clone_tree(&mut self, id: InstId) -> Option<InstId> {
        let (class, name, props, tags, kids) = {
            let i = self.get(id)?;
            (i.class.clone(), i.name.clone(), i.props.clone(), i.tags.clone(), i.children.clone())
        };
        if classes::is_service(&class) {
            return None;
        }
        let n = self.alloc(&class, &name);
        {
            let m = self.get_mut(n).unwrap();
            m.props = props;
            m.tags = tags;
        }
        for k in kids {
            if let Some(c) = self.clone_tree(k) {
                let _ = self.set_parent(c, Some(n));
            }
        }
        Some(n)
    }

    pub fn prop_def(&self, id: InstId, name: &str) -> Option<&'static PropDef> {
        let i = self.get(id)?;
        classes::prop_info(&i.class, &classes::canonical_prop(&i.class, name)?)
    }

    pub fn load_tree(&mut self, tree: &crate::rbx::Tree, parent: InstId) -> Vec<InstId> {
        let mut refs: HashMap<String, InstId> = HashMap::new();
        let mut pending: Vec<(InstId, String, String)> = vec![];
        let mut out = vec![];
        for r in &tree.roots {
            if let Some(id) = self.load_inst(r, parent, &mut refs, &mut pending) {
                out.push(id);
            }
        }
        for (id, prop, rf) in pending {
            if let Some(&t) = refs.get(&rf) {
                let _ = self.set_prop(id, &prop, Val::Ref(Some(t)));
            }
        }
        out
    }

    fn load_inst(&mut self, x: &crate::rbx::Inst, parent: InstId, refs: &mut HashMap<String, InstId>, pending: &mut Vec<(InstId, String, String)>) -> Option<InstId> {
        use crate::rbx::Value;
        let id = if classes::is_service(&x.cls) {
            self.service(&x.cls).ok()?
        } else if classes::class_info(&x.cls).is_none() || classes::is_creatable(&x.cls) {
            let id = self.alloc(&x.cls, &x.name);
            self.set_parent(id, Some(parent)).ok()?;
            id
        } else {
            return None;
        };
        if let Some(r) = &x.referent {
            refs.insert(r.clone(), id);
        }
        self.get_mut(id).unwrap().name = x.name.clone();
        let has_brick = x.props.contains_key("BrickColor");
        let has_color = x.props.contains_key("Color3uint8") || x.props.contains_key("Color");
        for (k, v) in &x.props {
            if (k == "Color3uint8" || k == "Color") && classes::prop_info(&x.cls, "BrickColor").is_some() {
                if let Value::Color3(r, g, b) = v {
                    if !has_brick {
                        let nb = crate::brickcolors::nearest(*r, *g, *b);
                        self.get_mut(id).unwrap().props.insert("BrickColor".into(), Val::Num(nb as f64));
                    }
                    if classes::prop_info(&x.cls, "Color").map(|p| p.ty == "C3").unwrap_or(false) {
                        self.get_mut(id).unwrap().props.insert("Color".into(), Val::Arr(vec![*r, *g, *b]));
                    }
                    continue;
                }
            }
            let canon = match classes::canonical_prop(&x.cls, k) {
                Some(c) => c,
                None => continue,
            };
            let ty = classes::prop_info(&x.cls, &canon).map(|p| p.ty.as_str()).unwrap_or("");
            let val = match v {
                Value::Str(s) => Val::Str(s.clone()),
                Value::Bool(b) => Val::Bool(*b),
                Value::Num(n) => Val::Num(*n),
                Value::Brick(b) => Val::Num(*b as f64),
                Value::Color3(r, g, b) => Val::Arr(vec![*r, *g, *b]),
                Value::V3(a, b, c) => Val::Arr(vec![*a, *b, *c]),
                Value::V2(a, b) => Val::Arr(vec![*a, *b]),
                Value::CFrame(p, m) => Val::Arr(vec![p[0], p[1], p[2], m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8]]),
                Value::Url(u) => Val::Str(u.clone()),
                Value::Ref(r) => {
                    pending.push((id, canon.clone(), r.clone()));
                    continue;
                }
            };
            if ty == "Ref" {
                continue;
            }
            self.get_mut(id).unwrap().props.insert(canon, val);
        }
        if has_brick && !has_color && classes::prop_info(&x.cls, "Color").map(|p| p.ty == "C3").unwrap_or(false) {
            if let Some(Val::Num(b)) = self.get(id).unwrap().props.get("BrickColor").cloned() {
                let c = crate::rbx::brick_rgb01(b as i64);
                self.get_mut(id).unwrap().props.insert("Color".into(), Val::Arr(c.to_vec()));
            }
        }
        for c in &x.children {
            self.load_inst(c, id, refs, pending);
        }
        Some(id)
    }
}

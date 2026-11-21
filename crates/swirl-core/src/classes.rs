use serde_json::Value as J;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct PropDef {
    pub name: String,
    pub ty: String,
    pub def: J,
    pub enum_type: Option<String>,
}

#[derive(Debug)]
pub struct ClassDef {
    pub name: String,
    pub base: Option<String>,
    pub is_abstract: bool,
    pub service: bool,
    pub nocreate: bool,
    pub era: i32,
    pub aliases: HashMap<String, String>,
    pub own: Vec<PropDef>,
    pub all: HashMap<String, PropDef>,
    pub chain: Vec<String>,
    pub alias_map: HashMap<String, String>,
}

pub struct ClassTable {
    pub classes: HashMap<String, ClassDef>,
    pub enums: HashMap<String, Vec<(String, i64)>>,
    pub replicated_services: Vec<String>,
    pub era2018: std::collections::HashSet<String>,
}

const TYPE_ALIASES: [(&str, &str); 6] = [("size", "Size"), ("shape", "Shape"), ("formFactorRaw", "FormFactor"), ("formFactor", "FormFactor"), ("archivable", "Archivable"), ("style", "Style")];

static TABLE: OnceLock<ClassTable> = OnceLock::new();

pub fn table() -> &'static ClassTable {
    TABLE.get_or_init(|| load(include_str!("../data/classes.json")))
}

fn load(src: &str) -> ClassTable {
    let j: J = serde_json::from_str(src).expect("classes.json");
    let mut classes: HashMap<String, ClassDef> = HashMap::new();
    let mut order: Vec<String> = vec![];
    for c in j["classes"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap().to_string();
        let own: Vec<PropDef> = c["own"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| PropDef { name: p["name"].as_str().unwrap().to_string(), ty: p["type"].as_str().unwrap().to_string(), def: p["def"].clone(), enum_type: p["enumType"].as_str().map(|s| s.to_string()) })
            .collect();
        let aliases = c["aliases"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect()).unwrap_or_default();
        order.push(name.clone());
        classes.insert(
            name.clone(),
            ClassDef {
                name,
                base: c["base"].as_str().map(|s| s.to_string()),
                is_abstract: c["abstract"].as_bool().unwrap_or(false),
                service: c["service"].as_bool().unwrap_or(false),
                nocreate: c["nocreate"].as_bool().unwrap_or(false),
                era: c["era"].as_i64().unwrap_or(2012) as i32,
                aliases,
                own,
                all: HashMap::new(),
                chain: vec![],
                alias_map: HashMap::new(),
            },
        );
    }
    for name in &order {
        let mut chain = vec![];
        let mut cur = Some(name.clone());
        while let Some(n) = cur {
            chain.push(n.clone());
            cur = classes.get(&n).and_then(|c| c.base.clone());
        }
        let mut all: HashMap<String, PropDef> = HashMap::new();
        let mut alias_map: HashMap<String, String> = TYPE_ALIASES.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        for n in chain.iter().rev() {
            if let Some(c) = classes.get(n) {
                for p in &c.own {
                    all.insert(p.name.clone(), p.clone());
                }
                for (k, v) in &c.aliases {
                    alias_map.insert(k.clone(), v.clone());
                }
            }
        }
        let c = classes.get_mut(name).unwrap();
        c.chain = chain;
        c.all = all;
        c.alias_map = alias_map;
    }
    let mut enums = HashMap::new();
    for (k, v) in j["enums"].as_object().unwrap() {
        let mut items: Vec<(String, i64)> = v.as_object().unwrap().iter().map(|(a, b)| (a.clone(), b.as_i64().unwrap_or(0))).collect();
        items.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        enums.insert(k.clone(), items);
    }
    ClassTable {
        classes,
        enums,
        replicated_services: j["replicatedServices"].as_array().unwrap().iter().filter_map(|s| s.as_str().map(|s| s.to_string())).collect(),
        era2018: j["era2018"].as_array().unwrap().iter().filter_map(|s| s.as_str().map(|s| s.to_string())).collect(),
    }
}

pub fn class_info(name: &str) -> Option<&'static ClassDef> {
    table().classes.get(name)
}

pub fn is_a(cls: &str, base: &str) -> bool {
    match class_info(cls) {
        Some(c) => c.chain.iter().any(|n| n == base),
        None => cls == base || base == "Instance",
    }
}

pub fn prop_info(cls: &str, name: &str) -> Option<&'static PropDef> {
    class_info(cls)?.all.get(name)
}

pub fn canonical_prop(cls: &str, name: &str) -> Option<String> {
    let c = match class_info(cls) {
        Some(c) => c,
        None => return Some(name.to_string()),
    };
    if c.all.contains_key(name) {
        return Some(name.to_string());
    }
    if let Some(a) = c.alias_map.get(name) {
        if c.all.contains_key(a) {
            return Some(a.clone());
        }
    }
    let mut up = String::new();
    let mut it = name.chars();
    if let Some(f) = it.next() {
        up.extend(f.to_uppercase());
        up.push_str(it.as_str());
    }
    if c.all.contains_key(&up) {
        return Some(up);
    }
    if name.len() > 8 && name.ends_with("Internal") && c.all.contains_key(&name[..name.len() - 8]) {
        return Some(name[..name.len() - 8].to_string());
    }
    if name == "Content" {
        return if c.chain.iter().any(|n| n == "PartOperation") && c.all.contains_key("AssetId") { Some("AssetId".into()) } else { None };
    }
    if name.len() > 7 && name.ends_with("Content") {
        let base = &name[..name.len() - 7];
        let legacy: Vec<&str> = match name {
            "AudioContent" => vec!["SoundId"],
            "MeshContent" => vec!["MeshId"],
            "AnimationContent" => vec!["AnimationId"],
            "TextureContent" => vec!["TextureId", "TextureID", "Texture"],
            "BaseTextureContent" => vec!["BaseTextureId"],
            "OverlayTextureContent" => vec!["OverlayTextureId"],
            "MoonTextureContent" => vec!["MoonTextureId"],
            "SunTextureContent" => vec!["SunTextureId"],
            "CursorIconContent" => vec!["CursorIcon"],
            _ => vec![base],
        };
        for cand in legacy {
            if c.all.contains_key(cand) {
                return Some(cand.to_string());
            }
        }
    }
    None
}

pub fn is_creatable(name: &str) -> bool {
    match class_info(name) {
        Some(c) => !c.is_abstract && !c.service && !c.nocreate && name != "DataModel" && name != "Player",
        None => false,
    }
}

pub fn is_service(name: &str) -> bool {
    class_info(name).map(|c| c.service).unwrap_or(false)
}

pub fn enum_name(enum_type: &str, value: i64) -> Option<&'static str> {
    table().enums.get(enum_type)?.iter().find(|e| e.1 == value).map(|e| e.0.as_str())
}

pub fn prop_era(cls: &str, name: &str) -> i32 {
    let mut cur = class_info(cls);
    while let Some(c) = cur {
        if table().era2018.contains(&format!("{}.{}", c.name, name)) {
            return 2018;
        }
        cur = c.base.as_deref().and_then(class_info);
    }
    2012
}

pub fn class_era(cls: &str) -> i32 {
    class_info(cls).map(|c| c.era).unwrap_or(2012)
}

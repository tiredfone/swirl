use std::collections::BTreeMap;
use swirl_core::instance::DataModel;
use swirl_core::instance::{InstId, Val};

const ROOT: &str = "CoreScriptsRoot/";

#[derive(Default)]
struct Node {
    dirs: BTreeMap<String, Node>,
    files: BTreeMap<String, String>,
}

fn tree_of(files: &BTreeMap<String, String>, prefix: &str) -> Node {
    let mut root = Node::default();
    for (path, src) in files {
        let rest = match path.strip_prefix(prefix) {
            Some(r) => r,
            None => continue,
        };
        let parts: Vec<&str> = rest.split('/').collect();
        let mut node = &mut root;
        for p in &parts[..parts.len() - 1] {
            node = node.dirs.entry(p.to_string()).or_default();
        }
        node.files.insert(parts[parts.len() - 1].to_string(), src.clone());
    }
    root
}

fn script_class(file: &str, top: bool) -> (&'static str, String) {
    if let Some(n) = file.strip_suffix(".client.lua") {
        return ("LocalScript", n.to_string());
    }
    if let Some(n) = file.strip_suffix(".server.lua") {
        return ("Script", n.to_string());
    }
    let n = file.strip_suffix(".lua").unwrap_or(file).to_string();
    (if top { "LocalScript" } else { "ModuleScript" }, n)
}

fn make(dm: &mut DataModel, class: &str, name: &str, source: Option<&str>) -> Option<InstId> {
    let id = dm.create_any(class, None).ok()?;
    dm.set_prop_quiet(id, "Name", Val::Str(name.to_string()));
    dm.get_mut_name(id, name);
    if let Some(s) = source {
        dm.set_prop_quiet(id, "Source", Val::Str(s.to_string()));
    }
    Some(id)
}

fn build_into(dm: &mut DataModel, node: &Node, parent: InstId, top: bool) {
    let mut made: BTreeMap<String, InstId> = BTreeMap::new();
    for (file, src) in &node.files {
        if !file.ends_with(".lua") {
            continue;
        }
        let (cls, name) = script_class(file, top);
        if let Some(id) = make(dm, cls, &name, Some(src)) {
            made.insert(name, id);
        }
    }
    for (dir, sub) in &node.dirs {
        let holder = match made.get(dir) {
            Some(h) => *h,
            None => match make(dm, "Folder", dir, None) {
                Some(h) => {
                    made.insert(dir.clone(), h);
                    h
                }
                None => continue,
            },
        };
        build_into(dm, sub, holder, false);
    }
    for id in made.values() {
        let _ = dm.set_parent(*id, Some(parent));
    }
}

pub struct Bundle {
    files: BTreeMap<String, String>,
}

impl Bundle {
    pub fn load(dir: &str) -> Option<Bundle> {
        let bytes = std::fs::read(format!("{}/corescripts2018/bundle.json", dir)).ok()?;
        let files: BTreeMap<String, String> = serde_json::from_slice(&bytes).ok()?;
        Some(Bundle { files })
    }

    pub fn prepare(&self, dm: &mut DataModel) -> Result<(), String> {
        for s in ["StarterPlayer", "RobloxReplicatedStorage", "ReplicatedFirst", "TweenService", "TextService", "LocalizationService", "HttpService", "MarketplaceService", "CollectionService", "CoreGui", "ScriptContext"] {
            dm.service(s)?;
        }
        let cg = dm.service("CoreGui")?;
        let rg = make(dm, "ScreenGui", "RobloxGui", None).ok_or("RobloxGui")?;
        dm.set_parent(rg, Some(cg))?;
        let modules = make(dm, "Folder", "Modules", None).ok_or("Modules")?;
        build_into(dm, &tree_of(&self.files, &format!("{}Modules/", ROOT)), modules, false);
        dm.set_parent(modules, Some(rg))?;
        let sp = dm.service("StarterPlayer")?;
        let sps = match dm.find_child_of_class(sp, "StarterPlayerScripts") {
            Some(s) => s,
            None => dm.create_any("StarterPlayerScripts", Some(sp))?,
        };
        dm.set_prop_quiet(sps, "Name", Val::Str("StarterPlayerScripts".into()));
        let scs = match dm.find_child_of_class(sp, "StarterCharacterScripts") {
            Some(s) => s,
            None => dm.create_any("StarterCharacterScripts", Some(sp))?,
        };
        dm.set_prop_quiet(scs, "Name", Val::Str("StarterCharacterScripts".into()));
        let ps = tree_of(&self.files, "PlayerScripts/StarterPlayerScripts/");
        for name in ["CameraScript", "ControlScript"] {
            if dm.find_child(sps, name, false).is_some() {
                continue;
            }
            let mut one = Node::default();
            if let Some(f) = ps.files.get(&format!("{}.lua", name)) {
                one.files.insert(format!("{}.lua", name), f.clone());
            }
            if let Some(d) = ps.dirs.get(name) {
                one.dirs.insert(name.to_string(), clone_node(d));
            }
            build_into(dm, &one, sps, true);
        }
        let cs = tree_of(&self.files, "PlayerScripts/StarterCharacterScripts/");
        if dm.find_child(scs, "Sound", false).is_none() {
            build_into(dm, &cs, scs, true);
        }
        Ok(())
    }

    pub fn server_starter(&self, dm: &mut DataModel) -> Option<InstId> {
        let src = self.files.get(&format!("{}ServerStarterScript.lua", ROOT))?;
        let sc = dm.service("ScriptContext").ok()?;
        let id = make(dm, "CoreScript", "ServerStarterScript", Some(src))?;
        dm.set_parent(id, Some(sc)).ok()?;
        Some(id)
    }
}

fn clone_node(n: &Node) -> Node {
    Node { dirs: n.dirs.iter().map(|(k, v)| (k.clone(), clone_node(v))).collect(), files: n.files.clone() }
}

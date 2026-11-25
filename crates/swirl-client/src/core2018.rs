use std::collections::BTreeMap;
use swirl_core::instance::{DataModel, InstId, Val};

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

fn patch_playerlist(src: &str) -> String {
    let a = "      if CUSTOM_ICONS[userIdStr] then\n        return CUSTOM_ICONS[userIdStr]";
    let a2 = "      local perkFolder = player:FindFirstChild(\"SolarPerks\")\n      local perkBadge = perkFolder and perkFolder:FindFirstChild(\"Badge\")\n      if perkBadge and perkBadge.Value ~= \"\" then\n        return perkBadge.Value\n      elseif CUSTOM_ICONS[userIdStr] then\n        return CUSTOM_ICONS[userIdStr]";
    let b = "      local iconImage = getCustomPlayerIcon(player)\n";
    let b2 = "      local iconImage = getCustomPlayerIcon(player)\n      local perkFolder = player:FindFirstChild(\"SolarPerks\")\n      local perkBadge = perkFolder and perkFolder:FindFirstChild(\"Badge\")\n      if perkBadge and perkBadge.Value ~= \"\" then\n        iconImage = perkBadge.Value\n      end\n";
    src.replacen(a, &a2, 1).replacen(b, &b2, 1)
}

fn make(dm: &mut DataModel, class: &str, name: &str, source: Option<&str>) -> Option<InstId> {
    let id = dm.create_any(class, None).ok()?;
    dm.set_prop_quiet(id, "Name", Val::Str(name.to_string()));
    dm.get_mut_name(id, name);
    if let Some(s) = source {
        let patched;
        let s = if name == "PlayerlistModule" {
            patched = patch_playerlist(s);
            patched.as_str()
        } else {
            s
        };
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
        if let Some(i) = make(dm, cls, &name, Some(src)) {
            made.insert(name, i);
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
    for (_, i) in made {
        let _ = dm.set_parent(i, Some(parent));
    }
}

pub fn build_modules(dm: &mut DataModel, files: &BTreeMap<String, String>, gui: InstId) -> Option<InstId> {
    let modules = make(dm, "Folder", "Modules", None)?;
    build_into(dm, &tree_of(files, &format!("{}Modules/", ROOT)), modules, false);
    let _ = dm.set_parent(modules, Some(gui));
    Some(modules)
}

pub fn core_script(dm: &mut DataModel, files: &BTreeMap<String, String>, name: &str, parent: InstId) -> Option<InstId> {
    let src = files.get(&format!("{}{}.lua", ROOT, name))?;
    let id = make(dm, "CoreScript", name, Some(src))?;
    let _ = dm.set_parent(id, Some(parent));
    Some(id)
}

pub fn starter_script(dm: &mut DataModel, files: &BTreeMap<String, String>, parent: InstId) -> Option<InstId> {
    core_script(dm, files, "StarterScript", parent)
}

use crate::fetch::Site;
use swirl_core::instance::{DataModel, InstId, Val};

#[derive(Clone, Default)]
pub struct Look {
    pub colors: Option<[i64; 6]>,
    pub assets: Vec<(u64, Vec<u8>)>,
}

fn int_of(xml: &str, name: &str) -> Option<i64> {
    let key = format!("name=\"{}\">", name);
    let i = xml.find(&key)? + key.len();
    let rest = &xml[i..];
    rest[..rest.find('<')?].trim().parse().ok()
}

fn strip_header(b: Vec<u8>) -> Vec<u8> {
    if b.first() == Some(&b'%') {
        if let Some(p) = b.iter().position(|c| *c == b'\n') {
            return b[p + 1..].to_vec();
        }
    }
    b
}

pub async fn fetch(site: &Site, assets_dir: &str, uid: i64) -> Look {
    let mut look = Look::default();
    let list = match site.get(&format!("/api/charapp/{}", uid)).await {
        Some(b) => String::from_utf8_lossy(&b).to_string(),
        None => return look,
    };
    let mut ids: Vec<u64> = vec![];
    for u in list.split(';').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if u.to_ascii_lowercase().contains("bodycolors") {
            if let Some(b) = site.get(&format!("/Asset/BodyColors.ashx/?userId={}", uid)).await {
                let xml = String::from_utf8_lossy(&b).to_string();
                let names = ["HeadColor", "TorsoColor", "LeftArmColor", "RightArmColor", "LeftLegColor", "RightLegColor"];
                let mut c = [194i64; 6];
                for (k, n) in names.iter().enumerate() {
                    if let Some(v) = int_of(&xml, n) {
                        c[k] = v;
                    }
                }
                look.colors = Some(c);
            }
            continue;
        }
        if let Some(i) = u.find("id=") {
            let digits: String = u[i + 3..].chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse() {
                ids.push(n);
            }
        }
    }
    for id in ids.into_iter().take(40) {
        let local = std::fs::read(format!("{}/assets/{}", assets_dir, id)).ok();
        let bytes = match local {
            Some(b) => Some(b),
            None => site.get(&format!("/asset/?id={}", id)).await,
        };
        if let Some(b) = bytes {
            look.assets.push((id, strip_header(b)));
        }
    }
    look
}

fn cf12(dm: &DataModel, id: InstId, key: &str) -> [f64; 12] {
    crate::character::cf_prop(dm, id, key)
}

fn attach(dm: &mut DataModel, model: InstId, acc: InstId) {
    let head = match dm.find_child(model, "Head", false) {
        Some(h) => h,
        None => return,
    };
    let handle = match dm.find_child(acc, "Handle", false) {
        Some(h) => h,
        None => return,
    };
    let ap = cf12(dm, acc, "AttachmentPoint");
    let top = [0.0, 0.5, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let w = match dm.create_any("Weld", None) {
        Ok(w) => w,
        Err(_) => return,
    };
    dm.set_prop_quiet(w, "Name", Val::Str("HeadWeld".into()));
    dm.get_mut_name(w, "HeadWeld");
    dm.set_prop_quiet(w, "Part0", Val::Ref(Some(head)));
    dm.set_prop_quiet(w, "Part1", Val::Ref(Some(handle)));
    dm.set_prop_quiet(w, "C0", Val::Arr(top.to_vec()));
    dm.set_prop_quiet(w, "C1", Val::Arr(ap.to_vec()));
    let hc = crate::character::mul12(&crate::character::mul12(&cf12(dm, head, "CFrame"), &top), &crate::character::inv12(&ap));
    dm.set_prop_quiet(handle, "CFrame", Val::Arr(hc.to_vec()));
    dm.set_prop_quiet(handle, "CanCollide", Val::Bool(false));
    dm.set_prop_quiet(handle, "Anchored", Val::Bool(false));
    let _ = dm.set_parent(w, Some(handle));
}

fn wearable(dm: &mut DataModel, model: InstId, inst: InstId) {
    let class = match dm.get(inst) {
        Some(i) => i.class.clone(),
        None => return,
    };
    let name = dm.get(inst).map(|i| i.name.clone()).unwrap_or_default();
    if swirl_core::classes::is_a(&class, "Accoutrement") {
        attach(dm, model, inst);
    } else if class == "Decal" && name.to_ascii_lowercase().contains("face") {
        if let Some(head) = dm.find_child(model, "Head", false) {
            if let Some(old) = dm.find_child(head, "face", false) {
                if old != inst {
                    dm.destroy(old);
                }
            }
            let _ = dm.set_parent(inst, Some(head));
            dm.set_prop_quiet(inst, "Face", Val::Num(5.0));
        }
    } else if class == "SpecialMesh" {
        if let Some(head) = dm.find_child(model, "Head", false) {
            let old = dm.find_child_of_class(head, "SpecialMesh");
            if let Some(o) = old {
                if o != inst {
                    dm.destroy(o);
                }
            }
            let _ = dm.set_parent(inst, Some(head));
        }
    } else if swirl_core::classes::is_a(&class, "Tool") || class == "HopperBin" {
        dm.destroy(inst);
    } else if class == "Model" {
        let kids: Vec<InstId> = dm.get(inst).map(|i| i.children.clone()).unwrap_or_default();
        for c in kids {
            let _ = dm.set_parent(c, Some(model));
            wearable(dm, model, c);
        }
        dm.destroy(inst);
    }
}

pub fn apply(dm: &mut DataModel, model: InstId, look: &Look) {
    for (id, bytes) in &look.assets {
        let tree = match swirl_core::rbx::parse(bytes) {
            Ok(t) => t,
            Err(_) => {
                eprintln!("appearance asset {} not readable", id);
                continue;
            }
        };
        let made = dm.load_tree(&tree, model);
        for m in made {
            wearable(dm, model, m);
        }
    }
}

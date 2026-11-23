use crate::character::{self, cf_prop, inv12, mul12, Char};
use swirl_core::instance::{DataModel, InstId, Val};

pub type Scale = [f64; 4];

pub const NO_SCALE: Scale = [1.0, 1.0, 1.0, 1.0];

const PI: f64 = std::f64::consts::PI;

fn id12(p: [f64; 3]) -> [f64; 12] {
    [p[0], p[1], p[2], 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
}

fn rx(a: f64) -> [f64; 12] {
    let (s, c) = a.sin_cos();
    [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, c, -s, 0.0, s, c]
}

fn size_prop(dm: &DataModel, id: InstId) -> [f64; 3] {
    match dm.get_prop(id, "Size") {
        Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
        _ => [1.0; 3],
    }
}

fn attachment(dm: &DataModel, part: InstId, name: &str) -> Option<InstId> {
    let p = dm.get(part)?;
    p.children.iter().copied().find(|c| dm.get(*c).map(|i| i.class == "Attachment" && i.name == name).unwrap_or(false))
}

fn make_attachment(dm: &mut DataModel, part: InstId, name: &str, cf: [f64; 12]) {
    if let Ok(a) = dm.create_any("Attachment", Some(part)) {
        dm.set_prop_quiet(a, "Name", Val::Str(name.into()));
        dm.get_mut_name(a, name);
        dm.set_prop_quiet(a, "CFrame", Val::Arr(cf.to_vec()));
    }
}

pub fn add_missing(dm: &mut DataModel, model: InstId, table: &[(&str, &[(&str, [f64; 3])])]) {
    for (pn, atts) in table {
        let part = match dm.find_child(model, pn, false) {
            Some(p) if dm.is_a(p, "BasePart") => p,
            _ => continue,
        };
        for (an, pos) in atts.iter() {
            if attachment(dm, part, an).is_some() {
                continue;
            }
            make_attachment(dm, part, an, id12(*pos));
        }
    }
}

pub fn r6_attachments(dm: &mut DataModel, model: InstId) {
    add_missing(
        dm,
        model,
        &[
            ("Head", &[("HatAttachment", [0.0, 0.6, 0.0]), ("HairAttachment", [0.0, 0.6, 0.0]), ("FaceFrontAttachment", [0.0, 0.0, -0.6]), ("FaceCenterAttachment", [0.0, 0.0, 0.0])]),
            (
                "Torso",
                &[
                    ("NeckAttachment", [0.0, 1.0, 0.0]), ("BodyFrontAttachment", [0.0, 0.0, -0.5]), ("BodyBackAttachment", [0.0, 0.0, 0.5]),
                    ("LeftCollarAttachment", [-1.0, 1.0, 0.0]), ("RightCollarAttachment", [1.0, 1.0, 0.0]),
                    ("WaistFrontAttachment", [0.0, -1.0, -0.5]), ("WaistCenterAttachment", [0.0, -1.0, 0.0]), ("WaistBackAttachment", [0.0, -1.0, 0.5]),
                ],
            ),
            ("Left Arm", &[("LeftShoulderAttachment", [0.0, 1.0, 0.0])]),
            ("Right Arm", &[("RightShoulderAttachment", [0.0, 1.0, 0.0])]),
            ("Left Leg", &[("LeftFootAttachment", [0.0, -1.0, 0.0])]),
            ("Right Leg", &[("RightFootAttachment", [0.0, -1.0, 0.0])]),
        ],
    );
}

fn r15_attachments(dm: &mut DataModel, model: InstId) {
    add_missing(
        dm,
        model,
        &[
            ("Head", &[("HatAttachment", [0.0, 0.6, 0.0]), ("HairAttachment", [0.0, 0.6, 0.0]), ("FaceFrontAttachment", [0.0, 0.0, -0.6]), ("FaceCenterAttachment", [0.0, 0.0, 0.0])]),
            (
                "UpperTorso",
                &[
                    ("NeckAttachment", [0.0, 0.8, 0.0]), ("BodyFrontAttachment", [0.0, -0.2, -0.5]), ("BodyBackAttachment", [0.0, -0.2, 0.5]),
                    ("LeftCollarAttachment", [-1.0, 0.8, 0.0]), ("RightCollarAttachment", [1.0, 0.8, 0.0]),
                ],
            ),
            ("LowerTorso", &[("WaistFrontAttachment", [0.0, -0.2, -0.5]), ("WaistCenterAttachment", [0.0, -0.2, 0.0]), ("WaistBackAttachment", [0.0, -0.2, 0.5])]),
            ("LeftUpperArm", &[("LeftShoulderAttachment", [0.0, 0.584, 0.0])]),
            ("RightUpperArm", &[("RightShoulderAttachment", [0.0, 0.584, 0.0])]),
        ],
    );
}

fn grip_default() -> [f64; 12] {
    mul12(&id12([0.0, -0.15, 0.0]), &{
        let mut m = rx(-PI / 2.0);
        m[0] = 0.0;
        m
    })
}

fn side_attachments(dm: &mut DataModel, model: InstId) {
    let hand = |dm: &DataModel, side: &str| dm.find_child(model, &format!("{}Hand", side), false);
    let lg = hand(dm, "Left").and_then(|h| attachment(dm, h, "LeftGripAttachment"));
    if let Some(r) = hand(dm, "Right") {
        if attachment(dm, r, "RightGripAttachment").is_none() {
            let mut cf = match lg {
                Some(a) => cf_prop(dm, a, "CFrame"),
                None => grip_default(),
            };
            if lg.is_some() {
                cf[0] = -cf[0];
            }
            make_attachment(dm, r, "RightGripAttachment", cf);
        }
    }
    if let Some(l) = hand(dm, "Left") {
        if attachment(dm, l, "LeftGripAttachment").is_none() {
            make_attachment(dm, l, "LeftGripAttachment", grip_default());
        }
    }
    for side in ["Left", "Right"] {
        if let Some(foot) = dm.find_child(model, &format!("{}Foot", side), false) {
            if dm.is_a(foot, "BasePart") && attachment(dm, foot, &format!("{}FootAttachment", side)).is_none() {
                let h = size_prop(dm, foot)[1] / 2.0;
                make_attachment(dm, foot, &format!("{}FootAttachment", side), id12([0.0, -h, 0.0]));
            }
        }
    }
    let ls = dm.find_child(model, "LeftUpperArm", false).and_then(|p| attachment(dm, p, "LeftShoulderAttachment"));
    let ru = dm.find_child(model, "RightUpperArm", false);
    if let (Some(ls), Some(ru)) = (ls, ru) {
        if attachment(dm, ru, "RightShoulderAttachment").is_none() {
            let mut cf = cf_prop(dm, ls, "CFrame");
            cf[0] = -cf[0];
            make_attachment(dm, ru, "RightShoulderAttachment", cf);
        }
    }
}

fn apply_package(dm: &mut DataModel, model: InstId, parts: &[InstId]) {
    let mut changed = false;
    for src in parts {
        let name = dm.name(*src).to_string();
        let tgt = match dm.find_child(model, &name, false) {
            Some(t) if dm.is_a(t, "BasePart") && name != "HumanoidRootPart" && name != "Head" && dm.is_a(*src, "BasePart") => t,
            _ => continue,
        };
        for k in ["MeshId", "TextureID"] {
            let v = dm.get_prop(*src, k).cloned().unwrap_or(Val::Str(String::new()));
            if dm.get_prop(tgt, k).is_some() {
                dm.set_prop_quiet(tgt, k, v);
            }
        }
        let size = dm.get_prop(*src, "Size").cloned();
        if let Some(s) = size.clone() {
            dm.set_prop_quiet(tgt, "Size", s);
        }
        if dm.get_prop(tgt, "InitialSize").is_some() {
            let init = dm.get_prop(*src, "InitialSize").cloned().or(size);
            if let Some(i) = init {
                dm.set_prop_quiet(tgt, "InitialSize", i);
            }
        }
        let atts: Vec<InstId> = dm.get(*src).map(|i| i.children.clone()).unwrap_or_default().into_iter().filter(|c| dm.get(*c).map(|i| i.class == "Attachment").unwrap_or(false)).collect();
        for a in atts {
            let an = dm.name(a).to_string();
            let cf = cf_prop(dm, a, "CFrame");
            match attachment(dm, tgt, &an) {
                Some(t) => dm.set_prop_quiet(t, "CFrame", Val::Arr(cf.to_vec())),
                None => make_attachment(dm, tgt, &an, cf),
            }
        }
        changed = true;
    }
    if !changed {
        return;
    }
    let mut all = vec![];
    dm.descendants(model, &mut all);
    for j in all.iter().copied() {
        if dm.get(j).map(|i| i.class != "Motor6D").unwrap_or(true) {
            continue;
        }
        let n = format!("{}RigAttachment", dm.name(j));
        let p0 = match dm.get_prop(j, "Part0") {
            Some(Val::Ref(Some(p))) => *p,
            _ => continue,
        };
        let p1 = match dm.get_prop(j, "Part1") {
            Some(Val::Ref(Some(p))) => *p,
            _ => continue,
        };
        if let (Some(a0), Some(a1)) = (attachment(dm, p0, &n), attachment(dm, p1, &n)) {
            let c0 = cf_prop(dm, a0, "CFrame");
            let c1 = cf_prop(dm, a1, "CFrame");
            dm.set_prop_quiet(j, "C0", Val::Arr(c0.to_vec()));
            dm.set_prop_quiet(j, "C1", Val::Arr(c1.to_vec()));
        }
    }
    let hum = dm.find_child_of_class(model, "Humanoid");
    let root = dm.find_child(model, "HumanoidRootPart", false);
    if let (Some(hum), Some(root)) = (hum, root) {
        let joint_of = |dm: &DataModel, p1: InstId| -> Option<InstId> {
            all.iter().copied().find(|d| dm.get(*d).map(|i| i.class == "Motor6D").unwrap_or(false) && matches!(dm.get_prop(*d, "Part1"), Some(Val::Ref(Some(p))) if *p == p1))
        };
        let mut lowest = f64::INFINITY;
        for foot in ["LeftFoot", "RightFoot"] {
            let mut chain: Vec<InstId> = vec![];
            let mut p = dm.find_child(model, foot, false);
            while let Some(cur) = p {
                if cur == root {
                    break;
                }
                let j = match joint_of(dm, cur) {
                    Some(j) => j,
                    None => break,
                };
                chain.insert(0, j);
                p = match dm.get_prop(j, "Part0") {
                    Some(Val::Ref(Some(q))) => Some(*q),
                    _ => None,
                };
            }
            if chain.is_empty() || !matches!(dm.get_prop(chain[0], "Part0"), Some(Val::Ref(Some(q))) if *q == root) {
                continue;
            }
            let mut cf = id12([0.0; 3]);
            for j in chain {
                cf = mul12(&mul12(&cf, &cf_prop(dm, j, "C0")), &inv12(&cf_prop(dm, j, "C1")));
            }
            if let Some(fp) = dm.find_child(model, foot, false) {
                lowest = lowest.min(cf[1] - size_prop(dm, fp)[1] / 2.0);
            }
        }
        if lowest.is_finite() {
            let rh = size_prop(dm, root)[1];
            dm.set_prop_quiet(hum, "HipHeight", Val::Num((-lowest - rh / 2.0).max(0.5)));
        }
    }
}

fn scale_rig(dm: &mut DataModel, model: InstId, hum: Option<InstId>, sc: [f64; 4]) {
    let (w, h, d, hd) = (sc[0], sc[1], sc[2], sc[3]);
    let body = [w, h, d];
    let head = [hd, hd, hd];
    let k_of = |dm: &DataModel, p: Option<InstId>| -> [f64; 3] {
        match p {
            Some(p) if dm.name(p) == "Head" => head,
            _ => body,
        }
    };
    let kids: Vec<InstId> = dm.get(model).map(|i| i.children.clone()).unwrap_or_default();
    for p in kids {
        if !dm.is_a(p, "BasePart") {
            continue;
        }
        let k = k_of(dm, Some(p));
        let sz = size_prop(dm, p);
        dm.set_prop_quiet(p, "Size", Val::Arr(vec![sz[0] * k[0], sz[1] * k[1], sz[2] * k[2]]));
        let cs: Vec<InstId> = dm.get(p).map(|i| i.children.clone()).unwrap_or_default();
        for c in cs {
            let class = dm.get(c).map(|i| i.class.clone()).unwrap_or_default();
            if class == "Attachment" {
                let mut cf = cf_prop(dm, c, "CFrame");
                cf[0] *= k[0];
                cf[1] *= k[1];
                cf[2] *= k[2];
                dm.set_prop_quiet(c, "CFrame", Val::Arr(cf.to_vec()));
            } else if class == "SpecialMesh" {
                if let Some(Val::Arr(ms)) = dm.get_prop(c, "Scale").cloned() {
                    dm.set_prop_quiet(c, "Scale", Val::Arr(vec![ms[0] * k[0], ms[1] * k[1], ms[2] * k[2]]));
                }
                if let Some(Val::Arr(o)) = dm.get_prop(c, "Offset").cloned() {
                    dm.set_prop_quiet(c, "Offset", Val::Arr(vec![o[0] * k[0], o[1] * k[1], o[2] * k[2]]));
                }
            } else if dm.is_a(c, "JointInstance") {
                let p0 = match dm.get_prop(c, "Part0") {
                    Some(Val::Ref(Some(q))) => Some(*q),
                    _ => None,
                };
                let p1 = match dm.get_prop(c, "Part1") {
                    Some(Val::Ref(Some(q))) => Some(*q),
                    _ => None,
                };
                for (key, owner) in [("C0", p0), ("C1", p1)] {
                    if let Some(Val::Arr(mut cf)) = dm.get_prop(c, key).cloned() {
                        let kk = k_of(dm, owner);
                        cf[0] *= kk[0];
                        cf[1] *= kk[1];
                        cf[2] *= kk[2];
                        dm.set_prop_quiet(c, key, Val::Arr(cf));
                    }
                }
            }
        }
    }
    if let Some(hum) = hum {
        let hip = match dm.get_prop(hum, "HipHeight") {
            Some(Val::Num(n)) if *n != 0.0 => *n,
            _ => 2.0,
        };
        dm.set_prop_quiet(hum, "HipHeight", Val::Num(hip * h));
        for (n, v) in [("BodyHeightScale", h), ("BodyWidthScale", w), ("BodyDepthScale", d), ("HeadScale", hd)] {
            let nv = match dm.find_child(hum, n, false) {
                Some(x) if dm.get(x).map(|i| i.class == "NumberValue").unwrap_or(false) => x,
                _ => match dm.create_any("NumberValue", Some(hum)) {
                    Ok(x) => {
                        dm.set_prop_quiet(x, "Name", Val::Str(n.into()));
                        dm.get_mut_name(x, n);
                        x
                    }
                    Err(_) => continue,
                },
            };
            dm.set_prop_quiet(nv, "Value", Val::Num(v));
        }
    }
}

pub fn scale_handle(dm: &mut DataModel, scale: Option<Scale>, handle: InstId, on_head: bool) {
    let sc = match scale {
        Some(s) => s,
        None => return,
    };
    let k = if on_head { sc[3] } else { (sc[0] + sc[1] + sc[2]) / 3.0 };
    if (k - 1.0).abs() < 1e-4 {
        return;
    }
    let sz = size_prop(dm, handle);
    dm.set_prop_quiet(handle, "Size", Val::Arr(vec![sz[0] * k, sz[1] * k, sz[2] * k]));
    let cs: Vec<InstId> = dm.get(handle).map(|i| i.children.clone()).unwrap_or_default();
    for c in cs {
        let class = dm.get(c).map(|i| i.class.clone()).unwrap_or_default();
        if class == "Attachment" {
            let mut cf = cf_prop(dm, c, "CFrame");
            cf[0] *= k;
            cf[1] *= k;
            cf[2] *= k;
            dm.set_prop_quiet(c, "CFrame", Val::Arr(cf.to_vec()));
        } else if dm.is_a(c, "DataModelMesh") {
            if let Some(Val::Arr(ms)) = dm.get_prop(c, "Scale").cloned() {
                dm.set_prop_quiet(c, "Scale", Val::Arr(vec![ms[0] * k, ms[1] * k, ms[2] * k]));
            }
            if let Some(Val::Arr(o)) = dm.get_prop(c, "Offset").cloned() {
                dm.set_prop_quiet(c, "Offset", Val::Arr(vec![o[0] * k, o[1] * k, o[2] * k]));
            }
        }
    }
}

pub fn attach_accessory(dm: &mut DataModel, model: InstId, acc: InstId, scale: Option<Scale>) {
    let head = match dm.find_child(model, "Head", false) {
        Some(h) => h,
        None => return,
    };
    let handle = match dm.find_child(acc, "Handle", false) {
        Some(h) if dm.is_a(h, "BasePart") => h,
        _ => return,
    };
    let hatts: Vec<InstId> = dm.get(handle).map(|i| i.children.clone()).unwrap_or_default().into_iter().filter(|c| dm.get(*c).map(|i| i.class == "Attachment").unwrap_or(false)).collect();
    let mut weld = |dm: &mut DataModel, name: &str, p0: InstId, c0: [f64; 12], c1: [f64; 12]| {
        let w = match dm.create_any("Weld", None) {
            Ok(w) => w,
            Err(_) => return,
        };
        dm.set_prop_quiet(w, "Name", Val::Str(name.into()));
        dm.get_mut_name(w, name);
        dm.set_prop_quiet(w, "Part0", Val::Ref(Some(p0)));
        dm.set_prop_quiet(w, "Part1", Val::Ref(Some(handle)));
        dm.set_prop_quiet(w, "C0", Val::Arr(c0.to_vec()));
        dm.set_prop_quiet(w, "C1", Val::Arr(c1.to_vec()));
        let hc = mul12(&mul12(&cf_prop(dm, p0, "CFrame"), &c0), &inv12(&c1));
        dm.set_prop_quiet(handle, "CFrame", Val::Arr(hc.to_vec()));
        dm.set_prop_quiet(handle, "CanCollide", Val::Bool(false));
        dm.set_prop_quiet(handle, "Anchored", Val::Bool(false));
        let _ = dm.set_parent(w, Some(handle));
    };
    for ha in hatts {
        let name = dm.name(ha).to_string();
        let parts: Vec<InstId> = dm.get(model).map(|i| i.children.clone()).unwrap_or_default();
        let mut found: Option<(InstId, InstId)> = None;
        for p in parts {
            if p == handle || !dm.is_a(p, "BasePart") {
                continue;
            }
            if let Some(a) = attachment(dm, p, &name) {
                found = Some((p, a));
                break;
            }
        }
        if let Some((part, body)) = found {
            let on_head = dm.name(part) == "Head";
            scale_handle(dm, scale, handle, on_head);
            let ha_cf = cf_prop(dm, ha, "CFrame");
            weld(dm, "AccessoryWeld", part, cf_prop(dm, body, "CFrame"), ha_cf);
            return;
        }
    }
    scale_handle(dm, scale, handle, true);
    let hk = scale.map(|s| s[3]).unwrap_or(1.0);
    let mut ap = cf_prop(dm, acc, "AttachmentPoint");
    ap[0] *= hk;
    ap[1] *= hk;
    ap[2] *= hk;
    weld(dm, "HeadWeld", head, id12([0.0, 0.5 * hk, 0.0]), ap);
}

pub struct R15Built {
    pub model: InstId,
    pub parts: Vec<(InstId, [f64; 12])>,
    pub anim: Vec<(String, InstId)>,
}

pub fn build(dm: &mut DataModel, workspace: InstId, player: InstId, name: &str, origin: [f64; 3], look: Option<&crate::appearance::Look>, scale: Scale, template: &[u8]) -> Result<(Char, Vec<(String, InstId)>), String> {
    let tmp = dm.create_any("Folder", None)?;
    let tree = swirl_core::rbx::parse(template).map_err(|e| format!("R15 template: {}", e))?;
    let made = dm.load_tree(&tree, tmp);
    let model = *made.first().ok_or("R15 template empty")?;
    dm.set_prop_quiet(model, "Name", Val::Str(name.into()));
    dm.get_mut_name(model, name);
    let root = dm.find_child(model, "HumanoidRootPart", false).ok_or("R15 template has no root")?;
    let hum = dm.find_child_of_class(model, "Humanoid");

    let mut pkg: Option<InstId> = None;
    let mut anim: Vec<(String, InstId)> = vec![];
    if let Some(l) = look {
        for (id, bytes) in &l.assets {
            let t = match swirl_core::rbx::parse(bytes) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let nodes = dm.load_tree(&t, tmp);
            for f in nodes {
                let class = dm.get(f).map(|i| i.class.clone()).unwrap_or_default();
                let fname = dm.name(f).to_string();
                if class != "Folder" {
                    dm.destroy(f);
                    continue;
                }
                if fname == "R15Anim" {
                    let svs: Vec<InstId> = dm.get(f).map(|i| i.children.clone()).unwrap_or_default();
                    for sv in svs {
                        if dm.get(sv).map(|i| i.class == "StringValue").unwrap_or(false) {
                            let n = dm.name(sv).to_string();
                            anim.retain(|(k, _)| *k != n);
                            anim.push((n, sv));
                        }
                    }
                } else if fname == "R15ArtistIntent" {
                    match pkg {
                        None => pkg = Some(f),
                        Some(p) => {
                            let kids: Vec<InstId> = dm.get(f).map(|i| i.children.clone()).unwrap_or_default();
                            for ch in kids {
                                let cn = dm.name(ch).to_string();
                                if let Some(old) = dm.find_child(p, &cn, false) {
                                    dm.destroy(old);
                                }
                                let _ = dm.set_parent(ch, Some(p));
                            }
                            dm.destroy(f);
                        }
                    }
                } else {
                    dm.destroy(f);
                }
                let _ = id;
            }
        }
    }
    if let Some(p) = pkg {
        let kids: Vec<InstId> = dm.get(p).map(|i| i.children.clone()).unwrap_or_default();
        apply_package(dm, model, &kids);
        dm.destroy(p);
    }
    side_attachments(dm, model);
    r15_attachments(dm, model);
    if scale.iter().any(|v| (v - 1.0).abs() > 1e-4) {
        scale_rig(dm, model, hum, scale);
    }
    let hip = match hum.and_then(|h| dm.get_prop(h, "HipHeight").cloned()) {
        Some(Val::Num(n)) if n != 0.0 => n,
        _ => 1.35,
    };
    let rc = cf_prop(dm, root, "CFrame");
    let target = [origin[0], origin[1] - 3.0 + hip + size_prop(dm, root)[1] / 2.0, origin[2]];
    let d = [target[0] - rc[0], target[1] - rc[1], target[2] - rc[2]];
    let mut all = vec![];
    dm.descendants(model, &mut all);
    let mut parts: Vec<(InstId, [f64; 12])> = vec![];
    let mut dead = vec![];
    for p in all.iter().copied() {
        if dm.is_a(p, "BasePart") {
            let mut cf = cf_prop(dm, p, "CFrame");
            cf[0] += d[0];
            cf[1] += d[1];
            cf[2] += d[2];
            dm.set_prop_quiet(p, "CFrame", Val::Arr(cf.to_vec()));
            dm.set_prop_quiet(p, "Locked", Val::Bool(true));
        } else if dm.get(p).map(|i| i.class == "Vector3Value" && i.name == "OriginalSize").unwrap_or(false) {
            dead.push(p);
        }
    }
    for p in dead {
        dm.destroy(p);
    }
    dm.set_prop_quiet(model, "PrimaryPart", Val::Ref(Some(root)));
    if let Some(h) = hum {
        dm.set_prop_quiet(h, "RigType", Val::Num(1.0));
    }
    let rcf = cf_prop(dm, root, "CFrame");
    let rinv = inv12(&rcf);
    let mut all2 = vec![];
    dm.descendants(model, &mut all2);
    for p in all2 {
        if dm.is_a(p, "BasePart") {
            parts.push((p, mul12(&rinv, &cf_prop(dm, p, "CFrame"))));
        }
    }
    let _ = (workspace, PI);
    dm.set_parent(model, None)?;
    let tmp_kids = dm.get(tmp).map(|i| i.children.clone()).unwrap_or_default();
    let _ = tmp_kids;
    Ok((Char { player, model, torso: root, parts, cf: rcf, vel: [0.0; 3], state: 7, seq: 1, j: None }, anim))
}

pub fn color_parts(dm: &mut DataModel, model: InstId) {
    let bc = match dm.find_child_of_class(model, "BodyColors") {
        Some(b) => b,
        None => return,
    };
    let map: [(&str, &str); 15] = [
        ("Head", "HeadColor"), ("UpperTorso", "TorsoColor"), ("LowerTorso", "TorsoColor"),
        ("LeftUpperArm", "LeftArmColor"), ("LeftLowerArm", "LeftArmColor"), ("LeftHand", "LeftArmColor"),
        ("RightUpperArm", "RightArmColor"), ("RightLowerArm", "RightArmColor"), ("RightHand", "RightArmColor"),
        ("LeftUpperLeg", "LeftLegColor"), ("LeftLowerLeg", "LeftLegColor"), ("LeftFoot", "LeftLegColor"),
        ("RightUpperLeg", "RightLegColor"), ("RightLowerLeg", "RightLegColor"), ("RightFoot", "RightLegColor"),
    ];
    for (pn, prop) in map {
        if let (Some(p), Some(Val::Num(n))) = (dm.find_child(model, pn, false), dm.get_prop(bc, prop).cloned()) {
            let c = swirl_core::rbx::brick_rgb01(n as i64);
            dm.set_prop_quiet(p, "BrickColor", Val::Num(n));
            dm.set_prop_quiet(p, "Color", Val::Arr(c.to_vec()));
        }
    }
    let _ = character::chocolate_dir;
}

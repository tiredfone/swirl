use std::collections::HashMap;
use swirl_core::brickcolors;
use swirl_core::instance::{DataModel, InstId, Val};

pub struct Char {
    pub player: InstId,
    pub model: InstId,
    pub torso: InstId,
    pub parts: Vec<(InstId, [f64; 12])>,
    pub cf: [f64; 12],
    pub vel: [f64; 3],
    pub state: u8,
    pub seq: i64,
    pub j: Option<[f64; 4]>,
}

fn id12(p: [f64; 3]) -> [f64; 12] {
    [p[0], p[1], p[2], 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
}

fn brick_color(dm: &mut DataModel, id: InstId, n: i64) {
    let c = swirl_core::rbx::brick_rgb01(n);
    dm.set_prop_quiet(id, "BrickColor", Val::Num(n as f64));
    dm.set_prop_quiet(id, "Color", Val::Arr(c.to_vec()));
    let _ = brickcolors::rgb(n);
}

fn make_part(dm: &mut DataModel, parent: InstId, name: &str, size: [f64; 3], offset: [f64; 3], origin: [f64; 3], brick: i64) -> Result<(InstId, [f64; 12]), String> {
    let id = dm.create_any("Part", None)?;
    dm.set_prop_quiet(id, "Name", Val::Str(name.to_string()));
    dm.get_mut_name(id, name);
    dm.set_prop_quiet(id, "Size", Val::Arr(size.to_vec()));
    dm.set_prop_quiet(id, "CFrame", Val::Arr(id12([origin[0] + offset[0], origin[1] + offset[1], origin[2] + offset[2]]).to_vec()));
    dm.set_prop_quiet(id, "Locked", Val::Bool(true));
    dm.set_prop_quiet(id, "FormFactor", Val::Num(3.0));
    dm.set_prop_quiet(id, "TopSurface", Val::Num(0.0));
    dm.set_prop_quiet(id, "BottomSurface", Val::Num(0.0));
    brick_color(dm, id, brick);
    dm.set_parent(id, Some(parent))?;
    Ok((id, id12(offset)))
}

fn rx(a: f64) -> [f64; 12] {
    let (s, c) = a.sin_cos();
    [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, c, -s, 0.0, s, c]
}

fn ry(a: f64) -> [f64; 12] {
    let (s, c) = a.sin_cos();
    [0.0, 0.0, 0.0, c, 0.0, s, 0.0, 1.0, 0.0, -s, 0.0, c]
}

fn rz(a: f64) -> [f64; 12] {
    let (s, c) = a.sin_cos();
    [0.0, 0.0, 0.0, c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0]
}

fn ang(x: f64, y: f64, z: f64, a: f64, b: f64, c: f64) -> [f64; 12] {
    let mut t = id12([x, y, z]);
    t = mul12(&t, &rx(a));
    t = mul12(&t, &ry(b));
    mul12(&t, &rz(c))
}

fn add_joints(dm: &mut DataModel, model: InstId, torso: InstId, era: i32, parts: &mut Vec<(InstId, [f64; 12])>, origin: [f64; 3]) -> Result<(), String> {
    use std::f64::consts::PI;
    let joints: [(&str, &str, [f64; 12], [f64; 12]); 5] = [
        ("Neck", "Head", ang(0.0, 1.0, 0.0, -PI / 2.0, 0.0, PI), ang(0.0, -0.5, 0.0, -PI / 2.0, 0.0, PI)),
        ("Right Shoulder", "Right Arm", ang(1.0, 0.5, 0.0, 0.0, PI / 2.0, 0.0), ang(-0.5, 0.5, 0.0, 0.0, PI / 2.0, 0.0)),
        ("Left Shoulder", "Left Arm", ang(-1.0, 0.5, 0.0, 0.0, -PI / 2.0, 0.0), ang(0.5, 0.5, 0.0, 0.0, -PI / 2.0, 0.0)),
        ("Right Hip", "Right Leg", ang(1.0, -1.0, 0.0, 0.0, PI / 2.0, 0.0), ang(0.5, 1.0, 0.0, 0.0, PI / 2.0, 0.0)),
        ("Left Hip", "Left Leg", ang(-1.0, -1.0, 0.0, 0.0, -PI / 2.0, 0.0), ang(-0.5, 1.0, 0.0, 0.0, -PI / 2.0, 0.0)),
    ];
    for (name, p1, c0, c1) in joints {
        let part = dm.find_child(model, p1, false).ok_or("missing part")?;
        let j = dm.create_any("Motor6D", Some(torso))?;
        dm.set_prop_quiet(j, "Name", Val::Str(name.to_string()));
        dm.get_mut_name(j, name);
        dm.set_prop_quiet(j, "Part0", Val::Ref(Some(torso)));
        dm.set_prop_quiet(j, "Part1", Val::Ref(Some(part)));
        dm.set_prop_quiet(j, "C0", Val::Arr(c0.to_vec()));
        dm.set_prop_quiet(j, "C1", Val::Arr(c1.to_vec()));
        dm.set_prop_quiet(j, "MaxVelocity", Val::Num(0.1));
    }
    if era == 2018 {
        let (root, off) = make_part(dm, model, "HumanoidRootPart", [2.0, 2.0, 1.0], [0.0, 0.0, 0.0], origin, 194)?;
        dm.set_prop_quiet(root, "Transparency", Val::Num(1.0));
        dm.set_prop_quiet(root, "CanCollide", Val::Bool(false));
        let r = [0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0];
        let j = dm.create_any("Motor6D", Some(root))?;
        dm.set_prop_quiet(j, "Name", Val::Str("RootJoint".into()));
        dm.get_mut_name(j, "RootJoint");
        dm.set_prop_quiet(j, "Part0", Val::Ref(Some(root)));
        dm.set_prop_quiet(j, "Part1", Val::Ref(Some(torso)));
        dm.set_prop_quiet(j, "C0", Val::Arr(r.to_vec()));
        dm.set_prop_quiet(j, "C1", Val::Arr(r.to_vec()));
        dm.set_prop_quiet(j, "MaxVelocity", Val::Num(0.1));
        dm.set_prop_quiet(model, "PrimaryPart", Val::Ref(Some(root)));
        parts.push((root, off));
    }
    Ok(())
}

pub fn build(dm: &mut DataModel, workspace: InstId, player: InstId, name: &str, origin: [f64; 3], look: Option<&crate::appearance::Look>, era: i32, r15: Option<(crate::r15::Scale, Vec<u8>)>) -> Result<(Char, Vec<(String, InstId)>), String> {
    let is_r15 = r15.is_some();
    let scale_of = r15.as_ref().map(|(s, _)| *s);
    let (model, torso, parts, anim) = if let Some((sc, tpl)) = r15 {
        let (ch, anim) = crate::r15::build(dm, workspace, player, name, origin, look, sc, &tpl)?;
        (ch.model, ch.torso, ch.parts, anim)
    } else {
        let model = dm.create_any("Model", None)?;
        dm.set_prop_quiet(model, "Name", Val::Str(name.to_string()));
        dm.get_mut_name(model, name);
        let hum = dm.create_any("Humanoid", Some(model))?;
        dm.set_prop_quiet(hum, "Name", Val::Str("Humanoid".into()));
        dm.get_mut_name(hum, "Humanoid");
        dm.set_prop_quiet(hum, "Health", Val::Num(100.0));
        dm.set_prop_quiet(hum, "MaxHealth", Val::Num(100.0));
        dm.set_prop_quiet(hum, "WalkSpeed", Val::Num(16.0));
        dm.set_prop_quiet(hum, "JumpPower", Val::Num(50.0));
        let (torso, torso_off) = make_part(dm, model, "Torso", [2.0, 2.0, 1.0], [0.0, 0.0, 0.0], origin, 23)?;
        let mut parts = vec![(torso, torso_off)];
        let defs: [(&str, [f64; 3], [f64; 3], i64); 5] = [
            ("Head", [2.0, 1.0, 1.0], [0.0, 1.5, 0.0], 24),
            ("Left Arm", [1.0, 2.0, 1.0], [-1.5, 0.0, 0.0], 24),
            ("Right Arm", [1.0, 2.0, 1.0], [1.5, 0.0, 0.0], 24),
            ("Left Leg", [1.0, 2.0, 1.0], [-0.5, -2.0, 0.0], 119),
            ("Right Leg", [1.0, 2.0, 1.0], [0.5, -2.0, 0.0], 119),
        ];
        for (n, size, off, b) in defs {
            let (id, o) = make_part(dm, model, n, size, off, origin, b)?;
            parts.push((id, o));
            if n == "Head" {
                let mesh = dm.create_any("SpecialMesh", Some(id))?;
                dm.set_prop_quiet(mesh, "MeshType", Val::Num(0.0));
                dm.set_prop_quiet(mesh, "Scale", Val::Arr(vec![1.25, 1.25, 1.25]));
            }
        }
        dm.set_prop_quiet(model, "PrimaryPart", Val::Ref(Some(torso)));
        add_joints(dm, model, torso, era, &mut parts, origin)?;
        crate::r15::r6_attachments(dm, model);
        (model, torso, parts, vec![])
    };
    if let Some(l) = look {
        if let Some(c) = l.colors {
            let order = ["Head", "Torso", "Left Arm", "Right Arm", "Left Leg", "Right Leg"];
            let props = ["HeadColor", "TorsoColor", "LeftArmColor", "RightArmColor", "LeftLegColor", "RightLegColor"];
            let bc = dm.create_any("BodyColors", Some(model))?;
            dm.set_prop_quiet(bc, "Name", Val::Str("Body Colors".into()));
            dm.get_mut_name(bc, "Body Colors");
            for k in 0..6 {
                if !is_r15 {
                    if let Some(p) = dm.find_child(model, order[k], false) {
                        brick_color(dm, p, c[k]);
                    }
                }
                dm.set_prop_quiet(bc, props[k], Val::Num(c[k] as f64));
            }
        }
        if is_r15 {
            crate::r15::color_parts(dm, model);
        }
        crate::appearance::apply(dm, model, l, scale_of);
    }
    dm.set_parent(model, Some(workspace))?;
    dm.set_prop(player, "Character", Val::Ref(Some(model)))?;
    let cf = if is_r15 { cf_prop(dm, torso, "CFrame") } else { id12(origin) };
    Ok((Char { player, model, torso, parts, cf, vel: [0.0; 3], state: 7, seq: 1, j: None }, anim))
}

pub fn assign_team(dm: &mut DataModel, player: InstId) {
    let teams = match dm.services.get("Teams") {
        Some(t) => *t,
        None => return,
    };
    let list: Vec<InstId> = dm.get(teams).map(|t| t.children.clone()).unwrap_or_default().into_iter().filter(|c| dm.get(*c).map(|i| i.class == "Team").unwrap_or(false) && matches!(dm.get_prop(*c, "AutoAssignable"), Some(Val::Bool(true)))).collect();
    if list.is_empty() {
        let _ = dm.set_prop(player, "Neutral", Val::Bool(true));
        return;
    }
    let players = dm.services.get("Players").copied();
    let members: Vec<InstId> = players.and_then(|p| dm.get(p)).map(|p| p.children.clone()).unwrap_or_default();
    let mut best = list[0];
    let mut bn = usize::MAX;
    for t in &list {
        let col = dm.get_prop(*t, "TeamColor").cloned();
        let n = members.iter().filter(|m| **m != player && matches!(dm.get_prop(**m, "Neutral"), Some(Val::Bool(false))) && dm.get_prop(**m, "TeamColor").cloned() == col).count();
        if n < bn {
            bn = n;
            best = *t;
        }
    }
    let col = dm.get_prop(best, "TeamColor").cloned().unwrap_or(Val::Num(194.0));
    let _ = dm.set_prop(player, "Neutral", Val::Bool(false));
    let _ = dm.set_prop(player, "TeamColor", col);
}

pub fn spawn_origin(dm: &DataModel, workspace: InstId, player: InstId) -> ([f64; 3], Option<InstId>) {
    let mut all = vec![];
    dm.descendants(workspace, &mut all);
    let spawns: Vec<InstId> = all
        .into_iter()
        .filter(|id| dm.get(*id).map(|i| i.class == "SpawnLocation").unwrap_or(false) && !matches!(dm.get_prop(*id, "Enabled"), Some(Val::Bool(false))))
        .collect();
    let neutral = |s: &InstId| matches!(dm.get_prop(*s, "Neutral"), Some(Val::Bool(true)));
    let pool: Vec<InstId> = if !matches!(dm.get_prop(player, "Neutral"), Some(Val::Bool(true))) {
        let col = dm.get_prop(player, "TeamColor").cloned();
        let team: Vec<InstId> = spawns.iter().copied().filter(|s| !neutral(s) && dm.get_prop(*s, "TeamColor").cloned() == col).collect();
        if team.is_empty() {
            spawns.iter().copied().filter(|s| neutral(s)).collect()
        } else {
            team
        }
    } else {
        let n: Vec<InstId> = spawns.iter().copied().filter(|s| neutral(s)).collect();
        if n.is_empty() {
            spawns.clone()
        } else {
            n
        }
    };
    let pool = if pool.is_empty() { spawns } else { pool };
    let pick = if pool.is_empty() {
        None
    } else {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() as usize).unwrap_or(0);
        Some(pool[n % pool.len()])
    };
    if let Some(s) = pick {
        if let (Some(Val::Arr(cf)), Some(Val::Arr(sz))) = (dm.get_prop(s, "CFrame"), dm.get_prop(s, "Size")) {
            if cf.len() >= 12 && sz.len() >= 3 {
                let top = cf[1] + cf[7].abs() * sz[1] / 2.0 + cf[6].abs() * sz[0] / 2.0 + cf[8].abs() * sz[2] / 2.0;
                return ([cf[0], top + 3.2, cf[2]], Some(s));
            }
        }
    }
    ([0.0, 5.0, 0.0], None)
}

pub fn mul12(a: &[f64; 12], b: &[f64; 12]) -> [f64; 12] {
    let mut r = [0.0; 9];
    for i in 0..3 {
        for j in 0..3 {
            r[i * 3 + j] = a[3 + i * 3] * b[3 + j] + a[4 + i * 3] * b[6 + j] + a[5 + i * 3] * b[9 + j];
        }
    }
    [
        a[3] * b[0] + a[4] * b[1] + a[5] * b[2] + a[0],
        a[6] * b[0] + a[7] * b[1] + a[8] * b[2] + a[1],
        a[9] * b[0] + a[10] * b[1] + a[11] * b[2] + a[2],
        r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8],
    ]
}

pub fn inv12(a: &[f64; 12]) -> [f64; 12] {
    let r = [a[3], a[6], a[9], a[4], a[7], a[10], a[5], a[8], a[11]];
    let p = [
        -(r[0] * a[0] + r[1] * a[1] + r[2] * a[2]),
        -(r[3] * a[0] + r[4] * a[1] + r[5] * a[2]),
        -(r[6] * a[0] + r[7] * a[1] + r[8] * a[2]),
    ];
    [p[0], p[1], p[2], r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8]]
}

pub fn apply_cf(dm: &mut DataModel, ch: &mut Char, cf: [f64; 12]) {
    ch.cf = cf;
    for (id, off) in ch.parts.clone() {
        let w = mul12(&cf, &off);
        dm.set_prop_quiet(id, "CFrame", Val::Arr(w.to_vec()));
    }
    sync_tools(dm, ch);
}

pub const RIGHT_GRIP_C0: [f64; 12] = [0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0];

pub fn arm_of(dm: &DataModel, model: InstId) -> Option<InstId> {
    dm.find_child(model, "Right Arm", false)
}

pub fn cf_prop(dm: &DataModel, id: InstId, key: &str) -> [f64; 12] {
    match dm.get_prop(id, key) {
        Some(Val::Arr(a)) if a.len() >= 12 => {
            let mut c = [0.0; 12];
            c.copy_from_slice(&a[..12]);
            c
        }
        _ => [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    }
}

pub fn sync_tools(dm: &mut DataModel, ch: &Char) {
    let arm = match arm_of(dm, ch.model) {
        Some(a) => a,
        None => return,
    };
    let weld = match dm.find_child(arm, "RightGrip", false) {
        Some(w) => w,
        None => return,
    };
    let handle = match dm.get_prop(weld, "Part1") {
        Some(Val::Ref(Some(h))) => *h,
        _ => return,
    };
    let c0 = cf_prop(dm, weld, "C0");
    let tool = dm.get(handle).and_then(|h| h.parent);
    let mut c1 = cf_prop(dm, weld, "C1");
    if let Some(t) = tool {
        let g = cf_prop(dm, t, "Grip");
        if g != c1 {
            c1 = g;
            let _ = dm.set_prop(weld, "C1", Val::Arr(g.to_vec()));
        }
    }
    let w = mul12(&mul12(&cf_prop(dm, arm, "CFrame"), &c0), &inv12(&c1));
    dm.set_prop_quiet(handle, "CFrame", Val::Arr(w.to_vec()));
}

pub type Chars = HashMap<InstId, Char>;

static CONTENT_DIR: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn chocolate_dir() -> Option<&'static str> {
    CONTENT_DIR.get().map(|s| s.as_str())
}

pub fn set_content_dir(dir: &str) {
    let _ = CONTENT_DIR.set(dir.to_string());
}

fn load_scripts(dm: &mut DataModel, model: InstId, path: &str, only: Option<&str>) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return,
    };
    let tree = match swirl_core::rbx::parse(&bytes) {
        Ok(t) => t,
        Err(_) => return,
    };
    let made = dm.load_tree(&tree, model);
    for m in made {
        let name = dm.get(m).map(|i| i.name.clone()).unwrap_or_default();
        if let Some(o) = only {
            if name != o {
                dm.destroy(m);
                continue;
            }
        }
        if let Some(gui) = dm.find_child(m, "HealthGUI", false) {
            if let Some(tray) = dm.find_child(gui, "Tray", false) {
                let _ = dm.set_prop(tray, "Visible", Val::Bool(false));
            }
        }
    }
}

pub fn add_scripts_r15(dm: &mut DataModel, model: InstId) {
    let dir = match CONTENT_DIR.get() {
        Some(d) => d.clone(),
        None => return,
    };
    load_scripts_except(dm, model, &format!("{}/character/humanoidAnimateLocalKeyframe.rbxm", dir), "Animate");
    load_scripts(dm, model, &format!("{}/character/humanoidHealth.rbxm", dir), Some("Health"));
    load_scripts(dm, model, &format!("{}/character/humanoidAnimateR15.rbxm", dir), None);
}

fn load_scripts_except(dm: &mut DataModel, model: InstId, path: &str, skip: &str) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return,
    };
    let tree = match swirl_core::rbx::parse(&bytes) {
        Ok(t) => t,
        Err(_) => return,
    };
    for m in dm.load_tree(&tree, model) {
        if dm.name(m) == skip {
            dm.destroy(m);
        }
    }
}

pub fn add_scripts(dm: &mut DataModel, model: InstId, era: i32) {
    let dir = match CONTENT_DIR.get() {
        Some(d) => d.clone(),
        None => return,
    };
    if era == 2018 {
        load_scripts(dm, model, &format!("{}/character/humanoidAnimateLocalKeyframe.rbxm", dir), None);
        load_scripts(dm, model, &format!("{}/character/humanoidHealth.rbxm", dir), Some("Health"));
    } else {
        for f in ["humanoidAnimateLocal", "humanoidSound", "humanoidHealth"] {
            load_scripts(dm, model, &format!("{}/character/{}.rbxm", dir, f), None);
        }
    }
}

pub fn copy_player_scripts(dm: &mut DataModel, player: InstId) {
    let ps = match dm.create_any("PlayerScripts", Some(player)) {
        Ok(p) => p,
        Err(_) => return,
    };
    dm.set_prop_quiet(ps, "Name", Val::Str("PlayerScripts".into()));
    dm.get_mut_name(ps, "PlayerScripts");
    let sp = match dm.services.get("StarterPlayer") {
        Some(s) => *s,
        None => return,
    };
    let sps = match dm.find_child_of_class(sp, "StarterPlayerScripts") {
        Some(s) => s,
        None => return,
    };
    let kids: Vec<InstId> = dm.get(sps).map(|s| s.children.clone()).unwrap_or_default();
    for k in kids {
        if let Some(c) = dm.clone_tree_always(k) {
            let _ = dm.set_parent(c, Some(ps));
        }
    }
}

pub fn copy_character_scripts(dm: &mut DataModel, model: InstId) {
    let sp = match dm.services.get("StarterPlayer") {
        Some(s) => *s,
        None => return,
    };
    let scs = match dm.find_child_of_class(sp, "StarterCharacterScripts") {
        Some(s) => s,
        None => return,
    };
    let kids: Vec<InstId> = dm.get(scs).map(|s| s.children.clone()).unwrap_or_default();
    for k in kids {
        let name = dm.get(k).map(|i| i.name.clone()).unwrap_or_default();
        if let Some(old) = dm.find_child(model, &name, false) {
            dm.destroy(old);
        }
        if let Some(c) = dm.clone_tree_always(k) {
            let _ = dm.set_parent(c, Some(model));
        }
    }
}

pub fn copy_starter_gui(dm: &mut DataModel, player: InstId, first: bool) {
    let gui = match dm.find_child_of_class(player, "PlayerGui") {
        Some(g) => g,
        None => return,
    };
    let sg = match dm.services.get("StarterGui") {
        Some(s) => *s,
        None => return,
    };
    if !first {
        let reset = !matches!(dm.get_prop(sg, "ResetPlayerGuiOnSpawn"), Some(Val::Bool(false)));
        if !reset {
            return;
        }
        let old: Vec<InstId> = dm.get(gui).map(|g| g.children.clone()).unwrap_or_default();
        for c in old {
            if dm.is_a(c, "ScreenGui") && matches!(dm.get_prop(c, "ResetOnSpawn"), Some(Val::Bool(false))) {
                continue;
            }
            dm.destroy(c);
        }
    }
    let kids: Vec<InstId> = dm.get(sg).map(|s| s.children.clone()).unwrap_or_default();
    for k in kids {
        if let Some(c) = dm.clone_tree(k) {
            let _ = dm.set_parent(c, Some(gui));
        }
    }
}

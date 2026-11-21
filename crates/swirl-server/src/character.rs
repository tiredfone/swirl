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

pub fn build(dm: &mut DataModel, workspace: InstId, player: InstId, name: &str, origin: [f64; 3]) -> Result<Char, String> {
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
    dm.set_parent(model, Some(workspace))?;
    dm.set_prop(player, "Character", Val::Ref(Some(model)))?;
    Ok(Char { player, model, torso, parts, cf: id12(origin), vel: [0.0; 3], state: 7, seq: 1 })
}

pub fn spawn_origin(dm: &DataModel, workspace: InstId) -> [f64; 3] {
    let mut all = vec![];
    dm.descendants(workspace, &mut all);
    for id in all {
        if let Some(i) = dm.get(id) {
            if i.class == "SpawnLocation" {
                if let (Some(Val::Arr(cf)), Some(Val::Arr(sz))) = (i.props.get("CFrame"), i.props.get("Size")) {
                    if cf.len() >= 3 && sz.len() >= 3 {
                        return [cf[0], cf[1] + sz[1] / 2.0 + 3.0, cf[2]];
                    }
                }
            }
        }
    }
    [0.0, 5.0, 0.0]
}

pub fn apply_cf(dm: &mut DataModel, ch: &mut Char, cf: [f64; 12]) {
    ch.cf = cf;
    let mul = |a: &[f64; 12], b: &[f64; 12]| -> [f64; 12] {
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
    };
    for (id, off) in ch.parts.clone() {
        let w = mul(&cf, &off);
        dm.set_prop_quiet(id, "CFrame", Val::Arr(w.to_vec()));
    }
}

pub type Chars = HashMap<InstId, Char>;

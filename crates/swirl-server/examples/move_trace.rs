use swirl_core::instance::Val;
use swirl_physics::humanoid::{Controller, Input};
use swirl_physics::Physics;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let variant = a[2].as_str();
    let era = if variant == "r15" { 2018 } else { 2012 };
    swirl_server::character::set_content_dir(&swirl_server::config::default_data_dir());
    let spec: serde_json::Value = serde_json::from_slice(&std::fs::read(&a[1]).unwrap()).unwrap();
    let r15 = if variant == "r15" {
        Some(std::fs::read(format!("{}/character/characterR15V2.rbxm", swirl_server::character::chocolate_dir().unwrap())).unwrap())
    } else {
        None
    };
    let mut out = serde_json::Map::new();
    for (name, sc) in spec.as_object().unwrap() {
        let mut dm = swirl_core::instance::DataModel::new("client");
        dm.era = era;
        let ws = dm.service("Workspace").unwrap();
        for p in sc["parts"].as_array().unwrap() {
            let id = dm.create_any("Part", None).unwrap();
            dm.set_prop_quiet(id, "Name", Val::Str(p[0].as_str().unwrap().into()));
            dm.set_prop_quiet(id, "Anchored", Val::Bool(true));
            dm.set_prop_quiet(id, "Size", Val::Arr(p[2].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect()));
            dm.set_prop_quiet(id, "CFrame", Val::Arr(p[1].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect()));
            dm.set_parent(id, Some(ws)).unwrap();
        }
        let mut ph = Physics::new();
        ph.attach_workspace(&mut dm, ws);
        let players = dm.service("Players").unwrap();
        let player = dm.create_any("Player", Some(players)).unwrap();
        let tpl = r15.clone().map(|b| ([1.0, 1.0, 1.0, 1.0], b));
        let (ch, _) = swirl_server::character::build(&mut dm, ws, player, "Tester", [0.0, 4.0, 0.0], None, era, tpl).unwrap();
        ph.exclude_model(&dm, ch.model);
        let hum = dm.find_child_of_class(ch.model, "Humanoid").unwrap();
        let mut ctrl = Controller::new(&mut ph, &dm, ch.model, hum);
        let c0 = [0.0, 4.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        ctrl.teleport(&mut ph, &dm, &c0);
        let mut trace = vec![];
        let dts: Vec<f64> = sc.get("dts").map(|d| d.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect()).unwrap_or(vec![1.0 / 60.0]);
        let mut di = 0usize;
        for st in sc["steps"].as_array().unwrap() {
            let face = st.get("face").map(|f| [f[0].as_f64().unwrap(), f[1].as_f64().unwrap(), f[2].as_f64().unwrap()]);
            let mv = st["mv"].as_array().unwrap();
            let mv = [mv[0].as_f64().unwrap(), mv[1].as_f64().unwrap(), mv[2].as_f64().unwrap()];
            let jump = st.get("jump").is_some();
            let mut tt = 0.0;
            while tt < st["s"].as_f64().unwrap() - 1e-9 {
                let dt = dts[di % dts.len()];
                di += 1;
                tt += dt;
                let r = ctrl.update(&mut ph, &mut dm, dt, &Input { mv, jump, face });
                ph.stir();
                ph.step(&mut dm, 1.0 / 60.0);
                trace.push(serde_json::json!([r.cf[0], r.cf[1], r.cf[2], r.state]));
            }
        }
        out.insert(name.clone(), serde_json::Value::Array(trace));
    }
    std::fs::write(&a[3], serde_json::Value::Object(out).to_string()).unwrap();
}

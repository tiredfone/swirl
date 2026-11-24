use swirl_core::instance::Val;
use swirl_server::game::Game;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let place: u64 = a[1].parse().unwrap();
    let era: i32 = a[2].parse().unwrap();
    let ticks: usize = a[3].parse().unwrap();
    let model = a.get(5).cloned().unwrap_or("Dummy".into());
    swirl_server::character::set_content_dir(&swirl_server::config::default_data_dir());
    let bytes = std::fs::read(format!("{}/assets/{}", std::env::var("ASSETS").unwrap_or("/tmp/swirl/fakeassets".into()), place)).unwrap();
    let mut g = Game::load_with(place, &bytes, era, &std::env::var("ASSETS").unwrap_or("/tmp/swirl/fakeassets".into()), "/tmp/swirl/trace-store").unwrap();
    if let Ok(src) = std::env::var("RUN") {
        g.run_code(&src, "trace");
    }
    let mut trace: Vec<serde_json::Value> = vec![];
    let mut lines = String::new();
    for _ in 0..ticks {
        for l in g.tick(1.0 / 30.0) {
            if matches!(l.kind.as_str(), "print" | "error" | "warn") {
                lines.push_str(&format!("[{}] {}\n", l.kind, l.msg));
            }
        }
        for (id, url) in g.host.take_linked() {
            let r = swirl_server::instance::linked_from_disk(&std::env::var("ASSETS").unwrap_or("/tmp/swirl/fakeassets".into()), &url).unwrap_or(Err("HTTP error".into()));
            g.host.provide_linked(id, r);
        }
        for id in g.union_requests() {
            let bytes = std::fs::read(format!("{}/assets/{}", std::env::var("ASSETS").unwrap_or("/tmp/swirl/fakeassets".into()), id)).ok();
            g.union_done(id, bytes);
        }
        g.flush_all(&[]);
        let dm = g.dm.borrow();
        let ws = dm.services["Workspace"];
        let kids = dm.get(ws).unwrap().children.clone();
        let mut row = vec![];
        if let Some(m) = kids.iter().find(|c| dm.name(**c) == model) {
            let mut flat: Vec<swirl_core::instance::InstId> = vec![];
            for p in dm.get(*m).unwrap().children.clone() {
                flat.push(p);
                if dm.get(p).map(|i| i.class == "Tool").unwrap_or(false) {
                    for q in dm.get(p).unwrap().children.clone() {
                        flat.push(q);
                    }
                }
            }
            for p in flat {
                if dm.is_a(p, "BasePart") {
                    let cf = match dm.get_prop(p, "CFrame") { Some(Val::Arr(x)) => x.clone(), _ => vec![] };
                    let v = match dm.get_prop(p, "Velocity") { Some(Val::Arr(x)) => x.clone(), _ => vec![] };
                    row.push(serde_json::json!([dm.name(p), cf, v]));
                }
            }
        }
        for p in kids {
            if dm.is_a(p, "BasePart") && !matches!(dm.get_prop(p, "Anchored"), Some(Val::Bool(true))) {
                let cf = match dm.get_prop(p, "CFrame") { Some(Val::Arr(x)) => x.clone(), _ => vec![] };
                let v = match dm.get_prop(p, "Velocity") { Some(Val::Arr(x)) => x.clone(), _ => vec![] };
                row.push(serde_json::json!([dm.name(p), cf, v]));
            }
        }
        trace.push(serde_json::Value::Array(row));
    }
    if std::env::var("COUNT").is_ok() {
        let dm = g.dm.borrow();
        let ws = dm.services["Workspace"];
        let mut all = vec![];
        dm.descendants(ws, &mut all);
        let mut cnt: std::collections::BTreeMap<String, usize> = Default::default();
        for i in all {
            *cnt.entry(dm.get(i).unwrap().class.clone()).or_default() += 1;
        }
        eprintln!("{:?}", cnt);
    }
    if std::env::var("NPCS").is_ok() {
        let dm = g.dm.borrow();
        eprintln!("tracked npcs: {}", g.npcs.len());
        let ws = dm.services["Workspace"];
        let mut all = vec![];
        dm.descendants(ws, &mut all);
        for m in all {
            if dm.get(m).map(|i| i.class == "Model").unwrap_or(false) {
                if let Some(h) = dm.find_child_of_class(m, "Humanoid") {
                    let torso = swirl_physics::humanoid::root_of(&dm, m);
                    let anch = torso.map(|t| matches!(dm.get_prop(t, "Anchored"), Some(Val::Bool(true))));
                    let hp = dm.get_prop(h, "Health");
                    eprintln!("humanoid model {} torso {:?} anchored {:?} head {} tracked {} health {:?}", dm.full_name(m), torso.map(|t| dm.name(t).to_string()), anch, dm.find_child(m, "Head", false).is_some(), g.npcs.contains_key(&m), hp);
                }
            }
        }
    }
    if std::env::var("DBG").is_ok() {
        let dm = g.dm.borrow();
        let r = g.physics.borrow().raycast(&dm, [2.0, 0.5, 0.0], [4.0, 0.0, 0.0], &[], None);
        eprintln!("ray hit {:?} at {:?}", r.part.map(|p| dm.name(p).to_string()), r.point);
    }
    std::fs::write(&a[4], serde_json::Value::Array(trace).to_string()).unwrap();
    std::fs::write(format!("{}.log", a[4]), lines).unwrap();
}

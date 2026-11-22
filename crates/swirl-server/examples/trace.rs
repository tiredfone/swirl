use swirl_core::instance::Val;
use swirl_server::game::Game;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let place: u64 = a[1].parse().unwrap();
    let era: i32 = a[2].parse().unwrap();
    let ticks: usize = a[3].parse().unwrap();
    let model = a.get(5).cloned().unwrap_or("Dummy".into());
    swirl_server::character::set_content_dir(&swirl_server::config::default_data_dir());
    let bytes = std::fs::read(format!("/tmp/swirl/fakeassets/assets/{}", place)).unwrap();
    let mut g = Game::load_with(place, &bytes, era, "/tmp/swirl/fakeassets", "/tmp/swirl/trace-store").unwrap();
    let mut trace: Vec<serde_json::Value> = vec![];
    let mut lines = String::new();
    for _ in 0..ticks {
        for l in g.tick(1.0 / 30.0) {
            if matches!(l.kind.as_str(), "print" | "error" | "warn") {
                lines.push_str(&format!("[{}] {}\n", l.kind, l.msg));
            }
        }
        g.flush_all(&[]);
        let dm = g.dm.borrow();
        let ws = dm.services["Workspace"];
        let kids = dm.get(ws).unwrap().children.clone();
        let mut row = vec![];
        if let Some(m) = kids.iter().find(|c| dm.name(**c) == model) {
            for p in dm.get(*m).unwrap().children.clone() {
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
    if std::env::var("DBG").is_ok() {
        let dm = g.dm.borrow();
        let r = g.physics.borrow().raycast(&dm, [2.0, 0.5, 0.0], [4.0, 0.0, 0.0], &[], None);
        eprintln!("ray hit {:?} at {:?}", r.part.map(|p| dm.name(p).to_string()), r.point);
    }
    std::fs::write(&a[4], serde_json::Value::Array(trace).to_string()).unwrap();
    std::fs::write(format!("{}.log", a[4]), lines).unwrap();
}

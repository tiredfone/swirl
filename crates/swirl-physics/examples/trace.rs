use swirl_core::instance::{DataModel, Val};
use swirl_physics::Physics;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).unwrap();
    let ticks: usize = args[2].parse().unwrap();
    let tree = swirl_core::rbx::parse(&bytes).unwrap();
    let mut dm = DataModel::new("server");
    let ws = dm.service("Workspace").unwrap();
    let root = dm.root;
    dm.load_tree(&tree, root);
    let mut phys = Physics::new();
    phys.attach_workspace(&mut dm, ws);
    let mut trace: Vec<serde_json::Value> = vec![];
    for _ in 0..ticks {
        phys.step(&mut dm, 1.0 / 60.0);
        dm.journal.clear();
        phys.reset_cursor();
        let mut all = vec![];
        dm.descendants(ws, &mut all);
        let mut row = vec![];
        for p in all {
            if !dm.is_a(p, "BasePart") || matches!(dm.get_prop(p, "Anchored"), Some(Val::Bool(true))) {
                continue;
            }
            let cf = match dm.get_prop(p, "CFrame") {
                Some(Val::Arr(a)) => a.clone(),
                _ => vec![],
            };
            let v = match dm.get_prop(p, "Velocity") {
                Some(Val::Arr(a)) => a.clone(),
                _ => vec![],
            };
            row.push(serde_json::json!([dm.name(p), cf, v]));
        }
        trace.push(serde_json::Value::Array(row));
    }
    std::fs::write(&args[3], serde_json::Value::Array(trace).to_string()).unwrap();
}

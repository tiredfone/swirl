use serde_json::{json, Value as J};
use swirl_core::instance::{DataModel, Val};
use swirl_physics::animator::Animator;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scen: J = serde_json::from_str(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let dir = &args[2];
    let mut dm = DataModel::new("client");
    let ws = dm.service("Workspace").unwrap();
    let model = dm.create("Model", Some(ws)).unwrap();
    let hum = dm.create("Humanoid", Some(model)).unwrap();
    let mut parts = std::collections::HashMap::new();
    for n in scen["parts"].as_array().unwrap() {
        let n = n.as_str().unwrap();
        let p = dm.create("Part", Some(model)).unwrap();
        dm.set_prop(p, "Name", Val::Str(n.into())).unwrap();
        parts.insert(n.to_string(), p);
    }
    let mut motors = vec![];
    for m in scen["motors"].as_array().unwrap() {
        let a = m[1].as_str().unwrap();
        let b = m[2].as_str().unwrap();
        let j = dm.create("Motor6D", Some(model)).unwrap();
        dm.set_prop(j, "Name", Val::Str(m[0].as_str().unwrap().into())).unwrap();
        dm.set_prop(j, "Part0", Val::Ref(Some(parts[a]))).unwrap();
        dm.set_prop(j, "Part1", Val::Ref(Some(parts[b]))).unwrap();
        motors.push((b.to_string(), j));
    }
    let mut an = Animator::default();
    let dt = 1.0 / 60.0;
    let ops = scen["ops"].as_array().unwrap();
    let mut oi = 0;
    let mut out: Vec<J> = vec![];
    for f in 0..scen["frames"].as_u64().unwrap() {
        while oi < ops.len() && ops[oi]["f"].as_u64().unwrap() <= f {
            let o = &ops[oi];
            let opn = |k: &str| o[k].as_f64();
            an.op(&mut dm, hum, o["key"].as_str().unwrap(), o["op"].as_str().unwrap(), o["id"].as_u64(), opn("a"), opn("b"), opn("c"), None, 0.0);
            oi += 1;
        }
        for id in std::mem::take(&mut an.wanted) {
            let bytes = std::fs::read(format!("{}/seq-{}.xml", dir, id)).ok();
            an.provide(id, bytes.as_deref());
        }
        an.step(&mut dm, dt);
        let row: Vec<J> = motors
            .iter()
            .map(|(n, j)| match an.anim.get(j) {
                Some(c) => json!([n, c.iter().map(|v| (v * 1e5).round() / 1e5).collect::<Vec<f64>>()]),
                None => json!([n, null]),
            })
            .collect();
        out.push(J::Array(row));
    }
    std::fs::write(&args[3], J::Array(out).to_string()).unwrap();
}

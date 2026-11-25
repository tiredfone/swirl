use serde_json::json;
use swirl_server::game::Game;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let place: u64 = a[1].parse().unwrap();
    let era: i32 = a[2].parse().unwrap();
    let dir = std::env::var("ASSETS").unwrap_or("/tmp/swirl/fakeassets".into());
    swirl_server::character::set_content_dir(&swirl_server::config::default_data_dir());
    let bytes = std::fs::read(format!("{}/assets/{}", dir, place)).unwrap();
    let init = json!({"bps": {"T": [2, 7]}, "watches": ["a", "x", "t[1]", "i"]});
    let mut g = Game::load_opts(place, &bytes, era, &dir, "/tmp/swirl/trace-store", Some(init)).unwrap();
    let cmds = ["over", "into", "into", "out", "continue", "continue", "continue"];
    let mut k = 0;
    let mut pauses = vec![];
    let mut lines = String::new();
    let mut pending: Option<(usize, String)> = None;
    for tick in 0..90usize {
        if let Some((at, c)) = pending.clone() {
            if tick >= at {
                g.debug_message(&json!({"cmd": c}));
                pending = None;
            }
        }
        for l in g.tick(1.0 / 30.0) {
            if matches!(l.kind.as_str(), "print" | "error" | "warn") {
                lines.push_str(&format!("[{}] {}\n", l.kind, l.msg));
            }
        }
        for p in g.take_debug() {
            pauses.push(p);
            pending = Some((tick + 2, cmds.get(k).copied().unwrap_or("continue").to_string()));
            k += 1;
        }
    }
    println!("{}", serde_json::Value::Array(pauses));
    print!("{}", lines);
}

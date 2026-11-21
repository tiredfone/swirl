use crate::paths::Paths;
use serde_json::{json, Value};
use std::fs;

#[derive(Default, Clone)]
pub struct State {
    pub client: Option<String>,
    pub launcher: Option<String>,
}

pub fn load(p: &Paths) -> State {
    let v: Value = fs::read_to_string(p.state_file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
    State {
        client: v.get("client").and_then(|x| x.as_str()).map(String::from),
        launcher: v.get("launcher").and_then(|x| x.as_str()).map(String::from),
    }
}

pub fn save(p: &Paths, s: &State) -> Result<(), String> {
    fs::create_dir_all(&p.root).map_err(|e| format!("Cannot create {}: {}", p.root.display(), e))?;
    let text = json!({"client": s.client, "launcher": s.launcher}).to_string();
    let tmp = p.root.join("current.json.tmp");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, p.state_file()).map_err(|e| e.to_string())
}

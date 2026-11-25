use mlua::{Function, Lua, Table, Value};
use serde_json::Value as J;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use swirl_core::classes;
use swirl_core::instance::{DataModel, InstId, Val};

#[derive(Clone, Debug)]
pub struct LogLine {
    pub kind: String,
    pub msg: String,
    pub id: u32,
}

#[derive(Clone, Debug)]
pub enum Wire {
    Nil,
    B(bool),
    N(f64),
    S(String),
    I(u32),
    V3([f64; 3]),
    Cf([f64; 12]),
    C3([f64; 3]),
    V2([f64; 2]),
    Ud2([f64; 4]),
    Bc(f64),
    E(String, f64),
    List(Vec<Wire>),
    Dict(Vec<(String, Wire)>),
    Ext(String, Vec<f64>),
}

pub enum Res {
    Ok(Vec<Wire>),
    Async(u32),
    Err(String),
}

pub trait ApiHandler {
    fn call(&mut self, d: &mut DataModel, id: InstId, method: &str, args: &[Wire]) -> Option<Res>;
    fn get(&mut self, _d: &DataModel, _id: InstId, _key: &str) -> Option<Wire> {
        None
    }
}

enum Q {
    Async(u32, Result<Vec<Wire>, String>),
    Start(u32),
    Stop(u32),
    Event(u32, String, Vec<Wire>),
}

pub enum Req {
    LoadCharacter(u32),
    Debris(u32, f64),
    EquipTool(u32, u32),
    UnequipTools(u32),
    Remote(u32, String, Vec<Wire>),
    InvokeResult(u32, f64, Vec<Wire>),
    MoveTo(u32, Option<[f64; 3]>, Option<u32>),
}

struct State {
    reqs: Vec<Req>,
    queue: Vec<Q>,
    subs: HashSet<String>,
    logs: Vec<LogLine>,
    running: HashSet<u32>,
    t0: crate::clock::Clock,
    settings: Option<u32>,
    user_settings: Option<u32>,
    handler: Option<Box<dyn ApiHandler>>,
    extra_methods: HashSet<String>,
    async_seq: u32,
    linked_on: bool,
    linked: HashMap<u32, String>,
    linked_pending: HashSet<u32>,
    linked_wanted: Vec<(u32, String)>,
    debug_out: Vec<String>,
}

pub struct Host {
    lua: Lua,
    dm: Rc<RefCell<DataModel>>,
    st: Rc<RefCell<State>>,
    step: Function,
    ext: Function,
    last_tick: Option<f64>,
}

const METHODS: &[&str] = &[
    "GetChildren", "children", "getChildren", "FindFirstChild", "findFirstChild", "FindFirstChildOfClass", "FindFirstChildWhichIsA", "GetDescendants", "GetFullName", "IsA", "isA",
    "IsDescendantOf", "IsAncestorOf", "Clone", "clone", "Destroy", "destroy", "ClearAllChildren", "GetService", "service", "getService", "WaitForChild", "Remove", "remove",
    "TakeDamage", "LoadCharacter", "AddItem", "GetPlayers", "getPlayers", "GetPlayerFromCharacter", "getPlayerFromCharacter", "Fire", "Invoke",
    "playerFromCharacter", "PlayerFromCharacter", "Play", "play", "Stop", "stop", "Pause", "pause", "Resume", "EquipTool", "UnequipTools", "Move",
    "FireServer", "FireClient", "FireAllClients", "InvokeServer", "InvokeClient",
    "FilterStringAsync", "FilterStringForPlayerAsync", "FilterStringForBroadcast", "CanUserChatAsync", "CanUsersChatAsync", "IsInGroup", "isInGroup", "GetRankInGroup", "GetRoleInGroup", "SetEntries", "IsUserFeatureEnabled", "__UserSettings", "__settings", "GetFFlag", "GetFastFlag", "DefineFastFlag", "GetFVariable", "GetFastVariable", "IsRunning", "IsStudio", "IsServer", "IsClient", "IsRunMode", "IsEdit", "GetRobloxVersion", "AddCoreScriptLocal",
];

struct Members {
    methods: Vec<(String, std::collections::HashSet<String>)>,
    lua_class: std::collections::HashMap<String, Vec<String>>,
}

static MEMBERS: std::sync::OnceLock<Members> = std::sync::OnceLock::new();

fn members() -> &'static Members {
    MEMBERS.get_or_init(|| {
        let j: serde_json::Value = serde_json::from_str(include_str!("../data/members.json")).expect("members.json");
        let strs = |v: &serde_json::Value| -> Vec<String> { v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default() };
        let methods = j["methods"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), strs(v).into_iter().collect())).collect()).unwrap_or_default();
        let lua_class = j["luaClass"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), strs(v))).collect()).unwrap_or_default();
        Members { methods, lua_class }
    })
}

fn method_known(d: &DataModel, id: InstId, class: &str, key: &str) -> bool {
    let m = members();
    let try_key = |k: &str| -> bool {
        if let Some(cs) = m.lua_class.get(k) {
            if cs.iter().any(|c| classes::is_a(class, c)) {
                return true;
            }
        }
        for (c, set) in &m.methods {
            if set.contains(k) && (classes::is_a(class, c) || (c == "DataModel" && id == d.root)) {
                return true;
            }
        }
        false
    };
    if try_key(key) {
        return true;
    }
    match key.chars().next() {
        Some(f) if f.is_ascii_lowercase() => try_key(&format!("{}{}", f.to_ascii_uppercase(), &key[1..])),
        _ => false,
    }
}

static STUBS: std::sync::OnceLock<Vec<(Vec<String>, String, serde_json::Value)>> = std::sync::OnceLock::new();

fn stubs() -> &'static Vec<(Vec<String>, String, serde_json::Value)> {
    STUBS.get_or_init(|| {
        let j: serde_json::Value = serde_json::from_str(include_str!("../data/stubs.json")).expect("stubs.json");
        j.as_array()
            .map(|a| {
                a.iter()
                    .map(|e| {
                        let names = e["names"].as_array().map(|n| n.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
                        (names, e["cls"].as_str().unwrap_or("").to_string(), e["value"].clone())
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

fn stub_wire(v: &serde_json::Value) -> Wire {
    match v {
        serde_json::Value::Null => Wire::Nil,
        serde_json::Value::Bool(b) => Wire::B(*b),
        serde_json::Value::Number(n) => Wire::N(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Wire::S(s.clone()),
        serde_json::Value::Array(_) => Wire::List(vec![]),
        _ => Wire::Dict(vec![]),
    }
}

static EVENT_TABLE: std::sync::OnceLock<Vec<(String, Vec<String>)>> = std::sync::OnceLock::new();

fn event_table() -> &'static Vec<(String, Vec<String>)> {
    EVENT_TABLE.get_or_init(|| {
        let j: serde_json::Value = serde_json::from_str(include_str!("../data/events.json")).expect("events.json");
        j.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default())).collect()).unwrap_or_default()
    })
}

fn event_names(class: &str) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = vec!["Changed", "ChildAdded", "ChildRemoved", "DescendantAdded", "DescendantRemoving", "AncestryChanged"];
    if classes::is_a(class, "BasePart") {
        v.extend(["Touched", "TouchEnded"]);
    }
    if class == "RunService" {
        v.extend(["Stepped", "Heartbeat", "RenderStepped"]);
    }
    if class == "BindableEvent" {
        v.push("Event");
    }
    if class == "RemoteFunction" {
        v.push("__Invoke");
    }
    if class == "RemoteEvent" {
        v.extend(["OnServerEvent", "OnClientEvent"]);
    }
    for (cls, list) in event_table() {
        if cls == class || classes::is_a(class, cls) {
            for e in list {
                v.push(e.as_str());
            }
        }
    }
    v
}

fn tagged(lua: &Lua, items: &[Value]) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    for (i, v) in items.iter().enumerate() {
        t.raw_set(i as i64, v.clone())?;
    }
    Ok(t)
}

fn s(lua: &Lua, v: &str) -> mlua::Result<Value> {
    Ok(Value::String(lua.create_string(v)?))
}

fn enc_into(lua: &Lua, out: &mut Vec<Value>, w: &Wire) -> mlua::Result<()> {
    match w {
        Wire::Nil => out.push(s(lua, "nil")?),
        Wire::B(b) => {
            out.push(s(lua, "b")?);
            out.push(Value::Boolean(*b));
        }
        Wire::N(n) => {
            out.push(s(lua, "n")?);
            out.push(Value::Number(*n));
        }
        Wire::S(x) => {
            out.push(s(lua, "s")?);
            out.push(s(lua, x)?);
        }
        Wire::I(i) => {
            out.push(s(lua, "i")?);
            out.push(Value::Number(*i as f64));
        }
        Wire::V3(v) => {
            out.push(s(lua, "v3")?);
            for x in v {
                out.push(Value::Number(*x));
            }
        }
        Wire::Cf(v) => {
            out.push(s(lua, "cf")?);
            for x in v {
                out.push(Value::Number(*x));
            }
        }
        Wire::C3(v) => {
            out.push(s(lua, "c3")?);
            for x in v {
                out.push(Value::Number(*x));
            }
        }
        Wire::V2(v) => {
            out.push(s(lua, "v2")?);
            for x in v {
                out.push(Value::Number(*x));
            }
        }
        Wire::Bc(v) => {
            out.push(s(lua, "bc")?);
            out.push(Value::Number(*v));
        }
        Wire::Ud2(v) => {
            out.push(s(lua, "ud2")?);
            for x in v {
                out.push(Value::Number(*x));
            }
        }
        Wire::Ext(t, v) => {
            out.push(s(lua, t)?);
            for x in v {
                out.push(Value::Number(*x));
            }
        }
        Wire::E(ty, n) => {
            out.push(s(lua, "e")?);
            out.push(s(lua, ty)?);
            out.push(Value::Number(*n));
        }
        Wire::List(l) => {
            out.push(s(lua, "l")?);
            out.push(Value::Number(l.len() as f64));
            for x in l {
                enc_into(lua, out, x)?;
            }
        }
        Wire::Dict(d) => {
            out.push(s(lua, "d")?);
            out.push(Value::Number(d.len() as f64));
            for (k, x) in d {
                out.push(s(lua, k)?);
                enc_into(lua, out, x)?;
            }
        }
    }
    Ok(())
}

fn decode_args(args: &[Value]) -> Vec<Wire> {
    let mut flat: Vec<J> = vec![];
    for a in args {
        flat.push(match a {
            Value::Nil => J::Null,
            Value::Boolean(b) => J::Bool(*b),
            Value::Number(n) => serde_json::Number::from_f64(*n).map(J::Number).unwrap_or(J::Null),
            Value::Integer(n) => J::from(*n),
            Value::String(x) => J::String(x.to_str().map(|x| x.to_string()).unwrap_or_default()),
            _ => J::Null,
        });
    }
    wire_from_json(&flat)
}

pub fn wire_from_json(a: &[J]) -> Vec<Wire> {
    let mut out = vec![];
    let mut i = 0;
    while i < a.len() {
        match one_wire(a, &mut i) {
            Some(w) => out.push(w),
            None => break,
        }
    }
    out
}

fn one_wire(a: &[J], i: &mut usize) -> Option<Wire> {
    let num = |x: Option<&J>| x.and_then(|v| v.as_f64()).unwrap_or(0.0);
    let tag = match a.get(*i) {
        Some(J::String(t)) => t.clone(),
        Some(_) => {
            *i += 1;
            return Some(Wire::Nil);
        }
        None => return None,
    };
    *i += 1;
    let take = |i: &mut usize, n: usize| -> Vec<f64> {
        let v: Vec<f64> = (0..n).map(|k| num(a.get(*i + k))).collect();
        *i += n;
        v
    };
    Some(match tag.as_str() {
        "nil" => Wire::Nil,
        "b" => {
            let v = matches!(a.get(*i), Some(J::Bool(true)));
            *i += 1;
            Wire::B(v)
        }
        "n" => Wire::N(take(i, 1)[0]),
        "s" => {
            let v = a.get(*i).and_then(|x| x.as_str()).unwrap_or("").to_string();
            *i += 1;
            Wire::S(v)
        }
        "i" => Wire::I(take(i, 1)[0] as u32),
        "ray" => {
            let v = take(i, 6);
            Wire::List(v.into_iter().map(Wire::N).collect())
        }
        "v3" => {
            let v = take(i, 3);
            Wire::V3([v[0], v[1], v[2]])
        }
        "c3" => {
            let v = take(i, 3);
            Wire::C3([v[0], v[1], v[2]])
        }
        "cf" => {
            let v = take(i, 12);
            let mut c = [0.0; 12];
            c.copy_from_slice(&v);
            Wire::Cf(c)
        }
        "v2" => {
            let v = take(i, 2);
            Wire::V2([v[0], v[1]])
        }
        "ud2" => {
            let v = take(i, 4);
            Wire::Ud2([v[0], v[1], v[2], v[3]])
        }
        "bc" => Wire::Bc(take(i, 1)[0]),
        "ud" => Wire::Ext("ud".into(), take(i, 2)),
        "nr" => Wire::Ext("nr".into(), take(i, 2)),
        "rect" => Wire::Ext("rect".into(), take(i, 4)),
        "ti" => Wire::Ext("ti".into(), take(i, 6)),
        "ns" | "cs" => {
            let per = if tag == "ns" { 3 } else { 4 };
            let n = take(i, 1)[0] as usize;
            let mut v = vec![n as f64];
            v.extend(take(i, n.min(20000) * per));
            Wire::Ext(tag.clone(), v)
        }
        "e" => {
            let ty = a.get(*i).and_then(|x| x.as_str()).unwrap_or("").to_string();
            *i += 1;
            let n = take(i, 1)[0];
            Wire::E(ty, n)
        }
        "l" => {
            let n = take(i, 1)[0] as usize;
            let mut l = vec![];
            for _ in 0..n.min(20000) {
                l.push(one_wire(a, i)?);
            }
            Wire::List(l)
        }
        "d" => {
            let n = take(i, 1)[0] as usize;
            let mut d = vec![];
            for _ in 0..n.min(20000) {
                let k = match a.get(*i) {
                    Some(J::String(k)) => k.clone(),
                    Some(J::Number(k)) => k.to_string(),
                    _ => String::new(),
                };
                *i += 1;
                d.push((k, one_wire(a, i)?));
            }
            Wire::Dict(d)
        }
        _ => Wire::Nil,
    })
}

pub fn wire_to_json(ws: &[Wire]) -> Vec<J> {
    let mut out = vec![];
    for w in ws {
        wire_push(&mut out, w);
    }
    out
}

fn wire_push(out: &mut Vec<J>, w: &Wire) {
    let n = |x: f64| serde_json::Number::from_f64(x).map(J::Number).unwrap_or(J::Null);
    match w {
        Wire::Nil => out.push(J::from("nil")),
        Wire::B(b) => {
            out.push(J::from("b"));
            out.push(J::Bool(*b));
        }
        Wire::N(x) => {
            out.push(J::from("n"));
            out.push(n(*x));
        }
        Wire::S(x) => {
            out.push(J::from("s"));
            out.push(J::from(x.as_str()));
        }
        Wire::I(x) => {
            out.push(J::from("i"));
            out.push(J::from(*x));
        }
        Wire::V3(v) | Wire::C3(v) => {
            out.push(J::from(if matches!(w, Wire::V3(_)) { "v3" } else { "c3" }));
            out.extend(v.iter().map(|x| n(*x)));
        }
        Wire::Cf(v) => {
            out.push(J::from("cf"));
            out.extend(v.iter().map(|x| n(*x)));
        }
        Wire::V2(v) => {
            out.push(J::from("v2"));
            out.extend(v.iter().map(|x| n(*x)));
        }
        Wire::Bc(v) => out.push(n(*v)),
        Wire::Ud2(v) => {
            out.push(J::from("ud2"));
            out.extend(v.iter().map(|x| n(*x)));
        }
        Wire::E(t, x) => {
            out.push(J::from("e"));
            out.push(J::from(t.as_str()));
            out.push(n(*x));
        }
        Wire::List(l) => {
            out.push(J::from("l"));
            out.push(J::from(l.len()));
            for x in l {
                wire_push(out, x);
            }
        }
        Wire::Ext(t, v) => {
            out.push(J::from(t.as_str()));
            for x in v {
                out.push(n(*x));
            }
        }
        Wire::Dict(d) => {
            out.push(J::from("d"));
            out.push(J::from(d.len()));
            for (k, x) in d {
                out.push(J::from(k.as_str()));
                wire_push(out, x);
            }
        }
    }
}

fn val_to_wire(ty: &str, enum_type: Option<&str>, v: &Val) -> Wire {
    match (ty, v) {
        ("enum", Val::Num(n)) if enum_type.is_some() => Wire::E(enum_type.unwrap().to_string(), *n),
        ("V2", Val::Arr(a)) if a.len() >= 2 => Wire::V2([a[0], a[1]]),
        ("UD2", Val::Arr(a)) if a.len() >= 4 => Wire::Ud2([a[0], a[1], a[2], a[3]]),
        ("UD", Val::Arr(a)) if a.len() >= 2 => Wire::Ext("ud".into(), a.clone()),
        ("NR", Val::Arr(a)) if a.len() >= 2 => Wire::Ext("nr".into(), a.clone()),
        ("Rect", Val::Arr(a)) if a.len() >= 4 => Wire::Ext("rect".into(), a.clone()),
        ("NS", Val::Arr(a)) if !a.is_empty() => Wire::Ext("ns".into(), a.clone()),
        ("CS", Val::Arr(a)) if !a.is_empty() => Wire::Ext("cs".into(), a.clone()),
        ("bool", Val::Bool(b)) => Wire::B(*b),
        ("string", Val::Str(x)) | ("Content", Val::Str(x)) => Wire::S(x.clone()),
        ("BC", Val::Num(n)) => Wire::Bc(*n),
        ("int", Val::Num(n)) | ("number", Val::Num(n)) | ("enum", Val::Num(n)) => Wire::N(*n),
        ("V3", Val::Arr(a)) if a.len() >= 3 => Wire::V3([a[0], a[1], a[2]]),
        ("C3", Val::Arr(a)) if a.len() >= 3 => Wire::C3([a[0], a[1], a[2]]),
        ("CF", Val::Arr(a)) if a.len() >= 12 => {
            let mut c = [0.0; 12];
            c.copy_from_slice(&a[..12]);
            Wire::Cf(c)
        }
        ("Ref", Val::Ref(Some(id))) => Wire::I(*id),
        _ => Wire::Nil,
    }
}

fn wire_to_val(ty: &str, w: &Wire) -> Option<Val> {
    Some(match (ty, w) {
        ("bool", Wire::B(b)) => Val::Bool(*b),
        ("bool", Wire::Nil) => Val::Bool(false),
        ("bool", Wire::N(n)) => Val::Bool(*n != 0.0 && !n.is_nan()),
        ("bool", Wire::S(x)) => Val::Bool(!x.is_empty()),
        ("bool", Wire::I(_)) | ("bool", Wire::V3(_)) | ("bool", Wire::Cf(_)) | ("bool", Wire::C3(_)) | ("bool", Wire::V2(_)) | ("bool", Wire::Ud2(_)) | ("bool", Wire::E(_, _)) | ("bool", Wire::Bc(_)) | ("bool", Wire::List(_)) | ("bool", Wire::Dict(_)) => Val::Bool(true),
        ("string", Wire::S(x)) | ("Content", Wire::S(x)) => Val::Str(x.clone()),
        ("string", Wire::N(n)) => Val::Str(n.to_string()),
        ("int", Wire::N(n)) => Val::Num(n.trunc()),
        ("number", Wire::N(n)) | ("enum", Wire::N(n)) | ("BC", Wire::N(n)) | ("BC", Wire::Bc(n)) => Val::Num(*n),
        ("enum", Wire::E(_, v)) => Val::Num(*v),
        ("V3", Wire::V3(a)) | ("C3", Wire::C3(a)) => Val::Arr(a.to_vec()),
        ("V2", Wire::V2(a)) => Val::Arr(a.to_vec()),
        ("UD2", Wire::Ud2(a)) => Val::Arr(a.to_vec()),
        ("UD", Wire::Ext(t, v)) if t == "ud" => Val::Arr(v.clone()),
        ("NR", Wire::Ext(t, v)) if t == "nr" => Val::Arr(v.clone()),
        ("Rect", Wire::Ext(t, v)) if t == "rect" => Val::Arr(v.clone()),
        ("NS", Wire::Ext(t, v)) if t == "ns" => Val::Arr(v.clone()),
        ("CS", Wire::Ext(t, v)) if t == "cs" => Val::Arr(v.clone()),
        ("CF", Wire::Cf(a)) => Val::Arr(a.to_vec()),
        ("Ref", Wire::I(i)) => Val::Ref(Some(*i)),
        ("Ref", Wire::Nil) => Val::Ref(None),
        _ => return None,
    })
}


fn cf_of(d: &DataModel, id: InstId, key: &str) -> [f64; 12] {
    match d.get_prop(id, key) {
        Some(Val::Arr(a)) if a.len() >= 12 => {
            let mut c = [0.0; 12];
            c.copy_from_slice(&a[..12]);
            c
        }
        _ => [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    }
}

fn virtual_get(d: &DataModel, id: InstId, class: &str, key: &str) -> Option<Wire> {
    if classes::is_a(class, "BasePart") && (key == "Position" || key == "position") {
        let c = cf_of(d, id, "CFrame");
        return Some(Wire::V3([c[0], c[1], c[2]]));
    }
    if classes::is_a(class, "Tool") {
        let g = cf_of(d, id, "Grip");
        return match key {
            "GripPos" => Some(Wire::V3([g[0], g[1], g[2]])),
            "GripRight" => Some(Wire::V3([g[3], g[6], g[9]])),
            "GripUp" => Some(Wire::V3([g[4], g[7], g[10]])),
            "GripForward" => Some(Wire::V3([-g[5], -g[8], -g[11]])),
            _ => None,
        };
    }
    if class == "Players" && (key == "NumPlayers" || key == "numPlayers") {
        let n = d.get(id).map(|p| p.children.iter().filter(|c| d.get(**c).map(|x| x.class == "Player" && !x.destroyed).unwrap_or(false)).count()).unwrap_or(0);
        return Some(Wire::N(n as f64));
    }
    if class == "Player" && key == "Team" {
        if matches!(d.get_prop(id, "Neutral"), Some(Val::Bool(true))) {
            return Some(Wire::Nil);
        }
        let tc = d.get_prop(id, "TeamColor").cloned();
        let teams = d.services.get("Teams").copied()?;
        let found = d.get(teams)?.children.iter().copied().find(|c| d.get(*c).map(|i| i.class == "Team").unwrap_or(false) && d.get_prop(*c, "TeamColor").cloned() == tc);
        return Some(found.map(Wire::I).unwrap_or(Wire::Nil));
    }
    if class == "Humanoid" && matches!(key, "Torso" | "LeftLeg" | "RightLeg" | "RootPart" | "SeatPart") {
        let model = d.get(id).and_then(|i| i.parent);
        let found = |n: &str| model.and_then(|m| d.find_child(m, n, false));
        let r = match key {
            "Torso" if d.era == 2018 => found("HumanoidRootPart").or_else(|| found("Torso")),
            "Torso" => found("Torso").or_else(|| found("HumanoidRootPart")),
            "RootPart" => found("HumanoidRootPart").or_else(|| found("Torso")),
            "LeftLeg" => found("Left Leg"),
            "RightLeg" => found("Right Leg"),
            _ => {
                let torso = found("Torso").or_else(|| found("HumanoidRootPart"));
                let mut seat = None;
                if let (Some(t), Some(m)) = (torso, model) {
                    let _ = m;
                    let ws = d.services.get("Workspace").copied();
                    let mut all = vec![];
                    if let Some(ws) = ws {
                        d.descendants(ws, &mut all);
                    }
                    for w in all {
                        if d.get(w).map(|i| i.name == "SeatWeld").unwrap_or(false) && matches!(d.get_prop(w, "Part1"), Some(Val::Ref(Some(p))) if *p == t) {
                            if let Some(Val::Ref(Some(s))) = d.get_prop(w, "Part0") {
                                seat = Some(*s);
                            }
                            break;
                        }
                    }
                }
                seat
            }
        };
        return Some(r.map(Wire::I).unwrap_or(Wire::Nil));
    }
    if class == "Player" && (key == "Backpack" || key == "PlayerGui") {
        return Some(d.find_child_of_class(id, key).map(Wire::I).unwrap_or(Wire::Nil));
    }
    None
}

fn virtual_set(d: &mut DataModel, id: InstId, class: &str, key: &str, w: &Wire) -> Option<bool> {
    if classes::is_a(class, "BasePart") && (key == "Position" || key == "position") {
        if let Wire::V3(v) = w {
            let mut c = cf_of(d, id, "CFrame");
            c[0] = v[0];
            c[1] = v[1];
            c[2] = v[2];
            let _ = d.set_prop(id, "CFrame", Val::Arr(c.to_vec()));
            return Some(true);
        }
        return Some(false);
    }
    if classes::is_a(class, "Tool") && matches!(key, "GripPos" | "GripRight" | "GripUp" | "GripForward") {
        if let Wire::V3(v) = w {
            let mut g = cf_of(d, id, "Grip");
            match key {
                "GripPos" => {
                    g[0] = v[0];
                    g[1] = v[1];
                    g[2] = v[2];
                }
                "GripRight" => {
                    g[3] = v[0];
                    g[6] = v[1];
                    g[9] = v[2];
                }
                "GripUp" => {
                    g[4] = v[0];
                    g[7] = v[1];
                    g[10] = v[2];
                }
                _ => {
                    g[5] = -v[0];
                    g[8] = -v[1];
                    g[11] = -v[2];
                }
            }
            let _ = d.set_prop(id, "Grip", Val::Arr(g.to_vec()));
            return Some(true);
        }
        return Some(false);
    }
    None
}

fn can_run_client(d: &DataModel, i: &swirl_core::instance::Instance) -> bool {
    let in_model = |anc: InstId| d.is_descendant_of(i.id, anc);
    match i.class.as_str() {
        "CoreScript" => in_model(d.root),
        "LocalScript" => {
            let lp = match d.local_player {
                Some(p) => p,
                None => return false,
            };
            if in_model(lp) {
                return true;
            }
            if let Some(Val::Ref(Some(ch))) = d.get_prop(lp, "Character") {
                if i.id == *ch || in_model(*ch) {
                    return true;
                }
            }
            if d.era == 2018 && d.services.get("Chat").map(|c| in_model(*c)).unwrap_or(false) {
                return true;
            }
            d.services.get("ReplicatedFirst").map(|rf| in_model(*rf)).unwrap_or(false)
        }
        _ => false,
    }
}

fn can_run(d: &DataModel, id: InstId) -> bool {
    let i = match d.get(id) {
        Some(i) if !i.destroyed => i,
        _ => return false,
    };
    if matches!(i.props.get("Disabled"), Some(Val::Bool(true))) {
        return false;
    }
    if d.side == "client" {
        return can_run_client(d, i);
    }
    if i.class != "Script" && i.class != "CoreScript" {
        return false;
    }
    if d.era == 2018 && i.class == "Script" && d.services.get("Chat").map(|c| d.is_descendant_of(i.id, *c)).unwrap_or(false) {
        return true;
    }
    let mut cur = i.parent;
    let mut guard = 0;
    while let Some(c) = cur {
        if let Some(ci) = d.get(c) {
            if matches!(ci.class.as_str(), "Workspace" | "ServerScriptService" | "Players" | "ScriptContext") && d.services.get(&ci.class) == Some(&c) {
                return true;
            }
            if ci.class == "StarterPack" || ci.class == "StarterGui" {
                return false;
            }
            cur = ci.parent;
        } else {
            return false;
        }
        guard += 1;
        if guard > 10000 {
            return false;
        }
    }
    false
}

fn scan_into(st: &Rc<RefCell<State>>, d: &DataModel, id: InstId) {
    let mut all = vec![id];
    d.descendants(id, &mut all);
    let mut s = st.borrow_mut();
    for i in all {
        if !s.running.contains(&i) && can_run(d, i) {
            s.running.insert(i);
            if s.linked_on && d.get(i).map(|x| x.class != "CoreScript").unwrap_or(false) {
                let has_src = matches!(d.get_prop(i, "Source"), Some(Val::Str(x)) if !x.is_empty());
                let url = match d.get_prop(i, "LinkedSource") {
                    Some(Val::Str(x)) if !x.is_empty() => x.clone(),
                    _ => String::new(),
                };
                if !has_src && !url.is_empty() && !s.linked.contains_key(&i) {
                    if s.linked_pending.insert(i) {
                        s.linked_wanted.push((i, url));
                    }
                    continue;
                }
            }
            s.queue.push(Q::Start(i));
        }
    }
}

fn push_event(st: &Rc<RefCell<State>>, id: u32, name: &str, args: Vec<Wire>) {
    let key = format!("{}:{}", id, name);
    if st.borrow().subs.contains(&key) {
        st.borrow_mut().queue.push(Q::Event(id, name.to_string(), args));
    }
}

fn after_parent_change(st: &Rc<RefCell<State>>, dm: &DataModel, id: InstId, old: Option<InstId>, new: Option<InstId>) {
    if let Some(o) = old {
        if Some(o) != new {
            push_event(st, o, "ChildRemoved", vec![Wire::I(id)]);
            let mut cur = Some(o);
            let mut guard = 0;
            while let Some(c) = cur {
                push_event(st, c, "DescendantRemoving", vec![Wire::I(id)]);
                cur = dm.get(c).and_then(|i| i.parent);
                guard += 1;
                if guard > 10000 {
                    break;
                }
            }
        }
    }
    if new.is_some() {
        scan_into(st, dm, id);
    }
    if let Some(n) = new {
        if Some(n) != old {
            push_event(st, n, "ChildAdded", vec![Wire::I(id)]);
            let mut cur = Some(n);
            let mut guard = 0;
            while let Some(c) = cur {
                push_event(st, c, "DescendantAdded", vec![Wire::I(id)]);
                cur = dm.get(c).and_then(|i| i.parent);
                guard += 1;
                if guard > 10000 {
                    break;
                }
            }
        }
    }
    if old != new {
        let mut all = vec![id];
        dm.descendants(id, &mut all);
        let parent = match new {
            Some(p) => Wire::I(p),
            None => Wire::Nil,
        };
        for x in all {
            push_event(st, x, "AncestryChanged", vec![Wire::I(id), parent.clone()]);
        }
        push_event(st, id, "Changed", vec![Wire::S("Parent".into())]);
    }
}

impl Host {
    pub fn new(dm: Rc<RefCell<DataModel>>) -> mlua::Result<Host> {
        let lua = unsafe { Lua::unsafe_new() };
        let st = Rc::new(RefCell::new(State { reqs: vec![], queue: vec![], subs: HashSet::new(), logs: vec![], running: HashSet::new(), t0: crate::clock::Clock::new(), settings: None, user_settings: None, handler: None, extra_methods: HashSet::new(), async_seq: 1, linked_on: false, linked: HashMap::new(), linked_pending: HashSet::new(), linked_wanted: vec![], debug_out: vec![] }));
        let j = lua.create_table()?;
        {
            let dm = dm.clone();
            let st_get = st.clone();
            j.set(
                "get",
                lua.create_function(move |lua, (id, key): (u32, String)| {
                    let d = dm.borrow();
                    let inst = match d.get(id) {
                        Some(i) => i,
                        _ => return Ok(Value::Table(tagged(lua, &[s(lua, "err")?, s(lua, "Instance no longer exists")?])?)),
                    };
                    if key == "Parent" {
                        return Ok(Value::Table(match inst.parent {
                            Some(p) => tagged(lua, &[s(lua, "i")?, Value::Number(p as f64)])?,
                            None => tagged(lua, &[s(lua, "nil")?])?,
                        }));
                    }
                    if key == "__ownerPlayer" {
                        return Ok(Value::Table(tagged(lua, &[s(lua, "nil")?])?));
                    }
                    if key == "ClassName" || key == "className" {
                        return Ok(Value::Table(tagged(lua, &[s(lua, "s")?, s(lua, &inst.class)?])?));
                    }
                    if key == "Name" {
                        return s(lua, &inst.name);
                    }
                    let hooked = {
                        let h = st_get.borrow_mut().handler.take();
                        match h {
                            Some(mut h) => {
                                let r = h.get(&d, id, &key);
                                st_get.borrow_mut().handler = Some(h);
                                r
                            }
                            None => None,
                        }
                    };
                    if let Some(w) = hooked {
                        let mut out = vec![];
                        enc_into(lua, &mut out, &w)?;
                        return Ok(Value::Table(tagged(lua, &out)?));
                    }
                    if let Some(w) = virtual_get(&d, id, &inst.class, &key) {
                        let mut out = vec![];
                        enc_into(lua, &mut out, &w)?;
                        return Ok(Value::Table(tagged(lua, &out)?));
                    }
                    let class_gated = match key.as_str() {
                        "Play" | "play" | "Stop" | "stop" | "Pause" | "pause" | "Resume" => Some(classes::is_a(&inst.class, "Sound") || classes::is_a(&inst.class, "AnimationTrack") || inst.class == "Tween"),
                        "Move" => Some(classes::is_a(&inst.class, "Humanoid") || inst.class == "Player"),
                        _ => None,
                    };
                    if class_gated != Some(false) && (METHODS.contains(&key.as_str()) || st_get.borrow().extra_methods.contains(&key) || method_known(&d, id, &inst.class, &key)) {
                        return Ok(Value::Table(tagged(lua, &[s(lua, "m")?, s(lua, &key)?])?));
                    }
                    let cap = {
                        let mut c = key.clone();
                        if let Some(f) = c.get(..1) {
                            c = f.to_uppercase() + &c[1..];
                        }
                        c
                    };
                    if event_names(&inst.class).contains(&cap.as_str()) {
                        return Ok(Value::Table(tagged(lua, &[s(lua, "ev")?, s(lua, &cap)?])?));
                    }
                    if let Some(pn) = classes::canonical_prop(&inst.class, &key) {
                        if let Some(p) = classes::prop_info(&inst.class, &pn) {
                            let mut out = vec![];
                            let w = inst.props.get(&pn).map(|v| val_to_wire(&p.ty, p.enum_type.as_deref(), v)).unwrap_or(Wire::Nil);
                            enc_into(lua, &mut out, &w)?;
                            return Ok(Value::Table(tagged(lua, &out)?));
                        }
                    }
                    if let Some(c) = d.find_child(id, &key, false) {
                        return Ok(Value::Table(tagged(lua, &[s(lua, "i")?, Value::Number(c as f64)])?));
                    }
                    if id == d.root {
                        let name = if key == "workspace" { "Workspace".to_string() } else { key.clone() };
                        if let Some(&sid) = d.services.get(&name) {
                            return Ok(Value::Table(tagged(lua, &[s(lua, "i")?, Value::Number(sid as f64)])?));
                        }
                    }
                    Ok(Value::Table(tagged(lua, &[s(lua, "err")?, s(lua, &format!("{} is not a valid member of {}", key, inst.class))?])?))
                })?,
            )?;
        }
        {
            let dm = dm.clone();
            let st2 = st.clone();
            j.set(
                "set",
                lua.create_function(move |lua, (id, key, rest): (u32, String, mlua::Variadic<Value>)| {
                    let args = decode_args(&rest);
                    let w = args.first().cloned().unwrap_or(Wire::Nil);
                    let mut d = dm.borrow_mut();
                    if d.get(id).is_none() {
                        return Ok(Value::String(lua.create_string("Instance no longer exists")?));
                    }
                    if key == "Parent" && d.get(id).map(|i| i.destroyed).unwrap_or(false) {
                        return Ok(Value::String(lua.create_string(&format!("The Parent property of {} is locked", d.name(id)))?));
                    }
                    if key == "Parent" {
                        let np = match w {
                            Wire::I(p) => Some(p),
                            Wire::Nil => None,
                            _ => return Ok(Value::String(lua.create_string("bad argument #3 to '?' (Object expected)")?)),
                        };
                        let old = d.get(id).and_then(|i| i.parent);
                        if let Err(e) = d.set_parent(id, np) {
                            return Ok(Value::String(lua.create_string(&e)?));
                        }
                        after_parent_change(&st2, &d, id, old, np);
                        return Ok(Value::Nil);
                    }
                    let class = d.get(id).unwrap().class.clone();
                    if let Some(r) = virtual_set(&mut d, id, &class, &key, &w) {
                        return if r { push_event(&st2, id, "Changed", vec![Wire::S(key.clone())]); Ok(Value::Nil) } else { Ok(Value::String(lua.create_string(&format!("invalid value for {}", key))?)) };
                    }
                    let pn = match classes::canonical_prop(&class, &key) {
                        Some(p) => p,
                        None => return Ok(Value::String(lua.create_string(&format!("{} is not a valid member of {}", key, class))?)),
                    };
                    let ty = classes::prop_info(&class, &pn).map(|p| p.ty.clone()).unwrap_or_default();
                    let enum_ty = classes::prop_info(&class, &pn).and_then(|p| p.enum_type.clone());
                    let w = match (&ty[..], &w, enum_ty) {
                        ("enum", Wire::S(name), Some(et)) => match classes::enum_value(&et, name) {
                            Some(v) => Wire::N(v as f64),
                            None => w,
                        },
                        _ => w,
                    };
                    let w = match (&pn[..], &w) {
                        ("Size", Wire::V3(v)) if d.is_a(id, "BasePart") => Wire::V3(snap_size(&d, id, &class, *v)),
                        _ => w,
                    };
                    match wire_to_val(&ty, &w) {
                        Some(v) => {
                            let _ = d.set_prop(id, &pn, v);
                            push_event(&st2, id, "Changed", vec![Wire::S(pn)]);
                            Ok(Value::Nil)
                        }
                        None => Ok(Value::String(lua.create_string(&format!("invalid value for {}", key))?)),
                    }
                })?,
            )?;
        }
        {
            let dm = dm.clone();
            let st2 = st.clone();
            j.set(
                "call",
                lua.create_function(move |lua, (id, method, rest): (u32, String, mlua::Variadic<Value>)| {
                    let method = match method.as_str() {
                        "isDescendantOf" => "IsDescendantOf".to_string(),
                        "isAncestorOf" => "IsAncestorOf".to_string(),
                        "getFullName" => "GetFullName".to_string(),
                        "clearAllChildren" => "ClearAllChildren".to_string(),
                        "takeDamage" => "TakeDamage".to_string(),
                        "loadCharacter" => "LoadCharacter".to_string(),
                        "addItem" => "AddItem".to_string(),
                        "resume" => "Resume".to_string(),
                        _ => method,
                    };
                    let args = decode_args(&rest);
                    let mut d = dm.borrow_mut();
                    let err = |m: &str| -> mlua::Result<Value> { Ok(Value::Table(tagged(lua, &[s(lua, "err")?, s(lua, m)?])?)) };
                    if d.get(id).is_none() {
                        return err("Instance no longer exists");
                    }
                    let ok = |w: Wire| -> mlua::Result<Value> {
                        let mut out = vec![s(lua, "ok")?];
                        enc_into(lua, &mut out, &w)?;
                        Ok(Value::Table(tagged(lua, &out)?))
                    };
                    let name_arg = |i: usize| match args.get(i) {
                        Some(Wire::S(x)) => x.clone(),
                        _ => String::new(),
                    };
                    let handled = {
                        let h = st2.borrow_mut().handler.take();
                        match h {
                            Some(mut h) => {
                                let r = h.call(&mut d, id, &method, &args);
                                st2.borrow_mut().handler = Some(h);
                                r
                            }
                            None => None,
                        }
                    };
                    if let Some(r) = handled {
                        return match r {
                            Res::Ok(ws) => {
                                let mut out = vec![s(lua, "ok")?];
                                for w in &ws {
                                    enc_into(lua, &mut out, w)?;
                                }
                                if ws.is_empty() {
                                    enc_into(lua, &mut out, &Wire::Nil)?;
                                }
                                Ok(Value::Table(tagged(lua, &out)?))
                            }
                            Res::Async(tok) => Ok(Value::Table(tagged(lua, &[s(lua, "async")?, Value::Number(tok as f64)])?)),
                            Res::Err(m) => err(&m),
                        };
                    }
                    match method.as_str() {
                        "__source" => {
                            let src = match d.get_prop(id, "Source") {
                                Some(Val::Str(x)) if !x.is_empty() => x.clone(),
                                _ => st2.borrow().linked.get(&id).cloned().unwrap_or_default(),
                            };
                            ok(Wire::S(src))
                        }
                        "__robloxLocked" => {
                            let b = d.services.get("CoreGui").map(|cg| *cg == id || d.is_descendant_of(id, *cg)).unwrap_or(false);
                            Ok(Value::Boolean(b))
                        }
                        "__coreCode" => {
                            let mut b = d.get(id).map(|i| i.class == "CoreScript").unwrap_or(false);
                            for svc in ["CoreGui", "RobloxReplicatedStorage"] {
                                if let Some(cg) = d.services.get(svc) {
                                    if d.is_descendant_of(id, *cg) {
                                        b = true;
                                    }
                                }
                            }
                            Ok(Value::Boolean(b))
                        }
                        "__TweenState" => ok(Wire::Nil),
                        "__DestroySelf" | "Destroy" | "destroy" | "Remove" | "remove" => {
                            let old = d.get(id).and_then(|i| i.parent);
                            d.destroy(id);
                            after_parent_change(&st2, &d, id, old, None);
                            ok(Wire::Nil)
                        }
                        "GetChildren" | "children" | "getChildren" => {
                            let kids: Vec<Wire> = d.get(id).unwrap().children.iter().map(|c| Wire::I(*c)).collect();
                            ok(Wire::List(kids))
                        }
                        "GetDescendants" => {
                            let mut v = vec![];
                            d.descendants(id, &mut v);
                            ok(Wire::List(v.into_iter().map(Wire::I).collect()))
                        }
                        "FindFirstChild" | "findFirstChild" => {
                            let rec = matches!(args.get(1), Some(Wire::B(true)));
                            match d.find_child(id, &name_arg(0), rec) {
                                Some(c) => ok(Wire::I(c)),
                                None => ok(Wire::Nil),
                            }
                        }
                        "FindFirstChildOfClass" => match d.find_child_of_class(id, &name_arg(0)) {
                            Some(c) => ok(Wire::I(c)),
                            None => ok(Wire::Nil),
                        },
                        "FindFirstChildWhichIsA" => {
                            let base = name_arg(0);
                            let found = d.get(id).unwrap().children.iter().copied().find(|c| d.is_a(*c, &base));
                            match found {
                                Some(c) => ok(Wire::I(c)),
                                None => ok(Wire::Nil),
                            }
                        }
                        "GetFullName" => ok(Wire::S(d.full_name(id))),
                        "IsA" | "isA" => {
                            let b = d.is_a(id, &name_arg(0));
                            Ok(Value::Boolean(b))
                        }
                        "IsDescendantOf" => {
                            let b = match args.first() {
                                Some(Wire::I(o)) => d.is_descendant_of(id, *o),
                                _ => false,
                            };
                            Ok(Value::Boolean(b))
                        }
                        "IsAncestorOf" => {
                            let b = match args.first() {
                                Some(Wire::I(o)) => d.is_descendant_of(*o, id),
                                _ => false,
                            };
                            Ok(Value::Boolean(b))
                        }
                        "Clone" | "clone" => match d.clone_tree(id) {
                            Some(c) => ok(Wire::I(c)),
                            None => ok(Wire::Nil),
                        },
                        "ClearAllChildren" => {
                            let kids = d.get(id).unwrap().children.clone();
                            for k in kids {
                                d.destroy(k);
                                after_parent_change(&st2, &d, k, Some(id), None);
                            }
                            ok(Wire::Nil)
                        }
                        "TakeDamage" => {
                            let has_ff = d.get(id).and_then(|i| i.parent).map(|p| d.find_child_of_class(p, "ForceField").is_some()).unwrap_or(false);
                            if has_ff {
                                return ok(Wire::Nil);
                            }
                            let n = match args.first() {
                                Some(Wire::N(n)) => *n,
                                _ => 0.0,
                            };
                            let cur = match d.get_prop(id, "Health") {
                                Some(Val::Num(h)) => *h,
                                _ => 100.0,
                            };
                            let nh = (cur - n).max(0.0);
                            let _ = d.set_prop(id, "Health", Val::Num(nh));
                            push_event(&st2, id, "HealthChanged", vec![Wire::N(nh)]);
                            ok(Wire::Nil)
                        }
                        "LoadCharacter" => {
                            st2.borrow_mut().reqs.push(Req::LoadCharacter(id));
                            ok(Wire::Nil)
                        }
                        "AddItem" => {
                            if let (Some(Wire::I(t)), Some(Wire::N(s))) = (args.first(), args.get(1)) {
                                st2.borrow_mut().reqs.push(Req::Debris(*t, *s));
                            }
                            ok(Wire::Nil)
                        }
                        "GetPlayers" | "getPlayers" => {
                            let l: Vec<Wire> = d.get(id).unwrap().children.iter().copied().filter(|c| d.get(*c).map(|i| i.class == "Player").unwrap_or(false)).map(Wire::I).collect();
                            ok(Wire::List(l))
                        }
                        "MoveTo" | "moveTo" if d.get(id).map(|i| i.class == "Humanoid").unwrap_or(false) => {
                            let pos = match args.first() {
                                Some(Wire::V3(v)) => Some(*v),
                                _ => None,
                            };
                            let part = match args.get(1) {
                                Some(Wire::I(p)) => Some(*p),
                                _ => None,
                            };
                            if let Some(p) = pos {
                                let _ = d.set_prop(id, "WalkToPoint", Val::Arr(p.to_vec()));
                            }
                            let _ = d.set_prop(id, "WalkToPart", Val::Ref(part));
                            st2.borrow_mut().reqs.push(Req::MoveTo(id, pos, part));
                            ok(Wire::Nil)
                        }
                        "Play" | "play" | "Stop" | "stop" | "Pause" | "pause" | "Resume" | "Move" => ok(Wire::Nil),
                        "__remote" => {
                            if let Some(Wire::S(op)) = args.first() {
                                st2.borrow_mut().reqs.push(Req::Remote(id, op.clone(), args[1..].to_vec()));
                            }
                            ok(Wire::Nil)
                        }
                        "__invokeResult" => {
                            let tok = match args.first() {
                                Some(Wire::N(t)) => *t,
                                _ => -1.0,
                            };
                            st2.borrow_mut().reqs.push(Req::InvokeResult(id, tok, args.get(1..).map(|a| a.to_vec()).unwrap_or_default()));
                            ok(Wire::Nil)
                        }
                        "EquipTool" => {
                            if let Some(Wire::I(t)) = args.first() {
                                st2.borrow_mut().reqs.push(Req::EquipTool(id, *t));
                            }
                            ok(Wire::Nil)
                        }
                        "UnequipTools" => {
                            st2.borrow_mut().reqs.push(Req::UnequipTools(id));
                            ok(Wire::Nil)
                        }
                        "GetPlayerFromCharacter" | "getPlayerFromCharacter" | "playerFromCharacter" | "PlayerFromCharacter" => {
                            let target = match args.first() {
                                Some(Wire::I(m)) => *m,
                                _ => 0,
                            };
                            let found = d.get(id).unwrap().children.iter().copied().find(|c| matches!(d.get_prop(*c, "Character"), Some(Val::Ref(Some(m))) if *m == target));
                            match found {
                                Some(p) => ok(Wire::I(p)),
                                None => ok(Wire::Nil),
                            }
                        }
                        "__settings" => {
                            let cur = st2.borrow().settings;
                            if let Some(g) = cur {
                                return ok(Wire::I(g));
                            }
                            match d.create_any("GlobalSettings", None) {
                                Ok(g) => {
                                    d.set_prop_quiet(g, "Name", Val::Str("Settings".into()));
                                    d.get_mut_name(g, "Settings");
                                    if let Ok(r) = d.create_any("RenderSettings", Some(g)) {
                                        d.set_prop_quiet(r, "Name", Val::Str("Rendering".into()));
                                        d.get_mut_name(r, "Rendering");
                                    }
                                    for n in ["Network", "Physics", "Diagnostics", "Game"] {
                                        if let Ok(c) = d.create_any("Instance", Some(g)) {
                                            d.set_prop_quiet(c, "Name", Val::Str(n.into()));
                                            d.get_mut_name(c, n);
                                        }
                                    }
                                    if let Ok(c) = d.create_any("GameSettings", Some(g)) {
                                        d.set_prop_quiet(c, "Name", Val::Str("Game Options".into()));
                                        d.get_mut_name(c, "Game Options");
                                    }
                                    st2.borrow_mut().settings = Some(g);
                                    ok(Wire::I(g))
                                }
                                Err(e) => err(&e),
                            }
                        }
                        "SetEntries" => ok(Wire::Nil),
                        "__CloseBound" => ok(Wire::Nil),
                        "FilterStringAsync" | "FilterStringForPlayerAsync" | "FilterStringForBroadcast" => ok(args.first().cloned().unwrap_or(Wire::Nil)),
                        "CanUserChatAsync" | "CanUsersChatAsync" => Ok(Value::Boolean(true)),
                        "IsInGroup" | "isInGroup" => Ok(Value::Boolean(false)),
                        "GetRankInGroup" => ok(Wire::N(0.0)),
                        "GetRoleInGroup" => ok(Wire::S("Guest".into())),
                        "IsUserFeatureEnabled" => Ok(Value::Boolean(false)),
                        "__UserSettings" => {
                            let cur = st2.borrow().user_settings;
                            if let Some(g) = cur {
                                return ok(Wire::I(g));
                            }
                            match d.create_any("UserSettings", None) {
                                Ok(g) => {
                                    d.set_prop_quiet(g, "Name", Val::Str("UserSettings".into()));
                                    d.get_mut_name(g, "UserSettings");
                                    if let Ok(gs) = d.create_any("UserGameSettings", Some(g)) {
                                        d.set_prop_quiet(gs, "Name", Val::Str("GameSettings".into()));
                                        d.get_mut_name(gs, "GameSettings");
                                        d.set_prop_quiet(gs, "ChatVisible", Val::Bool(true));
                                    }
                                    st2.borrow_mut().user_settings = Some(g);
                                    ok(Wire::I(g))
                                }
                                Err(e) => err(&e),
                            }
                        }
                        "GetFFlag" | "GetFastFlag" | "DefineFastFlag" => Ok(Value::Boolean(false)),
                        "GetFVariable" | "GetFastVariable" => ok(Wire::S(String::new())),
                        "IsRunning" | "IsServer" | "IsRunMode" => Ok(Value::Boolean(true)),
                        "IsStudio" | "IsClient" | "IsEdit" => Ok(Value::Boolean(false)),
                        "GetRobloxVersion" => ok(Wire::S("Swirl".into())),
                        "AddCoreScriptLocal" => ok(Wire::Nil),
                        "GetService" | "service" | "getService" => {
                            let n = name_arg(0);
                            match d.service(&n) {
                                Ok(sid) => ok(Wire::I(sid)),
                                Err(_) => err(&format!("{} is not a valid Service name", n)),
                            }
                        }
                        _ => {
                            let class = d.get(id).unwrap().class.clone();
                            for (names, cls, v) in stubs() {
                                if names.iter().any(|n| n == &method) && (classes::is_a(&class, cls) || (cls == "DataModel" && id == d.root)) {
                                    return match v {
                                        serde_json::Value::Bool(b) => Ok(Value::Boolean(*b)),
                                        _ => ok(stub_wire(v)),
                                    };
                                }
                            }
                            err(&format!("{} is not a valid member of {}", method, class))
                        }
                    }
                })?,
            )?;
        }
        {
            let dm = dm.clone();
            let st2 = st.clone();
            j.set(
                "new",
                lua.create_function(move |lua, (cls, parent): (String, Option<u32>)| {
                    let mut d = dm.borrow_mut();
                    match d.create(&cls, None) {
                        Ok(id) => {
                            if let Some(p) = parent {
                                if d.set_parent(id, Some(p)).is_ok() {
                                    after_parent_change(&st2, &d, id, None, Some(p));
                                }
                            }
                            Ok(Value::Number(id as f64))
                        }
                        Err(_) => Ok(Value::String(lua.create_string(&format!("Unable to create an Instance of type \"{}\"", cls))?)),
                    }
                })?,
            )?;
        }
        {
            let st2 = st.clone();
            j.set("sub", lua.create_function(move |_, (id, name): (u32, String)| {
                st2.borrow_mut().subs.insert(format!("{}:{}", id, name));
                Ok(())
            })?)?;
            let st2 = st.clone();
            j.set("unsub", lua.create_function(move |_, (id, name): (u32, String)| {
                st2.borrow_mut().subs.remove(&format!("{}:{}", id, name));
                Ok(())
            })?)?;
            let st2 = st.clone();
            j.set(
                "poll",
                lua.create_function(move |lua, ()| {
                    let q: Vec<Q> = std::mem::take(&mut st2.borrow_mut().queue);
                    let mut out: Vec<Value> = vec![];
                    for item in q {
                        match item {
                            Q::Start(id) => {
                                out.push(s(lua, "start")?);
                                out.push(Value::Number(id as f64));
                            }
                            Q::Stop(id) => {
                                out.push(s(lua, "stop")?);
                                out.push(Value::Number(id as f64));
                            }
                            Q::Async(tok, r) => {
                                let mut body: Vec<Value> = vec![];
                                match r {
                                    Ok(vals) => {
                                        body.push(s(lua, "ok")?);
                                        for v in &vals {
                                            enc_into(lua, &mut body, v)?;
                                        }
                                    }
                                    Err(m) => {
                                        body.push(s(lua, "err")?);
                                        body.push(s(lua, &m)?);
                                    }
                                }
                                out.push(s(lua, "async")?);
                                out.push(Value::Number(tok as f64));
                                out.push(Value::Number(body.len() as f64));
                                out.extend(body);
                            }
                            Q::Event(id, name, args) => {
                                out.push(s(lua, "ev")?);
                                out.push(Value::Number(id as f64));
                                out.push(s(lua, &name)?);
                                out.push(Value::Number(args.len() as f64));
                                for a in &args {
                                    enc_into(lua, &mut out, a)?;
                                }
                            }
                        }
                    }
                    Ok(Value::Table(tagged(lua, &out)?))
                })?,
            )?;
            j.set("alive", lua.create_function(|_, _id: u32| Ok(true))?)?;
            let st2 = st.clone();
            j.set("running", lua.create_function(move |_, id: u32| Ok(st2.borrow().running.contains(&id)))?)?;
            let st2 = st.clone();
            j.set(
                "log",
                lua.create_function(move |_, (kind, msg, id): (String, String, Option<u32>)| {
                    st2.borrow_mut().logs.push(LogLine { kind, msg, id: id.unwrap_or(0) });
                    Ok(())
                })?,
            )?;
            let st2 = st.clone();
            j.set(
                "now",
                lua.create_function(move |_, what: Option<String>| {
                    if what.as_deref() == Some("tick") {
                        Ok(crate::clock::tick())
                    } else {
                        Ok(st2.borrow().t0.elapsed())
                    }
                })?,
            )?;
        }
        let side = dm.borrow().side.clone();
        j.set("side", side)?;
        j.set("data", include_str!("../data/luadata.lua"))?;
        let rt: Table = lua.load(include_str!("../data/runtime.lua")).set_name("=runtime").call(j)?;
        let step: Function = rt.get("step")?;
        if dm.borrow().era == 2018 {
            let ext: Function = rt.get("extend")?;
            ext.call::<()>((include_str!("../data/runtime2018.lua"), "runtime2018"))?;
        }
        let ext: Function = rt.get("extend")?;
        Ok(Host { lua, dm, st, step, ext, last_tick: None })
    }

    pub fn set_handler(&mut self, h: Box<dyn ApiHandler>) {
        self.st.borrow_mut().handler = Some(h);
    }

    pub fn add_methods(&mut self, names: &[&str]) {
        let mut st = self.st.borrow_mut();
        for n in names {
            st.extra_methods.insert(n.to_string());
        }
    }

    pub fn new_token(&mut self) -> u32 {
        let mut st = self.st.borrow_mut();
        st.async_seq += 1;
        st.async_seq
    }

    pub fn resolve(&mut self, token: u32, r: Result<Vec<Wire>, String>) {
        self.st.borrow_mut().queue.push(Q::Async(token, r));
    }

    pub fn scan_scripts(&mut self) {
        let d = self.dm.borrow();
        scan_into(&self.st, &d, d.root);
    }

    pub fn notify_parent(&mut self, id: u32, old: Option<u32>, new: Option<u32>) {
        let d = self.dm.borrow();
        after_parent_change(&self.st, &d, id, old, new);
    }

    pub fn scan_subtree(&mut self, id: u32) {
        let d = self.dm.borrow();
        scan_into(&self.st, &d, id);
    }

    pub fn tick(&mut self, t: f64) {
        {
            let d = self.dm.borrow();
            let mut st = self.st.borrow_mut();
            let dead: Vec<InstId> = st.running.iter().copied().filter(|i| !can_run(&d, *i)).collect();
            for i in dead {
                st.running.remove(&i);
                st.queue.push(Q::Stop(i));
            }
        }
        let dt = self.last_tick.map(|l| (t - l).max(0.0)).unwrap_or(0.0);
        self.last_tick = Some(t);
        self.ext_call("step", t, dt);
        if let Err(e) = self.step.call::<()>(t) {
            self.st.borrow_mut().logs.push(LogLine { kind: "error".into(), msg: format!("scheduler: {}", e), id: 0 });
        }
    }

    pub fn render_step(&mut self, dt: f64) {
        let t = self.last_tick.unwrap_or(0.0);
        self.ext_call("renderStep", t, dt);
    }

    fn ext_call(&mut self, name: &str, t: f64, dt: f64) {
        let ext: mlua::Value = match self.lua.globals().get("__EXT") {
            Ok(v) => v,
            Err(_) => return,
        };
        if let mlua::Value::Table(tb) = ext {
            if let Ok(f) = tb.get::<Function>(name) {
                if let Err(e) = f.call::<()>((t, dt)) {
                    self.st.borrow_mut().logs.push(LogLine { kind: "error".into(), msg: format!("{}: {}", name, e), id: 0 });
                }
            }
        }
    }

    pub fn take_logs(&mut self) -> Vec<LogLine> {
        std::mem::take(&mut self.st.borrow_mut().logs)
    }

    pub fn fire(&mut self, id: u32, name: &str, args: Vec<Wire>) {
        push_event(&self.st, id, name, args);
    }

    pub fn has_listener(&self, id: u32, name: &str) -> bool {
        self.st.borrow().subs.contains(&format!("{}:{}", id, name))
    }

    pub fn subscribed(&self, name: &str) -> Vec<u32> {
        let suffix = format!(":{}", name);
        self.st.borrow().subs.iter().filter(|k| k.ends_with(&suffix)).filter_map(|k| k.split(':').next().and_then(|n| n.parse().ok())).collect()
    }

    pub fn extend(&mut self, src: &str, name: &str) -> Result<(), String> {
        self.ext.call::<()>((src.to_string(), name.to_string())).map_err(|e| e.to_string())
    }

    pub fn debug_attach(&mut self) -> Result<(), String> {
        let st = self.st.clone();
        let f = self
            .lua
            .create_function(move |_, json: String| {
                st.borrow_mut().debug_out.push(json);
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        self.lua.globals().set("__DBGSEND", f).map_err(|e| e.to_string())
    }

    pub fn debug_command(&mut self, cmd: &str, a: &str) {
        if let Ok(f) = self.lua.globals().get::<Function>("__DBGCMD") {
            let _ = f.call::<()>((cmd.to_string(), a.to_string(), String::new()));
        }
    }

    pub fn take_debug(&mut self) -> Vec<String> {
        std::mem::take(&mut self.st.borrow_mut().debug_out)
    }

    pub fn compile_error(&self, src: &str) -> Option<String> {
        self.lua.load(src).set_name("=chunk").into_function().err().map(|e| e.to_string())
    }

    pub fn enable_linked_source(&mut self) {
        self.st.borrow_mut().linked_on = true;
    }

    pub fn take_linked(&mut self) -> Vec<(u32, String)> {
        std::mem::take(&mut self.st.borrow_mut().linked_wanted)
    }

    pub fn provide_linked(&mut self, id: u32, src: Result<String, String>) {
        let (name, alive) = {
            let d = self.dm.borrow();
            (d.get(id).map(|_| d.name(id).to_string()).unwrap_or_default(), d.get(id).map(|i| !i.destroyed).unwrap_or(false))
        };
        let mut st = self.st.borrow_mut();
        st.linked_pending.remove(&id);
        match src {
            Ok(t) => {
                st.linked.insert(id, t);
            }
            Err(e) => {
                st.linked.insert(id, String::new());
                let url = match self.dm.borrow().get_prop(id, "LinkedSource") {
                    Some(Val::Str(x)) => x.clone(),
                    _ => String::new(),
                };
                st.logs.push(LogLine { kind: "warn".into(), msg: format!("{}: LinkedSource {} failed: {}", name, url, e), id: 0 });
            }
        }
        if alive && st.running.contains(&id) {
            st.queue.push(Q::Start(id));
        }
    }

    pub fn user_settings_id(&self) -> Option<u32> {
        self.st.borrow().user_settings
    }

    pub fn take_requests(&mut self) -> Vec<Req> {
        std::mem::take(&mut self.st.borrow_mut().reqs)
    }

    pub fn lua(&self) -> &Lua {
        &self.lua
    }
}

fn snap_size(d: &DataModel, id: InstId, class: &str, v: [f64; 3]) -> [f64; 3] {
    let fin = |n: f64| if n.is_finite() { n.abs() } else { 1.0 };
    let (mut x, mut y, mut z) = (fin(v[0]), fin(v[1]), fin(v[2]));
    if class == "TrussPart" {
        let f = |n: f64| ((n / 2.0).round() * 2.0).max(2.0);
        return [f(x), f(y), f(z)];
    }
    let ff = if classes::prop_info(class, "FormFactor").is_some() {
        match d.get_prop(id, "FormFactor") {
            Some(Val::Num(n)) => *n as i32,
            _ => 1,
        }
    } else {
        3
    };
    match ff {
        0 => {
            x = x.round().max(1.0);
            y = y.round().max(1.0);
            z = z.round().max(1.0);
        }
        1 => {
            x = x.round().max(1.0);
            y = ((y / 1.2).round() * 1.2).max(1.2);
            z = z.round().max(1.0);
        }
        2 => {
            x = x.round().max(1.0);
            y = ((y / 0.4).round() * 0.4).max(0.4);
            z = z.round().max(1.0);
        }
        _ => {
            x = x.max(0.2);
            y = y.max(0.2);
            z = z.max(0.2);
        }
    }
    [x, y, z]
}

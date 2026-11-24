use crate::host::{ApiHandler, Res, Wire};
use swirl_core::instance::{DataModel, InstId, Val};

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Default)]
pub struct CoreState {
    pub sources: HashMap<u64, Option<String>>,
    pub queue: Vec<(u64, Option<InstId>, String)>,
    pub wanted: Vec<u64>,
    pub libraries: HashMap<String, u64>,
    pub mouse: Option<InstId>,
    pub requested: std::collections::HashSet<u64>,
    pub out: Vec<String>,
    pub own_model: Option<InstId>,
    pub token: u32,
    pub models_wanted: Vec<(u32, u64)>,
    pub gui_keys: std::collections::HashSet<String>,
    pub gui_inset: [f64; 4],
    pub verbs: HashMap<InstId, String>,
    pub screen: (f64, f64),
    pub sound_ops: Vec<serde_json::Value>,
    pub fire_later: Vec<(InstId, &'static str)>,
    pub focus_req: Vec<(InstId, bool)>,
    pub kicked: Option<String>,
    pub own_state: Option<(InstId, u8)>,
    pub friends: HashMap<InstId, f64>,
    pub prompt_req: Vec<(String, u64, bool)>,
    pub finished: Vec<(String, Option<Wire>, u64, bool)>,
    pub mkt: HashMap<u32, u32>,
    pub mkt_seq: u32,
    pub pass_prompts: std::collections::HashSet<u64>,
    pub move_to: Option<(InstId, Option<[f64; 3]>)>,
    pub invokes: HashMap<u32, u32>,
    pub invoke_seq: u32,
    pub keys: std::collections::HashSet<String>,
    pub keys_code: std::collections::HashSet<i64>,
    pub mouse_pos: [f64; 2],
    pub ph: Option<Rc<RefCell<swirl_physics::Physics>>>,
    pub lua_move: Option<[f64; 3]>,
    pub cam_interp: Option<(InstId, [f64; 12], [f64; 12], Option<[f64; 3]>, f64, f64)>,
}

#[derive(Default)]
pub struct AnimState {
    pub an: swirl_physics::animator::Animator,
    pub track_hum: HashMap<InstId, InstId>,
}

pub struct TweenRec {
    pub serial: u32,
    pub t0: f64,
    pub time: f64,
    pub style: i64,
    pub dir: i64,
    pub pos: Option<[f64; 4]>,
    pub size: Option<[f64; 4]>,
    pub from_pos: [f64; 4],
    pub from_size: [f64; 4],
}

#[derive(Default)]
pub struct GuiState {
    pub now: f64,
    pub serial: u32,
    pub tweens: HashMap<InstId, TweenRec>,
    pub kinds: HashMap<InstId, (Option<u32>, Option<u32>)>,
    pub states: HashMap<u32, &'static str>,
}

fn bounce_out(mut x: f64) -> f64 {
    let (n1, d1) = (7.5625, 2.75);
    if x < 1.0 / d1 {
        n1 * x * x
    } else if x < 2.0 / d1 {
        x -= 1.5 / d1;
        n1 * x * x + 0.75
    } else if x < 2.5 / d1 {
        x -= 2.25 / d1;
        n1 * x * x + 0.9375
    } else {
        x -= 2.625 / d1;
        n1 * x * x + 0.984375
    }
}

fn easing(style: i64, dir: i64, t: f64) -> f64 {
    let f = |x: f64| -> f64 {
        match style {
            0 => x,
            1 => 1.0 - (x * std::f64::consts::PI / 2.0).cos(),
            2 => 2.70158 * x * x * x - 1.70158 * x * x,
            3 => x * x,
            4 => x * x * x * x,
            5 => x * x * x * x * x,
            6 => 1.0 - bounce_out(1.0 - x),
            7 => {
                if x <= 0.0 {
                    0.0
                } else if x >= 1.0 {
                    1.0
                } else {
                    -(2.0f64).powf(10.0 * x - 10.0) * ((x * 10.0 - 10.75) * (2.0 * std::f64::consts::PI / 3.0)).sin()
                }
            }
            _ => x * x,
        }
    };
    if dir == 0 {
        f(t)
    } else if dir == 1 {
        1.0 - f(1.0 - t)
    } else if t < 0.5 {
        f(t * 2.0) / 2.0
    } else {
        1.0 - f((1.0 - t) * 2.0) / 2.0
    }
}

impl GuiState {
    pub fn step(&mut self, d: &mut DataModel) {
        let ids: Vec<InstId> = self.tweens.keys().copied().collect();
        for g in ids {
            let alive = d.get(g).map(|i| !i.destroyed).unwrap_or(false);
            let rec = match self.tweens.get(&g) {
                Some(r) => r,
                None => continue,
            };
            let serial = rec.serial;
            if !alive {
                self.states.insert(serial, "canceled");
                self.tweens.remove(&g);
                self.kinds.remove(&g);
                continue;
            }
            let k = if rec.time <= 0.0 { 1.0 } else { ((self.now - rec.t0) / rec.time).min(1.0) };
            let e = easing(rec.style, rec.dir, k);
            let lerp = |a: &[f64; 4], b: &[f64; 4]| -> Vec<f64> { (0..4).map(|i| a[i] + (b[i] - a[i]) * e).collect() };
            if let Some(p) = rec.pos {
                let v = lerp(&rec.from_pos, &p);
                let _ = d.set_prop(g, "Position", Val::Arr(v));
            }
            if let Some(sz) = rec.size {
                let v = lerp(&rec.from_size, &sz);
                let _ = d.set_prop(g, "Size", Val::Arr(v));
            }
            if k >= 1.0 {
                self.states.insert(serial, "completed");
                self.tweens.remove(&g);
                self.kinds.remove(&g);
            }
        }
    }

    pub fn start(&mut self, d: &mut DataModel, g: InstId, pos: Option<[f64; 4]>, size: Option<[f64; 4]>, dir: i64, style: i64, time: f64, over: bool) -> Option<u32> {
        if self.tweens.contains_key(&g) && !over {
            return None;
        }
        if let Some(prev) = self.tweens.remove(&g) {
            self.states.insert(prev.serial, "canceled");
        }
        self.serial += 1;
        let serial = self.serial;
        self.states.insert(serial, "running");
        let get = |k: &str| match d.get_prop(g, k) {
            Some(Val::Arr(a)) if a.len() >= 4 => [a[0], a[1], a[2], a[3]],
            _ => [0.0; 4],
        };
        self.tweens.insert(g, TweenRec { serial, t0: self.now, time, style, dir, pos, size, from_pos: get("Position"), from_size: get("Size") });
        self.kinds.insert(g, (Some(serial), None));
        self.step(d);
        Some(serial)
    }
}

pub struct ClientApi {
    pub core: Rc<RefCell<CoreState>>,
    pub anim: Rc<RefCell<AnimState>>,
    pub gui: Rc<RefCell<GuiState>>,
    pub measure: Option<Rc<dyn crate::gui_layout::Measure>>,
}

fn pivot_of(d: &DataModel, id: InstId) -> [f64; 12] {
    let cfof = |p: InstId| match d.get_prop(p, "CFrame") {
        Some(Val::Arr(a)) => swirl_physics::cf::arr(a),
        _ => None,
    };
    let ident = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    if d.get(id).map(|i| swirl_core::classes::is_a(&i.class, "BasePart")).unwrap_or(false) {
        return cfof(id).unwrap_or(ident);
    }
    if let Some(Val::Ref(Some(pp))) = d.get_prop(id, "PrimaryPart") {
        if d.is_descendant_of(*pp, id) {
            if let Some(c) = cfof(*pp) {
                return c;
            }
        }
    }
    let mut ds = vec![];
    d.descendants(id, &mut ds);
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let mut any = false;
    for p in ds {
        if !d.get(p).map(|i| swirl_core::classes::is_a(&i.class, "BasePart")).unwrap_or(false) {
            continue;
        }
        let (c, sz) = match (cfof(p), d.get_prop(p, "Size")) {
            (Some(c), Some(Val::Arr(s))) if s.len() >= 3 => (c, [s[0], s[1], s[2]]),
            _ => continue,
        };
        any = true;
        for k in 0..3 {
            let e = c[3 + k * 3].abs() * sz[0] / 2.0 + c[4 + k * 3].abs() * sz[1] / 2.0 + c[5 + k * 3].abs() * sz[2] / 2.0;
            lo[k] = lo[k].min(c[k] - e);
            hi[k] = hi[k].max(c[k] + e);
        }
    }
    if !any {
        return ident;
    }
    swirl_physics::cf::with_pos(&ident, (lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0)
}

fn num(args: &[Wire], i: usize) -> Option<f64> {
    match args.get(i) {
        Some(Wire::N(n)) if n.is_finite() => Some(*n),
        _ => None,
    }
}

impl ApiHandler for ClientApi {
    fn get(&mut self, d: &DataModel, id: InstId, key: &str) -> Option<Wire> {
        if key == "TextBounds" && (d.is_a(id, "TextLabel") || d.is_a(id, "TextButton") || d.is_a(id, "TextBox")) {
            let m = self.measure.as_ref()?;
            let (w, h) = self.core.borrow().screen;
            let b = crate::gui_layout::text_bounds(d, id, m.as_ref(), [w, h]);
            return Some(Wire::V2(b));
        }
        None
    }

    fn call(&mut self, d: &mut DataModel, id: InstId, method: &str, args: &[Wire]) -> Option<Res> {
        let class = d.get(id)?.class.clone();
        let ok = |w: Vec<Wire>| Some(Res::Ok(w));
        match method {
            "SetDesiredAngle" if swirl_core::classes::is_a(&class, "Motor") => {
                let _ = d.set_prop(id, "DesiredAngle", Val::Num(num(args, 0).unwrap_or(0.0)));
                ok(vec![])
            }
            "LoadAnimation" | "loadAnimation" if class == "Humanoid" => {
                let anim = match args.first() {
                    Some(Wire::I(a)) if d.get(*a).map(|i| i.class == "Animation").unwrap_or(false) => *a,
                    _ => return Some(Res::Err("LoadAnimation requires an Animation object".into())),
                };
                let name = d.name(anim).to_string();
                let tr = d.create_any("AnimationTrack", None).ok()?;
                d.set_prop_quiet(tr, "Name", Val::Str(name.clone()));
                d.get_mut_name(tr, &name);
                d.set_prop_quiet(tr, "Animation", Val::Ref(Some(anim)));
                self.anim.borrow_mut().track_hum.insert(tr, id);
                ok(vec![Wire::I(tr)])
            }
            "Play" | "play" | "Stop" | "stop" | "AdjustSpeed" | "AdjustWeight" if class == "AnimationTrack" => {
                let mut st = self.anim.borrow_mut();
                let hum = match st.track_hum.get(&id) {
                    Some(h) => *h,
                    None => return ok(vec![]),
                };
                let aid = match d.get_prop(id, "Animation") {
                    Some(Val::Ref(Some(a))) => match d.get_prop(*a, "AnimationId") {
                        Some(Val::Str(u)) => swirl_physics::animator::anim_asset_id(u),
                        _ => None,
                    },
                    _ => None,
                };
                let key = format!("l{}", id);
                let (op, a, b, c) = match method {
                    "Play" | "play" => ("play", num(args, 0), num(args, 1), num(args, 2)),
                    "Stop" | "stop" => ("stop", num(args, 0), None, None),
                    "AdjustSpeed" => ("speed", num(args, 0), None, None),
                    _ => ("weight", num(args, 0), num(args, 1), None),
                };
                st.an.op(d, hum, &key, op, aid, a, b, c, Some(id), 0.0);
                ok(vec![])
            }
            "EquipTool" if class == "Humanoid" => {
                let own = self.core.borrow().own_model;
                if let (Some(Wire::I(t)), Some(m)) = (args.first(), own) {
                    if d.get(id).and_then(|i| i.parent) == Some(m) {
                        self.core.borrow_mut().out.push(format!("{{\"t\":\"equip\",\"id\":{}}}", t));
                    }
                }
                ok(vec![])
            }
            "UnequipTools" if class == "Humanoid" => {
                self.core.borrow_mut().out.push("{\"t\":\"unequip\",\"id\":-1}".to_string());
                ok(vec![])
            }
            "AddCoreScript" if class == "ScriptContext" => {
                let cid = num(args, 0).unwrap_or(0.0) as u64;
                let parent = match args.get(1) {
                    Some(Wire::I(p)) => Some(*p),
                    _ => None,
                };
                let name = match args.get(2) {
                    Some(Wire::S(n)) => n.clone(),
                    _ => "CoreScript".to_string(),
                };
                self.core.borrow_mut().queue.push((cid, parent, name));
                ok(vec![])
            }
            "AddStarterScript" if class == "ScriptContext" => {
                let cid = num(args, 0).unwrap_or(0.0) as u64;
                self.core.borrow_mut().queue.push((cid, Some(id), "StarterScript".to_string()));
                ok(vec![])
            }
            "RegisterLibrary" if class == "ScriptContext" => {
                let name = match args.first() {
                    Some(Wire::S(n)) => n.rsplit('/').next().unwrap_or("").to_string(),
                    _ => return ok(vec![]),
                };
                let lid = match args.get(1) {
                    Some(Wire::N(n)) => *n as u64,
                    Some(Wire::S(t)) => t.parse().unwrap_or(0),
                    _ => 0,
                };
                self.core.borrow_mut().libraries.insert(name, lid);
                ok(vec![])
            }
            "LibraryRegistrationComplete" | "SetTimeout" | "SetCollectScriptStats" if class == "ScriptContext" => ok(vec![]),
            "GetCoreScripts" if class == "ScriptContext" => ok(vec![Wire::List(vec![])]),
            "__librarySource" if id == d.root => {
                let c = self.core.borrow();
                let src = match args.first() {
                    Some(Wire::S(n)) => c.libraries.get(n).and_then(|i| c.sources.get(i)).cloned().flatten(),
                    _ => None,
                };
                ok(vec![src.map(Wire::S).unwrap_or(Wire::Nil)])
            }
            "GetMouse" if class == "Player" => {
                let m = self.core.borrow().mouse;
                ok(vec![m.map(Wire::I).unwrap_or(Wire::Nil)])
            }
            "LoadAsset" | "loadAsset" if class == "InsertService" => {
                let aid = num(args, 0).unwrap_or(0.0) as u64;
                let mut c = self.core.borrow_mut();
                c.token += 1;
                let tok = 1_000_000 + c.token;
                c.models_wanted.push((tok, aid));
                Some(Res::Async(tok))
            }
            "Insert" if class == "InsertService" => {
                if let (Some(Wire::I(m)), Some(ws)) = (args.first(), d.services.get("Workspace").copied()) {
                    let _ = d.set_parent(*m, Some(ws));
                }
                ok(vec![])
            }
            "ApproveAssetId" if class == "InsertService" => ok(vec![]),
            "AddKey" | "AddSpecialKey" | "RemoveKey" | "RemoveSpecialKey" if class == "GuiService" => {
                let key = match args.first() {
                    Some(Wire::E(_, v)) => format!("special:{}", *v as i64),
                    Some(Wire::S(k)) => k.to_lowercase(),
                    _ => String::new(),
                };
                let mut c = self.core.borrow_mut();
                if method.starts_with("Add") {
                    c.gui_keys.insert(key);
                } else {
                    c.gui_keys.remove(&key);
                }
                ok(vec![])
            }
            "SetGlobalGuiInset" if class == "GuiService" => {
                let g = |i: usize| num(args, i).unwrap_or(0.0);
                self.core.borrow_mut().gui_inset = [g(0), g(1), g(2), g(3)];
                ok(vec![])
            }
            "SetGlobalSizeOffsetPixel" if class == "GuiService" => {
                let y = num(args, 1).unwrap_or(0.0);
                self.core.borrow_mut().gui_inset = [0.0, 0.0, 0.0, -y];
                ok(vec![])
            }
            "OpenBrowserWindow" | "CloseBrowserWindow" if class == "GuiService" => ok(vec![]),
            "IsTenFootInterface" | "IsTouchDevice" if class == "GuiService" => ok(vec![Wire::B(false)]),
            "GetTeams" if class == "Teams" => {
                let kids: Vec<Wire> = d.get(id).map(|i| i.children.iter().copied().filter(|c| d.get(*c).map(|x| x.class == "Team").unwrap_or(false)).map(Wire::I).collect()).unwrap_or_default();
                ok(vec![Wire::List(kids)])
            }
            "TweenPosition" | "tweenPosition" | "TweenSize" | "tweenSize" | "TweenSizeAndPosition" | "tweenSizeAndPosition" if swirl_core::classes::is_a(&class, "GuiObject") => {
                let ud = |i: usize| match args.get(i) {
                    Some(Wire::Ud2(v)) => Some(*v),
                    _ => None,
                };
                let lower = method.to_lowercase();
                let (size, pos, rest) = if lower == "tweenposition" {
                    (None, ud(0), 1)
                } else if lower == "tweensize" {
                    (ud(0), None, 1)
                } else {
                    (ud(0), ud(1), 2)
                };
                let enum_val = |i: usize, table: &[&str], dflt: i64| -> i64 {
                    match args.get(i) {
                        Some(Wire::E(_, v)) => *v as i64,
                        Some(Wire::N(v)) => *v as i64,
                        Some(Wire::S(n)) => table.iter().position(|x| x == n).map(|p| p as i64).unwrap_or(dflt),
                        _ => dflt,
                    }
                };
                let dir = enum_val(rest, &["In", "Out", "InOut"], 1);
                let style = enum_val(rest + 1, &["Linear", "Sine", "Back", "Quad", "Quart", "Quint", "Bounce", "Elastic"], 3);
                let time = match args.get(rest + 2) {
                    Some(Wire::N(t)) if t.is_finite() => *t,
                    _ => 1.0,
                };
                let over = matches!(args.get(rest + 3), Some(Wire::B(true)));
                if pos.is_none() && size.is_none() {
                    return ok(vec![Wire::B(false)]);
                }
                match self.gui.borrow_mut().start(d, id, pos, size, dir, style, time, over) {
                    Some(s) => ok(vec![Wire::N(s as f64)]),
                    None => ok(vec![Wire::B(false)]),
                }
            }
            "__TweenState" if swirl_core::classes::is_a(&class, "GuiObject") => {
                let serial = num(args, 0).unwrap_or(0.0) as u32;
                let st = self.gui.borrow().states.get(&serial).copied().unwrap_or("canceled");
                ok(vec![Wire::S(st.to_string())])
            }
            "AddTag" | "RemoveTag" | "HasTag" | "GetTags" => {
                let (target, tag) = match (class.as_str(), args.first(), args.get(1)) {
                    ("CollectionService", Some(Wire::I(t)), Some(Wire::S(g))) => (*t, g.clone()),
                    ("CollectionService", Some(Wire::I(t)), None) => (*t, String::new()),
                    (_, Some(Wire::S(g)), _) => (id, g.clone()),
                    _ => (id, String::new()),
                };
                let inst = d.get_mut_inst(target)?;
                match method {
                    "AddTag" => {
                        if !inst.tags.contains(&tag) {
                            inst.tags.push(tag);
                        }
                        ok(vec![])
                    }
                    "RemoveTag" => {
                        inst.tags.retain(|t| *t != tag);
                        ok(vec![])
                    }
                    "HasTag" => ok(vec![Wire::B(inst.tags.contains(&tag))]),
                    _ => ok(vec![Wire::List(inst.tags.iter().map(|t| Wire::S(t.clone())).collect())]),
                }
            }
            "GetTagged" if class == "CollectionService" => {
                let tag = match args.first() {
                    Some(Wire::S(t)) => t.clone(),
                    _ => String::new(),
                };
                let mut all = vec![];
                let root = d.root;
                d.descendants(root, &mut all);
                let list: Vec<Wire> = all.into_iter().filter(|i| d.get(*i).map(|x| x.tags.contains(&tag)).unwrap_or(false)).map(Wire::I).collect();
                ok(vec![Wire::List(list)])
            }
            "GetCollection" if class == "CollectionService" => {
                let cls = match args.first() {
                    Some(Wire::S(t)) => t.clone(),
                    _ => String::new(),
                };
                let mut all = vec![];
                let root = d.root;
                d.descendants(root, &mut all);
                let list: Vec<Wire> = all.into_iter().filter(|i| d.get(*i).map(|x| x.class == cls).unwrap_or(false)).map(Wire::I).collect();
                ok(vec![Wire::List(list)])
            }
            "SetVerb" if swirl_core::classes::is_a(&class, "GuiButton") => {
                let verb = match args.first() {
                    Some(Wire::S(v)) => v.clone(),
                    _ => String::new(),
                };
                self.core.borrow_mut().verbs.insert(id, verb);
                ok(vec![])
            }
            "IsLoaded" if id == d.root => ok(vec![Wire::B(true)]),
            "GetMessage" if id == d.root => ok(vec![Wire::S(String::new())]),
            "SetMessage" | "ClearMessage" | "FinishShutdown" | "Shutdown" if id == d.root => ok(vec![]),
            "GetJobsInfo" if id == d.root => ok(vec![Wire::List(vec![])]),
            "IsDefaultLoadingGuiRemoved" if class == "ReplicatedFirst" => ok(vec![Wire::B(true)]),
            "RemoveDefaultLoadingGui" if class == "ReplicatedFirst" => ok(vec![]),
            "SetTutorialState" if class == "UserGameSettings" => ok(vec![]),
            "GetTutorialState" | "InStudioMode" | "InFullScreen" if class == "UserGameSettings" => ok(vec![Wire::B(false)]),
            "GetMaxQualityLevel" if class == "RenderSettings" => ok(vec![Wire::N(21.0)]),
            "GetCoreGuiEnabled" if class == "StarterGui" => ok(vec![Wire::B(true)]),
            "SetCoreGuiEnabled" | "SetCore" if class == "StarterGui" => ok(vec![]),
            "GetLogHistory" if class == "LogService" => ok(vec![Wire::List(vec![])]),
            "GetHttpResultHistory" if class == "LogService" => ok(vec![Wire::List(vec![])]),
            "RequestServerHttpResult" | "RequestHttpResultApproved" | "ExecuteScript" | "RequestServerOutput" if class == "LogService" => ok(vec![]),
            "AddSelectionParent" | "AddSelectionTuple" | "RemoveSelectionGroup" if class == "GuiService" => ok(vec![]),
            "GetScreenResolution" if class == "GuiService" => {
                let (w, h) = self.core.borrow().screen;
                ok(vec![Wire::V2([w, h])])
            }
            "IsKeyDown" if class == "UserInputService" => {
                let c = self.core.borrow();
                let down = match args.first() {
                    Some(Wire::S(k)) => c.keys.contains(&k.to_lowercase()),
                    Some(Wire::E(_, v)) => c.keys_code.contains(&(*v as i64)),
                    _ => false,
                };
                ok(vec![Wire::B(down)])
            }
            "GetMouseLocation" if class == "UserInputService" => {
                let m = self.core.borrow().mouse_pos;
                ok(vec![Wire::V2(m)])
            }
            "IsLuaTouchControls" if class == "UserInputService" => ok(vec![Wire::B(false)]),
            "RotateCamera" | "ZoomCamera" if class == "UserInputService" => ok(vec![]),
            "Create" if class == "TweenService" => ok(vec![Wire::Nil]),
            "Message" if class == "TestService" => ok(vec![]),
            "Check" | "Require" | "Fail" | "Error" | "Warn" | "Checkpoint" | "Done" if class == "TestService" => ok(vec![]),
            "Chat" | "chat" | "TeamChat" | "teamChat" | "WhisperChat" if class == "Players" => {
                let (msg, to, kind) = match (method, args.first(), args.get(1)) {
                    ("WhisperChat", Some(Wire::S(m)), Some(Wire::I(t))) => (m.clone(), Some(*t), "whisper"),
                    ("TeamChat" | "teamChat", Some(Wire::S(m)), _) => (m.clone(), None, "team"),
                    (_, Some(Wire::S(m)), _) => (m.clone(), None, "all"),
                    _ => (String::new(), None, "all"),
                };
                if !msg.is_empty() {
                    let mut j = serde_json::json!({"t": "chat", "m": msg, "k": kind});
                    if let Some(t) = to {
                        j["to"] = serde_json::json!(t);
                    }
                    self.core.borrow_mut().out.push(j.to_string());
                }
                ok(vec![])
            }
            "__invokeServer" if class == "RemoteFunction" => {
                let mut c = self.core.borrow_mut();
                c.token += 1;
                let tok = 2_000_000 + c.token;
                c.invoke_seq += 1;
                let k = c.invoke_seq;
                c.invokes.insert(k, tok);
                let a = crate::host::wire_to_json(args);
                c.out.push(serde_json::json!({"t": "rf", "id": id, "k": k, "a": a}).to_string());
                Some(Res::Async(tok))
            }
            "__hasProperty" => {
                let n = match args.first() {
                    Some(Wire::S(x)) => x.clone(),
                    _ => String::new(),
                };
                ok(vec![Wire::B(swirl_core::classes::canonical_prop(&class, &n).is_some())])
            }
            "__newTween" if class == "TweenService" => {
                let tw = d.create_any("Tween", None).ok()?;
                d.set_prop_quiet(tw, "Name", Val::Str("Tween".into()));
                if let Some(Wire::I(t)) = args.first() {
                    d.set_prop_quiet(tw, "Instance", Val::Ref(Some(*t)));
                }
                ok(vec![Wire::I(tw)])
            }
            "GetPivot" if class == "Model" || swirl_core::classes::is_a(&class, "BasePart") => ok(vec![Wire::Cf(pivot_of(d, id))]),
            "PivotTo" | "SetPrimaryPartCFrame" if class == "Model" || swirl_core::classes::is_a(&class, "BasePart") => {
                let goal = match args.first() {
                    Some(Wire::Cf(c)) if swirl_physics::cf::is_finite(c) => *c,
                    _ => return ok(vec![]),
                };
                if class == "Model" {
                    let cur = pivot_of(d, id);
                    let rel = swirl_physics::cf::mul(&goal, &swirl_physics::cf::inv(&cur));
                    let mut ds = vec![];
                    d.descendants(id, &mut ds);
                    for p in ds {
                        if d.get(p).map(|i| swirl_core::classes::is_a(&i.class, "BasePart")).unwrap_or(false) {
                            if let Some(Val::Arr(a)) = d.get_prop(p, "CFrame") {
                                if let Some(c) = swirl_physics::cf::arr(a) {
                                    let n = swirl_physics::cf::orthonormalize(&swirl_physics::cf::mul(&rel, &c));
                                    let _ = d.set_prop(p, "CFrame", Val::Arr(n.to_vec()));
                                }
                            }
                        }
                    }
                } else {
                    let _ = d.set_prop(id, "CFrame", Val::Arr(swirl_physics::cf::orthonormalize(&goal).to_vec()));
                }
                ok(vec![])
            }
            "Move" if class == "Player" || class == "Humanoid" => {
                let dir = match args.first() {
                    Some(Wire::V3(v)) => *v,
                    _ => return ok(vec![]),
                };
                let rel = matches!(args.get(1), Some(Wire::B(true)));
                let (mut x, mut z) = (dir[0], dir[2]);
                let len = (x * x + z * z).sqrt();
                let mut c = self.core.borrow_mut();
                if len < 1e-3 {
                    c.lua_move = None;
                    return ok(vec![]);
                }
                if len > 1.0 {
                    x /= len;
                    z /= len;
                }
                if rel {
                    let cam = d.services.get("Workspace").and_then(|w| match d.get_prop(*w, "CurrentCamera") {
                        Some(Val::Ref(Some(c))) => Some(*c),
                        _ => None,
                    });
                    if let Some(Val::Arr(a)) = cam.and_then(|c| d.get_prop(c, "CFrame")) {
                        if a.len() >= 12 {
                            let (mut fx, mut fz) = (-a[5], -a[11]);
                            let fl = (fx * fx + fz * fz).sqrt();
                            let fl = if fl == 0.0 { 1.0 } else { fl };
                            fx /= fl;
                            fz /= fl;
                            let (rx, rz) = (-fz, fx);
                            c.lua_move = Some([rx * x - fx * z, 0.0, rz * x - fz * z]);
                            return ok(vec![]);
                        }
                    }
                }
                c.lua_move = Some([x, 0.0, z]);
                ok(vec![])
            }
            "Interpolate" if class == "Camera" => {
                let cf = match args.first() {
                    Some(Wire::Cf(c)) => *c,
                    _ => return ok(vec![]),
                };
                let focus = match args.get(1) {
                    Some(Wire::Cf(f)) => Some([f[0], f[1], f[2]]),
                    Some(Wire::V3(f)) => Some(*f),
                    _ => None,
                };
                let goal = match focus {
                    Some(f) => swirl_physics::cf::look_at(cf[0], cf[1], cf[2], f[0], f[1], f[2]),
                    None => cf,
                };
                let dur = num(args, 2).unwrap_or(0.0);
                let start = match d.get_prop(id, "CFrame") {
                    Some(Val::Arr(a)) => swirl_physics::cf::arr(a),
                    _ => None,
                };
                match start {
                    Some(st) if dur > 0.05 => {
                        let t0 = self.gui.borrow().now;
                        self.core.borrow_mut().cam_interp = Some((id, st, goal, focus, t0, dur));
                    }
                    _ => {
                        self.core.borrow_mut().cam_interp = None;
                        let _ = d.set_prop(id, "CFrame", Val::Arr(goal.to_vec()));
                        if let Some(f) = focus {
                            let _ = d.set_prop(id, "Focus", Val::Arr(swirl_physics::cf::with_pos(&swirl_physics::cf::rot_only(&goal), f[0], f[1], f[2]).to_vec()));
                        }
                    }
                }
                ok(vec![])
            }
            "MoveTo" | "moveTo" if class == "Humanoid" => {
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
                self.core.borrow_mut().move_to = Some((id, pos));
                ok(vec![])
            }
            "GetProductInfo" | "PlayerOwnsAsset" | "UserOwnsGamePassAsync" | "GetRobuxBalance" | "PerformPurchase" if class == "MarketplaceService" || (class == "GamePassService") => {
                let uid_of = |d: &DataModel, w: Option<&Wire>| -> f64 {
                    match w {
                        Some(Wire::I(p)) if d.get(*p).map(|i| i.class == "Player").unwrap_or(false) => match d.get_prop(*p, "userId") {
                            Some(Val::Num(n)) => *n,
                            _ => 0.0,
                        },
                        Some(Wire::N(n)) => *n,
                        _ => 0.0,
                    }
                };
                let (op, body) = match method {
                    "GetProductInfo" => {
                        let kind = match args.get(1) {
                            Some(Wire::E(_, v)) if *v as i64 == 1 => "product",
                            Some(Wire::E(_, v)) if *v as i64 == 2 => "gamepass",
                            Some(Wire::N(v)) if *v as i64 == 1 => "product",
                            Some(Wire::N(v)) if *v as i64 == 2 => "gamepass",
                            _ => "asset",
                        };
                        ("info", serde_json::json!({"id": num(args, 0).unwrap_or(0.0) as i64, "kind": kind}))
                    }
                    "GetRobuxBalance" => ("balance", serde_json::json!({})),
                    "PerformPurchase" => {
                        let kind = match args.first() {
                            Some(Wire::E(_, v)) if *v as i64 == 1 => "product",
                            Some(Wire::N(v)) if *v as i64 == 1 => "product",
                            _ => "asset",
                        };
                        ("buy", serde_json::json!({"kind": kind, "id": num(args, 1).unwrap_or(0.0) as i64, "price": num(args, 2).unwrap_or(0.0).round() as i64}))
                    }
                    _ => ("owns", serde_json::json!({"uid": uid_of(d, args.first()) as i64, "id": num(args, 1).unwrap_or(0.0) as i64})),
                };
                let mut c = self.core.borrow_mut();
                c.token += 1;
                let tok = 3_000_000 + c.token;
                c.mkt_seq += 1;
                let k = c.mkt_seq;
                c.mkt.insert(k, tok);
                let mut j = body;
                j["t"] = serde_json::json!("mkt");
                j["op"] = serde_json::json!(op);
                j["k"] = serde_json::json!(k);
                c.out.push(j.to_string());
                let _ = op;
                Some(Res::Async(tok))
            }
            "PlayerHasPass" if class == "GamePassService" => ok(vec![Wire::B(false)]),
            "PlayerCanMakePurchases" if class == "MarketplaceService" => {
                let uid = match args.first() {
                    Some(Wire::I(p)) => match d.get_prop(*p, "userId") {
                        Some(Val::Num(n)) => *n,
                        _ => 0.0,
                    },
                    Some(Wire::N(n)) => *n,
                    _ => 0.0,
                };
                ok(vec![Wire::B(uid > 0.0)])
            }
            "PromptPurchase" | "PromptProductPurchase" | "PromptGamePassPurchase" if class == "MarketplaceService" => {
                match args.first() {
                    Some(Wire::I(p)) if d.get(*p).map(|i| i.class == "Player").unwrap_or(false) => {}
                    _ => return Some(Res::Err("Argument 1 missing or nil".into())),
                }
                let pid = num(args, 1).unwrap_or(0.0) as u64;
                let equip = matches!(args.get(2), Some(Wire::B(true)));
                let kind = if method == "PromptGamePassPurchase" { "gamepass" } else if method == "PromptProductPurchase" { "product" } else { "asset" };
                self.core.borrow_mut().prompt_req.push((kind.to_string(), pid, equip));
                ok(vec![])
            }
            "SignalPromptPurchaseFinished" | "SignalPromptGamePassPurchaseFinished" | "SignalPromptProductPurchaseFinished" if class == "MarketplaceService" => {
                let pid = num(args, 1).unwrap_or(0.0) as u64;
                let okf = matches!(args.get(2), Some(Wire::B(true)));
                let mut kind = if method == "SignalPromptGamePassPurchaseFinished" { "gamepass" } else if method == "SignalPromptProductPurchaseFinished" { "product" } else { "asset" };
                let mut c = self.core.borrow_mut();
                if kind == "asset" && c.pass_prompts.remove(&pid) {
                    kind = "gamepass";
                }
                c.finished.push((kind.to_string(), args.first().cloned(), pid, okf));
                c.out.push(serde_json::json!({"t": "pfin", "kind": kind, "id": pid, "ok": okf}).to_string());
                ok(vec![])
            }
            "SignalClientPurchaseSuccess" if class == "MarketplaceService" => {
                self.core.borrow_mut().out.push("{\"t\":\"rcpt\"}".to_string());
                ok(vec![])
            }
            "ReportAssetSale" | "ReportRobuxUpsellStarted" if class == "MarketplaceService" => ok(vec![]),
            "CaptureFocus" | "ReleaseFocus" if class == "TextBox" => {
                self.core.borrow_mut().focus_req.push((id, method == "CaptureFocus"));
                ok(vec![])
            }
            "Kick" if class == "Player" => {
                if Some(id) == d.local_player {
                    self.core.borrow_mut().kicked = Some(match args.first() {
                        Some(Wire::S(m)) if !m.is_empty() => m.clone(),
                        _ => "You have been kicked".to_string(),
                    });
                }
                ok(vec![])
            }
            "GetState" if class == "Humanoid" => {
                let st = self.core.borrow().own_state;
                let v = match st {
                    Some((h, s)) if h == id => [8.0, 8.0, 3.0, 5.0, 13.0, 12.0, 15.0, 8.0, 4.0].get(s as usize).copied().unwrap_or(8.0),
                    _ => 8.0,
                };
                ok(vec![Wire::E("HumanoidStateType".into(), v)])
            }
            "GetSunDirection" | "GetMoonDirection" if class == "Lighting" => {
                let tod = match d.get_prop(id, "TimeOfDay") {
                    Some(Val::Str(s)) => s.clone(),
                    _ => "14:00:00".into(),
                };
                let p: Vec<f64> = tod.split(':').map(|x| x.trim().parse().unwrap_or(0.0)).collect();
                let min = p.first().copied().unwrap_or(0.0) * 60.0 + p.get(1).copied().unwrap_or(0.0) + p.get(2).copied().unwrap_or(0.0) / 60.0;
                let lat = match d.get_prop(id, "GeographicLatitude") {
                    Some(Val::Num(n)) => *n * std::f64::consts::PI / 180.0,
                    _ => 41.7333 * std::f64::consts::PI / 180.0,
                };
                let h = (min / 1440.0 * 2.0 - 1.0) * std::f64::consts::PI;
                let (x, y, z) = (-h.sin(), h.cos() * (lat - 0.4).cos(), h.cos() * (lat - 0.4).sin());
                let l = (x * x + y * y + z * z).sqrt().max(1e-12);
                let sgn = if method == "GetMoonDirection" { -1.0 } else { 1.0 };
                ok(vec![Wire::V3([sgn * x / l, sgn * y / l, sgn * z / l])])
            }
            "DistanceFromCharacter" if class == "Player" => {
                let p = match args.first() {
                    Some(Wire::V3(v)) => *v,
                    _ => return ok(vec![Wire::N(0.0)]),
                };
                let ch = match d.get_prop(id, "Character") {
                    Some(Val::Ref(Some(c))) => *c,
                    _ => return ok(vec![Wire::N(0.0)]),
                };
                let h = d.find_child(ch, "Head", false).or_else(|| d.find_child(ch, "Torso", false));
                let c = match h.and_then(|h| d.get_prop(h, "CFrame")) {
                    Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
                    _ => return ok(vec![Wire::N(0.0)]),
                };
                ok(vec![Wire::N(((c[0] - p[0]).powi(2) + (c[1] - p[1]).powi(2) + (c[2] - p[2]).powi(2)).sqrt())])
            }
            "FindService" | "findService" if id == d.root => {
                let n = match args.first() {
                    Some(Wire::S(n)) => n.clone(),
                    Some(Wire::I(i)) => d.get(*i).map(|x| x.class.clone()).unwrap_or_default(),
                    _ => String::new(),
                };
                ok(vec![d.services.get(&n).map(|s| Wire::I(*s)).unwrap_or(Wire::Nil)])
            }
            "GenerateGUID" if class == "HttpService" => {
                let mut c = self.core.borrow_mut();
                c.token += 1;
                let n = c.token as u64 * 2654435761u64;
                ok(vec![Wire::S(format!("{{{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}}}", n as u32, (n >> 8) as u16, (n >> 20) as u16 & 0xfff, (n >> 4) as u16 & 0xfff, n.wrapping_mul(40503)))])
            }
            "GetAsync" | "PostAsync" | "HttpGet" | "HttpGetAsync" | "HttpPostAsync" if class == "HttpService" || id == d.root => Some(Res::Err("Http requests are not enabled".into())),
            "GetFriendStatus" if class == "Player" => {
                let other = match args.first() {
                    Some(Wire::I(o)) => *o,
                    _ => 0,
                };
                let st = if Some(id) == d.local_player { self.core.borrow().friends.get(&other).copied().unwrap_or(1.0) } else { 1.0 };
                ok(vec![Wire::E("FriendStatus".into(), st)])
            }
            "IsFriendsWith" | "isFriendsWith" if class == "Player" => {
                let uid = num(args, 0).unwrap_or(0.0);
                let mut other = None;
                if let Some(pl) = d.services.get("Players") {
                    for c in d.get(*pl).map(|i| i.children.clone()).unwrap_or_default() {
                        if matches!(d.get_prop(c, "userId"), Some(Val::Num(n)) if *n == uid) {
                            other = Some(c);
                        }
                    }
                }
                let st = if Some(id) == d.local_player { other.and_then(|o| self.core.borrow().friends.get(&o).copied()).unwrap_or(1.0) } else { 1.0 };
                ok(vec![Wire::B(st == 2.0)])
            }
            "GetFriendsOnline" if class == "Player" => ok(vec![Wire::List(vec![])]),
            "RequestFriendship" | "RevokeFriendship" if class == "Player" => {
                if let (Some(Wire::I(o)), true) = (args.first(), Some(id) == d.local_player) {
                    if *o != id && d.get(*o).map(|i| i.class == "Player").unwrap_or(false) {
                        self.core.borrow_mut().out.push(serde_json::json!({"t": "friend", "id": o, "on": method == "RequestFriendship"}).to_string());
                    }
                }
                ok(vec![])
            }
            "ReportAbuse" if class == "Players" => ok(vec![]),
            "ClearCharacterAppearance" | "SetSuperSafeChat" | "SetUnder13" | "SetMembershipType" | "SetAccountAge" if class == "Player" => ok(vec![]),
            "GetUnder13" if class == "Player" => ok(vec![Wire::B(false)]),
            "GetGroupsAsync" if class == "GroupService" => ok(vec![Wire::List(vec![])]),
            "GetGroupInfoAsync" if class == "GroupService" => ok(vec![Wire::Nil]),
            "Play" | "play" | "Stop" | "stop" | "Pause" | "pause" | "Resume" | "resume" if class == "Sound" => {
                let on = matches!(method, "Play" | "play" | "Resume" | "resume");
                let _ = d.set_prop(id, "IsPlaying", Val::Bool(on));
                let op = match method {
                    "Play" | "play" => "play",
                    "Stop" | "stop" => "stop",
                    "Pause" | "pause" => "pause",
                    _ => "resume",
                };
                let mut c = self.core.borrow_mut();
                c.sound_ops.push(serde_json::json!({"op": op, "id": id}));
                match op {
                    "play" => c.fire_later.push((id, "Played")),
                    "stop" => c.fire_later.push((id, "Stopped")),
                    _ => {}
                }
                ok(vec![])
            }
            "ChangeState" | "SetStateEnabled" if class == "Humanoid" => ok(vec![]),
            "GetPlayingAnimationTracks" if class == "Humanoid" => ok(vec![Wire::List(self.anim.borrow().an.playing(id).into_iter().map(Wire::I).collect())]),
            _ if d.era == 2018 => self.call2018(d, id, &class, method, args),
            _ => None,
        }
    }
}

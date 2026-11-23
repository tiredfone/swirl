use serde_json::{json, Value as J};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use swirl_core::instance::{DataModel, InstId, Val};
use swirl_lua::host::{ApiHandler, Res, Wire};

pub struct Seq {
    pub length: f64,
    pub looped: bool,
    pub frames: Vec<(f64, String)>,
}

pub struct Track {
    pub hum: Option<InstId>,
    pub key: String,
    pub anim_id: Option<u64>,
    pub seq: Option<Rc<Seq>>,
    pub target: f64,
    pub w: f64,
    pub weight: f64,
    pub speed: f64,
    pub fade: f64,
    pub time: f64,
    pub playing: bool,
}

pub struct ApiState {
    pub assets_dir: String,
    pub data_dir: String,
    pub place_id: u64,
    pub era: i32,
    pub stores: HashMap<InstId, String>,
    pub store_keys: HashMap<String, InstId>,
    pub data: HashMap<String, HashMap<String, String>>,
    pub data_loaded: bool,
    pub data_dirty: bool,
    pub players: HashMap<i64, PlayerBag>,
    pub players_dirty: HashSet<i64>,
    pub tracks: HashMap<InstId, Track>,
    pub seqs: HashMap<u64, Option<Rc<Seq>>>,
    pub events: Vec<(InstId, String, Vec<Wire>)>,
    pub broadcasts: Vec<J>,
    pub completions: Vec<(u32, Result<Vec<Wire>, String>)>,
    pub token: u32,
    pub social: Vec<SocialReq>,
    pub directed: Vec<(InstId, J)>,
    pub receipt_results: Vec<(u32, String)>,
    pub assets_wanted: Vec<(u32, u64)>,
    pub path_cache: HashMap<String, (f64, (i32, Vec<([f64; 3], bool)>))>,
    pub now_ms: f64,
    pub info_cache: HashMap<String, (std::time::Instant, Option<J>)>,
    pub kicks: Vec<(InstId, String)>,
    pub hum_state: HashMap<InstId, u8>,
}

#[derive(Default, Clone)]
pub struct PlayerBag {
    pub n: HashMap<String, f64>,
    pub s: HashMap<String, String>,
    pub b: HashMap<String, bool>,
    pub i: HashMap<String, String>,
}

#[derive(Clone, Copy)]
pub enum SocialKind {
    Bool(&'static str),
    Info,
    Award,
    Mkt(InstId, i64, u8),
    Pending(InstId),
    Grant,
    Report,
}

pub struct SocialReq {
    pub token: u32,
    pub body: J,
    pub kind: SocialKind,
}

pub fn json_wire(v: &J) -> Wire {
    match v {
        J::Null => Wire::Nil,
        J::Bool(b) => Wire::B(*b),
        J::Number(n) => Wire::N(n.as_f64().unwrap_or(0.0)),
        J::String(s) => Wire::S(s.clone()),
        J::Array(a) => Wire::List(a.iter().map(json_wire).collect()),
        J::Object(o) => Wire::Dict(o.iter().map(|(k, v)| (k.clone(), json_wire(v))).collect()),
    }
}

pub type Shared = Rc<RefCell<ApiState>>;

pub fn new_state(assets_dir: &str, data_dir: &str, place_id: u64, era: i32) -> Shared {
    Rc::new(RefCell::new(ApiState {
        assets_dir: assets_dir.to_string(),
        data_dir: data_dir.to_string(),
        place_id,
        era,
        stores: HashMap::new(),
        store_keys: HashMap::new(),
        data: HashMap::new(),
        data_loaded: false,
        data_dirty: false,
        players: HashMap::new(),
        players_dirty: HashSet::new(),
        tracks: HashMap::new(),
        seqs: HashMap::new(),
        events: vec![],
        broadcasts: vec![],
        completions: vec![],
        token: 1_000_000,
        social: vec![],
        directed: vec![],
        receipt_results: vec![],
        assets_wanted: vec![],
        path_cache: HashMap::new(),
        now_ms: 0.0,
        info_cache: HashMap::new(),
        kicks: vec![],
        hum_state: HashMap::new(),
    }))
}

pub const METHODS: &[&str] = &[
    "__dataStore", "__dsGet", "__dsSet", "__dsSorted", "LoadAnimation", "loadAnimation", "GetPlayingAnimationTracks", "AdjustSpeed", "AdjustWeight",
    "GetTranslatorForPlayer", "WaitForDataReady", "waitForDataReady", "SaveNumber", "SaveString", "SaveBoolean", "LoadNumber", "LoadString", "LoadBoolean",
    "SaveInstance", "LoadInstance", "GetPlayerByUserId", "GetPlayerByID", "GetUserIdFromNameAsync", "GetNameFromUserIdAsync",
    "AwardBadge", "UserHasBadge", "PlayerOwnsAsset", "UserOwnsGamePassAsync", "PlayerHasPass", "GetProductInfo", "PlayerCanMakePurchases", "PromptPurchase",
    "PromptProductPurchase", "PromptGamePassPurchase", "Teleport", "TeleportToSpawnByName", "TeleportToPlaceInstance", "GetRobuxBalance", "PerformPurchase",
    "LoadAsset", "loadAsset", "Insert", "FindFirstAncestor", "FindFirstAncestorOfClass", "GetDebugId", "GetServerTimeNow", "AddTag", "RemoveTag", "HasTag", "GetTags", "GetTagged",
    "FindPartOnRay", "findPartOnRay", "FindPartOnRayWithIgnoreList", "findPartOnRayWithIgnoreList", "FindPartOnRayWithWhitelist", "GetMass", "getMass", "GetConnectedParts",
    "GetRootPart", "GetTouchingParts", "BreakJoints", "breakJoints", "MakeJoints", "makeJoints", "__ComputePath",
    "MoveTo", "moveTo", "TranslateBy", "GetModelCFrame", "getModelCFrame", "GetModelSize", "getModelSize", "GetExtentsSize", "GetPrimaryPartCFrame", "SetPrimaryPartCFrame",
    "GetPivot", "PivotTo",
    "__receiptResult", "SetCell", "GetCell", "SetCells", "GetWaterCell", "SetWaterCell", "AutowedgeCell", "AutowedgeCells", "CellCenterToWorld", "CellCornerToWorld", "WorldToCell", "WorldToCellPreferSolid", "WorldToCellPreferEmpty", "CountCells", "Clear", "ConvertToSmooth", "FillBlock", "FillBall", "FillCylinder", "FillWedge", "FillRegion", "ReplaceMaterial", "__ReadVoxels", "__WriteVoxels", "__MaterialIndex", "__MaterialEnum", "GetMaterialColor", "SetMaterialColor", "CopyRegion", "PasteRegion", "GetTeams", "GetMinutesAfterMidnight", "SetMinutesAfterMidnight", "getMinutesAfterMidnight", "setMinutesAfterMidnight",
];

fn user_id_of(d: &DataModel, w: Option<&Wire>) -> i64 {
    match w {
        Some(Wire::I(p)) => match d.get_prop(*p, "userId") {
            Some(Val::Num(n)) => *n as i64,
            _ => 0,
        },
        Some(Wire::N(n)) => *n as i64,
        _ => 0,
    }
}

fn anim_asset_id(url: &str) -> Option<u64> {
    let s = url.trim();
    let digits = |t: &str| -> Option<u64> {
        let d: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        if d.is_empty() { None } else { d.parse().ok() }
    };
    if let Some(i) = s.to_ascii_lowercase().find("id=") {
        return digits(&s[i + 3..]);
    }
    if let Some(r) = s.strip_prefix("rbxassetid://") {
        return digits(r);
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        return digits(s);
    }
    None
}

fn read_asset(dir: &str, id: u64) -> Option<Vec<u8>> {
    std::fs::read(format!("{}/assets/{}", dir, id)).ok().or_else(|| std::fs::read(format!("{}/cache/{}", dir, id)).ok())
}

fn find_sequence(nodes: &[swirl_core::rbx::Inst]) -> Option<Seq> {
    for n in nodes {
        if n.cls == "KeyframeSequence" {
            let looped = matches!(n.props.get("Loop"), Some(swirl_core::rbx::Value::Bool(true)));
            let mut frames: Vec<(f64, String)> = vec![];
            for k in &n.children {
                if k.cls == "Keyframe" {
                    let t = k.props.get("Time").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    frames.push((t, k.name.clone()));
                }
            }
            frames.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let length = frames.last().map(|f| f.0).unwrap_or(0.0);
            return Some(Seq { length, looped, frames });
        }
        if let Some(s) = find_sequence(&n.children) {
            return Some(s);
        }
    }
    None
}

pub fn make_model(d: &mut DataModel, bytes: &[u8]) -> Result<InstId, String> {
    let tree = swirl_core::rbx::parse_limit(bytes, 64 * 1024 * 1024).map_err(|_| "asset is not a model".to_string())?;
    let model = d.create_any("Model", None)?;
    d.set_prop_quiet(model, "Name", Val::Str("Model".into()));
    d.get_mut_name(model, "Model");
    d.load_tree(&tree, model);
    Ok(model)
}

impl ApiState {
    pub fn finish_social(&mut self, kind: SocialKind, r: Option<J>) -> (Result<Vec<Wire>, String>, Option<String>) {
        match kind {
            SocialKind::Bool(field) => (Ok(vec![Wire::B(r.map(|j| j[field].as_bool().unwrap_or(false)).unwrap_or(false))]), None),
            SocialKind::Award => {
                let awarded = r.as_ref().map(|j| j["awarded"].as_bool().unwrap_or(false)).unwrap_or(false);
                let msg = r.as_ref().and_then(|j| if awarded { j["message"].as_str().map(|m| m.to_string()) } else { None });
                (Ok(vec![Wire::B(awarded)]), msg)
            }
            SocialKind::Mkt(..) | SocialKind::Pending(_) | SocialKind::Grant | SocialKind::Report => (Ok(vec![]), None),
            SocialKind::Info => match r.and_then(|j| if j["info"].is_null() { None } else { Some(j["info"].clone()) }) {
                Some(info) => (Ok(vec![json_wire(&info)]), None),
                None => (Err("Invalid asset id".into()), None),
            },
        }
    }

    pub fn sequence(&mut self, id: u64) -> Option<Rc<Seq>> {
        if let Some(s) = self.seqs.get(&id) {
            return s.clone();
        }
        let seq = read_asset(&self.assets_dir, id).and_then(|b| swirl_core::rbx::parse(&b).ok()).and_then(|t| find_sequence(&t.roots)).map(|mut s| {
            if id == 180426354 {
                s.looped = true;
            }
            Rc::new(s)
        });
        self.seqs.insert(id, seq.clone());
        seq
    }

    fn data_path(&self) -> String {
        format!("{}/{}.json", self.data_dir, self.place_id)
    }

    fn load_data(&mut self) {
        if self.data_loaded {
            return;
        }
        self.data_loaded = true;
        if let Ok(b) = std::fs::read(self.data_path()) {
            if let Ok(J::Object(o)) = serde_json::from_slice::<J>(&b) {
                for (store, v) in o {
                    if let J::Object(m) = v {
                        let mut h = HashMap::new();
                        for (k, val) in m {
                            if let Some(s) = val.as_str() {
                                h.insert(k, s.to_string());
                            }
                        }
                        self.data.insert(store, h);
                    }
                }
            }
        }
    }

    fn bag_path(&self, uid: i64) -> String {
        format!("{}/players/{}/{}.json", self.data_dir, self.place_id, uid)
    }

    pub fn bag(&mut self, uid: i64) -> &mut PlayerBag {
        if !self.players.contains_key(&uid) {
            let mut bag = PlayerBag::default();
            if let Ok(b) = std::fs::read(self.bag_path(uid)) {
                if let Ok(j) = serde_json::from_slice::<J>(&b) {
                    if let Some(o) = j["n"].as_object() {
                        for (k, v) in o {
                            if let Some(x) = v.as_f64() {
                                bag.n.insert(k.clone(), x);
                            }
                        }
                    }
                    if let Some(o) = j["s"].as_object() {
                        for (k, v) in o {
                            if let Some(x) = v.as_str() {
                                bag.s.insert(k.clone(), x.to_string());
                            }
                        }
                    }
                    if let Some(o) = j["b"].as_object() {
                        for (k, v) in o {
                            if let Some(x) = v.as_bool() {
                                bag.b.insert(k.clone(), x);
                            }
                        }
                    }
                    if let Some(o) = j["i"].as_object() {
                        for (k, v) in o {
                            if let Some(x) = v.as_str() {
                                bag.i.insert(k.clone(), x.to_string());
                            }
                        }
                    }
                }
            }
            self.players.insert(uid, bag);
        }
        self.players.get_mut(&uid).unwrap()
    }

    pub fn flush(&mut self) {
        if self.data_dirty {
            self.data_dirty = false;
            let mut o = serde_json::Map::new();
            for (store, m) in &self.data {
                let mut h = serde_json::Map::new();
                for (k, v) in m {
                    h.insert(k.clone(), J::String(v.clone()));
                }
                o.insert(store.clone(), J::Object(h));
            }
            let _ = std::fs::create_dir_all(&self.data_dir);
            let path = self.data_path();
            let tmp = format!("{}.{}.tmp", path, std::process::id());
            if std::fs::write(&tmp, J::Object(o).to_string()).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
        let dirty: Vec<i64> = self.players_dirty.drain().collect();
        for uid in dirty {
            if let Some(bag) = self.players.get(&uid) {
                let j = json!({"n": bag.n, "s": bag.s, "b": bag.b, "i": bag.i});
                let path = self.bag_path(uid);
                let _ = std::fs::create_dir_all(format!("{}/players/{}", self.data_dir, self.place_id));
                let tmp = format!("{}.{}.tmp", path, std::process::id());
                if std::fs::write(&tmp, j.to_string()).is_ok() {
                    let _ = std::fs::rename(&tmp, &path);
                }
            }
        }
    }

    pub fn step_tracks(&mut self, dt: f64, dm: &mut DataModel) {
        let ids: Vec<InstId> = self.tracks.keys().copied().collect();
        for id in ids {
            let gone = dm.get(id).map(|i| i.destroyed).unwrap_or(true);
            if gone {
                self.tracks.remove(&id);
                continue;
            }
            let mut ev: Vec<(String, Vec<Wire>)> = vec![];
            let mut stopped = false;
            {
                let tr = self.tracks.get_mut(&id).unwrap();
                if !tr.playing {
                    continue;
                }
                let rate = if tr.fade > 0.0 { dt / tr.fade } else { f64::INFINITY };
                tr.w = if tr.target > tr.w { (tr.w + rate).min(tr.target) } else { (tr.w - rate).max(tr.target) };
                if tr.target == 0.0 && tr.w <= 0.0 {
                    tr.playing = false;
                    stopped = true;
                } else if let Some(seq) = tr.seq.clone() {
                    let prev = tr.time;
                    let mut now = prev + dt * tr.speed;
                    let len = seq.length;
                    if now < 0.0 {
                        now = if seq.looped && len > 0.0 { ((now % len) + len) % len } else { 0.0 };
                    }
                    if tr.target > 0.0 {
                        if len > 0.0 && now >= len {
                            for (t, n) in &seq.frames {
                                if *t > prev && *t <= len {
                                    ev.push(("KeyframeReached".into(), vec![Wire::S(n.clone())]));
                                }
                            }
                            if seq.looped {
                                now %= len;
                                for (t, n) in &seq.frames {
                                    if *t <= now {
                                        ev.push(("KeyframeReached".into(), vec![Wire::S(n.clone())]));
                                    }
                                }
                            } else {
                                now = len;
                                tr.target = 0.0;
                                tr.fade = 0.0;
                                tr.w = 0.0;
                                tr.playing = false;
                                stopped = true;
                            }
                        } else {
                            for (t, n) in &seq.frames {
                                if *t > prev && *t <= now {
                                    ev.push(("KeyframeReached".into(), vec![Wire::S(n.clone())]));
                                }
                            }
                        }
                    }
                    tr.time = now;
                }
            }
            ev.retain(|(_, a)| !matches!(a.first(), Some(Wire::S(n)) if n == "Keyframe"));
            for (n, a) in ev {
                self.events.push((id, n, a));
            }
            if stopped {
                let _ = dm.set_prop(id, "IsPlaying", Val::Bool(false));
                self.events.push((id, "Stopped".into(), vec![]));
            }
        }
    }
}

pub struct Api(pub Shared, pub Rc<RefCell<swirl_physics::Physics>>);

fn num(a: &[Wire], i: usize) -> Option<f64> {
    match a.get(i) {
        Some(Wire::N(n)) if n.is_finite() => Some(*n),
        _ => None,
    }
}

fn text(a: &[Wire], i: usize) -> String {
    match a.get(i) {
        Some(Wire::S(s)) => s.clone(),
        Some(Wire::N(n)) => n.to_string(),
        _ => String::new(),
    }
}

impl ApiHandler for Api {
    fn call(&mut self, d: &mut DataModel, id: InstId, method: &str, args: &[Wire]) -> Option<Res> {
        if crate::terrain_api::METHODS.contains(&method) {
            return crate::terrain_api::call(&mut self.1.borrow_mut(), d, id, method, args);
        }
        if crate::extra_api::METHODS.contains(&method) {
            let r = crate::extra_api::call(&mut self.0.borrow_mut(), d, id, method, args);
            if r.is_some() {
                return r;
            }
        }
        if !METHODS.contains(&method) && !matches!(method, "Play" | "play" | "Stop" | "stop") {
            return None;
        }
        let mut st = self.0.borrow_mut();
        let class = d.get(id).map(|i| i.class.clone()).unwrap_or_default();
        match method {
            "__dataStore" => {
                let name = text(args, 0);
                let scope = text(args, 1);
                let ordered = matches!(args.get(2), Some(Wire::B(true)));
                let key = format!("{}|{}|{}", if ordered { "o" } else { "g" }, scope, name);
                if let Some(i) = st.store_keys.get(&key) {
                    if d.get(*i).map(|x| !x.destroyed).unwrap_or(false) {
                        return Some(Res::Ok(vec![Wire::I(*i)]));
                    }
                }
                let ds = d.create_any(if ordered { "OrderedDataStore" } else { "GlobalDataStore" }, None).ok()?;
                d.set_prop_quiet(ds, "Name", Val::Str(name));
                d.get_mut_name(ds, &text(args, 0));
                st.stores.insert(ds, key.clone());
                st.store_keys.insert(key, ds);
                Some(Res::Ok(vec![Wire::I(ds)]))
            }
            "__dsGet" => {
                st.load_data();
                let key = st.stores.get(&id)?.clone();
                let v = st.data.get(&key).and_then(|m| m.get(&text(args, 0))).cloned();
                Some(Res::Ok(vec![v.map(Wire::S).unwrap_or(Wire::Nil)]))
            }
            "__dsSet" => {
                st.load_data();
                let key = st.stores.get(&id)?.clone();
                let k = text(args, 0);
                match args.get(1) {
                    Some(Wire::S(v)) => {
                        st.data.entry(key).or_default().insert(k, v.clone());
                    }
                    _ => {
                        if let Some(m) = st.data.get_mut(&key) {
                            m.remove(&k);
                        }
                    }
                }
                st.data_dirty = true;
                Some(Res::Ok(vec![]))
            }
            "__dsSorted" => {
                st.load_data();
                let key = st.stores.get(&id)?.clone();
                let asc = matches!(args.first(), Some(Wire::B(true)));
                let size = num(args, 1).map(|n| n as i64).filter(|n| *n > 0).unwrap_or(50).clamp(1, 100) as usize;
                let mut rows: Vec<(String, f64)> = st.data.get(&key).map(|m| m.iter().map(|(k, v)| (k.clone(), serde_json::from_str::<f64>(v).unwrap_or(0.0))).collect()).unwrap_or_default();
                rows.sort_by(|a, b| {
                    let o = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
                    if asc { o } else { o.reverse() }
                });
                let list: Vec<Wire> = rows.into_iter().take(size).map(|(k, v)| Wire::Dict(vec![("key".into(), Wire::S(k)), ("value".into(), Wire::N(v))])).collect();
                Some(Res::Ok(vec![Wire::List(list)]))
            }
            "GetTranslatorForPlayer" => Some(Res::Ok(vec![Wire::Nil])),
            "__receiptResult" if class == "MarketplaceService" => {
                let tok = num(args, 0).map(|n| n as u32).unwrap_or(0);
                st.receipt_results.push((tok, text(args, 1)));
                Some(Res::Ok(vec![]))
            }
            "FindPartOnRay" | "findPartOnRay" | "FindPartOnRayWithIgnoreList" | "findPartOnRayWithIgnoreList" | "FindPartOnRayWithWhitelist" if class == "Workspace" => {
                let ray = match args.first() {
                    Some(Wire::List(l)) if l.len() == 6 => l.iter().map(|w| if let Wire::N(n) = w { *n } else { 0.0 }).collect::<Vec<f64>>(),
                    _ => return Some(Res::Ok(vec![Wire::Nil, Wire::V3([0.0; 3]), Wire::V3([0.0, 1.0, 0.0])])),
                };
                let o = [ray[0], ray[1], ray[2]];
                let dd = [ray[3], ray[4], ray[5]];
                let mut ignore: Vec<InstId> = vec![];
                match args.get(1) {
                    Some(Wire::I(i)) => ignore.push(*i),
                    Some(Wire::List(l)) => {
                        for w in l {
                            if let Wire::I(i) = w {
                                ignore.push(*i);
                            }
                        }
                    }
                    _ => {}
                }
                if method == "FindPartOnRayWithWhitelist" {
                    let mut allow: HashSet<InstId> = HashSet::new();
                    for x in ignore.drain(..) {
                        allow.insert(x);
                        let mut v = vec![];
                        d.descendants(x, &mut v);
                        allow.extend(v);
                    }
                    let ws = d.services.get("Workspace").copied()?;
                    let mut all = vec![];
                    d.descendants(ws, &mut all);
                    ignore = all.into_iter().filter(|p| d.is_a(*p, "BasePart") && !allow.contains(p)).collect();
                }
                let ph = self.1.borrow();
                let r = ph.raycast(d, o, dd, &ignore, None);
                Some(Res::Ok(vec![r.part.map(Wire::I).unwrap_or(Wire::Nil), Wire::V3(r.point), Wire::V3(r.normal)]))
            }
            "MoveTo" | "moveTo" | "TranslateBy" | "GetModelCFrame" | "getModelCFrame" | "GetModelSize" | "getModelSize" | "GetExtentsSize" | "GetPrimaryPartCFrame" | "SetPrimaryPartCFrame" | "GetPivot" | "PivotTo" if class == "Model" || (method == "PivotTo" && d.is_a(id, "BasePart")) => {
                use swirl_physics::cf::{self, CF};
                let parts_of = |d: &DataModel, m: InstId| -> Vec<InstId> {
                    let mut all = vec![];
                    d.descendants(m, &mut all);
                    all.into_iter().filter(|p| d.is_a(*p, "BasePart")).collect()
                };
                let cf_of = |d: &DataModel, p: InstId| -> CF {
                    match d.get_prop(p, "CFrame") {
                        Some(Val::Arr(a)) => cf::arr(a).unwrap_or(cf::IDENTITY),
                        _ => cf::IDENTITY,
                    }
                };
                let bounds = |d: &DataModel, parts: &[InstId]| -> ([f64; 3], [f64; 3]) {
                    let mut lo = [f64::INFINITY; 3];
                    let mut hi = [f64::NEG_INFINITY; 3];
                    for p in parts {
                        let c = cf_of(d, *p);
                        let s = match d.get_prop(*p, "Size") {
                            Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
                            _ => [1.0; 3],
                        };
                        let h = [s[0] / 2.0, s[1] / 2.0, s[2] / 2.0];
                        for k in 0..3 {
                            let e = c[3 + k * 3].abs() * h[0] + c[4 + k * 3].abs() * h[1] + c[5 + k * 3].abs() * h[2];
                            lo[k] = lo[k].min(c[k] - e);
                            hi[k] = hi[k].max(c[k] + e);
                        }
                    }
                    if parts.is_empty() { ([0.0; 3], [0.0; 3]) } else { (lo, hi) }
                };
                let primary = |d: &DataModel, m: InstId| -> Option<InstId> {
                    match d.get_prop(m, "PrimaryPart") {
                        Some(Val::Ref(Some(p))) if d.is_descendant_of(*p, m) => Some(*p),
                        _ => None,
                    }
                };
                let cfw = |w: Option<&Wire>| -> Option<CF> {
                    match w {
                        Some(Wire::Cf(c)) => Some(*c),
                        _ => None,
                    }
                };
                match method {
                    "MoveTo" | "moveTo" => {
                        let target = match args.first() {
                            Some(Wire::V3(v)) => *v,
                            _ => return Some(Res::Ok(vec![])),
                        };
                        let parts = parts_of(d, id);
                        let (lo, hi) = bounds(d, &parts);
                        let ctr = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
                        let size_y = parts.iter().map(|p| match d.get_prop(*p, "Size") { Some(Val::Arr(a)) if a.len() >= 3 => a[1], _ => 1.0 }).fold(1.0f64, f64::max);
                        let mut lo_y = f64::INFINITY;
                        let mut hi_y = f64::NEG_INFINITY;
                        for p in &parts {
                            let c = cf_of(d, *p);
                            let s = match d.get_prop(*p, "Size") { Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]], _ => [1.0; 3] };
                            let e = c[6].abs() * s[0] / 2.0 + c[7].abs() * s[1] / 2.0 + c[8].abs() * s[2] / 2.0;
                            lo_y = lo_y.min(c[1] - e);
                            hi_y = hi_y.max(c[1] + e);
                        }
                        let half = if lo_y.is_finite() { (hi_y - lo_y) / 2.0 } else { size_y / 2.0 };
                        let mut t = target;
                        let r = self.1.borrow().raycast(d, [t[0], t[1] + 200.0, t[2]], [0.0, -400.0, 0.0], &[id], None);
                        if r.part.is_some() && r.point[1] + half > t[1] {
                            t = [t[0], r.point[1] + half, t[2]];
                        }
                        let model_cf = match primary(d, id) {
                            Some(pp) => cf::with_pos(&cf_of(d, pp), ctr[0], ctr[1], ctr[2]),
                            None => [ctr[0], ctr[1], ctr[2], 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                        };
                        let delta = [t[0] - model_cf[0], t[1] - model_cf[1], t[2] - model_cf[2]];
                        for p in parts {
                            let mut c = cf_of(d, p);
                            c[0] += delta[0];
                            c[1] += delta[1];
                            c[2] += delta[2];
                            let _ = d.set_prop(p, "CFrame", Val::Arr(c.to_vec()));
                        }
                        Some(Res::Ok(vec![]))
                    }
                    "TranslateBy" => {
                        let delta = match args.first() { Some(Wire::V3(v)) => *v, _ => return Some(Res::Ok(vec![])) };
                        for p in parts_of(d, id) {
                            let mut c = cf_of(d, p);
                            c[0] += delta[0];
                            c[1] += delta[1];
                            c[2] += delta[2];
                            let _ = d.set_prop(p, "CFrame", Val::Arr(c.to_vec()));
                        }
                        Some(Res::Ok(vec![]))
                    }
                    "GetModelCFrame" | "getModelCFrame" => {
                        let parts = parts_of(d, id);
                        let (lo, hi) = bounds(d, &parts);
                        let ctr = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
                        let c = match primary(d, id) {
                            Some(pp) => cf::with_pos(&cf_of(d, pp), ctr[0], ctr[1], ctr[2]),
                            None => [ctr[0], ctr[1], ctr[2], 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                        };
                        Some(Res::Ok(vec![Wire::Cf(c)]))
                    }
                    "GetModelSize" | "getModelSize" | "GetExtentsSize" => {
                        let parts = parts_of(d, id);
                        let (lo, hi) = bounds(d, &parts);
                        Some(Res::Ok(vec![Wire::V3([hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]])]))
                    }
                    "GetPrimaryPartCFrame" => Some(Res::Ok(vec![match d.get_prop(id, "PrimaryPart") {
                        Some(Val::Ref(Some(pp))) => Wire::Cf(cf_of(d, *pp)),
                        _ => Wire::Nil,
                    }])),
                    "SetPrimaryPartCFrame" => {
                        let target = match cfw(args.first()) { Some(c) => c, None => return Some(Res::Ok(vec![])) };
                        let pp = match d.get_prop(id, "PrimaryPart") {
                            Some(Val::Ref(Some(p))) => *p,
                            _ => return Some(Res::Err("Model:SetPrimaryPartCFrame() failed because no PrimaryPart has been set".into())),
                        };
                        let rel = cf::mul(&target, &cf::inv(&cf_of(d, pp)));
                        for p in parts_of(d, id) {
                            let nc = cf::orthonormalize(&cf::mul(&rel, &cf_of(d, p)));
                            let _ = d.set_prop(p, "CFrame", Val::Arr(nc.to_vec()));
                        }
                        Some(Res::Ok(vec![]))
                    }
                    "GetPivot" => {
                        if let Some(pp) = primary(d, id) {
                            return Some(Res::Ok(vec![Wire::Cf(cf_of(d, pp))]));
                        }
                        let parts = parts_of(d, id);
                        if parts.is_empty() {
                            return Some(Res::Ok(vec![Wire::Cf(cf::IDENTITY)]));
                        }
                        let (lo, hi) = bounds(d, &parts);
                        Some(Res::Ok(vec![Wire::Cf([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])]))
                    }
                    _ => {
                        let target = match cfw(args.first()) { Some(c) if cf::is_finite(&c) => c, _ => return Some(Res::Ok(vec![])) };
                        if class != "Model" {
                            let _ = d.set_prop(id, "CFrame", Val::Arr(cf::orthonormalize(&target).to_vec()));
                            return Some(Res::Ok(vec![]));
                        }
                        let pivot = match primary(d, id) {
                            Some(pp) => cf_of(d, pp),
                            None => {
                                let parts = parts_of(d, id);
                                let (lo, hi) = bounds(d, &parts);
                                [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
                            }
                        };
                        let rel = cf::mul(&target, &cf::inv(&pivot));
                        for p in parts_of(d, id) {
                            let nc = cf::orthonormalize(&cf::mul(&rel, &cf_of(d, p)));
                            let _ = d.set_prop(p, "CFrame", Val::Arr(nc.to_vec()));
                        }
                        Some(Res::Ok(vec![]))
                    }
                }
            }
            "GetMass" | "getMass" if d.is_a(id, "BasePart") => {
                let s = match d.get_prop(id, "Size") {
                    Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
                    _ => [1.0; 3],
                };
                Some(Res::Ok(vec![Wire::N(s[0] * s[1] * s[2] * 0.7)]))
            }
            "GetConnectedParts" if d.is_a(id, "BasePart") => {
                let l = self.1.borrow().connected_parts(id);
                Some(Res::Ok(vec![Wire::List(l.into_iter().map(Wire::I).collect())]))
            }
            "GetRootPart" if d.is_a(id, "BasePart") => Some(Res::Ok(vec![Wire::I(self.1.borrow().root_part(id))])),
            "GetTouchingParts" if d.is_a(id, "BasePart") => {
                let l = self.1.borrow().touching_parts(id);
                Some(Res::Ok(vec![Wire::List(l.into_iter().map(Wire::I).collect())]))
            }
            "BreakJoints" | "breakJoints" | "MakeJoints" | "makeJoints" if d.is_a(id, "BasePart") || class == "Model" => {
                let parts: Vec<InstId> = if class == "Model" {
                    let mut all = vec![];
                    d.descendants(id, &mut all);
                    all.into_iter().filter(|p| d.is_a(*p, "BasePart")).collect()
                } else {
                    vec![id]
                };
                let mut ph = self.1.borrow_mut();
                if method.starts_with('B') || method.starts_with('b') {
                    for p in parts {
                        ph.break_joints(d, p);
                    }
                } else {
                    ph.make_joints(d, &parts);
                }
                Some(Res::Ok(vec![]))
            }
            "__ComputePath" if class == "PathfindingService" => {
                let v3 = |w: Option<&Wire>| match w {
                    Some(Wire::V3(v)) => Some(*v),
                    _ => None,
                };
                let (start, goal) = match (v3(args.first()), v3(args.get(1))) {
                    (Some(a), Some(b)) => (a, b),
                    _ => return Some(Res::Ok(vec![Wire::Dict(vec![("status".into(), Wire::N(5.0)), ("points".into(), Wire::List(vec![])), ("jumps".into(), Wire::List(vec![]))])])),
                };
                let radius = num(args, 2).filter(|r| *r != 0.0).unwrap_or(2.0);
                let height = num(args, 3).filter(|r| *r != 0.0).unwrap_or(5.0);
                let can_jump = !matches!(args.get(4), Some(Wire::B(false)));
                let key = [start[0] / 4.0, start[2] / 4.0, goal[0] / 4.0, goal[2] / 4.0].iter().map(|v| format!("{}", (v + 0.5).floor() as i64)).collect::<Vec<_>>().join(",");
                let now = st.now_ms;
                let cached = st.path_cache.get(&key).filter(|(t, _)| now - *t < 400.0).map(|(_, r)| (r.0, r.1.clone()));
                let (status, pts) = match cached {
                    Some((s, mut p)) => {
                        if let Some(f) = p.first_mut() {
                            *f = (start, false);
                        }
                        (s, p)
                    }
                    None => {
                        let ph = self.1.borrow();
                        let solid = |p: InstId| -> bool {
                            if matches!(d.get_prop(p, "CanCollide"), Some(Val::Bool(false))) {
                                return false;
                            }
                            let mut x = d.get(p).and_then(|i| i.parent);
                            let mut n = 0;
                            while let Some(xi) = x {
                                if n >= 3 {
                                    break;
                                }
                                if d.get(xi).map(|i| i.class == "Model").unwrap_or(false) && d.find_child_of_class(xi, "Humanoid").is_some() {
                                    return false;
                                }
                                x = d.get(xi).and_then(|i| i.parent);
                                n += 1;
                            }
                            true
                        };
                        let cast = |o: [f64; 3], dir: [f64; 3]| -> swirl_physics::path::CastHit {
                            let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
                            let r = ph.raycast(d, o, dir, &[], Some(&solid));
                            let miss = r.part.is_none() && ((r.point[0] - o[0]).powi(2) + (r.point[1] - o[1]).powi(2) + (r.point[2] - o[2]).powi(2)).sqrt() >= len - 1e-3;
                            swirl_physics::path::CastHit { hit: !miss, point: r.point, normal: r.normal }
                        };
                        let res = swirl_physics::path::find_path(&cast, start, goal, radius, height, can_jump);
                        let pts: Vec<([f64; 3], bool)> = res.points.iter().map(|w| (w.pos, w.jump)).collect();
                        if st.path_cache.len() > 64 {
                            st.path_cache.clear();
                        }
                        st.path_cache.insert(key, (now, (res.status, pts.clone())));
                        (res.status, pts)
                    }
                };
                let points: Vec<Wire> = pts.iter().map(|(p, _)| Wire::V3(*p)).collect();
                let jumps: Vec<Wire> = pts.iter().map(|(_, j)| Wire::B(*j)).collect();
                Some(Res::Ok(vec![Wire::Dict(vec![("status".into(), Wire::N(status as f64)), ("points".into(), Wire::List(points)), ("jumps".into(), Wire::List(jumps))])]))
            }
            "LoadAsset" | "loadAsset" if class == "InsertService" => {
                let id = num(args, 0).map(|n| n as u64).unwrap_or(0);
                if id == 0 {
                    return Some(Res::Err("InsertService is not available".into()));
                }
                match read_asset(&st.assets_dir, id) {
                    Some(b) => match make_model(d, &b) {
                        Ok(m) => Some(Res::Ok(vec![Wire::I(m)])),
                        Err(e) => Some(Res::Err(e)),
                    },
                    None => {
                        st.token += 1;
                        let token = st.token;
                        st.assets_wanted.push((token, id));
                        Some(Res::Async(token))
                    }
                }
            }
            "Insert" if class == "InsertService" => {
                if let (Some(Wire::I(m)), Some(ws)) = (args.first(), d.services.get("Workspace").copied()) {
                    let _ = d.set_parent(*m, Some(ws));
                }
                Some(Res::Ok(vec![]))
            }
            "FindFirstAncestor" | "FindFirstAncestorOfClass" => {
                let want = text(args, 0);
                let mut cur = d.get(id).and_then(|i| i.parent);
                while let Some(p) = cur {
                    let i = d.get(p)?;
                    if (method == "FindFirstAncestor" && i.name == want) || (method != "FindFirstAncestor" && i.class == want) {
                        return Some(Res::Ok(vec![Wire::I(p)]));
                    }
                    cur = i.parent;
                }
                Some(Res::Ok(vec![Wire::Nil]))
            }
            "GetDebugId" => Some(Res::Ok(vec![Wire::S(format!("{}", id))])),
            "GetServerTimeNow" => Some(Res::Ok(vec![Wire::N(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0))])),
            "AddTag" | "RemoveTag" | "HasTag" | "GetTags" => {
                let tag = text(args, 0);
                let ok = d.get(id).is_some();
                if !ok {
                    return Some(Res::Err("Instance expected".into()));
                }
                let target = match args.first() {
                    Some(Wire::I(t)) if class == "CollectionService" => *t,
                    _ => id,
                };
                let tag = if class == "CollectionService" { text(args, 1) } else { tag };
                let _ = &tag;
                let inst = d.get_mut_inst(target)?;
                match method {
                    "AddTag" => {
                        if !inst.tags.contains(&tag) {
                            inst.tags.push(tag);
                        }
                        Some(Res::Ok(vec![]))
                    }
                    "RemoveTag" => {
                        inst.tags.retain(|t| *t != tag);
                        Some(Res::Ok(vec![]))
                    }
                    "HasTag" => Some(Res::Ok(vec![Wire::B(inst.tags.contains(&tag))])),
                    _ => Some(Res::Ok(vec![Wire::List(inst.tags.iter().map(|t| Wire::S(t.clone())).collect())])),
                }
            }
            "GetTagged" if class == "CollectionService" => {
                let tag = text(args, 0);
                let mut all = vec![];
                let root = d.root;
                d.descendants(root, &mut all);
                let list: Vec<Wire> = all.into_iter().filter(|i| d.get(*i).map(|x| x.tags.contains(&tag)).unwrap_or(false)).map(Wire::I).collect();
                Some(Res::Ok(vec![Wire::List(list)]))
            }
            "GetTeams" if class == "Teams" => {
                let kids: Vec<Wire> = d.get(id)?.children.iter().copied().filter(|c| d.get(*c).map(|x| x.class == "Team").unwrap_or(false)).map(Wire::I).collect();
                Some(Res::Ok(vec![Wire::List(kids)]))
            }
            "GetMinutesAfterMidnight" | "getMinutesAfterMidnight" if class == "Lighting" => {
                let tod = match d.get_prop(id, "TimeOfDay") {
                    Some(Val::Str(s)) => s.clone(),
                    _ => "14:00:00".into(),
                };
                let p: Vec<f64> = tod.split(':').map(|x| x.parse().unwrap_or(0.0)).collect();
                Some(Res::Ok(vec![Wire::N(p.first().copied().unwrap_or(0.0) * 60.0 + p.get(1).copied().unwrap_or(0.0) + p.get(2).copied().unwrap_or(0.0) / 60.0)]))
            }
            "SetMinutesAfterMidnight" | "setMinutesAfterMidnight" if class == "Lighting" => {
                let m = num(args, 0).unwrap_or(0.0).rem_euclid(1440.0);
                let h = (m / 60.0).floor();
                let mm = (m % 60.0).floor();
                let ss = ((m % 1.0) * 60.0).floor();
                let _ = d.set_prop(id, "TimeOfDay", Val::Str(format!("{:02}:{:02}:{:02}", h as i64, mm as i64, ss as i64)));
                Some(Res::Ok(vec![]))
            }
            "AwardBadge" | "UserHasBadge" if class == "BadgeService" => {
                let uid = num(args, 0).map(|n| n as i64).unwrap_or(0);
                let bid = num(args, 1).map(|n| n as i64).unwrap_or(0);
                if uid <= 0 || bid <= 0 {
                    return Some(Res::Ok(vec![Wire::B(false)]));
                }
                st.token += 1;
                let token = st.token;
                let place = st.place_id;
                if method == "AwardBadge" {
                    st.social.push(SocialReq { token, body: json!({"action": "award", "userId": uid, "badgeId": bid, "placeId": place}), kind: SocialKind::Award });
                } else {
                    st.social.push(SocialReq { token, body: json!({"action": "hasBadge", "userId": uid, "badgeId": bid}), kind: SocialKind::Bool("has") });
                }
                Some(Res::Async(token))
            }
            "PlayerOwnsAsset" | "UserOwnsGamePassAsync" | "PlayerHasPass" if class == "MarketplaceService" || class == "GamePassService" => {
                let uid = user_id_of(d, args.first());
                let id = num(args, 1).map(|n| n as i64).unwrap_or(0);
                if uid <= 0 || id <= 0 {
                    return Some(Res::Ok(vec![Wire::B(false)]));
                }
                st.token += 1;
                let token = st.token;
                st.social.push(SocialReq { token, body: json!({"action": "ownsAsset", "userId": uid, "assetId": id}), kind: SocialKind::Bool("owns") });
                Some(Res::Async(token))
            }
            "GetProductInfo" if class == "MarketplaceService" => {
                let id = num(args, 0).map(|n| n as i64).unwrap_or(0);
                let kind = match args.get(1) {
                    Some(Wire::E(_, v)) if *v == 1.0 => "product",
                    Some(Wire::E(_, v)) if *v == 2.0 => "gamepass",
                    Some(Wire::N(v)) if *v == 1.0 => "product",
                    Some(Wire::N(v)) if *v == 2.0 => "gamepass",
                    _ => "asset",
                };
                if id <= 0 {
                    return Some(Res::Err(format!("Invalid {} id", if kind == "product" { "product" } else { "asset" })));
                }
                st.token += 1;
                let token = st.token;
                st.social.push(SocialReq { token, body: json!({"action": "productInfo", "kind": kind, "id": id}), kind: SocialKind::Info });
                Some(Res::Async(token))
            }
            "PlayerCanMakePurchases" if class == "MarketplaceService" => Some(Res::Ok(vec![Wire::B(user_id_of(d, args.first()) > 0)])),
            "PromptPurchase" | "PromptProductPurchase" | "PromptGamePassPurchase" if class == "MarketplaceService" => {
                let pl = match args.first() {
                    Some(Wire::I(p)) if d.get(*p).map(|i| i.class == "Player").unwrap_or(false) => *p,
                    _ => return Some(Res::Err("Argument 1 missing or nil".into())),
                };
                let id = num(args, 1).map(|n| n as i64).unwrap_or(0);
                if id > 0 {
                    let kind = match method {
                        "PromptPurchase" => "asset",
                        "PromptProductPurchase" => "product",
                        _ => "gamepass",
                    };
                    let equip = matches!(args.get(2), Some(Wire::B(true))) && method != "PromptGamePassPurchase";
                    st.directed.push((pl, json!({"t": "prompt", "kind": kind, "id": id, "equip": equip})));
                }
                Some(Res::Ok(vec![]))
            }
            "Teleport" | "TeleportToSpawnByName" | "TeleportToPlaceInstance" if class == "TeleportService" => {
                if let (Some(place), Some(Wire::I(pl))) = (num(args, 0), args.get(1)) {
                    if place > 0.0 {
                        st.directed.push((*pl, json!({"t": "teleport", "place": place as i64})));
                    }
                }
                Some(Res::Ok(vec![]))
            }
            "WaitForDataReady" | "waitForDataReady" => Some(Res::Ok(vec![Wire::B(true)])),
            "SaveNumber" | "SaveString" | "SaveBoolean" | "LoadNumber" | "LoadString" | "LoadBoolean" | "SaveInstance" | "LoadInstance" if class == "Player" => {
                let uid = match d.get_prop(id, "userId") {
                    Some(Val::Num(n)) => *n as i64,
                    _ => 0,
                };
                if uid <= 0 {
                    return Some(Res::Ok(vec![]));
                }
                let k = text(args, 0);
                let bag = st.bag(uid);
                match method {
                    "SaveNumber" => {
                        bag.n.insert(k, num(args, 1).unwrap_or(0.0));
                    }
                    "SaveString" => {
                        bag.s.insert(k, text(args, 1));
                    }
                    "SaveBoolean" => {
                        bag.b.insert(k, matches!(args.get(1), Some(Wire::B(true))));
                    }
                    "LoadNumber" => return Some(Res::Ok(vec![Wire::N(bag.n.get(&k).copied().unwrap_or(0.0))])),
                    "LoadString" => return Some(Res::Ok(vec![Wire::S(bag.s.get(&k).cloned().unwrap_or_default())])),
                    "LoadBoolean" => return Some(Res::Ok(vec![Wire::B(bag.b.get(&k).copied().unwrap_or(false))])),
                    "SaveInstance" => {
                        return Some(Res::Ok(vec![]));
                    }
                    _ => return Some(Res::Ok(vec![Wire::Nil])),
                }
                st.players_dirty.insert(uid);
                Some(Res::Ok(vec![]))
            }
            "LoadAnimation" | "loadAnimation" if class == "Humanoid" || class == "Animator" => {
                let anim = match args.first() {
                    Some(Wire::I(a)) if d.get(*a).map(|i| i.class == "Animation").unwrap_or(false) => *a,
                    _ => return Some(Res::Err("LoadAnimation requires an Animation object".into())),
                };
                let hum = if class == "Humanoid" { Some(id) } else { d.get(id).and_then(|i| i.parent).filter(|p| d.get(*p).map(|x| x.class == "Humanoid").unwrap_or(false)) };
                let tr = d.create_any("AnimationTrack", None).ok()?;
                let name = d.get(anim).map(|a| a.name.clone()).unwrap_or_default();
                d.set_prop_quiet(tr, "Name", Val::Str(name.clone()));
                d.get_mut_name(tr, &name);
                let _ = d.set_prop(tr, "Animation", Val::Ref(Some(anim)));
                st.tracks.insert(tr, Track { hum, key: format!("s{}", tr), anim_id: None, seq: None, target: 0.0, w: 0.0, weight: 1.0, speed: 1.0, fade: 0.1, time: 0.0, playing: false });
                Some(Res::Ok(vec![Wire::I(tr)]))
            }
            "GetPlayingAnimationTracks" => {
                let hum = if class == "Humanoid" { id } else { d.get(id).and_then(|i| i.parent)? };
                let list: Vec<Wire> = st.tracks.iter().filter(|(_, t)| t.hum == Some(hum) && t.playing && t.target > 0.0).map(|(i, _)| Wire::I(*i)).collect();
                Some(Res::Ok(vec![Wire::List(list)]))
            }
            "Play" | "play" | "Stop" | "stop" | "AdjustSpeed" | "AdjustWeight" if class == "AnimationTrack" => {
                let playing_op = matches!(method, "Play" | "play");
                let anim = match d.get_prop(id, "Animation") {
                    Some(Val::Ref(Some(a))) => Some(*a),
                    _ => None,
                };
                let aid = anim.and_then(|a| match d.get_prop(a, "AnimationId") {
                    Some(Val::Str(s)) => anim_asset_id(s),
                    _ => None,
                });
                let seq = if playing_op { aid.and_then(|a| st.sequence(a)) } else { None };
                let (op, a, b, c, key, hum_id) = {
                    let tr = st.tracks.get_mut(&id)?;
                    match method {
                        "Play" | "play" => {
                            tr.anim_id = aid;
                            tr.seq = seq.clone();
                            tr.target = 1.0;
                            tr.fade = num(args, 0).unwrap_or(0.1).max(0.0);
                            tr.weight = num(args, 1).unwrap_or(1.0).max(0.0);
                            tr.speed = num(args, 2).unwrap_or(1.0);
                            tr.time = 0.0;
                            if tr.fade == 0.0 {
                                tr.w = 1.0;
                            }
                            tr.playing = true;
                            ("play", Some(tr.fade), Some(tr.weight), Some(tr.speed), tr.key.clone(), tr.hum)
                        }
                        "Stop" | "stop" => {
                            tr.target = 0.0;
                            tr.fade = num(args, 0).unwrap_or(0.1).max(0.0);
                            if tr.fade == 0.0 {
                                tr.w = 0.0;
                            }
                            ("stop", Some(tr.fade), None, None, tr.key.clone(), tr.hum)
                        }
                        "AdjustSpeed" => {
                            tr.speed = num(args, 0).unwrap_or(1.0);
                            ("speed", Some(tr.speed), None, None, tr.key.clone(), tr.hum)
                        }
                        _ => {
                            tr.weight = num(args, 0).unwrap_or(1.0).max(0.0);
                            tr.fade = num(args, 1).unwrap_or(0.1).max(0.0);
                            ("weight", Some(tr.weight), Some(tr.fade), None, tr.key.clone(), tr.hum)
                        }
                    }
                };
                if op == "play" {
                    let _ = d.set_prop(id, "IsPlaying", Val::Bool(true));
                    if let Some(s) = &seq {
                        let _ = d.set_prop(id, "Length", Val::Num(s.length));
                        let _ = d.set_prop(id, "Looped", Val::Bool(s.looped));
                    }
                }
                if let Some(h) = hum_id {
                    st.broadcasts.push(json!({"t": "anim", "h": h, "k": key, "op": op, "id": aid.unwrap_or(0), "a": a, "b": b, "c": c}));
                }
                Some(Res::Ok(vec![]))
            }
            _ => None,
        }
    }

    fn get(&mut self, _d: &DataModel, id: InstId, key: &str) -> Option<Wire> {
        let st = self.0.borrow();
        let tr = st.tracks.get(&id)?;
        match key {
            "TimePosition" => Some(Wire::N(tr.time)),
            "Speed" => Some(Wire::N(tr.speed)),
            "WeightCurrent" => Some(Wire::N(tr.w * tr.weight)),
            "WeightTarget" => Some(Wire::N(tr.target * tr.weight)),
            _ => None,
        }
    }
}

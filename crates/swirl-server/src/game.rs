use crate::character::{self, Char};
use crate::codec;
use std::collections::{HashMap, HashSet};
use serde_json::{json, Value as J};
use std::cell::RefCell;
use std::rc::Rc;
use swirl_core::classes;
use swirl_core::instance::{Change, DataModel, InstId, Val};
use swirl_lua::host::Wire;
use swirl_lua::{Host, LogLine, Req};
use crate::touch::{overlap, Box3};

pub struct Game {
    pub place_id: u64,
    pub era: i32,
    pub dm: Rc<RefCell<DataModel>>,
    pub roots: Vec<InstId>,
    pub host: Host,
    next_user: i64,
    pub t: f64,
    pub chars: HashMap<InstId, Char>,
    pending: HashMap<InstId, Vec<J>>,
    dirty: HashSet<InstId>,
    touching: HashSet<(InstId, InstId)>,
    dead: HashMap<InstId, f64>,
    debris: Vec<(f64, InstId)>,
    spawn_cache: Vec<InstId>,
    spawn_refresh: f64,
}

impl Game {
    pub fn load(place_id: u64, bytes: &[u8], era: i32) -> Result<Game, String> {
        let mut tree = swirl_core::rbx::parse_limit(bytes, 256 * 1024 * 1024)?;
        let mut dm = DataModel::new("server");
        dm.era = era;
        for s in ["Workspace", "Players", "Lighting", "StarterGui", "StarterPack", "Teams", "SoundService", "Debris", "ReplicatedStorage", "ServerStorage", "ServerScriptService", "InsertService", "RunService", "BadgeService", "Chat"] {
            dm.service(s)?;
        }
        for n in tree.roots.iter_mut() {
            let wraps: Vec<usize> = n.children.iter().enumerate().filter(|(_, c)| c.cls == "Model" && c.name == n.cls).map(|(i, _)| i).collect();
            if wraps.len() == 1 {
                let i = wraps[0];
                let w = n.children.remove(i);
                for (k, c) in w.children.into_iter().enumerate() {
                    n.children.insert(i + k, c);
                }
            }
        }
        let allowed = ["Workspace", "Lighting", "StarterGui", "StarterPack", "Teams", "SoundService", "ReplicatedStorage", "ServerStorage", "ServerScriptService", "Players", "Debris", "StarterPlayer", "ReplicatedFirst", "Chat"];
        tree.roots.retain(|n| dm.services.contains_key(&n.cls) || allowed.contains(&n.cls.as_str()));
        let root = dm.root;
        dm.load_tree(&tree, root);
        dm.set_prop(root, "PlaceId", Val::Num(place_id as f64)).ok();
        dm.set_prop(root, "Name", Val::Str("Game".into())).ok();
        let ws = dm.service("Workspace")?;
        let cam = match dm.find_child_of_class(ws, "Camera") {
            Some(c) => c,
            None => dm.create_any("Camera", Some(ws))?,
        };
        dm.set_prop(ws, "CurrentCamera", Val::Ref(Some(cam))).ok();
        if dm.find_child_of_class(ws, "Terrain").is_none() {
            let t = dm.create_any("Terrain", Some(ws))?;
            dm.set_prop(t, "Anchored", Val::Bool(true)).ok();
            dm.set_prop(t, "Locked", Val::Bool(true)).ok();
        }
        let mut roots = vec![];
        for n in classes::table().replicated_services.clone() {
            if let Ok(id) = dm.service(&n) {
                roots.push(id);
            }
        }
        dm.service("NetworkServer").ok();
        dm.journal.clear();
        let dm = Rc::new(RefCell::new(dm));
        let mut host = Host::new(dm.clone()).map_err(|e| e.to_string())?;
        host.scan_scripts();
        Ok(Game { place_id, era, dm, roots, host, next_user: 1, t: 0.0, chars: HashMap::new(), pending: HashMap::new(), dirty: HashSet::new(), touching: HashSet::new(), dead: HashMap::new(), debris: vec![], spawn_cache: vec![], spawn_refresh: 0.0 })
    }

    pub fn join(&mut self, name: &str) -> (J, InstId) {
        let uid = self.next_user;
        self.next_user += 1;
        let (player, snap) = {
            let mut dm = self.dm.borrow_mut();
            let players = dm.service("Players").unwrap();
            let player = dm.create_any("Player", None).unwrap();
            let _ = dm.set_prop(player, "Name", Val::Str(name.to_string()));
            let _ = dm.set_prop(player, "userId", Val::Num(uid as f64));
            let _ = dm.set_prop(player, "UserId", Val::Num(uid as f64));
            let _ = dm.set_prop(player, "CharacterAppearance", Val::Str(format!("/api/charapp/{}", uid)));
            let _ = dm.set_prop(player, "AccountAge", Val::Num(100.0));
            let _ = dm.set_prop(player, "DataReady", Val::Bool(false));
            let _ = dm.set_prop(player, "DataComplexityLimit", Val::Num(45000.0));
            for (cls, nm) in [("Backpack", "Backpack"), ("PlayerGui", "PlayerGui")] {
                if let Ok(c) = dm.create_any(cls, Some(player)) {
                    let _ = dm.set_prop(c, "Name", Val::Str(nm.to_string()));
                }
            }
            character::assign_team(&mut dm, player);
            dm.set_parent(player, Some(players)).unwrap();
            let snap = codec::snapshot_for(&dm, &self.roots, player);
            dm.journal.clear();
            (player, snap)
        };
        let players = self.dm.borrow().services["Players"];
        self.host.fire(players, "PlayerAdded", vec![swirl_lua::host::Wire::I(player)]);
        self.spawn_character(player, name);
        let msg = json!({
            "t": "init", "place": self.place_id, "era": self.era, "you": player,
            "user": { "id": uid, "name": name }, "snap": snap, "asm": [], "anims": [], "terrain": J::Null
        });
        (msg, player)
    }

    pub fn spawn_character(&mut self, player: InstId, name: &str) {
        let mut dm = self.dm.borrow_mut();
        let ws = dm.services["Workspace"];
        let origin = character::spawn_origin(&dm, ws, player);
        match character::build(&mut dm, ws, player, name, origin) {
            Ok(ch) => {
                let own = json!({"t": "own", "id": ch.model, "s": ch.seq, "c": ch.cf});
                self.pending.entry(player).or_default().push(own);
                self.chars.insert(ch.model, ch);
            }
            Err(e) => eprintln!("character spawn failed: {}", e),
        }
    }

    pub fn leave(&mut self, player: InstId) {
        let models: Vec<InstId> = self.chars.iter().filter(|(_, c)| c.player == player).map(|(m, _)| *m).collect();
        let mut dm = self.dm.borrow_mut();
        for m in models {
            dm.destroy(m);
            self.chars.remove(&m);
        }
        dm.destroy(player);
        self.pending.remove(&player);
    }

    pub fn message(&mut self, player: InstId, text: &str) {
        let v: J = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return,
        };
        if v["t"] == "char" {
            let arr = |k: &str, n: usize| -> Option<Vec<f64>> {
                let a = v[k].as_array()?;
                if a.len() < n {
                    return None;
                }
                let o: Vec<f64> = a.iter().take(n).map(|x| x.as_f64().unwrap_or(f64::NAN)).collect();
                if o.iter().all(|x| x.is_finite()) {
                    Some(o)
                } else {
                    None
                }
            };
            let (cf, vel) = match (arr("c", 12), arr("v", 3)) {
                (Some(c), Some(vv)) => (c, vv),
                _ => return,
            };
            let seq = v["s"].as_i64().unwrap_or(0);
            let state = v["st"].as_u64().unwrap_or(7) as u8;
            let model = match self.chars.iter().find(|(_, c)| c.player == player) {
                Some((m, _)) => *m,
                None => return,
            };
            let mut dm = self.dm.borrow_mut();
            let ch = self.chars.get_mut(&model).unwrap();
            if seq < ch.seq {
                return;
            }
            let mut c12 = [0.0; 12];
            c12.copy_from_slice(&cf);
            character::apply_cf(&mut dm, ch, c12);
            ch.vel = [vel[0], vel[1], vel[2]];
            ch.state = state;
            self.dirty.insert(model);
        }
    }

    fn box_of(dm: &DataModel, id: InstId) -> Option<Box3> {
        let i = dm.get(id)?;
        let cf = match i.props.get("CFrame") {
            Some(Val::Arr(a)) if a.len() >= 12 => a.clone(),
            _ => return None,
        };
        let sz = match i.props.get("Size") {
            Some(Val::Arr(a)) if a.len() >= 3 => a.clone(),
            _ => return None,
        };
        Some(Box3 { c: [cf[0], cf[1], cf[2]], r: [cf[3], cf[4], cf[5], cf[6], cf[7], cf[8], cf[9], cf[10], cf[11]], h: [sz[0] / 2.0, sz[1] / 2.0, sz[2] / 2.0] })
    }

    fn respawn(&mut self, player: InstId) {
        let old: Vec<InstId> = self.chars.iter().filter(|(_, c)| c.player == player).map(|(m, _)| *m).collect();
        let name = {
            let mut dm = self.dm.borrow_mut();
            for m in old {
                dm.destroy(m);
                self.chars.remove(&m);
                self.dead.remove(&m);
            }
            dm.get(player).map(|i| i.name.clone()).unwrap_or_default()
        };
        self.spawn_character(player, &name);
    }

    pub fn tick(&mut self, dt: f64) -> Vec<LogLine> {
        self.t += dt;
        self.host.tick(self.t);
        for r in self.host.take_requests() {
            match r {
                Req::LoadCharacter(p) => self.respawn(p),
                Req::Debris(i, s) => self.debris.push((self.t + s, i)),
            }
        }
        let due: Vec<InstId> = self.debris.iter().filter(|(t, _)| *t <= self.t).map(|(_, i)| *i).collect();
        if !due.is_empty() {
            self.debris.retain(|(t, _)| *t > self.t);
            let mut dm = self.dm.borrow_mut();
            for i in due {
                dm.destroy(i);
            }
        }
        let mut died: Vec<(InstId, InstId)> = vec![];
        let mut respawn: Vec<InstId> = vec![];
        {
            let dm = self.dm.borrow();
            for (m, c) in self.chars.iter() {
                if self.dead.contains_key(m) {
                    if self.dead[m] <= self.t {
                        respawn.push(c.player);
                    }
                    continue;
                }
                if let Some(h) = dm.get(*m).and_then(|mi| mi.children.iter().copied().find(|k| dm.get(*k).map(|i| i.class == "Humanoid").unwrap_or(false))) {
                    if matches!(dm.get_prop(h, "Health"), Some(Val::Num(x)) if *x <= 0.0) {
                        died.push((*m, h));
                    }
                }
            }
        }
        for (m, h) in died {
            self.dead.insert(m, self.t + 5.0);
            self.host.fire(h, "Died", vec![]);
        }
        for p in respawn {
            self.respawn(p);
        }
        for name in ["Stepped", "Heartbeat"] {
            for id in self.host.subscribed(name) {
                let args = if name == "Stepped" { vec![Wire::N(self.t), Wire::N(dt)] } else { vec![Wire::N(dt)] };
                self.host.fire(id, name, args);
            }
        }
        if self.spawn_refresh <= self.t {
            self.spawn_refresh = self.t + 2.0;
            let dm = self.dm.borrow();
            let ws = dm.services["Workspace"];
            let mut all = vec![];
            dm.descendants(ws, &mut all);
            self.spawn_cache = all
                .into_iter()
                .filter(|id| dm.get(*id).map(|i| i.class == "SpawnLocation").unwrap_or(false) && matches!(dm.get_prop(*id, "AllowTeamChangeOnTouch"), Some(Val::Bool(true))) && !matches!(dm.get_prop(*id, "Neutral"), Some(Val::Bool(true))))
                .collect();
        }
        let touch_parts: Vec<InstId> = {
            let mut v = self.host.subscribed("Touched");
            v.extend(self.spawn_cache.iter().copied());
            v.sort();
            v.dedup();
            v
        };
        let end_parts: Vec<InstId> = self.host.subscribed("TouchEnded");
        if !touch_parts.is_empty() || !end_parts.is_empty() {
            let mut now: HashSet<(InstId, InstId)> = HashSet::new();
            {
                let dm = self.dm.borrow();
                let mut watch: Vec<InstId> = touch_parts.clone();
                watch.extend(end_parts.iter().copied());
                watch.sort();
                watch.dedup();
                for p in watch {
                    let pb = match Self::box_of(&dm, p) {
                        Some(b) => b,
                        None => continue,
                    };
                    for (m, c) in self.chars.iter() {
                        if self.dead.contains_key(m) {
                            continue;
                        }
                        for (cp, _) in c.parts.iter() {
                            if let Some(cb) = Self::box_of(&dm, *cp) {
                                if overlap(&pb, &cb) {
                                    now.insert((p, *cp));
                                }
                            }
                        }
                    }
                }
            }
            for (p, cp) in now.difference(&self.touching).copied().collect::<Vec<_>>() {
                if self.spawn_cache.contains(&p) {
                    let owner = self.chars.values().find(|c| c.parts.iter().any(|(id, _)| *id == cp)).map(|c| c.player);
                    if let Some(o) = owner {
                        let mut dm = self.dm.borrow_mut();
                        let col = dm.get_prop(p, "TeamColor").cloned();
                        if let Some(col) = col {
                            let _ = dm.set_prop(o, "TeamColor", col);
                            let _ = dm.set_prop(o, "Neutral", Val::Bool(false));
                        }
                    }
                }
                if self.host.subscribed("Touched").contains(&p) {
                    self.host.fire(p, "Touched", vec![Wire::I(cp)]);
                }
            }
            for (p, cp) in self.touching.difference(&now).copied().collect::<Vec<_>>() {
                self.host.fire(p, "TouchEnded", vec![Wire::I(cp)]);
            }
            self.touching = now;
        }
        self.host.take_logs()
    }

    pub fn flush_all(&mut self, players: &[InstId]) -> Vec<(InstId, String)> {
        let ops: Vec<J> = {
            let mut dm = self.dm.borrow_mut();
            if dm.journal.is_empty() {
                vec![]
            } else {
                let journal = std::mem::take(&mut dm.journal);
                codec::ops_for(&dm, &self.roots, &journal)
            }
        };
        let dirty: Vec<InstId> = self.dirty.drain().collect();
        let mut out = vec![];
        for &p in players {
            let mut msg = serde_json::Map::new();
            msg.insert("t".into(), json!("rep"));
            msg.insert("o".into(), J::Array(ops.clone()));
            let ch: Vec<J> = dirty
                .iter()
                .filter_map(|m| self.chars.get(m))
                .filter(|c| c.player != p)
                .map(|c| {
                    let r = |x: f64| (x * 10000.0).round() / 10000.0;
                    let mut rec: Vec<J> = vec![json!(c.model)];
                    rec.extend(c.cf.iter().map(|x| json!(r(*x))));
                    rec.extend(c.vel.iter().map(|x| json!((x * 100.0).round() / 100.0)));
                    rec.push(json!(c.state));
                    J::Array(rec)
                })
                .collect();
            if !ch.is_empty() {
                msg.insert("ch".into(), J::Array(ch));
            }
            if let Some(after) = self.pending.remove(&p) {
                if !after.is_empty() {
                    msg.insert("after".into(), J::Array(after));
                }
            }
            if msg.len() > 2 || !ops.is_empty() {
                out.push((p, J::Object(msg).to_string()));
            }
        }
        out
    }

    pub fn flush(&mut self) -> Option<String> {
        let mut dm = self.dm.borrow_mut();
        if dm.journal.is_empty() {
            return None;
        }
        let journal = std::mem::take(&mut dm.journal);
        let ops = codec::ops_for(&dm, &self.roots, &journal);
        if ops.is_empty() {
            return None;
        }
        Some(json!({ "t": "rep", "o": ops }).to_string())
    }
}

#[allow(dead_code)]
fn unused(_: Change) {}

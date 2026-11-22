use crate::character::{self, Char};
use crate::codec;
use std::collections::{HashMap, HashSet};
use serde_json::{json, Value as J};
use std::cell::RefCell;
use std::rc::Rc;
use swirl_core::classes;
use swirl_core::instance::{Change, DataModel, InstId, Val};
use swirl_lua::host::Wire;
use swirl_lua::{wire_from_json, wire_to_json, Host, LogLine, Req};
use crate::touch::{overlap, Box3};

#[derive(Clone, Default, Debug)]
pub struct UserInfo {
    pub id: i64,
    pub name: String,
    pub membership: i64,
    pub age: i64,
    pub dev: bool,
    pub r15: bool,
    pub scale: J,
    pub perks_badge: String,
    pub perks_chat: Option<String>,
    pub perks_name: Option<String>,
    pub animations: Vec<String>,
    pub gear_genres: Vec<String>,
    pub gear_map: J,
}

pub struct Game {
    pub place_id: u64,
    pub era: i32,
    pub dm: Rc<RefCell<DataModel>>,
    pub roots: Vec<InstId>,
    pub host: Host,
    pub(crate) next_user: i64,
    pub t: f64,
    pub chars: HashMap<InstId, Char>,
    pub(crate) pending: HashMap<InstId, Vec<J>>,
    pub(crate) dirty: HashSet<InstId>,
    pub(crate) touching: HashSet<(InstId, InstId)>,
    pub(crate) dead: HashMap<InstId, f64>,
    pub(crate) debris: Vec<(f64, InstId)>,
    pub(crate) spawn_cache: Vec<InstId>,
    pub(crate) spawn_refresh: f64,
    pub(crate) mice: HashMap<InstId, InstId>,
    pub(crate) loose: Vec<InstId>,
    pub(crate) loose_refresh: f64,
    pub(crate) joined_at: HashMap<InstId, usize>,
    pub(crate) looks: HashMap<InstId, crate::appearance::Look>,
    pub(crate) gui_copied: HashSet<InstId>,
    pub(crate) invokes: HashMap<u32, (InstId, i64)>,
    pub(crate) invoke_seq: u32,
    pub remote_out: Vec<(Vec<InstId>, J)>,
    pub anim_state: HashMap<(InstId, String), J>,
    pub(crate) last_chat: HashMap<InstId, f64>,
    pub api: crate::api::Shared,
    pub job: String,
    pub(crate) chat_log: Vec<String>,
    pub(crate) last_report: HashMap<InstId, f64>,
    pub(crate) mkt_ids: HashMap<u32, i64>,
    pub(crate) bought: HashMap<InstId, HashSet<String>>,
    pub(crate) receipt_busy: HashSet<InstId>,
    pub(crate) receipt_queue: HashMap<InstId, Vec<J>>,
    pub(crate) receipt_wait: HashMap<u32, (InstId, J)>,
    pub(crate) receipt_seq: u32,
    pub(crate) receipt_retry: Vec<(f64, InstId)>,
    pub(crate) mkt_window: HashMap<InstId, (f64, u32)>,
    pub physics: Rc<RefCell<swirl_physics::Physics>>,
    pub npcs: crate::npc::Npcs,
    pub(crate) frame: u64,
    pub(crate) ph_cache: Vec<[f64; 16]>,
    pub(crate) asm_sent: HashMap<InstId, Vec<i64>>,
    pub(crate) asm_seen: u64,
    pub(crate) asm_diff: Option<(Vec<J>, Vec<InstId>)>,
    pub(crate) ph_far: HashMap<InstId, HashMap<InstId, [f64; 16]>>,
    pub(crate) ph_n: HashMap<InstId, u32>,
    pub(crate) flush_at: f64,
    pub allow_gear: bool,
    pub(crate) pre_logs: Vec<LogLine>,
    pub kicks: Vec<(InstId, String)>,
}

impl Game {
    pub fn load(place_id: u64, bytes: &[u8], era: i32) -> Result<Game, String> {
        Game::load_with(place_id, bytes, era, "/var/www/assets", "/tmp/swirl-data")
    }

    pub fn load_with(place_id: u64, bytes: &[u8], era: i32, assets_dir: &str, data_dir: &str) -> Result<Game, String> {
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
        {
            let mut all = vec![];
            dm.descendants(root, &mut all);
            for d in all {
                if dm.get(d).map(|i| i.class == "Humanoid").unwrap_or(false) {
                    let mh = matches!(dm.get_prop(d, "MaxHealth"), Some(Val::Num(n)) if *n > 0.0);
                    if !mh {
                        let _ = dm.set_prop(d, "MaxHealth", Val::Num(100.0));
                        if !matches!(dm.get_prop(d, "Health"), Some(Val::Num(n)) if *n > 0.0) {
                            let _ = dm.set_prop(d, "Health", Val::Num(100.0));
                        }
                    }
                }
            }
        }
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
        let bundle = if era == 2018 { character::chocolate_dir().and_then(crate::core2018::Bundle::load) } else { None };
        if let Some(b) = bundle.as_ref() {
            b.prepare(&mut dm)?;
        }
        let mut physics = swirl_physics::Physics::new();
        physics.attach_workspace(&mut dm, ws);
        dm.journal.clear();
        physics.reset_cursor();
        let physics = Rc::new(RefCell::new(physics));
        let dm = Rc::new(RefCell::new(dm));
        let mut host = Host::new(dm.clone()).map_err(|e| e.to_string())?;
        let api = crate::api::new_state(assets_dir, data_dir, place_id, era);
        host.set_handler(Box::new(crate::api::Api(api.clone(), physics.clone())));
        host.add_methods(crate::api::METHODS);
        host.scan_scripts();
        if let Some(b) = bundle.as_ref() {
            let started = b.server_starter(&mut dm.borrow_mut());
            if let Some(id) = started {
                host.scan_subtree(id);
            }
        }
        let mut g = Game { place_id, era, dm, roots, host, next_user: 1, t: 0.0, chars: HashMap::new(), pending: HashMap::new(), dirty: HashSet::new(), touching: HashSet::new(), dead: HashMap::new(), debris: vec![], spawn_cache: vec![], spawn_refresh: 0.0, mice: HashMap::new(), loose: vec![], loose_refresh: 0.0, joined_at: HashMap::new(), looks: HashMap::new(), gui_copied: HashSet::new(), invokes: HashMap::new(), invoke_seq: 1, remote_out: vec![], anim_state: HashMap::new(), last_chat: HashMap::new(), api, job: String::new(), chat_log: vec![], last_report: HashMap::new(), mkt_ids: HashMap::new(), bought: HashMap::new(), receipt_busy: HashSet::new(), receipt_queue: HashMap::new(), receipt_wait: HashMap::new(), receipt_seq: 0, receipt_retry: vec![], mkt_window: HashMap::new(), physics, npcs: crate::npc::Npcs::new(), frame: 0, ph_cache: vec![], asm_sent: HashMap::new(), asm_seen: 0, asm_diff: None, ph_far: HashMap::new(), ph_n: HashMap::new(), flush_at: 0.0, allow_gear: true, pre_logs: vec![], kicks: vec![] };
        g.scan_npcs_all();
        for _ in 0..3 {
            g.host.tick(0.0);
        }
        Ok(g)
    }

    pub fn join(&mut self, name: &str, user: Option<i64>, look: crate::appearance::Look) -> (J, InstId) {
        let uid = match user {
            Some(u) => u,
            None => {
                self.next_user += 1;
                self.next_user - 1
            }
        };
        self.join_user(&UserInfo { id: uid, name: name.to_string(), age: 100, ..Default::default() }, look)
    }

    pub fn join_user(&mut self, user: &UserInfo, look: crate::appearance::Look) -> (J, InstId) {
        let uid = user.id;
        let name = user.name.as_str();
        let (player, snap) = {
            let mut dm = self.dm.borrow_mut();
            let players = dm.service("Players").unwrap();
            let player = dm.create_any("Player", None).unwrap();
            let _ = dm.set_prop(player, "Name", Val::Str(name.to_string()));
            let _ = dm.set_prop(player, "userId", Val::Num(uid as f64));
            let _ = dm.set_prop(player, "UserId", Val::Num(uid as f64));
            let _ = dm.set_prop(player, "CharacterAppearance", Val::Str(format!("/api/charapp/{}", uid)));
            let _ = dm.set_prop(player, "AccountAge", Val::Num(user.age as f64));
            let _ = dm.set_prop(player, "MembershipType", Val::Num(user.membership as f64));
            let _ = dm.set_prop(player, "DataReady", Val::Bool(false));
            let _ = dm.set_prop(player, "DataComplexityLimit", Val::Num(45000.0));
            for (cls, nm) in [("Backpack", "Backpack"), ("PlayerGui", "PlayerGui")] {
                if let Ok(c) = dm.create_any(cls, Some(player)) {
                    let _ = dm.set_prop(c, "Name", Val::Str(nm.to_string()));
                }
            }
            if !user.perks_badge.is_empty() || user.perks_chat.is_some() || user.perks_name.is_some() {
                if let Ok(cfg) = dm.create_any("Configuration", Some(player)) {
                    dm.set_prop_quiet(cfg, "Name", Val::Str("SolarPerks".into()));
                    dm.get_mut_name(cfg, "SolarPerks");
                    if !user.perks_badge.is_empty() {
                        if let Ok(v) = dm.create_any("StringValue", Some(cfg)) {
                            dm.set_prop_quiet(v, "Name", Val::Str("Badge".into()));
                            dm.get_mut_name(v, "Badge");
                            let _ = dm.set_prop(v, "Value", Val::Str(user.perks_badge.clone()));
                        }
                    }
                    for (hex, nm) in [(&user.perks_chat, "ChatColor"), (&user.perks_name, "NameColor")] {
                        if let Some(h) = hex {
                            if let Ok(n) = u32::from_str_radix(h.trim_start_matches('#'), 16) {
                                if let Ok(v) = dm.create_any("Color3Value", Some(cfg)) {
                                    dm.set_prop_quiet(v, "Name", Val::Str(nm.into()));
                                    dm.get_mut_name(v, nm);
                                    let _ = dm.set_prop(v, "Value", Val::Arr(vec![((n >> 16) & 255) as f64 / 255.0, ((n >> 8) & 255) as f64 / 255.0, (n & 255) as f64 / 255.0]));
                                }
                            }
                        }
                    }
                }
            }
            character::assign_team(&mut dm, player);
            if self.era == 2018 {
                character::copy_player_scripts(&mut dm, player);
            }
            dm.set_parent(player, Some(players)).unwrap();
            let snap = codec::snapshot_for(&dm, &self.roots, player);
            self.joined_at.insert(player, dm.journal.len());
            (player, snap)
        };
        self.looks.insert(player, look);
        if self.era == 2018 {
            self.receipt_retry.push((self.t + 8.0, player));
        }
        for ((_, _), m) in self.anim_state.iter() {
            self.remote_out.push((vec![player], m.clone()));
        }
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
        let mut spawned: Option<InstId> = None;
        {
            let mut dm = self.dm.borrow_mut();
            let ws = dm.services["Workspace"];
            let origin = character::spawn_origin(&dm, ws, player);
            let look = self.looks.get(&player);
            match character::build(&mut dm, ws, player, name, origin, look, self.era) {
                Ok(ch) => {
                    character::add_scripts(&mut dm, ch.model, self.era);
                    if self.era == 2018 {
                        character::copy_character_scripts(&mut dm, ch.model);
                    }
                    let first = self.gui_copied.insert(player);
                    character::copy_starter_gui(&mut dm, player, first);
                    let own = json!({"t": "own", "id": ch.model, "s": ch.seq, "c": ch.cf});
                    self.pending.entry(player).or_default().push(own);
                    spawned = Some(ch.model);
                    self.chars.insert(ch.model, ch);
                }
                Err(e) => {
                    eprintln!("character spawn failed: {}", e);
                    return;
                }
            }
        }
        if let Some(m) = spawned {
            self.physics.borrow_mut().exclude_model(&self.dm.borrow(), m);
            self.host.scan_subtree(m);
            self.host.scan_subtree(player);
        }
        self.fill_backpack(player);
    }

    fn fill_backpack(&mut self, player: InstId) {
        let made: Vec<(InstId, InstId)> = {
            let mut dm = self.dm.borrow_mut();
            let bp = match dm.find_child_of_class(player, "Backpack") {
                Some(b) => b,
                None => return,
            };
            let old: Vec<InstId> = dm.get(bp).map(|b| b.children.clone()).unwrap_or_default();
            for c in old {
                dm.destroy(c);
            }
            let sp = match dm.services.get("StarterPack") {
                Some(s) => *s,
                None => return,
            };
            let kids: Vec<InstId> = dm.get(sp).map(|s| s.children.clone()).unwrap_or_default();
            let mut out = vec![];
            for k in kids {
                if let Some(c) = dm.clone_tree(k) {
                    if dm.set_parent(c, Some(bp)).is_ok() {
                        out.push((c, bp));
                    }
                }
            }
            out
        };
        for (c, bp) in made {
            self.host.notify_parent(c, None, Some(bp));
        }
        let gear: Vec<(u64, Vec<u8>)> = self.looks.get(&player).map(|l| l.gear.clone()).unwrap_or_default();
        if self.allow_gear && !gear.is_empty() {
            let added: Vec<(InstId, InstId)> = {
                let mut dm = self.dm.borrow_mut();
                let bp = match dm.find_child_of_class(player, "Backpack") {
                    Some(b) => b,
                    None => return,
                };
                let mut out = vec![];
                for (_, bytes) in gear {
                    let tree = match swirl_core::rbx::parse(&bytes) {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    let tmp = match dm.create_any("Model", None) {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    let made = dm.load_tree(&tree, tmp);
                    let tool = made.iter().copied().find(|m| dm.get(*m).map(|i| i.class == "Tool" || i.class == "HopperBin").unwrap_or(false));
                    if let Some(t) = tool {
                        if dm.set_parent(t, Some(bp)).is_ok() {
                            out.push((t, bp));
                        }
                    }
                    dm.destroy(tmp);
                }
                out
            };
            for (c, bp) in added {
                self.host.notify_parent(c, None, Some(bp));
            }
        }
    }

    fn mouse_of(&mut self, player: InstId) -> InstId {
        if let Some(m) = self.mice.get(&player) {
            if self.dm.borrow().get(*m).map(|i| !i.destroyed).unwrap_or(false) {
                return *m;
            }
        }
        let mut dm = self.dm.borrow_mut();
        let m = dm.create_any("PlayerMouse", None).unwrap();
        dm.get_mut_name(m, "Mouse");
        dm.set_prop_quiet(m, "Name", Val::Str("Mouse".into()));
        dm.set_prop_quiet(m, "ViewSizeX", Val::Num(1024.0));
        dm.set_prop_quiet(m, "ViewSizeY", Val::Num(768.0));
        self.mice.insert(player, m);
        m
    }

    fn char_of(&self, player: InstId) -> Option<InstId> {
        self.chars.iter().find(|(m, c)| c.player == player && !self.dead.contains_key(*m)).map(|(m, _)| *m)
    }

    fn tool_owned(&self, player: InstId, tool: InstId) -> bool {
        let dm = self.dm.borrow();
        match dm.get(tool) {
            Some(i) if i.class == "Tool" && !i.destroyed => {}
            _ => return false,
        }
        let bp = dm.find_child_of_class(player, "Backpack");
        let model = self.char_of(player);
        bp.map(|b| dm.get(tool).and_then(|i| i.parent) == Some(b)).unwrap_or(false) || model.map(|m| dm.get(tool).and_then(|i| i.parent) == Some(m)).unwrap_or(false)
    }

    pub fn equip(&mut self, player: InstId, tool: InstId) {
        let model = match self.char_of(player) {
            Some(m) => m,
            None => return,
        };
        let others: Vec<InstId> = {
            let dm = self.dm.borrow();
            dm.get(model).map(|m| m.children.iter().copied().filter(|c| *c != tool && dm.get(*c).map(|i| i.class == "Tool").unwrap_or(false)).collect()).unwrap_or_default()
        };
        for o in others {
            self.unequip(player, o);
        }
        let old_parent = self.dm.borrow().get(tool).and_then(|i| i.parent);
        {
            let mut dm = self.dm.borrow_mut();
            if old_parent != Some(model) {
                let _ = dm.set_parent(tool, Some(model));
            }
        }
        if old_parent != Some(model) {
            self.host.notify_parent(tool, old_parent, Some(model));
        }
        let weld = {
            let mut dm = self.dm.borrow_mut();
            let requires = !matches!(dm.get_prop(tool, "RequiresHandle"), Some(Val::Bool(false)));
            let handle = dm.find_child(tool, "Handle", false);
            let arm = character::arm_of(&dm, model);
            match (requires, handle, arm) {
                (true, Some(h), Some(a)) if dm.get(h).map(|i| swirl_core::classes::is_a(&i.class, "BasePart")).unwrap_or(false) => {
                    if let Some(old) = dm.find_child(a, "RightGrip", false) {
                        dm.destroy(old);
                    }
                    let w = dm.create_any("Weld", None).unwrap();
                    dm.set_prop_quiet(w, "Name", Val::Str("RightGrip".into()));
                    dm.get_mut_name(w, "RightGrip");
                    dm.set_prop_quiet(w, "Part0", Val::Ref(Some(a)));
                    dm.set_prop_quiet(w, "Part1", Val::Ref(Some(h)));
                    dm.set_prop_quiet(w, "C0", Val::Arr(character::RIGHT_GRIP_C0.to_vec()));
                    let grip = character::cf_prop(&dm, tool, "Grip");
                    dm.set_prop_quiet(w, "C1", Val::Arr(grip.to_vec()));
                    let _ = dm.set_parent(w, Some(a));
                    dm.set_prop_quiet(h, "CanCollide", Val::Bool(false));
                    dm.set_prop_quiet(h, "Anchored", Val::Bool(false));
                    if let Some(ch) = self.chars.get(&model) {
                        character::sync_tools(&mut dm, ch);
                    }
                    Some((w, a))
                }
                _ => None,
            }
        };
        if let Some((w, a)) = weld {
            self.host.notify_parent(w, None, Some(a));
        }
        self.dirty.insert(model);
        let mouse = self.mouse_of(player);
        self.host.fire(tool, "Equipped", vec![Wire::I(mouse)]);
    }

    pub fn unequip(&mut self, player: InstId, tool: InstId) {
        let model = match self.char_of(player) {
            Some(m) => m,
            None => return,
        };
        let bp = {
            let mut dm = self.dm.borrow_mut();
            if dm.get(tool).and_then(|i| i.parent) != Some(model) {
                return;
            }
            if let Some(a) = character::arm_of(&dm, model) {
                if let Some(g) = dm.find_child(a, "RightGrip", false) {
                    dm.destroy(g);
                }
            }
            dm.find_child_of_class(player, "Backpack")
        };
        self.host.fire(tool, "Unequipped", vec![]);
        if let Some(b) = bp {
            let _ = self.dm.borrow_mut().set_parent(tool, Some(b));
            self.host.notify_parent(tool, Some(model), Some(b));
        }
        self.dirty.insert(model);
    }

    fn drop_tool(&mut self, player: InstId, tool: InstId) {
        let model = match self.char_of(player) {
            Some(m) => m,
            None => return,
        };
        if !matches!(self.dm.borrow().get_prop(tool, "CanBeDropped"), Some(Val::Bool(true))) {
            return;
        }
        let ws = {
            let mut dm = self.dm.borrow_mut();
            if dm.get(tool).and_then(|i| i.parent) != Some(model) {
                return;
            }
            if let Some(a) = character::arm_of(&dm, model) {
                if let Some(g) = dm.find_child(a, "RightGrip", false) {
                    dm.destroy(g);
                }
            }
            if let (Some(h), Some(c)) = (dm.find_child(tool, "Handle", false), self.chars.get(&model)) {
                let f = [c.cf[0] - c.cf[5] * 3.0, c.cf[1] + 2.0, c.cf[2] - c.cf[11] * 3.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
                dm.set_prop_quiet(h, "CFrame", Val::Arr(f.to_vec()));
                dm.set_prop_quiet(h, "CanCollide", Val::Bool(true));
            }
            dm.services["Workspace"]
        };
        self.host.fire(tool, "Unequipped", vec![]);
        let _ = self.dm.borrow_mut().set_parent(tool, Some(ws));
        self.host.notify_parent(tool, Some(model), Some(ws));
        self.dirty.insert(model);
    }

    pub fn social_requests(&mut self) -> Vec<crate::api::SocialReq> {
        std::mem::take(&mut self.api.borrow_mut().social)
    }

    pub fn social_done(&mut self, token: u32, kind: crate::api::SocialKind, r: Option<J>) {
        use crate::api::SocialKind;
        match kind {
            SocialKind::Mkt(pl, k, op) => {
                let reply = match op {
                    0 => json!({"info": r.as_ref().map(|j| j["info"].clone()).unwrap_or(J::Null)}),
                    1 => json!({"owns": r.as_ref().and_then(|j| j["owns"].as_bool()).unwrap_or(false)}),
                    2 => json!({"robux": r.as_ref().and_then(|j| j["robux"].as_i64()).unwrap_or(0)}),
                    _ => {
                        let success = r.as_ref().and_then(|j| j["success"].as_bool()).unwrap_or(false);
                        let status = r.as_ref().and_then(|j| j["status"].as_str()).unwrap_or("Error").to_string();
                        let mut out = json!({"success": success, "status": status});
                        if success || status == "AlreadyOwned" {
                            let key = format!("{}:{}", if op == 4 { "product" } else { "asset" }, self.mkt_ids.get(&token).copied().unwrap_or(0));
                            self.bought.entry(pl).or_default().insert(key);
                        }
                        if success && op == 4 {
                            let receipt = r.as_ref().map(|j| j["receipt"].to_string().trim_matches('"').to_string()).unwrap_or_default();
                            out["receipt"] = json!(receipt);
                            self.remote_out.push((vec![pl], json!({"t": "pver", "receipt": receipt, "product": self.mkt_ids.get(&token).copied().unwrap_or(0)})));
                            self.queue_receipts(pl);
                        }
                        out
                    }
                };
                self.mkt_ids.remove(&token);
                self.remote_out.push((vec![pl], json!({"t": "mktr", "k": k, "r": reply})));
                return;
            }
            SocialKind::Pending(pl) => {
                let list: Vec<J> = r.as_ref().and_then(|j| j["receipts"].as_array().cloned()).unwrap_or_default();
                self.receipt_busy.remove(&pl);
                if !list.is_empty() {
                    self.receipt_queue.entry(pl).or_default().extend(list);
                    self.next_receipt(pl);
                }
                return;
            }
            SocialKind::Grant | SocialKind::Report => return,
            _ => {}
        }
        let (res, msg) = self.api.borrow_mut().finish_social(kind, r);
        if let Some(m) = msg {
            let text: String = m.chars().take(200).collect();
            self.remote_out.push((vec![], json!({"t": "sys", "m": text})));
        }
        self.host.resolve(token, res);
    }

    pub fn queue_receipts(&mut self, pl: InstId) {
        if self.era != 2018 || self.receipt_busy.contains(&pl) {
            return;
        }
        self.receipt_busy.insert(pl);
        let uid = match self.dm.borrow().get_prop(pl, "userId") {
            Some(Val::Num(n)) => *n as i64,
            _ => 0,
        };
        let mut a = self.api.borrow_mut();
        a.token += 1;
        let token = a.token;
        let place = a.place_id;
        a.social.push(crate::api::SocialReq { token, body: json!({"action": "pendingReceipts", "userId": uid, "placeId": place}), kind: crate::api::SocialKind::Pending(pl) });
    }

    fn next_receipt(&mut self, pl: InstId) {
        if self.receipt_wait.values().any(|(p, _)| *p == pl) {
            return;
        }
        let rc = match self.receipt_queue.get_mut(&pl).and_then(|q| if q.is_empty() { None } else { Some(q.remove(0)) }) {
            Some(r) => r,
            None => return,
        };
        let uid = match self.dm.borrow().get_prop(pl, "userId") {
            Some(Val::Num(n)) => *n as i64,
            _ => 0,
        };
        self.receipt_seq += 1;
        let token = 900_000 + self.receipt_seq;
        self.receipt_wait.insert(token, (pl, rc.clone()));
        let mps = match self.dm.borrow_mut().service("MarketplaceService") {
            Ok(m) => m,
            Err(_) => return,
        };
        let info = Wire::Dict(vec![
            ("PlayerId".into(), Wire::N(uid as f64)),
            ("PlaceIdWherePurchased".into(), Wire::N(self.place_id as f64)),
            ("PurchaseId".into(), Wire::S(rc["id"].to_string().trim_matches('"').to_string())),
            ("ProductId".into(), Wire::N(rc["productId"].as_f64().unwrap_or(0.0))),
            ("CurrencyType".into(), Wire::E("CurrencyType".into(), 1.0)),
            ("CurrencySpent".into(), Wire::N(rc["robux"].as_f64().unwrap_or(0.0))),
        ]);
        self.host.fire(mps, "__Receipt", vec![Wire::N(token as f64), info]);
    }

    pub fn asset_requests(&mut self) -> Vec<(u32, u64)> {
        std::mem::take(&mut self.api.borrow_mut().assets_wanted)
    }

    pub fn asset_done(&mut self, token: u32, bytes: Option<Vec<u8>>) {
        let r = match bytes {
            Some(b) => {
                let mut dm = self.dm.borrow_mut();
                crate::api::make_model(&mut dm, &b).map(|m| vec![Wire::I(m)])
            }
            None => Err("asset is not available".to_string()),
        };
        self.host.resolve(token, r);
    }

    pub fn run_code(&mut self, source: &str, by: &str) {
        self.host_logs_push(format!("Running server code from {}", by));
        let made = {
            let mut dm = self.dm.borrow_mut();
            let sss = match dm.service("ServerScriptService") {
                Ok(s) => s,
                Err(_) => return,
            };
            match dm.create_any("Script", None) {
                Ok(sc) => {
                    dm.set_prop_quiet(sc, "Name", Val::Str("ServerCode".into()));
                    dm.get_mut_name(sc, "ServerCode");
                    dm.set_prop_quiet(sc, "Source", Val::Str(source.to_string()));
                    let _ = dm.set_parent(sc, Some(sss));
                    Some(sc)
                }
                Err(_) => None,
            }
        };
        if let Some(sc) = made {
            self.host.scan_subtree(sc);
        }
    }

    fn host_logs_push(&mut self, msg: String) {
        self.pre_logs.push(LogLine { kind: "print".into(), msg, id: 0 });
    }

    pub fn stop(&mut self) {
        self.api.borrow_mut().flush();
    }

    pub fn leave(&mut self, player: InstId) {
        self.anim_state.retain(|(p, _), _| *p != player);
        let models: Vec<InstId> = self.chars.iter().filter(|(_, c)| c.player == player).map(|(m, _)| *m).collect();
        let mut dm = self.dm.borrow_mut();
        for m in models {
            dm.destroy(m);
            self.chars.remove(&m);
        }
        dm.destroy(player);
        self.pending.remove(&player);
        self.looks.remove(&player);
    }

    pub fn message(&mut self, player: InstId, text: &str) {
        let v: J = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return,
        };
        let tool = v["id"].as_u64().map(|x| x as InstId);
        match v["t"].as_str().unwrap_or("") {
            "anim" => {
                let model = match self.chars.iter().find(|(_, c)| c.player == player) {
                    Some((m, _)) => *m,
                    None => return,
                };
                let hum = self.dm.borrow().find_child_of_class(model, "Humanoid");
                let h = v["h"].as_u64().map(|x| x as InstId);
                if hum.is_none() || h != hum {
                    return;
                }
                let op = v["op"].as_str().unwrap_or("");
                if !["play", "stop", "speed", "weight", "time"].contains(&op) {
                    return;
                }
                let key = format!("p{}:{}", player, v["k"].as_i64().unwrap_or(0));
                let mut m = v.clone();
                m["k"] = json!(key);
                self.anim_state.retain(|(p, k), _| !(*p == player && *k == key && op == "stop"));
                if op == "play" {
                    self.anim_state.insert((player, key.clone()), m.clone());
                }
                let ps = {
                    let dm = self.dm.borrow();
                    dm.services.get("Players").and_then(|p| dm.get(*p)).map(|p| p.children.clone()).unwrap_or_default()
                };
                let others: Vec<InstId> = ps.into_iter().filter(|p| *p != player).collect();
                if !others.is_empty() {
                    self.remote_out.push((others, m));
                }
            }
            "report" => {
                if self.t - self.last_report.get(&player).copied().unwrap_or(-999.0) < 20.0 {
                    return;
                }
                self.last_report.insert(player, self.t);
                let (rid, rname) = {
                    let dm = self.dm.borrow();
                    let id = match dm.get_prop(player, "userId") {
                        Some(Val::Num(n)) => *n as i64,
                        _ => 0,
                    };
                    (id, dm.name(player).to_string())
                };
                let target = v["p"].as_u64().map(|x| x as InstId).filter(|t| self.dm.borrow().get(*t).map(|i| i.class == "Player" && !i.destroyed).unwrap_or(false));
                let tid = target.and_then(|t| match self.dm.borrow().get_prop(t, "userId") {
                    Some(Val::Num(n)) => Some(*n as i64),
                    _ => None,
                }).unwrap_or(0);
                let clean = |s: &str, n: usize| -> String { s.chars().filter(|c| !c.is_control()).take(n).collect::<String>().trim().to_string() };
                let reason = clean(v["r"].as_str().unwrap_or(""), 80);
                let chat: Vec<String> = self.chat_log.iter().rev().take(25).rev().cloned().collect();
                let mut a = self.api.borrow_mut();
                a.token += 1;
                let token = a.token;
                let place = a.place_id;
                let job = self.job.clone();
                eprintln!("report from {} ({}) about {}: {}", rname, rid, tid, reason);
                a.social.push(crate::api::SocialReq { token, body: json!({"reporterId": rid, "targetId": tid, "placeId": place, "job": job, "reason": reason, "description": clean(v["m"].as_str().unwrap_or(""), 1500), "chat": chat}), kind: crate::api::SocialKind::Report });
            }
            "mkt" => {
                let (win, cnt) = self.mkt_window.get(&player).copied().unwrap_or((0.0, 0));
                let (win, cnt) = if self.t - win > 10.0 { (self.t, 0) } else { (win, cnt) };
                self.mkt_window.insert(player, (win, cnt + 1));
                let k = v["k"].as_i64().unwrap_or(0);
                if cnt + 1 > 30 {
                    self.remote_out.push((vec![player], json!({"t": "mktr", "k": k, "r": J::Null})));
                    return;
                }
                let uid = match self.dm.borrow().get_prop(player, "userId") {
                    Some(Val::Num(n)) => *n as i64,
                    _ => 0,
                };
                let id = v["id"].as_f64().unwrap_or(0.0).max(0.0).floor() as i64;
                let kind_s = v["kind"].as_str().unwrap_or("asset");
                let mut a = self.api.borrow_mut();
                a.token += 1;
                let token = a.token;
                let place = a.place_id;
                let (body, op) = match v["op"].as_str().unwrap_or("") {
                    "info" => (json!({"action": "productInfo", "kind": if ["asset", "gamepass", "product"].contains(&kind_s) { kind_s } else { "asset" }, "id": id}), 0u8),
                    "owns" => {
                        let who = v["uid"].as_f64().unwrap_or(0.0).floor() as i64;
                        if who <= 0 || id <= 0 {
                            drop(a);
                            self.remote_out.push((vec![player], json!({"t": "mktr", "k": k, "r": {"owns": false}})));
                            return;
                        }
                        (json!({"action": "ownsAsset", "userId": who, "assetId": id}), 1u8)
                    }
                    "balance" => (json!({"action": "balance", "userId": uid}), 2u8),
                    "buy" => {
                        let product = kind_s == "product";
                        let price = v["price"].as_f64().unwrap_or(0.0).floor() as i64;
                        (json!({"action": "purchase", "userId": uid, "placeId": place, "kind": if product { "product" } else { "asset" }, "id": id, "price": price}), if product { 4u8 } else { 3u8 })
                    }
                    _ => {
                        drop(a);
                        self.remote_out.push((vec![player], json!({"t": "mktr", "k": k, "r": J::Null})));
                        return;
                    }
                };
                a.social.push(crate::api::SocialReq { token, body, kind: crate::api::SocialKind::Mkt(player, k, op) });
                drop(a);
                self.mkt_ids.insert(token, id);
            }
            "pfin" => {
                let id = v["id"].as_f64().unwrap_or(0.0).max(0.0).floor() as i64;
                let kind = match v["kind"].as_str() {
                    Some("product") => "product",
                    Some("gamepass") => "gamepass",
                    _ => "asset",
                };
                let key = format!("{}:{}", if kind == "product" { "product" } else { "asset" }, id);
                let ok = v["ok"].as_bool().unwrap_or(false) && self.bought.get(&player).map(|b| b.contains(&key)).unwrap_or(false);
                if ok {
                    if let Some(b) = self.bought.get_mut(&player) {
                        b.remove(&key);
                    }
                }
                let mps = match self.dm.borrow_mut().service("MarketplaceService") {
                    Ok(m) => m,
                    Err(_) => return,
                };
                match kind {
                    "gamepass" => self.host.fire(mps, "PromptGamePassPurchaseFinished", vec![Wire::I(player), Wire::N(id as f64), Wire::B(ok)]),
                    "asset" => self.host.fire(mps, "PromptPurchaseFinished", vec![Wire::I(player), Wire::N(id as f64), Wire::B(ok)]),
                    _ => {
                        let uid = match self.dm.borrow().get_prop(player, "userId") {
                            Some(Val::Num(n)) => *n,
                            _ => 0.0,
                        };
                        self.host.fire(mps, "PromptProductPurchaseFinished", vec![Wire::N(uid), Wire::N(id as f64), Wire::B(ok)]);
                    }
                }
            }
            "re" | "rf" if self.era == 2018 => {
                let id = match tool {
                    Some(i) => i,
                    None => return,
                };
                let want = if v["t"] == "re" { "RemoteEvent" } else { "RemoteFunction" };
                if !self.dm.borrow().get(id).map(|i| i.class == want && !i.destroyed).unwrap_or(false) {
                    return;
                }
                let args = match v["a"].as_array() {
                    Some(a) if a.len() <= 20000 => wire_from_json(a),
                    _ => vec![],
                };
                let mut full = vec![Wire::I(player)];
                if v["t"] == "re" {
                    full.extend(args);
                    self.host.fire(id, "OnServerEvent", full);
                } else {
                    let tok = self.invoke_seq;
                    self.invoke_seq += 1;
                    self.invokes.insert(tok, (player, v["k"].as_i64().unwrap_or(0)));
                    let mut f = vec![Wire::N(tok as f64)];
                    f.extend(full);
                    f.extend(args);
                    self.host.fire(id, "__Invoke", f);
                }
            }
            "equip" => {
                if let Some(t) = tool {
                    if self.tool_owned(player, t) {
                        self.equip(player, t);
                    }
                }
            }
            "unequip" => {
                if let Some(t) = tool {
                    if self.tool_owned(player, t) {
                        self.unequip(player, t);
                    }
                }
            }
            "drop" => {
                if let Some(t) = tool {
                    if self.tool_owned(player, t) {
                        self.drop_tool(player, t);
                    }
                }
            }
            "act" => {
                if let Some(t) = tool {
                    let held = match self.char_of(player) {
                        Some(m) => self.dm.borrow().get(t).and_then(|i| i.parent) == Some(m) && self.dm.borrow().get(t).map(|i| i.class == "Tool").unwrap_or(false),
                        None => false,
                    };
                    if held {
                        let mouse = self.mouse_of(player);
                        if v["down"].as_bool().unwrap_or(false) {
                            if !matches!(self.dm.borrow().get_prop(t, "Enabled"), Some(Val::Bool(false))) {
                                self.host.fire(t, "Activated", vec![]);
                            }
                            self.host.fire(mouse, "Button1Down", vec![]);
                        } else {
                            self.host.fire(t, "Deactivated", vec![]);
                            self.host.fire(mouse, "Button1Up", vec![]);
                        }
                    }
                }
            }
            "chat" => {
                let raw = v["m"].as_str().unwrap_or("");
                let msg: String = raw.chars().filter(|c| !c.is_control()).take(200).collect::<String>().trim().to_string();
                if msg.is_empty() || self.t - self.last_chat.get(&player).copied().unwrap_or(-10.0) < 0.6 {
                    return;
                }
                self.last_chat.insert(player, self.t);
                let (name, uid, team, neutral, players) = {
                    let dm = self.dm.borrow();
                    let name = dm.get(player).map(|i| i.name.clone()).unwrap_or_default();
                    let uid = match dm.get_prop(player, "userId") {
                        Some(Val::Num(n)) => *n,
                        _ => 0.0,
                    };
                    let ps = dm.services.get("Players").and_then(|p| dm.get(*p)).map(|p| p.children.clone()).unwrap_or_default();
                    (name, uid, dm.get_prop(player, "TeamColor").cloned(), matches!(dm.get_prop(player, "Neutral"), Some(Val::Bool(true))), ps)
                };
                let kind = if v["k"] == "team" { "team" } else { "all" };
                let targets: Vec<InstId> = if kind == "team" {
                    let dm = self.dm.borrow();
                    players.into_iter().filter(|o| !neutral && !matches!(dm.get_prop(*o, "Neutral"), Some(Val::Bool(true))) && dm.get_prop(*o, "TeamColor").cloned() == team).collect()
                } else {
                    vec![]
                };
                self.chat_log.push(format!("{}: {}", name, msg).chars().take(200).collect());
                if self.chat_log.len() > 40 {
                    self.chat_log.remove(0);
                }
                self.host.fire(player, "Chatted", vec![Wire::S(msg.clone()), Wire::Nil]);
                self.remote_out.push((targets, json!({"t": "chat", "p": player, "n": name, "u": uid, "m": msg, "k": kind})));
            }
            "mouse" => {
                let mouse = self.mouse_of(player);
                let mut dm = self.dm.borrow_mut();
                if let Some(h) = v["h"].as_array() {
                    if h.len() >= 3 {
                        let p: Vec<f64> = h.iter().take(3).map(|x| x.as_f64().unwrap_or(0.0)).collect();
                        dm.set_prop_quiet(mouse, "Hit", Val::Arr(vec![p[0], p[1], p[2], 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]));
                    }
                }
                if let Some(t) = v["tg"].as_u64() {
                    if dm.get(t as InstId).is_some() {
                        dm.set_prop_quiet(mouse, "Target", Val::Ref(Some(t as InstId)));
                    }
                }
            }
            _ => {}
        }
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
            ch.j = arr("j", 4).map(|a| [a[0].clamp(-7.0, 7.0), a[1].clamp(-7.0, 7.0), a[2].clamp(-7.0, 7.0), a[3].clamp(-7.0, 7.0)]);
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
        self.api.borrow_mut().now_ms = self.t * 1000.0;
        self.host.tick(self.t);
        {
            let mut a = self.api.borrow_mut();
            a.step_tracks(dt, &mut self.dm.borrow_mut());
            let events = std::mem::take(&mut a.events);
            let casts = std::mem::take(&mut a.broadcasts);
            let directed = std::mem::take(&mut a.directed);
            let done = std::mem::take(&mut a.completions);
            let rres = std::mem::take(&mut a.receipt_results);
            if self.t >= self.flush_at {
                self.flush_at = self.t + 2.0;
                a.flush();
            }
            drop(a);
            for (id, name, args) in events {
                self.host.fire(id, &name, args);
            }
            for m in casts {
                if m["op"] == "play" {
                    self.anim_state.insert((0, format!("{}", m["k"])), m.clone());
                } else if m["op"] == "stop" {
                    self.anim_state.remove(&(0, format!("{}", m["k"])));
                }
                self.remote_out.push((vec![], m));
            }
            for (tok, r) in done {
                self.host.resolve(tok, r);
            }
            for (pl, m) in directed {
                self.remote_out.push((vec![pl], m));
            }
            for (tok, result) in rres {
                if let Some((pl, rc)) = self.receipt_wait.remove(&tok) {
                    let _ = rc.clone();
                    if result.ends_with("PurchaseGranted") {
                        let uid = match self.dm.borrow().get_prop(pl, "userId") {
                            Some(Val::Num(n)) => *n as i64,
                            _ => 0,
                        };
                        let mut a = self.api.borrow_mut();
                        a.token += 1;
                        let t = a.token;
                        let place = a.place_id;
                        a.social.push(crate::api::SocialReq { token: t, body: json!({"action": "grantReceipt", "receiptId": rc["id"], "userId": uid, "placeId": place}), kind: crate::api::SocialKind::Grant });
                        drop(a);
                        self.next_receipt(pl);
                    } else if result != "NoHandler" {
                        self.receipt_retry.push((self.t + 30.0, pl));
                    }
                }
            }
            let due: Vec<InstId> = self.receipt_retry.iter().filter(|(t, _)| *t <= self.t).map(|(_, p)| *p).collect();
            self.receipt_retry.retain(|(t, _)| *t > self.t);
            for p in due {
                self.queue_receipts(p);
            }
        }
        for r in self.host.take_requests() {
            match r {
                Req::LoadCharacter(p) => self.respawn(p),
                Req::Debris(i, s) => self.debris.push((self.t + s, i)),
                Req::EquipTool(h, t) => {
                    let owner = self.chars.values().find(|c| self.dm.borrow().find_child_of_class(c.model, "Humanoid") == Some(h)).map(|c| c.player);
                    if let Some(p) = owner {
                        if self.tool_owned(p, t) {
                            self.equip(p, t);
                        }
                    }
                }
                Req::UnequipTools(h) => {
                    let owner = self.chars.values().find(|c| self.dm.borrow().find_child_of_class(c.model, "Humanoid") == Some(h)).map(|c| (c.player, c.model));
                    if let Some((p, m)) = owner {
                        let tools: Vec<InstId> = {
                            let dm = self.dm.borrow();
                            dm.get(m).map(|i| i.children.iter().copied().filter(|c| dm.get(*c).map(|x| x.class == "Tool").unwrap_or(false)).collect()).unwrap_or_default()
                        };
                        for t in tools {
                            self.unequip(p, t);
                        }
                    }
                }
                Req::Remote(id, op, args) => {
                    let enc = J::Array(wire_to_json(&args));
                    match op.as_str() {
                        "fc" => {
                            if let Some(Wire::I(pl)) = args.first() {
                                let rest = J::Array(wire_to_json(&args[1..]));
                                self.remote_out.push((vec![*pl], json!({"t": "re", "id": id, "a": rest})));
                            }
                        }
                        "fa" => self.remote_out.push((vec![], json!({"t": "re", "id": id, "a": enc}))),
                        _ => {}
                    }
                }
                Req::MoveTo(h, pos, part) => self.set_move_to(h, pos, part),
                Req::InvokeResult(_, tok, args) => {
                    if let Some((pl, k)) = self.invokes.remove(&(tok as u32)) {
                        self.remote_out.push((vec![pl], json!({"t": "rfr", "k": k, "a": J::Array(wire_to_json(&args))})));
                    }
                }
            }
        }
        self.step_physics();
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
        if self.loose_refresh <= self.t {
            self.loose_refresh = self.t + 1.0;
            let dm = self.dm.borrow();
            let ws = dm.services["Workspace"];
            let mut all = vec![];
            dm.descendants(ws, &mut all);
            self.loose = all
                .into_iter()
                .filter(|id| {
                    let h = match dm.get(*id) {
                        Some(h) if h.name == "Handle" && swirl_core::classes::is_a(&h.class, "BasePart") => h,
                        _ => return false,
                    };
                    match h.parent.and_then(|t| dm.get(t)) {
                        Some(t) => t.class == "Tool" && !t.parent.map(|p| self.chars.contains_key(&p)).unwrap_or(false),
                        None => false,
                    }
                })
                .collect();
        }
        let mut pickups: Vec<(InstId, InstId)> = vec![];
        if !self.loose.is_empty() {
            let dm = self.dm.borrow();
            for h in self.loose.iter().copied() {
                let tool = match dm.get(h).and_then(|i| i.parent) {
                    Some(t) if dm.get(t).map(|i| i.class == "Tool" && !i.destroyed).unwrap_or(false) => t,
                    _ => continue,
                };
                if self.chars.contains_key(&dm.get(tool).and_then(|i| i.parent).unwrap_or(0)) {
                    continue;
                }
                let hb = match Self::box_of(&dm, h) {
                    Some(b) => b,
                    None => continue,
                };
                for (m, c) in self.chars.iter() {
                    if self.dead.contains_key(m) {
                        continue;
                    }
                    if c.parts.iter().any(|(cp, _)| Self::box_of(&dm, *cp).map(|cb| overlap(&hb, &cb)).unwrap_or(false)) {
                        pickups.push((c.player, tool));
                        break;
                    }
                }
            }
        }
        for (p, tool) in pickups {
            let bp = self.dm.borrow().find_child_of_class(p, "Backpack");
            let old = self.dm.borrow().get(tool).and_then(|i| i.parent);
            if let Some(b) = bp {
                let _ = self.dm.borrow_mut().set_parent(tool, Some(b));
                self.host.notify_parent(tool, old, Some(b));
                self.equip(p, tool);
            }
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
        let mut logs = std::mem::take(&mut self.pre_logs);
        logs.extend(self.host.take_logs());
        logs
    }

    fn step_physics(&mut self) {
        self.frame += 1;
        for ev in ["Touched", "TouchEnded"] {
            for id in self.host.subscribed(ev) {
                self.physics.borrow_mut().touch_interest(id);
            }
        }
        self.scan_npcs_journal();
        self.update_npcs(1.0 / 30.0);
        {
            let mut dm = self.dm.borrow_mut();
            self.physics.borrow_mut().step(&mut dm, 1.0 / 30.0);
        }
        let touches = std::mem::take(&mut self.physics.borrow_mut().touches);
        for t in touches {
            let alive = {
                let dm = self.dm.borrow();
                dm.get(t.a).map(|i| !i.destroyed).unwrap_or(false) && dm.get(t.b).map(|i| !i.destroyed).unwrap_or(false)
            };
            if !alive {
                continue;
            }
            let ev = if t.started { "Touched" } else { "TouchEnded" };
            self.host.fire(t.a, ev, vec![Wire::I(t.b)]);
            self.host.fire(t.b, ev, vec![Wire::I(t.a)]);
        }
        let fallen = std::mem::take(&mut self.physics.borrow_mut().fallen);
        if !fallen.is_empty() {
            let mut dm = self.dm.borrow_mut();
            for parts in fallen {
                for p in parts {
                    dm.destroy(p);
                }
            }
        }
        if self.asm_seen != self.physics.borrow_mut().asm_epoch {
            self.asm_seen = self.physics.borrow_mut().asm_epoch;
            let now = self.physics.borrow_mut().assembly_list();
            let mut as_: Vec<J> = vec![];
            for (r, d) in &now {
                if self.asm_sent.get(r) != Some(d) {
                    as_.push(json!([r, d]));
                }
            }
            let ad: Vec<InstId> = self.asm_sent.keys().copied().filter(|r| !now.contains_key(r)).collect();
            self.asm_sent = now.into_iter().collect();
            self.asm_diff = if as_.is_empty() && ad.is_empty() { None } else { Some((as_, ad)) };
        } else {
            self.asm_diff = None;
        }
        if self.frame % 2 == 0 {
            let dm = self.dm.borrow();
            self.ph_cache = self.physics.borrow_mut().collect_phys(&dm);
        }
    }

    fn phys_for(&mut self, player: InstId, ph: Vec<[f64; 16]>) -> Vec<[f64; 16]> {
        const FAR: f64 = 200.0;
        let focus = self.chars.values().find(|c| c.player == player && !self.dead.contains_key(&c.model)).map(|c| c.cf);
        let far = self.ph_far.entry(player).or_default();
        let pos = match focus {
            Some(p) => p,
            None => {
                if far.is_empty() {
                    return ph;
                }
                let mut all: Vec<[f64; 16]> = far.values().copied().collect();
                far.clear();
                all.extend(ph);
                return all;
            }
        };
        let mut out = vec![];
        for r in ph {
            let (dx, dy, dz) = (r[1] - pos[0], r[2] - pos[1], r[3] - pos[2]);
            if dx * dx + dy * dy + dz * dz > FAR * FAR {
                far.insert(r[0] as InstId, r);
            } else {
                far.remove(&(r[0] as InstId));
                out.push(r);
            }
        }
        let n = self.ph_n.entry(player).or_insert(0);
        *n += 1;
        if !far.is_empty() && *n % 4 == 0 {
            out.extend(far.values().copied());
            far.clear();
        }
        out
    }

    pub fn flush_all(&mut self, players: &[InstId]) -> Vec<(InstId, String)> {
        let journal = {
            let mut dm = self.dm.borrow_mut();
            std::mem::take(&mut dm.journal)
        };
        let dirty: Vec<InstId> = self.dirty.drain().collect();
        let mut out = vec![];
        for &p in players {
            let from = self.joined_at.remove(&p).unwrap_or(0).min(journal.len());
            let ops: Vec<J> = if journal.len() <= from {
                vec![]
            } else {
                let dm = self.dm.borrow();
                codec::ops_for(&dm, &self.roots, &journal[from..], Some(p))
            };
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
                    if let Some(j) = c.j {
                        rec.extend(j.iter().map(|x| json!((x * 1000.0).round() / 1000.0)));
                    }
                    J::Array(rec)
                })
                .collect();
            let mut ch = ch;
            ch.extend(self.npc_records());
            if !ch.is_empty() {
                msg.insert("ch".into(), J::Array(ch));
            }
            if self.frame % 2 == 0 {
                let ph = self.phys_for(p, self.ph_cache.clone());
                if !ph.is_empty() {
                    msg.insert("ph".into(), J::Array(ph.iter().map(|r| J::Array(r.iter().map(|x| json!(x)).collect())).collect()));
                }
            }
            if let Some((as_, ad)) = &self.asm_diff {
                if !as_.is_empty() {
                    msg.insert("as".into(), J::Array(as_.clone()));
                }
                if !ad.is_empty() {
                    msg.insert("ad".into(), json!(ad));
                }
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
        let ops = codec::ops_for(&dm, &self.roots, &journal, None);
        if ops.is_empty() {
            return None;
        }
        Some(json!({ "t": "rep", "o": ops }).to_string())
    }
}

#[allow(dead_code)]
fn unused(_: Change) {}

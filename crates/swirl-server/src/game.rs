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
use crate::touch::Box3;

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
    pub(crate) last_sit: HashMap<InstId, f64>,
    pub(crate) avatars: HashMap<InstId, (bool, [f64; 4])>,
    pub(crate) friend_retry: Vec<(f64, InstId)>,
    pub(crate) board_ctl: HashMap<InstId, (i64, HashMap<i64, bool>)>,
    pub(crate) npc_scan: usize,
}

impl Game {
    pub fn load(place_id: u64, bytes: &[u8], era: i32) -> Result<Game, String> {
        Game::load_with(place_id, bytes, era, "/var/www/assets", "/tmp/swirl-data")
    }

    pub fn load_with(place_id: u64, bytes: &[u8], era: i32, assets_dir: &str, data_dir: &str) -> Result<Game, String> {
        Game::load_opts(place_id, bytes, era, assets_dir, data_dir, None)
    }

    pub fn load_opts(place_id: u64, bytes: &[u8], era: i32, assets_dir: &str, data_dir: &str, test: Option<J>) -> Result<Game, String> {
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
        physics.union_enabled = true;
        physics.attach_workspace(&mut dm, ws);
        if let Some(t) = dm.find_child_of_class(ws, "Terrain") {
            let mut td = swirl_physics::terrain::TerrainData::new(era);
            if let Some(Val::Str(raw)) = dm.get(t).and_then(|i| i.props.get("ChocolateTerrain")) {
                let _ = td.decode(raw);
            }
            td.take_dirty();
            physics.set_terrain(t, td);
        }
        dm.journal.clear();
        physics.reset_cursor();
        let physics = Rc::new(RefCell::new(physics));
        let dm = Rc::new(RefCell::new(dm));
        let mut host = Host::new(dm.clone()).map_err(|e| e.to_string())?;
        let api = crate::api::new_state(assets_dir, data_dir, place_id, era);
        host.set_handler(Box::new(crate::api::Api(api.clone(), physics.clone())));
        host.add_methods(crate::api::METHODS);
        if let Some(init) = &test {
            crate::debugger::DebugAgent::attach(&mut host)?;
            if !init.is_null() {
                crate::debugger::DebugAgent::handle(&mut host, &json!({"cmd": "bps", "bps": init["bps"]}));
                crate::debugger::DebugAgent::handle(&mut host, &json!({"cmd": "watches", "watches": init["watches"]}));
            }
        }
        host.enable_linked_source();
        {
            let mut a = api.borrow_mut();
            for (name, id) in crate::instance::LIBRARIES {
                a.libraries.insert(name.to_string(), *id);
            }
            a.lib_src = crate::instance::libraries_disk(assets_dir);
        }
        host.scan_scripts();
        if let Some(b) = bundle.as_ref() {
            let started = b.server_starter(&mut dm.borrow_mut());
            if let Some(id) = started {
                host.scan_subtree(id);
            }
        }
        let mut g = Game { place_id, era, dm, roots, host, next_user: 1, t: 0.0, chars: HashMap::new(), pending: HashMap::new(), dirty: HashSet::new(), touching: HashSet::new(), dead: HashMap::new(), debris: vec![], spawn_cache: vec![], spawn_refresh: 0.0, mice: HashMap::new(), loose: vec![], loose_refresh: 0.0, joined_at: HashMap::new(), looks: HashMap::new(), gui_copied: HashSet::new(), invokes: HashMap::new(), invoke_seq: 1, remote_out: vec![], anim_state: HashMap::new(), last_chat: HashMap::new(), api, job: String::new(), chat_log: vec![], last_report: HashMap::new(), mkt_ids: HashMap::new(), bought: HashMap::new(), receipt_busy: HashSet::new(), receipt_queue: HashMap::new(), receipt_wait: HashMap::new(), receipt_seq: 0, receipt_retry: vec![], mkt_window: HashMap::new(), physics, npcs: crate::npc::Npcs::new(), frame: 0, ph_cache: vec![], asm_sent: HashMap::new(), asm_seen: 0, asm_diff: None, ph_far: HashMap::new(), ph_n: HashMap::new(), flush_at: 0.0, allow_gear: true, pre_logs: vec![], kicks: vec![], last_sit: HashMap::new(), avatars: HashMap::new(), friend_retry: vec![], board_ctl: HashMap::new(), npc_scan: 0 };
        g.scan_npcs_all();
        for _ in 0..3 {
            g.host.tick(0.0);
        }
        Ok(g)
    }

    fn terrain_init(&self) -> J {
        match self.physics.borrow().terrain.as_ref() {
            Some(t) if !t.chunks.is_empty() => J::String(t.encode(None)),
            _ => J::Null,
        }
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
        let sc = |k: &str| user.scale.get(k).and_then(|v| v.as_f64()).unwrap_or(1.0);
        self.avatars.insert(player, (user.r15 && self.era == 2018, [sc("width"), sc("height"), sc("depth"), sc("head")]));
        if self.era == 2018 {
            self.receipt_retry.push((self.t + 8.0, player));
        }
        for ((_, _), m) in self.anim_state.iter() {
            self.remote_out.push((vec![player], m.clone()));
        }
        let players = self.dm.borrow().services["Players"];
        self.host.fire(players, "PlayerAdded", vec![swirl_lua::host::Wire::I(player)]);
        self.spawn_character(player, name);
        self.friend_load(player, false);
        let msg = json!({
            "t": "init", "place": self.place_id, "era": self.era, "you": player,
            "user": { "id": uid, "name": name }, "snap": snap, "asm": [], "anims": [], "terrain": self.terrain_init()
        });
        (msg, player)
    }

    pub fn friend_load(&mut self, me: InstId, retried: bool) {
        let (uid, others) = {
            let dm = self.dm.borrow();
            let ps = dm.services.get("Players").and_then(|p| dm.get(*p)).map(|p| p.children.clone()).unwrap_or_default();
            let others: Vec<i64> = ps.into_iter().filter(|p| *p != me).map(|p| crate::api::ApiState::user_of(&dm, p)).collect();
            (crate::api::ApiState::user_of(&dm, me), others)
        };
        if others.is_empty() || uid == 0 {
            return;
        }
        let mut a = self.api.borrow_mut();
        a.token += 1;
        let token = a.token;
        a.social.push(crate::api::SocialReq { token, body: json!({"action": "statuses", "userId": uid, "others": others}), kind: crate::api::SocialKind::FriendLoad(me, retried) });
    }

    fn set_friend_status(&mut self, a: InstId, b: InstId, st: i64) {
        let (ua, ub) = {
            let dm = self.dm.borrow();
            (crate::api::ApiState::user_of(&dm, a), crate::api::ApiState::user_of(&dm, b))
        };
        let rev = if st == 3 { 4 } else if st == 4 { 3 } else { st };
        {
            let mut ap = self.api.borrow_mut();
            ap.friends.insert((ua, ub), st);
            ap.friends.insert((ub, ua), rev);
        }
        self.remote_out.push((vec![a], json!({"t": "fs", "s": {b.to_string(): st}})));
        self.remote_out.push((vec![b], json!({"t": "fs", "s": {a.to_string(): rev}})));
    }

    pub fn spawn_character(&mut self, player: InstId, name: &str) {
        let mut spawned: Option<InstId> = None;
        {
            let mut dm = self.dm.borrow_mut();
            let ws = dm.services["Workspace"];
            let (origin, spawn_loc) = character::spawn_origin(&dm, ws, player);
            let look = self.looks.get(&player);
            let (is_r15, sc) = self.avatars.get(&player).copied().unwrap_or((false, [1.0; 4]));
            let tpl = if is_r15 { character::chocolate_dir().and_then(|d| std::fs::read(format!("{}/character/characterR15V2.rbxm", d)).ok()) } else { None };
            let r15 = tpl.map(|t| (sc, t));
            let is_r15 = r15.is_some();
            match character::build(&mut dm, ws, player, name, origin, look, self.era, r15) {
                Ok((ch, anim)) => {
                    if let Some(sp) = spawn_loc {
                        if let Some(Val::Num(d)) = dm.get_prop(sp, "Duration").cloned() {
                            if d > 0.0 {
                                if let Ok(ff) = dm.create_any("ForceField", Some(ch.model)) {
                                    dm.set_prop_quiet(ff, "Name", Val::Str("ForceField".into()));
                                    dm.get_mut_name(ff, "ForceField");
                                    self.debris.push((self.t + d, ff));
                                }
                            }
                        }
                    }
                    if is_r15 {
                        character::add_scripts_r15(&mut dm, ch.model);
                        if let Some(animate) = dm.find_child(ch.model, "Animate", false) {
                            for (slot, sv) in anim {
                                let target = match dm.find_child(animate, &slot, false) {
                                    Some(t) => t,
                                    None => continue,
                                };
                                let kids: Vec<InstId> = dm.get(sv).map(|i| i.children.clone()).unwrap_or_default();
                                if kids.is_empty() {
                                    continue;
                                }
                                let old: Vec<InstId> = dm.get(target).map(|i| i.children.clone()).unwrap_or_default();
                                for o in old {
                                    dm.destroy(o);
                                }
                                for k in kids {
                                    let _ = dm.set_parent(k, Some(target));
                                }
                            }
                        }
                    } else {
                        character::add_scripts(&mut dm, ch.model, self.era);
                    }
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
            self.track_char(m, Some(player));
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
        let shared = self.api.borrow().mice.get(&player).copied();
        if let Some(m) = shared {
            if self.dm.borrow().get(m).map(|i| !i.destroyed).unwrap_or(false) {
                self.mice.insert(player, m);
                return m;
            }
        }
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
        self.api.borrow_mut().mice.insert(player, m);
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
        if let Some(model) = self.char_of(player) {
            self.equip_in(Some(player), model, tool);
        }
    }

    pub fn unequip(&mut self, player: InstId, tool: InstId) {
        if let Some(model) = self.char_of(player) {
            self.unequip_in(Some(player), model, tool);
        }
    }

    pub fn equip_in(&mut self, player: Option<InstId>, model: InstId, tool: InstId) {
        let others: Vec<InstId> = {
            let dm = self.dm.borrow();
            dm.get(model).map(|m| m.children.iter().copied().filter(|c| *c != tool && dm.get(*c).map(|i| i.class == "Tool").unwrap_or(false)).collect()).unwrap_or_default()
        };
        for o in others {
            self.unequip_in(player, model, o);
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
            let arm = character::arm_of(&dm, model).or_else(|| dm.find_child(model, "RightHand", false));
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
                    let c0 = if dm.name(a) == "RightHand" {
                        match dm.find_child(a, "RightGripAttachment", false).and_then(|g| dm.get_prop(g, "CFrame").cloned()) {
                            Some(Val::Arr(v)) if v.len() == 12 => v,
                            _ => swirl_physics::cf::angles(-std::f64::consts::FRAC_PI_2, 0.0, 0.0).iter().enumerate().map(|(i, x)| if i == 1 { -0.15 } else { *x }).collect(),
                        }
                    } else {
                        character::RIGHT_GRIP_C0.to_vec()
                    };
                    dm.set_prop_quiet(w, "C0", Val::Arr(c0));
                    let grip = character::cf_prop(&dm, tool, "Grip");
                    dm.set_prop_quiet(w, "C1", Val::Arr(grip.to_vec()));
                    let _ = dm.set_parent(w, Some(a));
                    Some((w, a))
                }
                _ => None,
            }
        };
        if let Some((w, a)) = weld {
            self.host.notify_parent(w, None, Some(a));
        }
        self.dirty.insert(model);
        if let Some(n) = self.npcs.get_mut(&model) {
            n.dirty = true;
        }
        let mouse = player.map(|p| Wire::I(self.mouse_of(p))).unwrap_or(Wire::Nil);
        self.host.fire(tool, "Equipped", vec![mouse]);
    }

    pub fn unequip_in(&mut self, player: Option<InstId>, model: InstId, tool: InstId) {
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
            player.and_then(|p| dm.find_child_of_class(p, "Backpack"))
        };
        self.host.fire(tool, "Unequipped", vec![]);
        if let Some(b) = bp {
            let _ = self.dm.borrow_mut().set_parent(tool, Some(b));
            self.host.notify_parent(tool, Some(model), Some(b));
        } else if player.is_some() {
            self.dm.borrow_mut().destroy(tool);
        } else {
            let _ = self.dm.borrow_mut().set_parent(tool, None);
            self.host.notify_parent(tool, Some(model), None);
        }
        self.dirty.insert(model);
        if let Some(n) = self.npcs.get_mut(&model) {
            n.dirty = true;
        }
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
            if let (Some(h), Some(c)) = (dm.find_child(tool, "Handle", false), self.npcs.get(&model).map(|n| n.torso_cf)) {
                let f = [c[0] - c[5] * 3.0, c[1] + 2.0, c[2] - c[11] * 3.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
                dm.set_prop_quiet(h, "CFrame", Val::Arr(f.to_vec()));
            }
            dm.services["Workspace"]
        };
        if let Some(n) = self.npcs.get_mut(&model) {
            n.dirty = true;
        }
        self.host.fire(tool, "Unequipped", vec![]);
        let _ = self.dm.borrow_mut().set_parent(tool, Some(ws));
        self.host.notify_parent(tool, Some(model), Some(ws));
        self.dirty.insert(model);
    }

    pub fn debug_message(&mut self, msg: &J) {
        crate::debugger::DebugAgent::handle(&mut self.host, msg);
    }

    pub fn take_debug(&mut self) -> Vec<J> {
        crate::debugger::DebugAgent::drain(&mut self.host)
    }

    pub fn union_requests(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.physics.borrow_mut().union_wanted)
    }

    pub fn union_done(&mut self, id: u64, bytes: Option<Vec<u8>>) {
        let mesh = bytes.and_then(|b| swirl_physics::csg::mesh_data_from_asset(&b));
        let dm = self.dm.borrow();
        self.physics.borrow_mut().provide_union(&dm, id, mesh);
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
            SocialKind::FriendLoad(me, retried) => {
                let map = r.as_ref().and_then(|j| j["statuses"].as_object().cloned());
                match map {
                    None => {
                        if !retried {
                            self.friend_retry.push((self.t + 2.0, me));
                        }
                    }
                    Some(m) => {
                        let alive = self.dm.borrow().get(me).map(|i| !i.destroyed).unwrap_or(false);
                        if alive {
                            let ps = {
                                let dm = self.dm.borrow();
                                dm.services.get("Players").and_then(|p| dm.get(*p)).map(|p| p.children.clone()).unwrap_or_default()
                            };
                            for o in ps {
                                let uid = crate::api::ApiState::user_of(&self.dm.borrow(), o);
                                if let Some(st) = m.get(&uid.to_string()).and_then(|v| v.as_i64()) {
                                    if st != 0 && o != me {
                                        self.set_friend_status(me, o, st);
                                    }
                                }
                            }
                        }
                    }
                }
                return;
            }
            SocialKind::FriendChange(a, b) => {
                let alive = |i: InstId| self.dm.borrow().get(i).map(|x| !x.destroyed).unwrap_or(false);
                if let Some(j) = r {
                    if alive(a) && alive(b) {
                        let st = j["status"].as_i64().filter(|v| *v != 0).unwrap_or(1);
                        self.set_friend_status(a, b, st);
                        if let Some(e) = j["event"].as_i64() {
                            self.remote_out.push((vec![a, b], json!({"t": "fre", "a": a, "b": b, "e": e})));
                        }
                    }
                }
                return;
            }
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
            self.npcs.shift_remove(&m);
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
            "jump" => self.jump_message(player),
            "drive" => self.drive_message(player, &v),
            "click" | "hover" => {
                let cd = match tool {
                    Some(c) => c,
                    None => return,
                };
                let ok = self.dm.borrow().get(cd).map(|i| !i.destroyed && swirl_core::classes::is_a(&i.class, "ClickDetector")).unwrap_or(false);
                if !ok {
                    return;
                }
                if v["t"] == "hover" {
                    let ev = if v["on"].as_bool().unwrap_or(false) || v["on"].as_i64().unwrap_or(0) != 0 { "MouseHoverEnter" } else { "MouseHoverLeave" };
                    self.host.fire(cd, ev, vec![Wire::I(player)]);
                    return;
                }
                let model = match self.char_of(player) {
                    Some(m) => m,
                    None => return,
                };
                let near = {
                    let dm = self.dm.borrow();
                    let ws = dm.services["Workspace"];
                    let part = dm.get(cd).and_then(|i| i.parent);
                    let pp = part.and_then(|p| if dm.is_a(p, "BasePart") { Some(p) } else { dm.get(p).and_then(|i| i.children.iter().copied().find(|c| dm.is_a(*c, "BasePart"))) });
                    match (pp, self.npcs.get(&model)) {
                        (Some(pp), Some(n)) if dm.is_descendant_of(cd, ws) => {
                            let pcf = match dm.get_prop(pp, "CFrame") {
                                Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
                                _ => [0.0; 3],
                            };
                            let sz = match dm.get_prop(pp, "Size") {
                                Some(Val::Arr(a)) if a.len() >= 3 => a[0].max(a[1]).max(a[2]),
                                _ => 0.0,
                            };
                            let d = ((pcf[0] - n.torso_cf[0]).powi(2) + (pcf[1] - n.torso_cf[1]).powi(2) + (pcf[2] - n.torso_cf[2]).powi(2)).sqrt();
                            let max = match dm.get_prop(cd, "MaxActivationDistance") {
                                Some(Val::Num(m)) => *m,
                                _ => 32.0,
                            };
                            d <= max + 8.0 + sz
                        }
                        _ => false,
                    }
                };
                if near {
                    self.host.fire(cd, "MouseClick", vec![Wire::I(player)]);
                }
            }
            "key" => {
                let k: String = v["k"].as_str().unwrap_or("").chars().map(|c| if c.is_control() { ' ' } else { c }).take(8).collect::<String>().to_lowercase();
                if k.is_empty() {
                    return;
                }
                let mouse = self.mouse_of(player);
                let ev = if v["down"].as_bool().unwrap_or(false) || v["down"].as_i64().unwrap_or(0) != 0 { "KeyDown" } else { "KeyUp" };
                self.host.fire(mouse, ev, vec![Wire::S(k)]);
            }
            "hp" => {
                let model = match self.char_of(player) {
                    Some(m) => m,
                    None => return,
                };
                let hv = match v["v"].as_f64() {
                    Some(x) if x.is_finite() => x,
                    _ => return,
                };
                let hum = self.dm.borrow().find_child_of_class(model, "Humanoid");
                if let Some(h) = hum {
                    let cur = match self.dm.borrow().get_prop(h, "Health") {
                        Some(Val::Num(c)) => *c,
                        _ => 100.0,
                    };
                    if hv < cur {
                        let _ = self.dm.borrow_mut().set_prop(h, "Health", Val::Num(hv.max(0.0)));
                        self.host.fire(h, "Changed", vec![Wire::S("Health".into())]);
                        self.host.fire(h, "HealthChanged", vec![Wire::N(hv.max(0.0))]);
                    }
                }
            }
            "break" => {
                let model = match self.char_of(player) {
                    Some(m) => m,
                    None => return,
                };
                if let Some(part) = tool {
                    let ok = {
                        let dm = self.dm.borrow();
                        dm.get(part).map(|i| !i.destroyed).unwrap_or(false) && dm.is_a(part, "BasePart") && dm.is_descendant_of(part, model)
                    };
                    if ok {
                        let mut dm = self.dm.borrow_mut();
                        self.physics.borrow_mut().break_joints(&mut dm, part);
                        drop(dm);
                        if let Some(n) = self.npcs.get_mut(&model) {
                            n.dirty = true;
                        }
                    }
                }
            }
            "rcpt" => {
                self.queue_receipts(player);
            }
            "unimpl" => {}
            "friend" => {
                let other = v["id"].as_u64().map(|x| x as InstId);
                if let Some(o) = other {
                    let dm = self.dm.borrow();
                    self.api.borrow_mut().change_friendship(&dm, player, o, v["on"].as_bool().unwrap_or(false) || v["on"].as_i64().unwrap_or(0) != 0);
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
            "rfr" if self.era == 2018 => {
                let k = v["k"].as_u64().unwrap_or(0) as u32;
                let mut a = self.api.borrow_mut();
                if a.invoke_out.get(&k).map(|(p, _)| *p) == Some(player) {
                    a.invoke_out.remove(&k);
                    let args = match v["a"].as_array() {
                        Some(x) if x.len() <= 20000 => wire_from_json(x),
                        _ => vec![],
                    };
                    a.completions.push((k, Ok(args)));
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
                let msg: String = raw.chars().map(|c| if (c as u32) < 0x20 || c as u32 == 0x7f { ' ' } else { c }).take(200).collect::<String>().trim().to_string();
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
                let kind = if v["k"] == "team" { "team" } else if v["k"] == "whisper" { "whisper" } else { "all" };
                let whisper_to: Option<InstId> = if kind == "whisper" {
                    let t = v["to"].as_u64().map(|t| t as InstId);
                    let dm = self.dm.borrow();
                    t.filter(|t| dm.get(*t).map(|i| i.class == "Player" && !i.destroyed).unwrap_or(false))
                } else {
                    None
                };
                if kind == "whisper" && whisper_to.is_none() {
                    return;
                }
                let head: Option<InstId> = {
                    let dm = self.dm.borrow();
                    match dm.get_prop(player, "Character") {
                        Some(Val::Ref(Some(m))) => dm.find_child(*m, "Head", false),
                        _ => None,
                    }
                };
                let targets: Vec<InstId> = if kind == "whisper" {
                    vec![player, whisper_to.unwrap()]
                } else if kind == "team" {
                    let dm = self.dm.borrow();
                    players.into_iter().filter(|o| !neutral && !matches!(dm.get_prop(*o, "Neutral"), Some(Val::Bool(true))) && dm.get_prop(*o, "TeamColor").cloned() == team).collect()
                } else {
                    vec![]
                };
                self.chat_log.push(format!("{}: {}", name, msg).chars().take(200).collect());
                if self.chat_log.len() > 40 {
                    self.chat_log.remove(0);
                }
                self.host.fire(player, "Chatted", vec![Wire::S(msg.clone()), whisper_to.map(Wire::I).unwrap_or(Wire::Nil)]);
                let mut out = json!({"t": "chat", "p": player, "n": name, "u": uid, "m": msg, "k": kind, "h": head});
                if let Some(t) = whisper_to {
                    out["to"] = json!(t);
                }
                self.remote_out.push((targets, out));
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
                if let Some(x) = v["x"].as_i64() {
                    dm.set_prop_quiet(mouse, "X", Val::Num(x as f64));
                }
                if let Some(y) = v["y"].as_i64() {
                    dm.set_prop_quiet(mouse, "Y", Val::Num(y as f64));
                }
                drop(dm);
                if let Some(b) = v["b"].as_str() {
                    let ev = match b {
                        "down" => "Button1Down",
                        "up" => "Button1Up",
                        "move" => "Move",
                        _ => "Idle",
                    };
                    self.host.fire(mouse, ev, vec![]);
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
            let ang = arr("j", 4).map(|a| [a[0], a[1], a[2], a[3]]);
            self.char_message(player, cf, [vel[0], vel[1], vel[2]], seq, state, ang);
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
                self.npcs.shift_remove(&m);
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
            let fdue: Vec<InstId> = self.friend_retry.iter().filter(|(t, _)| *t <= self.t).map(|(_, p)| *p).collect();
            self.friend_retry.retain(|(t, _)| *t > self.t);
            for p in fdue {
                if self.dm.borrow().get(p).map(|i| !i.destroyed).unwrap_or(false) {
                    self.friend_load(p, true);
                }
            }
            let due: Vec<InstId> = self.receipt_retry.iter().filter(|(t, _)| *t <= self.t).map(|(_, p)| *p).collect();
            self.receipt_retry.retain(|(t, _)| *t > self.t);
            for p in due {
                self.queue_receipts(p);
            }
        }
        let tkeys = self.physics.borrow_mut().terrain_flush();
        if !tkeys.is_empty() {
            let d = self.physics.borrow().terrain.as_ref().map(|t| t.encode(Some(&tkeys))).unwrap_or_default();
            self.remote_out.push((vec![], json!({"t": "terrain", "d": d})));
        }
        for r in self.host.take_requests() {
            match r {
                Req::LoadCharacter(p) => self.respawn(p),
                Req::Debris(i, s) => self.debris.push((self.t + s, i)),
                Req::EquipTool(h, t) => {
                    let model = self.dm.borrow().get(h).and_then(|i| i.parent);
                    if let Some(m) = model {
                        let owner = self.npcs.get(&m).map(|n| n.owner);
                        match owner {
                            Some(Some(p)) => {
                                if self.tool_owned(p, t) {
                                    self.equip(p, t);
                                }
                            }
                            Some(None) => self.equip_in(None, m, t),
                            None => {}
                        }
                    }
                }
                Req::UnequipTools(h) => {
                    let model = self.dm.borrow().get(h).and_then(|i| i.parent);
                    if let Some(m) = model {
                        let owner = self.npcs.get(&m).map(|n| n.owner);
                        if let Some(owner) = owner {
                            let tools: Vec<InstId> = {
                                let dm = self.dm.borrow();
                                dm.get(m).map(|i| i.children.iter().copied().filter(|c| dm.get(*c).map(|x| x.class == "Tool").unwrap_or(false)).collect()).unwrap_or_default()
                            };
                            for t in tools {
                                self.unequip_in(owner, m, t);
                            }
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
        let mut respawn: Vec<InstId> = vec![];
        for (m, c) in self.chars.iter() {
            if let Some(at) = self.dead.get(m) {
                if *at <= self.t {
                    respawn.push(c.player);
                }
            }
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
        self.handle_explosions();
        let impulses = std::mem::take(&mut self.api.borrow_mut().board_impulses);
        for (b, v) in impulses {
            self.physics.borrow_mut().board_impulse(b, v);
        }
        {
            let mut dm = self.dm.borrow_mut();
            self.physics.borrow_mut().step(&mut dm, 1.0 / 30.0);
        }
        self.board_state_events();
        let touches = std::mem::take(&mut self.physics.borrow_mut().touches);
        for t in touches {
            let alive = {
                let dm = self.dm.borrow();
                dm.get(t.a).map(|i| !i.destroyed).unwrap_or(false) && dm.get(t.b).map(|i| !i.destroyed).unwrap_or(false)
            };
            if !alive {
                continue;
            }
            let (ca, cb) = {
                let dm = self.dm.borrow();
                let ph = self.physics.borrow();
                (swirl_physics::humanoid::live_character_of(&ph, &dm, t.a), swirl_physics::humanoid::live_character_of(&ph, &dm, t.b))
            };
            if ca.is_some() && ca == cb {
                continue;
            }
            let ev = if t.started { "Touched" } else { "TouchEnded" };
            self.host.fire(t.a, ev, vec![Wire::I(t.b)]);
            self.host.fire(t.b, ev, vec![Wire::I(t.a)]);
            if t.started {
                self.handle_touch_started(t.a, t.b, ca, cb);
            }
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
        let focus = self.npcs.values().find(|n| n.owner == Some(player) && !n.dead).map(|n| n.torso_cf);
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
        self.scan_npcs_journal();
        let journal = {
            let mut dm = self.dm.borrow_mut();
            self.physics.borrow_mut().sync(&mut dm);
            let taken = std::mem::take(&mut dm.journal);
            self.physics.borrow_mut().reset_cursor();
            self.npc_scan = 0;
            taken
        };
        self.dirty.clear();
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
            let ch: Vec<J> = self.npc_records(Some(p));
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

use crate::codec;
use serde_json::{json, Value as J};
use swirl_core::classes;
use swirl_core::instance::{DataModel, InstId, Val};

pub struct Game {
    pub place_id: u64,
    pub era: i32,
    pub dm: DataModel,
    pub roots: Vec<InstId>,
    next_user: i64,
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
        Ok(Game { place_id, era, dm, roots, next_user: 1 })
    }

    pub fn join(&mut self, name: &str) -> (J, InstId) {
        let uid = self.next_user;
        self.next_user += 1;
        let players = self.dm.service("Players").unwrap();
        let player = self.dm.create_any("Player", None).unwrap();
        let _ = self.dm.set_prop(player, "Name", Val::Str(name.to_string()));
        let _ = self.dm.set_prop(player, "userId", Val::Num(uid as f64));
        let _ = self.dm.set_prop(player, "UserId", Val::Num(uid as f64));
        let _ = self.dm.set_prop(player, "CharacterAppearance", Val::Str(format!("/api/charapp/{}", uid)));
        let _ = self.dm.set_prop(player, "AccountAge", Val::Num(100.0));
        let _ = self.dm.set_prop(player, "DataReady", Val::Bool(false));
        let _ = self.dm.set_prop(player, "DataComplexityLimit", Val::Num(45000.0));
        for (cls, nm) in [("Backpack", "Backpack"), ("PlayerGui", "PlayerGui")] {
            if let Ok(c) = self.dm.create_any(cls, Some(player)) {
                let _ = self.dm.set_prop(c, "Name", Val::Str(nm.to_string()));
            }
        }
        self.dm.set_parent(player, Some(players)).unwrap();
        let snap = codec::snapshot_for(&self.dm, &self.roots, player);
        let msg = json!({
            "t": "init", "place": self.place_id, "era": self.era, "you": player,
            "user": { "id": uid, "name": name }, "snap": snap, "asm": [], "anims": [], "terrain": J::Null
        });
        (msg, player)
    }

    pub fn leave(&mut self, player: InstId) {
        self.dm.destroy(player);
    }
}

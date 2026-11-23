use crate::game::Game;
use indexmap::IndexMap;
use serde_json::{json, Value as J};
use swirl_core::instance::{Change, DataModel, InstId, Val};
use swirl_lua::host::Wire;
use swirl_physics::cf::{self, CF};
use swirl_physics::humanoid::{self, CharacterProxy, Controller, Input, Rig};

const HUM_STATE: [f64; 9] = [8.0, 8.0, 3.0, 5.0, 13.0, 12.0, 15.0, 14.0, 4.0];

pub struct MoveTo {
    pub pos: Option<[f64; 3]>,
    pub part: Option<InstId>,
    pub t: f64,
}

pub struct Seat {
    pub seat: InstId,
    pub weld: InstId,
}

pub struct Npc {
    pub owner: Option<InstId>,
    pub seat: Option<Seat>,
    pub net_angles: Option<[f64; 4]>,
    pub tp_seq: i64,
    pub move_budget: f64,
    pub last_update: f64,
    pub fall_speed: f64,
    pub model: InstId,
    pub hum: InstId,
    pub rig: Rig,
    pub proxy: Option<CharacterProxy>,
    pub ctrl: Option<Controller>,
    pub torso_cf: CF,
    pub vel: [f64; 3],
    pub state: u8,
    pub dead: bool,
    pub dirty: bool,
    pub move_to: Option<MoveTo>,
    pub last_running: Option<i64>,
    pub last_swim: Option<i64>,
    pub last_step_cf: Option<CF>,
}

pub type Npcs = IndexMap<InstId, Npc>;

impl Game {
    pub fn scan_npcs_all(&mut self) {
        let ids: Vec<InstId> = {
            let dm = self.dm.borrow();
            let ws = dm.services["Workspace"];
            let mut all = vec![];
            dm.descendants(ws, &mut all);
            all.into_iter().filter(|i| dm.get(*i).map(|x| x.class == "Model").unwrap_or(false)).collect()
        };
        for m in ids {
            self.track_npc(m);
        }
    }

    fn model_of(&self, dm: &DataModel, id: InstId) -> Option<InstId> {
        let mut cur = Some(id);
        let mut guard = 0;
        while let Some(c) = cur {
            if self.npcs.contains_key(&c) {
                return Some(c);
            }
            cur = dm.get(c).and_then(|i| i.parent);
            guard += 1;
            if guard > 200 {
                break;
            }
        }
        None
    }

    fn scan_char_changes(&mut self) {
        if self.npcs.is_empty() {
            return;
        }
        let mut tele: Vec<(InstId, InstId)> = vec![];
        let mut grips: Vec<InstId> = vec![];
        let mut dirty: Vec<InstId> = vec![];
        {
            let dm = self.dm.borrow();
            for ch in dm.journal.iter() {
                match ch {
                    Change::Parent { id, .. } => {
                        if let Some(m) = self.model_of(&dm, *id) {
                            dirty.push(m);
                        }
                        let mut inner = vec![];
                        dm.descendants(*id, &mut inner);
                        for m in self.npcs.keys() {
                            if inner.contains(m) {
                                dirty.push(*m);
                            }
                        }
                    }
                    Change::Prop(id, name) => {
                        let i = match dm.get(*id) {
                            Some(i) => i,
                            None => continue,
                        };
                        if (name == "Part0" || name == "Part1") && dm.is_a(*id, "JointInstance") {
                            if let Some(m) = self.model_of(&dm, *id) {
                                dirty.push(m);
                            }
                        } else if name == "CFrame" && dm.is_a(*id, "BasePart") {
                            if let Some(m) = self.model_of(&dm, *id) {
                                tele.push((m, *id));
                            }
                        } else if name == "Grip" && i.class == "Tool" {
                            grips.push(*id);
                        }
                    }
                    _ => {}
                }
            }
        }
        for m in dirty {
            if let Some(n) = self.npcs.get_mut(&m) {
                n.dirty = true;
            }
        }
        for tool in grips {
            let mut dm = self.dm.borrow_mut();
            let model = match dm.get(tool).and_then(|i| i.parent) {
                Some(m) => m,
                None => continue,
            };
            let handle = dm.find_child(tool, "Handle", false);
            let arm = dm.find_child(model, "Right Arm", false).or_else(|| dm.find_child(model, "RightHand", false));
            let w = arm.and_then(|a| dm.find_child(a, "RightGrip", false));
            if let (Some(w), Some(h)) = (w, handle) {
                if matches!(dm.get_prop(w, "Part1"), Some(Val::Ref(Some(p))) if *p == h) {
                    let g = dm.get_prop(tool, "Grip").cloned();
                    if let Some(g) = g {
                        let _ = dm.set_prop(w, "C1", g);
                    }
                }
            }
        }
        let mut done: Vec<InstId> = vec![];
        for (m, part) in tele {
            if done.contains(&m) {
                continue;
            }
            let (dead, owner, torso_cf, root) = {
                let dm = self.dm.borrow();
                let n = &self.npcs[&m];
                (n.dead, n.owner, n.torso_cf, humanoid::root_of(&dm, m))
            };
            if dead {
                continue;
            }
            let tcf = {
                let dm = self.dm.borrow();
                let pc = match dm.get_prop(part, "CFrame") {
                    Some(Val::Arr(a)) => cf::arr(a).unwrap_or(cf::IDENTITY),
                    _ => continue,
                };
                if Some(part) == root {
                    pc
                } else {
                    let n = &self.npcs[&m];
                    let poses = n.rig.pose(&self.physics.borrow(), &dm, &torso_cf);
                    match poses.get(&part) {
                        Some(cur) => cf::mul(&pc, &cf::mul(&cf::inv(cur), &torso_cf)),
                        None => continue,
                    }
                }
            };
            done.push(m);
            let tcf = cf::orthonormalize(&tcf);
            let n = self.npcs.get_mut(&m).unwrap();
            n.torso_cf = tcf;
            n.vel = [0.0; 3];
            if n.ctrl.is_some() {
                let ph = self.physics.clone();
                let dmc = self.dm.clone();
                let dm = dmc.borrow();
                n.ctrl.as_mut().unwrap().teleport(&mut ph.borrow_mut(), &dm, &tcf);
            }
            if let Some(pl) = owner {
                n.tp_seq += 1;
                let s = n.tp_seq;
                let pend = self.pending.entry(pl).or_default();
                pend.retain(|x| x["t"] != "tp");
                pend.push(json!({"t": "tp", "s": s, "c": tcf.iter().map(|v| (v * 1e4).round() / 1e4).collect::<Vec<f64>>()}));
            }
        }
    }

    pub fn scan_npcs_journal(&mut self) {
        self.scan_char_changes();
        let ids: Vec<InstId> = {
            let dm = self.dm.borrow();
            let mut out = vec![];
            for ch in dm.journal.iter() {
                if let Change::Parent { id, new: Some(_), .. } = ch {
                    let mut list = vec![*id];
                    dm.descendants(*id, &mut list);
                    for i in list {
                        if dm.get(i).map(|x| x.class == "Model" && !x.destroyed).unwrap_or(false) {
                            out.push(i);
                        }
                    }
                }
            }
            out
        };
        for m in ids {
            self.track_npc(m);
        }
    }

    pub fn track_npc(&mut self, model: InstId) {
        if self.chars.contains_key(&model) || self.physics_is_excluded(model) {
            return;
        }
        self.track_char(model, None);
    }

    pub fn track_char(&mut self, model: InstId, owner: Option<InstId>) {
        if self.npcs.contains_key(&model) {
            return;
        }
        let dm = self.dm.borrow();
        let ws = dm.services["Workspace"];
        let hum = match dm.find_child_of_class(model, "Humanoid") {
            Some(h) => h,
            None => return,
        };
        let torso = match humanoid::root_of(&dm, model) {
            Some(t) => t,
            None => return,
        };
        let head = match dm.find_child(model, "Head", false) {
            Some(h) => h,
            None => return,
        };
        let _ = head;
        if !dm.is_descendant_of(model, ws) {
            return;
        }
        if matches!(dm.get_prop(torso, "Anchored"), Some(Val::Bool(true))) {
            return;
        }
        if matches!(dm.get_prop(hum, "Health"), Some(Val::Num(h)) if *h <= 0.0) {
            return;
        }
        let rig = Rig::new(&dm, model);
        let torso_cf = match dm.get_prop(torso, "CFrame") {
            Some(Val::Arr(a)) => cf::arr(a).unwrap_or(cf::IDENTITY),
            _ => cf::IDENTITY,
        };
        drop(dm);
        let dm = self.dm.clone();
        let dmr = dm.borrow();
        let proxy = CharacterProxy::new(&mut *self.physics.borrow_mut(), &dmr, model);
        self.physics.borrow_mut().remove_char_parts(&dmr, model);
        drop(dmr);
        self.npcs.insert(model, Npc { owner, seat: None, net_angles: None, tp_seq: 1, move_budget: 6.0, last_update: self.t, fall_speed: 0.0, model, hum, rig, proxy: Some(proxy), ctrl: None, torso_cf, vel: [0.0; 3], state: humanoid::IDLE, dead: false, dirty: false, move_to: None, last_running: None, last_swim: None, last_step_cf: None });
    }

    fn physics_is_excluded(&self, model: InstId) -> bool {
        self.physics.borrow_mut().excluded.contains(&model)
    }

    pub fn set_move_to(&mut self, hum: InstId, pos: Option<[f64; 3]>, part: Option<InstId>) {
        let t = self.t;
        let model = self.dm.borrow().get(hum).and_then(|i| i.parent);
        if let Some(m) = model {
            if let Some(n) = self.npcs.get_mut(&m) {
                n.move_to = Some(MoveTo { pos, part, t });
            }
        }
    }

    pub fn npc_events_pub(&mut self, model: InstId, prev: u8) {
        self.npc_events(model, prev);
    }

    fn npc_events(&mut self, model: InstId, prev: u8) {
        let (hum, st, vel, last_running, last_swim) = {
            let n = &self.npcs[&model];
            (n.hum, n.state, n.vel, n.last_running, n.last_swim)
        };
        let alive = self.dm.borrow().get(hum).map(|i| !i.destroyed).unwrap_or(false);
        if !alive {
            return;
        }
        let ws = {
            let dm = self.dm.borrow();
            match dm.get_prop(hum, "WalkSpeed") {
                Some(Val::Num(n)) => *n,
                _ => 16.0,
            }
        };
        if st != prev {
            if prev == humanoid::JUMP {
                self.host.fire(hum, "Jumping", vec![Wire::B(false)]);
            }
            if prev == humanoid::FALL {
                self.host.fire(hum, "FreeFalling", vec![Wire::B(false)]);
            }
            if st == humanoid::JUMP {
                self.host.fire(hum, "Jumping", vec![Wire::B(true)]);
            }
            if st == humanoid::FALL {
                self.host.fire(hum, "FreeFalling", vec![Wire::B(true)]);
            }
            if st == humanoid::CLIMB {
                self.host.fire(hum, "Climbing", vec![Wire::N(ws * 0.7)]);
            }
            if self.era == 2018 {
                let en = |i: u8| Wire::E("HumanoidStateType".into(), HUM_STATE.get(i as usize).copied().unwrap_or(8.0));
                self.host.fire(hum, "StateChanged", vec![en(prev), en(st)]);
            }
        }
        let speed = if st == humanoid::WALK || st == humanoid::SWIM { (vel[0] * vel[0] + vel[2] * vel[2]).sqrt() } else { 0.0 };
        let running = if speed > 0.5 { speed.round() as i64 } else { 0 };
        if st == humanoid::SWIM {
            if Some(running) != last_swim || prev != st {
                self.npcs.get_mut(&model).unwrap().last_swim = Some(running);
                self.host.fire(hum, "Swimming", vec![Wire::N(running as f64)]);
            }
        } else {
            self.npcs.get_mut(&model).unwrap().last_swim = None;
        }
        if st == humanoid::SWIM {
            return;
        }
        if Some(running) != last_running {
            self.npcs.get_mut(&model).unwrap().last_running = Some(running);
            self.host.fire(hum, "Running", vec![Wire::N(running as f64)]);
        }
    }

    fn die_npc(&mut self, model: InstId) {
        let (hum, vel) = match self.npcs.get_mut(&model) {
            Some(n) if !n.dead => {
                n.dead = true;
                (n.hum, n.vel)
            }
            _ => return,
        };
        self.physics.borrow_mut().dead_chars.insert(model);
        self.unseat(model);
        let owner = self.npcs.get(&model).and_then(|n| n.owner);
        if let Some(pl) = owner {
            self.dead.insert(model, self.t + 5.0);
            self.physics.borrow_mut().include_model(model);
            let _ = pl;
        }
        {
            let mut dm = self.dm.borrow_mut();
            if matches!(dm.get_prop(hum, "Health"), Some(Val::Num(h)) if *h > 0.0) && dm.get(hum).map(|i| !i.destroyed).unwrap_or(false) {
                let _ = dm.set_prop(hum, "Health", Val::Num(0.0));
            }
        }
        self.drop_npc(model);
        let parts: Vec<InstId> = {
            let dm = self.dm.borrow();
            let mut all = vec![];
            dm.descendants(model, &mut all);
            all.into_iter().filter(|i| dm.is_a(*i, "BasePart")).collect()
        };
        {
            let mut dm = self.dm.borrow_mut();
            let mut seed = (self.t * 1000.0) as u64 ^ (model as u64).wrapping_mul(0x9e3779b97f4a7c15);
            let mut rnd = || -> [f64; 2] {
                let mut n = || {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    ((seed % 1000) as f64 / 1000.0 - 0.5) * 8.0
                };
                [n(), n()]
            };
            self.physics.borrow_mut().activate_parts(&mut dm, &parts, vel, &mut rnd);
        }
        self.host.fire(hum, "Died", vec![]);
        self.debris.push((self.t + 15.0, model));
    }

    pub fn drop_npc(&mut self, model: InstId) {
        if let Some(n) = self.npcs.get_mut(&model) {
            if let Some(mut p) = n.proxy.take() {
                p.destroy(&mut *self.physics.borrow_mut());
            }
            if let Some(mut c) = n.ctrl.take() {
                c.destroy(&mut *self.physics.borrow_mut());
            }
        }
    }

    pub fn update_npcs(&mut self, dt: f64) {
        let models: Vec<InstId> = self.npcs.keys().copied().collect();
        let (destroy_y, ws) = {
            let dm = self.dm.borrow();
            let ws = dm.services["Workspace"];
            (match dm.get_prop(ws, "FallenPartsDestroyHeight") {
                Some(Val::Num(n)) => *n,
                _ => -500.0,
            }, ws)
        };
        for model in models {
            let (hum, dead) = match self.npcs.get(&model) {
                Some(n) => (n.hum, n.dead),
                None => continue,
            };
            if dead {
                continue;
            }
            let (gone, health, neck_ok, root) = {
                let dm = self.dm.borrow();
                let gone = dm.get(model).map(|i| i.destroyed).unwrap_or(true) || dm.get(hum).map(|i| i.destroyed || i.parent != Some(model)).unwrap_or(true);
                let health = match dm.get_prop(hum, "Health") {
                    Some(Val::Num(h)) => *h,
                    _ => 100.0,
                };
                let n = &self.npcs[&model];
                let head = dm.find_child(model, "Head", false);
                let neck_ok = head.map(|h| n.rig.parts.contains(&h)).unwrap_or(false);
                (gone, health, neck_ok, humanoid::root_of(&dm, model))
            };
            if gone {
                self.die_npc(model);
                continue;
            }
            {
                let dm = self.dm.borrow();
                let n = self.npcs.get_mut(&model).unwrap();
                if !n.dirty {
                    n.dirty = n.rig.order.iter().any(|(_, j, _, _)| dm.get(*j).map(|i| i.destroyed).unwrap_or(true) || !dm.is_descendant_of(*j, model));
                }
                if n.dirty {
                    n.dirty = false;
                    n.rig.build(&dm);
                    if let Some(p) = n.proxy.as_mut() {
                        p.refresh(&mut *self.physics.borrow_mut(), &dm);
                    }
                }
            }
            let neck_ok = {
                let dm = self.dm.borrow();
                let n = &self.npcs[&model];
                let head = dm.find_child(model, "Head", false);
                head.map(|h| n.rig.parts.contains(&h)).unwrap_or(false) && neck_ok
            };
            if health <= 0.0 || !neck_ok || root.is_none() {
                self.die_npc(model);
                continue;
            }
            if self.npcs[&model].torso_cf[1] < destroy_y {
                {
                    let mut dm = self.dm.borrow_mut();
                    let _ = dm.set_prop(hum, "Health", Val::Num(0.0));
                }
                self.die_npc(model);
                continue;
            }
            let _ = ws;
            let mut mv = [0.0f64; 3];
            let mut finished: Option<bool> = None;
            {
                let dm = self.dm.borrow();
                let n = self.npcs.get_mut(&model).unwrap();
                if let (true, Some(m)) = (n.owner.is_none() && n.seat.is_none(), &n.move_to) {
                    let tp: Option<[f64; 3]> = match m.part {
                        Some(p) if dm.get(p).map(|i| !i.destroyed).unwrap_or(false) => match dm.get_prop(p, "CFrame") {
                            Some(Val::Arr(a)) if a.len() >= 3 => Some([a[0], a[1], a[2]]),
                            _ => None,
                        },
                        _ => m.pos,
                    };
                    if let Some(tp) = tp {
                        let d = [tp[0] - n.torso_cf[0], 0.0, tp[2] - n.torso_cf[2]];
                        let l = (d[0] * d[0] + d[2] * d[2]).sqrt();
                        if l < 1.5 {
                            n.move_to = None;
                            finished = Some(true);
                        } else if self.t - m.t > 8.0 {
                            n.move_to = None;
                            finished = Some(false);
                        } else {
                            mv = d;
                        }
                    }
                }
            }
            if let Some(ok) = finished {
                self.host.fire(hum, "MoveToFinished", vec![Wire::B(ok)]);
            }
            let is_npc = self.npcs[&model].owner.is_none() && self.npcs[&model].seat.is_none();
            let want_jump = is_npc && {
                let dm = self.dm.borrow();
                matches!(dm.get_prop(hum, "Jump"), Some(Val::Bool(true)))
            };
            if want_jump {
                let mut dm = self.dm.borrow_mut();
                let _ = dm.set_prop(hum, "Jump", Val::Bool(false));
            }
            let dmc = self.dm.clone();
            let mut dm = dmc.borrow_mut();
            let mut n = self.npcs.shift_remove(&model).unwrap();
            let prev = n.state;
            let seated = match &n.seat {
                Some(sx) => {
                    let gone = dm.get(sx.seat).map(|i| i.destroyed).unwrap_or(true) || dm.get(sx.weld).map(|i| i.destroyed).unwrap_or(true) || !matches!(dm.get_prop(sx.weld, "Part0"), Some(Val::Ref(Some(p))) if *p == sx.seat);
                    if gone {
                        Some(None)
                    } else {
                        let sc = cf::arr(match dm.get_prop(sx.seat, "CFrame") { Some(Val::Arr(a)) => a, _ => &[] }).unwrap_or(cf::IDENTITY);
                        let c0 = cf::arr(match dm.get_prop(sx.weld, "C0") { Some(Val::Arr(a)) => a, _ => &[] }).unwrap_or(cf::IDENTITY);
                        let c1 = cf::arr(match dm.get_prop(sx.weld, "C1") { Some(Val::Arr(a)) => a, _ => &[] }).unwrap_or(cf::IDENTITY);
                        let v = match dm.get_prop(sx.seat, "Velocity") { Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]], _ => [0.0; 3] };
                        Some(Some((cf::mul(&cf::mul(&sc, &c0), &cf::inv(&c1)), v)))
                    }
                }
                None => None,
            };
            let mut unseat_now = false;
            let mut healed: Option<f64> = None;
            if let Some(owner) = n.owner {
                let loadable = n.seat.is_none() && matches!(n.state, humanoid::IDLE | humanoid::WALK | humanoid::STAND);
                let _ = loadable;
                let h = match dm.get_prop(hum, "Health") { Some(Val::Num(h)) => *h, _ => 100.0 };
                let mh = match dm.get_prop(hum, "MaxHealth") { Some(Val::Num(h)) => *h, _ => 100.0 };
                if h < mh {
                    let nh = (h + mh * 0.01 * dt).min(mh);
                    let _ = dm.set_prop(hum, "Health", Val::Num(nh));
                    healed = Some(nh);
                }
                let _ = owner;
            }
            match seated {
                Some(None) => unseat_now = true,
                Some(Some((c, v))) => {
                    n.torso_cf = c;
                    n.state = humanoid::SIT;
                    n.vel = v;
                }
                None => {
                    if n.owner.is_none() {
                        if n.ctrl.is_none() {
                            n.ctrl = Some(Controller::new(&mut *self.physics.borrow_mut(), &dm, model, hum));
                        }
                        let r = n.ctrl.as_mut().unwrap().update(&mut *self.physics.borrow_mut(), &mut dm, dt, &Input { mv, jump: want_jump, face: None });
                        n.torso_cf = r.cf;
                        n.vel = r.vel;
                        n.state = r.state;
                    }
                }
            }
            let poses = n.rig.pose(&self.physics.borrow(), &dm, &n.torso_cf);
            for (p, c) in poses.iter() {
                self.physics.borrow_mut().write_prop(&mut dm, *p, "CFrame", Val::Arr(c.to_vec()));
            }
            if let Some(rt) = humanoid::root_of(&dm, model) {
                self.physics.borrow_mut().write_prop(&mut dm, rt, "Velocity", Val::Arr(n.vel.to_vec()));
            }
            if let Some(p) = n.proxy.as_mut() {
                p.update(&mut *self.physics.borrow_mut(), &dm, &n.torso_cf, &poses);
            }
            drop(dm);
            if n.last_step_cf.map(|l| l != n.torso_cf).unwrap_or(true) {
                self.physics.borrow_mut().stir();
                n.last_step_cf = Some(n.torso_cf);
            }
            self.npcs.insert(model, n);
            if let Some(nh) = healed {
                self.host.fire(hum, "Changed", vec![Wire::S("Health".into())]);
                self.host.fire(hum, "HealthChanged", vec![Wire::N(nh)]);
            }
            if unseat_now {
                self.unseat(model);
            }
            self.npc_events(model, prev);
        }
    }

    pub fn npc_records(&self, viewer: Option<InstId>) -> Vec<J> {
        let r = |x: f64| (x * 10000.0).round() / 10000.0;
        self.npcs
            .values()
            .filter(|n| !n.dead && !(n.owner.is_some() && n.owner == viewer && n.seat.is_none()))
            .map(|n| {
                let mut rec: Vec<J> = vec![json!(n.model)];
                rec.extend(n.torso_cf.iter().map(|x| json!(r(*x))));
                rec.extend(n.vel.iter().map(|x| json!((x * 100.0).round() / 100.0)));
                rec.push(json!(n.state));
                if n.owner.is_some() {
                    if let Some(a) = n.net_angles {
                        rec.extend(a.iter().map(|x| json!(x)));
                    }
                }
                J::Array(rec)
            })
            .collect()
    }
}

#[allow(dead_code)]
fn unused(_: &DataModel) {}

use crate::game::Game;
use crate::npc::Seat;
use serde_json::json;
use swirl_core::instance::{DataModel, InstId, Val};
use swirl_lua::host::Wire;
use swirl_physics::cf::{self, CF};
use swirl_physics::humanoid;

fn cf_of(dm: &DataModel, id: InstId, key: &str) -> CF {
    match dm.get_prop(id, key) {
        Some(Val::Arr(a)) => cf::arr(a).unwrap_or(cf::IDENTITY),
        _ => cf::IDENTITY,
    }
}

fn size_of(dm: &DataModel, id: InstId) -> [f64; 3] {
    match dm.get_prop(id, "Size") {
        Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
        _ => [1.0; 3],
    }
}

fn flat(c: &CF) -> Vec<f64> {
    c.iter().map(|v| (v * 1e4).round() / 1e4).collect()
}

fn sit_height(dm: &DataModel, model: InstId, root: InstId) -> f64 {
    let lower = dm.find_child(model, "LowerTorso", false);
    let leg = dm.find_child(model, "RightUpperLeg", false);
    let hip = leg.and_then(|l| dm.find_child(l, "RightHip", false));
    let c0 = hip.map(|h| cf_of(dm, h, "C0"));
    match (lower, leg, c0) {
        (Some(lo), Some(lg), Some(c0)) => {
            let rel = cf_of(dm, lo, "CFrame")[1] - cf_of(dm, root, "CFrame")[1] + c0[1];
            let h = -rel + (size_of(dm, lg)[0].max(0.0) / 2.0);
            h.clamp(0.3, 1.5)
        }
        _ => 1.0,
    }
}

impl Game {
    pub fn sit(&mut self, model: InstId, seat: InstId) {
        let (hum, dead) = match self.npcs.get(&model) {
            Some(n) if n.seat.is_none() => (n.hum, n.dead),
            _ => return,
        };
        if dead {
            return;
        }
        {
            let dm = self.dm.borrow();
            if matches!(dm.get_prop(seat, "Disabled"), Some(Val::Bool(true))) || matches!(dm.get_prop(seat, "Occupant"), Some(Val::Ref(Some(_)))) {
                return;
            }
        }
        if self.t - self.last_sit.get(&model).copied().unwrap_or(0.0) < 1.2 {
            return;
        }
        let w = {
            let mut dm = self.dm.borrow_mut();
            let torso = match humanoid::root_of(&dm, model) {
                Some(t) => t,
                None => return,
            };
            let w = match dm.create_any("Weld", None) {
                Ok(w) => w,
                Err(_) => return,
            };
            dm.set_prop_quiet(w, "Name", Val::Str("SeatWeld".into()));
            dm.get_mut_name(w, "SeatWeld");
            dm.set_prop_quiet(w, "Part0", Val::Ref(Some(seat)));
            dm.set_prop_quiet(w, "Part1", Val::Ref(Some(torso)));
            let c0 = cf::mul(&cf::IDENTITY, &[0.0, size_of(&dm, seat)[1] / 2.0 + sit_height(&dm, model, torso), 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
            dm.set_prop_quiet(w, "C0", Val::Arr(c0.to_vec()));
            dm.set_prop_quiet(w, "C1", Val::Arr(cf::IDENTITY.to_vec()));
            let _ = dm.set_parent(w, Some(seat));
            w
        };
        self.host.notify_parent(w, None, Some(seat));
        {
            let ph = self.physics.clone();
            let dm = self.dm.borrow();
            if let Some(p) = self.npcs.get_mut(&model).and_then(|n| n.proxy.as_mut()) {
                p.set_seated(&mut ph.borrow_mut(), true);
            }
            drop(dm);
        }
        if let Some(n) = self.npcs.get_mut(&model) {
            n.seat = Some(Seat { seat, weld: w, board: false, controller: None });
        }
        {
            let mut dm = self.dm.borrow_mut();
            let _ = dm.set_prop(hum, "Sit", Val::Bool(true));
            let _ = dm.set_prop(seat, "Occupant", Val::Ref(Some(hum)));
        }
        self.host.fire(hum, "Changed", vec![Wire::S("Sit".into())]);
        self.host.fire(seat, "Changed", vec![Wire::S("Occupant".into())]);
        self.host.fire(hum, "Seated", vec![Wire::B(true), Wire::I(seat)]);
    }

    pub fn mount_board(&mut self, model: InstId, board: InstId) {
        let (hum, dead, seated) = match self.npcs.get(&model) {
            Some(n) => (n.hum, n.dead, n.seat.is_some()),
            None => return,
        };
        if seated || dead || matches!(self.dm.borrow().get_prop(board, "ControllingHumanoid"), Some(Val::Ref(Some(_)))) {
            return;
        }
        if self.t - self.last_sit.get(&model).copied().unwrap_or(0.0) < 1.2 {
            return;
        }
        let (w, ctl) = {
            let mut dm = self.dm.borrow_mut();
            let torso = match humanoid::root_of(&dm, model) {
                Some(t) => t,
                None => return,
            };
            let bcf = cf_of(&dm, board, "CFrame");
            let tcf = cf_of(&dm, torso, "CFrame");
            if tcf[1] < bcf[1] + 1.0 {
                return;
            }
            let r15 = dm.find_child(model, "UpperTorso", false).is_some();
            let hip = if r15 {
                let hh = match dm.get_prop(hum, "HipHeight") {
                    Some(Val::Num(n)) if *n != 0.0 => *n,
                    _ => 2.0,
                };
                hh + size_of(&dm, torso)[1] / 2.0
            } else {
                3.0
            };
            let w = match dm.create_any("Weld", None) {
                Ok(w) => w,
                Err(_) => return,
            };
            dm.set_prop_quiet(w, "Name", Val::Str("SeatWeld".into()));
            dm.get_mut_name(w, "SeatWeld");
            dm.set_prop_quiet(w, "Part0", Val::Ref(Some(board)));
            dm.set_prop_quiet(w, "Part1", Val::Ref(Some(torso)));
            let c0 = [0.0, size_of(&dm, board)[1] / 2.0 + hip, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
            dm.set_prop_quiet(w, "C0", Val::Arr(c0.to_vec()));
            dm.set_prop_quiet(w, "C1", Val::Arr(cf::IDENTITY.to_vec()));
            let _ = dm.set_parent(w, Some(board));
            let ctl = match dm.create_any("SkateboardController", None) {
                Ok(c) => c,
                Err(_) => return,
            };
            dm.set_prop_quiet(ctl, "Name", Val::Str("SkateboardController".into()));
            dm.get_mut_name(ctl, "SkateboardController");
            let _ = dm.set_parent(ctl, Some(board));
            (w, ctl)
        };
        self.host.notify_parent(w, None, Some(board));
        self.host.notify_parent(ctl, None, Some(board));
        self.board_ctl.insert(ctl, (0, std::collections::HashMap::new()));
        {
            let ph = self.physics.clone();
            if let Some(p) = self.npcs.get_mut(&model).and_then(|n| n.proxy.as_mut()) {
                p.set_seated(&mut ph.borrow_mut(), true);
            }
        }
        if let Some(n) = self.npcs.get_mut(&model) {
            n.seat = Some(Seat { seat: board, weld: w, board: true, controller: Some(ctl) });
        }
        {
            let mut dm = self.dm.borrow_mut();
            let _ = dm.set_prop(hum, "PlatformStand", Val::Bool(true));
            let _ = dm.set_prop(board, "ControllingHumanoid", Val::Ref(Some(hum)));
            let _ = dm.set_prop(board, "Controller", Val::Ref(Some(ctl)));
        }
        self.host.fire(hum, "Changed", vec![Wire::S("PlatformStand".into())]);
        self.host.fire(board, "Changed", vec![Wire::S("ControllingHumanoid".into())]);
        self.host.fire(board, "Changed", vec![Wire::S("Controller".into())]);
        self.host.fire(board, "equipped", vec![Wire::I(hum), Wire::I(ctl)]);
        self.host.fire(board, "Equipped", vec![Wire::I(hum), Wire::I(ctl)]);
    }

    fn unseat_board(&mut self, model: InstId) {
        let (board, weld, ctl, hum, owner) = match self.npcs.get_mut(&model) {
            Some(n) => match n.seat.take() {
                Some(s) => (s.seat, s.weld, s.controller, n.hum, n.owner),
                None => return,
            },
            None => return,
        };
        {
            let ph = self.physics.clone();
            if let Some(p) = self.npcs.get_mut(&model).and_then(|n| n.proxy.as_mut()) {
                p.set_seated(&mut ph.borrow_mut(), false);
            }
        }
        self.last_sit.insert(model, self.t);
        let alive = |s: &Self, i: InstId| s.dm.borrow().get(i).map(|x| !x.destroyed).unwrap_or(false);
        if alive(self, weld) {
            self.dm.borrow_mut().destroy(weld);
        }
        if alive(self, board) {
            {
                let mut dm = self.dm.borrow_mut();
                let _ = dm.set_prop(board, "Throttle", Val::Num(0.0));
                let _ = dm.set_prop(board, "Steer", Val::Num(0.0));
                let _ = dm.set_prop(board, "ControllingHumanoid", Val::Ref(None));
                let _ = dm.set_prop(board, "Controller", Val::Ref(None));
            }
            for p in ["Throttle", "Steer", "ControllingHumanoid", "Controller"] {
                self.host.fire(board, "Changed", vec![Wire::S(p.into())]);
            }
            self.host.fire(board, "unequipped", vec![Wire::I(board)]);
            self.host.fire(board, "Unequipped", vec![Wire::I(board)]);
        }
        if let Some(c) = ctl {
            if alive(self, c) {
                self.dm.borrow_mut().destroy(c);
            }
            self.board_ctl.remove(&c);
        }
        if alive(self, hum) {
            let _ = self.dm.borrow_mut().set_prop(hum, "PlatformStand", Val::Bool(false));
            self.host.fire(hum, "Changed", vec![Wire::S("PlatformStand".into())]);
        }
        let n = self.npcs.get_mut(&model).unwrap();
        n.torso_cf = cf::mul(&n.torso_cf, &[0.0, 1.5, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        n.vel = [0.0, 20.0, 0.0];
        let tc = n.torso_cf;
        if let Some(pl) = owner {
            n.tp_seq += 1;
            let s = n.tp_seq;
            self.pending.entry(pl).or_default().retain(|m| m["t"] != "tp");
            self.pending.entry(pl).or_default().push(json!({"t": "tp", "s": s, "c": flat(&tc), "v": [0, 25, 0]}));
        } else {
            let ph = self.physics.clone();
            let dmc = self.dm.clone();
            let dm = dmc.borrow();
            if let Some(c) = self.npcs.get_mut(&model).and_then(|n| n.ctrl.as_mut()) {
                c.teleport(&mut ph.borrow_mut(), &dm, &tc);
            }
        }
    }

    pub fn unseat(&mut self, model: InstId) {
        if self.npcs.get(&model).and_then(|n| n.seat.as_ref()).map(|s| s.board).unwrap_or(false) {
            self.unseat_board(model);
            return;
        }
        let (seat, weld, hum, owner) = match self.npcs.get_mut(&model) {
            Some(n) => match n.seat.take() {
                Some(s) => (s.seat, s.weld, n.hum, n.owner),
                None => return,
            },
            None => return,
        };
        {
            let ph = self.physics.clone();
            if let Some(p) = self.npcs.get_mut(&model).and_then(|n| n.proxy.as_mut()) {
                p.set_seated(&mut ph.borrow_mut(), false);
            }
        }
        self.last_sit.insert(model, self.t);
        {
            let mut dm = self.dm.borrow_mut();
            if dm.get(weld).map(|i| !i.destroyed).unwrap_or(false) {
                dm.destroy(weld);
            }
            if dm.get(seat).map(|i| !i.destroyed).unwrap_or(false) {
                let _ = dm.set_prop(seat, "Occupant", Val::Ref(None));
                if dm.get(seat).map(|i| i.class == "VehicleSeat").unwrap_or(false) {
                    let _ = dm.set_prop(seat, "Throttle", Val::Num(0.0));
                    let _ = dm.set_prop(seat, "Steer", Val::Num(0.0));
                }
            }
        }
        if self.dm.borrow().get(seat).map(|i| !i.destroyed).unwrap_or(false) {
            self.host.fire(seat, "Changed", vec![Wire::S("Occupant".into())]);
        }
        let alive = self.dm.borrow().get(hum).map(|i| !i.destroyed).unwrap_or(false);
        if alive {
            self.host.fire(hum, "Changed", vec![Wire::S("Sit".into())]);
            let _ = self.dm.borrow_mut().set_prop(hum, "Sit", Val::Bool(false));
            self.host.fire(hum, "Seated", vec![Wire::B(false), Wire::Nil]);
        }
        let n = self.npcs.get_mut(&model).unwrap();
        n.torso_cf = cf::mul(&n.torso_cf, &[0.0, 3.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        n.vel = [0.0, 30.0, 0.0];
        let tc = n.torso_cf;
        if let Some(pl) = owner {
            n.tp_seq += 1;
            let s = n.tp_seq;
            self.pending.entry(pl).or_default().retain(|m| m["t"] != "tp");
            self.pending.entry(pl).or_default().push(json!({"t": "tp", "s": s, "c": flat(&tc), "v": [0, 40, 0]}));
        } else {
            let ph = self.physics.clone();
            let dmc = self.dm.clone();
            let dm = dmc.borrow();
            if let Some(c) = self.npcs.get_mut(&model).and_then(|n| n.ctrl.as_mut()) {
                c.teleport(&mut ph.borrow_mut(), &dm, &tc);
            }
        }
    }

    pub fn handle_touch_started(&mut self, a: InstId, b: InstId, ca: Option<InstId>, cb: Option<InstId>) {
        for (part, ch) in [(a, cb), (b, ca)] {
            let model = match ch {
                Some(m) => m,
                None => continue,
            };
            let (owner, seated) = match self.npcs.get(&model) {
                Some(n) => (n.owner, n.seat.is_some()),
                None => continue,
            };
            let (class, name, parent_tool, in_ws) = {
                let dm = self.dm.borrow();
                let class = dm.get(part).map(|i| i.class.clone()).unwrap_or_default();
                let name = dm.name(part).to_string();
                let parent = dm.get(part).and_then(|i| i.parent);
                let tool = parent.filter(|p| dm.get(*p).map(|i| i.class == "Tool").unwrap_or(false));
                let ws = dm.services["Workspace"];
                (class, name, tool, dm.is_descendant_of(part, ws))
            };
            if (class == "Seat" || class == "VehicleSeat") && !seated {
                self.sit(model, part);
            }
            if class == "SkateboardPlatform" && !seated {
                self.mount_board(model, part);
            }
            if name == "Handle" && in_ws {
                if let (Some(tool), Some(pl)) = (parent_tool, owner) {
                    let live = {
                        let dm = self.dm.borrow();
                        let ph = self.physics.borrow();
                        humanoid::live_character_of(&ph, &dm, part).is_some()
                    };
                    if !live {
                        let bp = self.dm.borrow().find_child_of_class(pl, "Backpack");
                        if let Some(bp) = bp {
                            let old = self.dm.borrow().get(tool).and_then(|i| i.parent);
                            let _ = self.dm.borrow_mut().set_parent(tool, Some(bp));
                            self.host.notify_parent(tool, old, Some(bp));
                            self.equip(pl, tool);
                        }
                    }
                }
            }
            if class == "SpawnLocation" {
                if let Some(pl) = owner {
                    let mut dm = self.dm.borrow_mut();
                    if matches!(dm.get_prop(part, "AllowTeamChangeOnTouch"), Some(Val::Bool(true))) && !matches!(dm.get_prop(part, "Neutral"), Some(Val::Bool(true))) {
                        if let Some(col) = dm.get_prop(part, "TeamColor").cloned() {
                            let _ = dm.set_prop(pl, "TeamColor", col);
                            let _ = dm.set_prop(pl, "Neutral", Val::Bool(false));
                        }
                    }
                }
            }
        }
    }

    pub fn char_message(&mut self, player: InstId, cfv: Vec<f64>, vel: [f64; 3], seq: i64, state: u8, angles: Option<[f64; 4]>) {
        let model = match self.npcs.iter().find(|(_, n)| n.owner == Some(player)).map(|(m, _)| *m) {
            Some(m) => m,
            None => return,
        };
        let ws = {
            let dm = self.dm.borrow();
            let n = &self.npcs[&model];
            match dm.get_prop(n.hum, "WalkSpeed") {
                Some(Val::Num(w)) => *w,
                _ => 16.0,
            }
        };
        let t = self.t;
        let n = self.npcs.get_mut(&model).unwrap();
        if n.dead || n.seat.is_some() || seq < n.tp_seq {
            return;
        }
        if let Some(a) = angles {
            n.net_angles = Some(a.map(|x| ((x * 1000.0).round() / 1000.0).clamp(-7.0, 7.0)));
            let ang = n.net_angles.unwrap();
            let mut dm = self.dm.borrow_mut();
            if let Some(torso) = dm.find_child(model, "Torso", false) {
                for (i, name) in ["Right Shoulder", "Left Shoulder", "Right Hip", "Left Hip"].iter().enumerate() {
                    if let Some(j) = dm.find_child(torso, name, false) {
                        if dm.is_a(j, "Motor") {
                            dm.set_prop_quiet(j, "CurrentAngle", Val::Num(ang[i]));
                        }
                    }
                }
            }
        }
        let mut c12 = [0.0; 12];
        c12.copy_from_slice(&cfv[..12]);
        let elapsed = (t - n.last_update).max(0.0);
        let dtt = elapsed.max(0.1);
        let rate = ws.max(16.0) * 2.5;
        let budget = (n.move_budget + rate * elapsed).min(rate * 1.5 + 6.0);
        let dist = ((c12[0] - n.torso_cf[0]).powi(2) + (c12[2] - n.torso_cf[2]).powi(2)).sqrt();
        let dy = c12[1] - n.torso_cf[1];
        if dist > budget + 4.0 || dy > 120.0 * dtt + 8.0 {
            n.tp_seq += 1;
            n.move_budget = 6.0;
            let s = n.tp_seq;
            let tc = n.torso_cf;
            let pend = self.pending.entry(player).or_default();
            pend.retain(|m| m["t"] != "tp");
            pend.push(json!({"t": "tp", "s": s, "c": flat(&tc)}));
            return;
        }
        n.move_budget = (budget - dist).max(0.0);
        n.torso_cf = cf::orthonormalize(&c12);
        n.vel = vel;
        let prev = n.state;
        n.state = state.min(8);
        n.last_update = t;
        if n.state == humanoid::JUMP || n.state == humanoid::FALL {
            n.fall_speed = n.fall_speed.min(vel[1]);
        } else if prev == humanoid::JUMP || prev == humanoid::FALL {
            n.fall_speed = 0.0;
        }
        self.npc_events_pub(model, prev);
    }
}

impl Game {
    pub fn jump_message(&mut self, player: InstId) {
        let model = match self.npcs.iter().find(|(_, n)| n.owner == Some(player)).map(|(m, _)| *m) {
            Some(m) => m,
            None => return,
        };
        if self.npcs.get(&model).map(|n| n.seat.is_some()).unwrap_or(false) {
            self.unseat(model);
        }
    }

    pub fn drive_message(&mut self, player: InstId, v: &serde_json::Value) {
        let model = match self.npcs.iter().find(|(_, n)| n.owner == Some(player)).map(|(m, _)| *m) {
            Some(m) => m,
            None => return,
        };
        let (seat, ctl, board) = match self.npcs.get(&model).and_then(|n| n.seat.as_ref()) {
            Some(s) => (s.seat, s.controller, s.board),
            None => return,
        };
        let clamp = |x: &serde_json::Value| -> i64 {
            let f = x.as_f64().filter(|f| f.is_finite()).unwrap_or(0.0);
            (f.trunc() as i32 as i64).clamp(-1, 1)
        };
        let th = clamp(&v["th"]);
        let st = clamp(&v["st"]);
        if board {
            let (cur_th, cur_st) = {
                let dm = self.dm.borrow();
                let g = |k: &str| match dm.get_prop(seat, k) {
                    Some(Val::Num(n)) => *n as i64,
                    _ => 0,
                };
                (g("Throttle"), g("Steer"))
            };
            let axis = th != cur_th || st != cur_st;
            {
                let mut dm = self.dm.borrow_mut();
                let _ = dm.set_prop(seat, "Throttle", Val::Num(th as f64));
                let _ = dm.set_prop(seat, "Steer", Val::Num(st as f64));
            }
            self.host.fire(seat, "Changed", vec![Wire::S("Throttle".into())]);
            self.host.fire(seat, "Changed", vec![Wire::S("Steer".into())]);
            if let Some(c) = ctl {
                if self.dm.borrow().get(c).map(|i| !i.destroyed).unwrap_or(false) {
                    {
                        let mut dm = self.dm.borrow_mut();
                        let _ = dm.set_prop(c, "Throttle", Val::Num(th as f64));
                        let _ = dm.set_prop(c, "Steer", Val::Num(st as f64));
                    }
                    self.host.fire(c, "Changed", vec![Wire::S("Throttle".into())]);
                    self.host.fire(c, "Changed", vec![Wire::S("Steer".into())]);
                    let last_th = self.board_ctl.get(&c).map(|d| d.0).unwrap_or(0);
                    if axis {
                        let which = if th != last_th { "Throttle" } else { "Steer" };
                        self.host.fire(c, "AxisChanged", vec![Wire::S(which.into())]);
                    }
                    self.board_ctl.entry(c).or_insert((0, std::collections::HashMap::new())).0 = th;
                    for (key, code) in [("j", 32i64), ("d", 33i64)] {
                        let down = match &v[key] {
                            serde_json::Value::Bool(b) => *b,
                            serde_json::Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
                            serde_json::Value::String(s) => !s.is_empty(),
                            _ => false,
                        };
                        let was = self.board_ctl.get(&c).and_then(|d| d.1.get(&code).copied()).unwrap_or(false);
                        if down == was {
                            continue;
                        }
                        self.board_ctl.entry(c).or_insert((0, std::collections::HashMap::new())).1.insert(code, down);
                        self.host.fire(c, "ButtonChanged", vec![Wire::E("Button".into(), code as f64)]);
                        if down && code == 32 && !self.host.has_listener(c, "ButtonChanged") {
                            let grounded = {
                                let dm = self.dm.borrow();
                                self.physics.borrow().board_grounded(&dm, seat)
                            };
                            if grounded {
                                self.physics.borrow_mut().board_impulse(seat, [0.0, 32.0, 0.0]);
                            }
                        }
                        if down && code == 33 {
                            self.unseat(model);
                            return;
                        }
                    }
                }
            }
            return;
        }
        let is_vehicle = self.dm.borrow().get(seat).map(|i| i.class == "VehicleSeat").unwrap_or(false);
        if !is_vehicle {
            return;
        }
        {
            let mut dm = self.dm.borrow_mut();
            let _ = dm.set_prop(seat, "Throttle", Val::Num(th as f64));
            let _ = dm.set_prop(seat, "Steer", Val::Num(st as f64));
        }
        self.host.fire(seat, "Changed", vec![Wire::S("Throttle".into())]);
        self.host.fire(seat, "Changed", vec![Wire::S("Steer".into())]);
    }

    pub fn board_state_events(&mut self) {
        let evs = std::mem::take(&mut self.physics.borrow_mut().board_events);
        for (board, state, old) in evs {
            self.host.fire(board, "MoveStateChanged", vec![Wire::E("MoveState".into(), state as f64), Wire::E("MoveState".into(), old as f64)]);
        }
    }
}

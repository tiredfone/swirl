use std::collections::{HashMap, HashSet};
use swirl_core::instance::{DataModel, InstId, Val};
use swirl_lua::host::Wire;
use swirl_physics::cf::CF;
use swirl_physics::humanoid::{self, CharacterProxy, Controller, Input, Rig};
use swirl_physics::Physics;

const HUM_STATE: [f64; 9] = [8.0, 8.0, 3.0, 5.0, 13.0, 12.0, 15.0, 14.0, 4.0];

pub struct Own {
    pub model: InstId,
    pub hum: InstId,
    pub ctrl: Controller,
    pub rig: Rig,
    pub cf: CF,
    pub vel: [f64; 3],
    pub state: u8,
    pub run_state: Option<u8>,
    pub last_run: Option<f64>,
    pub swim_state: Option<u8>,
    pub last_swim: Option<f64>,
    pub climb_state: Option<u8>,
    pub last_climb: Option<f64>,
    pub fall_for: f64,
    pub fall_sent: bool,
    pub target: Option<CF>,
    pub seated_on: Option<InstId>,
    pub drive: Option<Vec<i32>>,
}

pub struct Remote {
    pub model: InstId,
    pub rig: Rig,
    pub proxy: CharacterProxy,
    pub cf: CF,
    pub target: CF,
    pub vel: [f64; 3],
    pub state: u8,
    pub net_angles: Option<Vec<f64>>,
    pub t: f64,
}

pub struct ClientChars {
    pub cf_marks: Vec<InstId>,
    pub remotes: HashMap<InstId, Remote>,
    pub rig_dirty: bool,
    pub ph: Physics,
    pub excluded: HashSet<InstId>,
    pub own: Option<Own>,
    pub pending_own: Option<(InstId, CF, i64)>,
    pub input: ([f64; 3], bool),
    pub tp_seq: i64,
    pub move_to: Option<[f64; 3]>,
    pub drive_in: [bool; 5],
    pub send_acc: f64,
    pub out: Vec<String>,
}

pub type Fired = (InstId, &'static str, Vec<Wire>);

fn cf12(v: &[f64]) -> Option<CF> {
    if v.len() < 12 {
        return None;
    }
    let mut c = [0.0; 12];
    c.copy_from_slice(&v[..12]);
    Some(c)
}

impl ClientChars {
    pub fn new(dm: &mut DataModel) -> ClientChars {
        let mut ph = Physics::new();
        if let Some(ws) = dm.services.get("Workspace").copied() {
            ph.attach_workspace(dm, ws);
        }
        dm.journal.clear();
        ph.reset_cursor();
        let mut c = ClientChars { cf_marks: vec![], remotes: HashMap::new(), rig_dirty: false, ph, excluded: HashSet::new(), own: None, pending_own: None, input: ([0.0; 3], false), tp_seq: 0, move_to: None, drive_in: [false; 5], send_acc: 0.0, out: vec![] };
        c.track(dm);
        c
    }

    pub fn track(&mut self, dm: &DataModel) {
        let ws = match dm.services.get("Workspace") {
            Some(w) => *w,
            None => return,
        };
        let kids: Vec<InstId> = dm.get(ws).map(|i| i.children.clone()).unwrap_or_default();
        for m in kids {
            if dm.get(m).map(|i| i.class == "Model").unwrap_or(false) && dm.find_child_of_class(m, "Humanoid").is_some() && !self.excluded.contains(&m) {
                self.excluded.insert(m);
                self.ph.exclude_model(dm, m);
            }
        }
        let models: Vec<InstId> = self.excluded.iter().copied().collect();
        for m in models {
            if self.remotes.contains_key(&m) || self.own.as_ref().map(|o| o.model == m).unwrap_or(false) {
                continue;
            }
            if dm.get(m).map(|i| i.destroyed).unwrap_or(true) {
                continue;
            }
            let hum = match dm.find_child_of_class(m, "Humanoid") {
                Some(h) => h,
                None => continue,
            };
            let torso = match humanoid::root_of(dm, m) {
                Some(t) => t,
                None => continue,
            };
            if dm.find_child(m, "Head", false).is_none() || matches!(dm.get_prop(hum, "Health"), Some(Val::Num(h)) if *h <= 0.0) || matches!(dm.get_prop(torso, "Anchored"), Some(Val::Bool(true))) {
                continue;
            }
            let c = match dm.get_prop(torso, "CFrame") {
                Some(Val::Arr(a)) if a.len() >= 12 => {
                    let mut c = [0.0; 12];
                    c.copy_from_slice(&a[..12]);
                    c
                }
                _ => continue,
            };
            let proxy = CharacterProxy::with_sensors(&mut self.ph, dm, m, false);
            self.remotes.insert(m, Remote { model: m, rig: Rig::new(dm, m), proxy, cf: c, target: c, vel: [0.0; 3], state: humanoid::IDLE, net_angles: None, t: 0.0 });
        }
        let gone: Vec<InstId> = self.remotes.keys().copied().filter(|m| dm.get(*m).map(|i| i.destroyed).unwrap_or(true) || dm.services.get("Workspace").map(|w| !dm.is_descendant_of(*m, *w)).unwrap_or(true)).collect();
        for m in gone {
            if let Some(mut r) = self.remotes.remove(&m) {
                r.proxy.destroy(&mut self.ph);
            }
            self.excluded.remove(&m);
        }
        if let Some((m, c, q)) = self.pending_own {
            if dm.get(m).map(|i| !i.destroyed).unwrap_or(false) && self.excluded.contains(&m) {
                self.pending_own = None;
                self.take_own(dm, m, c, q);
            }
        }
    }

    pub fn take_own(&mut self, dm: &DataModel, model: InstId, cf: CF, seq: i64) {
        if !self.excluded.contains(&model) {
            self.pending_own = Some((model, cf, seq));
            return;
        }
        let hum = match dm.find_child_of_class(model, "Humanoid") {
            Some(h) => h,
            None => return,
        };
        if let Some(mut r) = self.remotes.remove(&model) {
            r.proxy.destroy(&mut self.ph);
        }
        if let Some(mut o) = self.own.take() {
            o.ctrl.destroy(&mut self.ph);
        }
        let mut ctrl = Controller::new(&mut self.ph, dm, model, hum);
        ctrl.teleport(&mut self.ph, dm, &cf);
        self.tp_seq = seq;
        let rig = Rig::new(dm, model);
        self.own = Some(Own { model, hum, ctrl, rig, cf, vel: [0.0; 3], state: humanoid::FALL, run_state: None, last_run: None, swim_state: None, last_swim: None, climb_state: None, last_climb: None, fall_for: 0.0, fall_sent: false, target: None, seated_on: None, drive: None });
    }

    pub fn teleport(&mut self, dm: &DataModel, c: &[f64], seq: i64) {
        self.tp_seq = seq;
        if let (Some(o), Some(c)) = (self.own.as_mut(), cf12(c)) {
            o.ctrl.teleport(&mut self.ph, dm, &c);
        }
    }

    pub fn update(&mut self, dm: &mut DataModel, dt: f64) -> Vec<Fired> {
        let mut fired: Vec<Fired> = vec![];
        let era18 = dm.era == 2018;
        let (mv, jump) = self.input;
        if let Some(o) = self.own.as_mut() {
            if dm.get(o.hum).map(|i| i.destroyed).unwrap_or(true) {
                return fired;
            }
            let seat = seated(dm, o.model, o.hum);
            if seat != o.seated_on {
                o.seated_on = seat;
                o.drive = None;
                fired.push((o.hum, "Seated", vec![Wire::B(seat.is_some()), seat.map(Wire::I).unwrap_or(Wire::Nil)]));
            }
            if let Some(seat) = seat {
                let class = dm.get(seat).map(|i| i.class.clone()).unwrap_or_default();
                let [fw, bk, lf, rt, dis] = self.drive_in;
                let th = fw as i32 - bk as i32;
                let st = rt as i32 - lf as i32;
                if class == "SkateboardPlatform" {
                    let cur = vec![th, st, jump as i32, dis as i32];
                    if o.drive.as_ref() != Some(&cur) {
                        o.drive = Some(cur);
                        self.out.push(serde_json::json!({"t": "drive", "th": th, "st": st, "j": jump, "d": dis}).to_string());
                    }
                    o.state = humanoid::STAND;
                } else {
                    if jump {
                        self.out.push(serde_json::json!({"t": "jump"}).to_string());
                    }
                    if class == "VehicleSeat" {
                        let cur = vec![th, st];
                        if o.drive.as_ref() != Some(&cur) {
                            o.drive = Some(cur);
                            self.out.push(serde_json::json!({"t": "drive", "th": th, "st": st}).to_string());
                        }
                    }
                    o.state = humanoid::SIT;
                }
                if let Some(t) = o.target {
                    o.cf = swirl_physics::cf::lerp(&o.cf, &t, (dt * 15.0).min(1.0));
                }
                let cf = o.cf;
                o.ctrl.teleport(&mut self.ph, dm, &cf);
                return fired;
            }
            let mut mv = mv;
            let user = (mv[0] * mv[0] + mv[2] * mv[2]).sqrt() > 1e-3;
            if let (Some(t), false) = (self.move_to, user) {
                let dxz = [t[0] - o.cf[0], 0.0, t[2] - o.cf[2]];
                if (dxz[0] * dxz[0] + dxz[2] * dxz[2]).sqrt() < 1.5 {
                    self.move_to = None;
                    fired.push((o.hum, "MoveToFinished", vec![Wire::B(true)]));
                } else {
                    mv = dxz;
                }
            } else if user {
                self.move_to = None;
            }
            let r = o.ctrl.update(&mut self.ph, dm, dt, &Input { mv, jump, face: None });
            o.cf = r.cf;
            o.vel = r.vel;
            let prev = o.state;
            if r.state != prev {
                o.state = r.state;
                if r.state == humanoid::JUMP && prev != humanoid::JUMP {
                    fired.push((o.hum, "Jumping", vec![Wire::B(true)]));
                }
                if r.state == humanoid::FALL && !era18 {
                    fired.push((o.hum, "FreeFalling", vec![Wire::B(true)]));
                }
                if r.state == humanoid::CLIMB && !era18 {
                    let ws = match dm.get_prop(o.hum, "WalkSpeed") {
                        Some(Val::Num(n)) => *n,
                        _ => 16.0,
                    };
                    fired.push((o.hum, "Climbing", vec![Wire::N(ws * 0.7)]));
                }
                if era18 {
                    let a = HUM_STATE.get(prev as usize).copied().unwrap_or(8.0);
                    let b = HUM_STATE.get(r.state as usize).copied().unwrap_or(8.0);
                    fired.push((o.hum, "StateChanged", vec![Wire::E("HumanoidStateType".into(), a), Wire::E("HumanoidStateType".into(), b)]));
                }
            }
            if era18 {
                if r.state == humanoid::FALL {
                    o.fall_for += dt;
                    if !o.fall_sent && (o.fall_for >= 0.12 || o.climb_state == Some(humanoid::JUMP)) {
                        o.fall_sent = true;
                        fired.push((o.hum, "FreeFalling", vec![Wire::B(true)]));
                    }
                } else {
                    o.fall_for = 0.0;
                    o.fall_sent = false;
                }
                if r.state == humanoid::CLIMB {
                    let cs = if r.vel[1].abs() > 0.5 { r.vel[1] } else { 0.0 };
                    if o.climb_state != Some(r.state) || o.last_climb.map(|l| (l - cs).abs() > 0.5).unwrap_or(true) {
                        o.last_climb = Some(cs);
                        fired.push((o.hum, "Climbing", vec![Wire::N(cs)]));
                    }
                }
            }
            o.climb_state = Some(r.state);
            if r.state == humanoid::WALK || r.state == humanoid::IDLE {
                let raw = (r.vel[0] * r.vel[0] + r.vel[2] * r.vel[2]).sqrt();
                let sp = if r.state == humanoid::WALK && raw > 0.5 { raw } else { 0.0 };
                let landed = o.run_state != Some(r.state);
                if landed || o.last_run.map(|l| (sp > 0.0) != (l > 0.0) || (l - sp).abs() > 0.5).unwrap_or(true) {
                    o.last_run = Some(sp);
                    fired.push((o.hum, "Running", vec![Wire::N(sp)]));
                }
            }
            o.run_state = Some(r.state);
            if r.state == humanoid::SWIM {
                let h = (r.vel[0] * r.vel[0] + r.vel[2] * r.vel[2]).sqrt();
                let sw = if h > 0.5 { h.round() } else { 0.0 };
                if o.last_swim != Some(sw) || o.swim_state != Some(r.state) {
                    o.last_swim = Some(sw);
                    fired.push((o.hum, "Swimming", vec![Wire::N(sw)]));
                }
            }
            o.swim_state = Some(r.state);
            let fallen = match dm.services.get("Workspace").and_then(|w| dm.get_prop(*w, "FallenPartsDestroyHeight")) {
                Some(Val::Num(n)) => *n,
                _ => -500.0,
            };
            if o.cf[1] < fallen {
                let _ = dm.set_prop(o.hum, "Health", Val::Num(0.0));
            }
            self.send_acc += dt;
            if self.send_acc >= 1.0 / 30.0 {
                self.send_acc = 0.0;
                let r4 = |v: f64| (v * 1e4).round() / 1e4;
                let r2 = |v: f64| (v * 100.0).round() / 100.0;
                let limbs: Vec<f64> = match dm.find_child(o.model, "Torso", false) {
                    Some(t) => ["Right Shoulder", "Left Shoulder", "Right Hip", "Left Hip"]
                        .iter()
                        .map(|n| match dm.find_child(t, n, false) {
                            Some(j) if dm.is_a(j, "Motor") => match dm.get_prop(j, "CurrentAngle") {
                                Some(Val::Num(a)) => (a * 1000.0).round() / 1000.0,
                                _ => 0.0,
                            },
                            _ => 0.0,
                        })
                        .collect(),
                    None => vec![],
                };
                let msg = serde_json::json!({"t": "char", "s": self.tp_seq, "c": o.cf.iter().map(|v| r4(*v)).collect::<Vec<f64>>(), "v": r.vel.iter().map(|v| r2(*v)).collect::<Vec<f64>>(), "st": r.state, "j": limbs});
                self.out.push(msg.to_string());
            }
        }
        fired
    }

    pub fn pose(&mut self, dm: &mut DataModel) {
        if let Some(o) = self.own.as_ref() {
            let poses = o.rig.pose(&self.ph, dm, &o.cf);
            for (p, c) in poses.iter() {
                dm.set_prop_quiet(*p, "CFrame", Val::Arr(c.to_vec()));
                self.cf_marks.push(*p);
            }
        }
    }

    pub fn step_physics(&mut self, dm: &mut DataModel) {
        self.ph.stir();
        self.ph.step(dm, 1.0 / 60.0);
        dm.journal.clear();
        self.ph.reset_cursor();
    }
}

#[derive(Default)]
pub struct MouseState {
    pub hover: Option<InstId>,
    pub press: Option<InstId>,
    pub down: bool,
    pub x: f64,
    pub y: f64,
}

fn surface_of(dm: &DataModel, part: InstId, n: [f64; 3]) -> f64 {
    let cf = match dm.get_prop(part, "CFrame") {
        Some(Val::Arr(a)) if a.len() >= 12 => swirl_physics::cf::arr(a),
        _ => None,
    };
    let class_terrain = dm.get(part).map(|i| i.class == "Terrain").unwrap_or(false);
    let cf = match (cf, class_terrain) {
        (Some(c), false) => c,
        _ => return if n[1] > 0.7 { 1.0 } else if n[1] < -0.7 { 4.0 } else { 0.0 },
    };
    let l = swirl_physics::cf::vector(&swirl_physics::cf::inv(&cf), n[0], n[1], n[2]);
    let (ax, ay, az) = (l[0].abs(), l[1].abs(), l[2].abs());
    if ay >= ax && ay >= az {
        if l[1] > 0.0 { 1.0 } else { 4.0 }
    } else if ax >= az {
        if l[0] > 0.0 { 0.0 } else { 3.0 }
    } else if l[2] > 0.0 {
        2.0
    } else {
        5.0
    }
}

impl ClientChars {
    pub fn update_mouse(&mut self, dm: &mut DataModel, mouse: InstId, ms: &mut MouseState, o: [f64; 3], d: [f64; 3], over_gui: bool, k: f64) {
        let mut ignore: Vec<InstId> = vec![];
        if let Some(own) = self.own.as_ref() {
            ignore.push(own.model);
        }
        if let Some(Val::Ref(Some(tf))) = dm.get_prop(mouse, "TargetFilter") {
            ignore.push(*tf);
        }
        let dir = [d[0] * 1000.0, d[1] * 1000.0, d[2] * 1000.0];
        let hit = self.ph.raycast(dm, o, dir, &ignore, None);
        let origin = swirl_physics::cf::look_at(o[0], o[1], o[2], o[0] + d[0], o[1] + d[1], o[2] + d[2]);
        let hcf = swirl_physics::cf::with_pos(&swirl_physics::cf::look_at(o[0], o[1], o[2], hit.point[0], hit.point[1], hit.point[2]), hit.point[0], hit.point[1], hit.point[2]);
        dm.set_prop_quiet(mouse, "Origin", Val::Arr(origin.to_vec()));
        dm.set_prop_quiet(mouse, "Hit", Val::Arr(hcf.to_vec()));
        dm.set_prop_quiet(mouse, "Target", Val::Ref(hit.part));
        let surf = hit.part.map(|p| surface_of(dm, p, hit.normal)).unwrap_or(0.0);
        dm.set_prop_quiet(mouse, "TargetSurface", Val::Num(surf));
        dm.set_prop_quiet(mouse, "X", Val::Num(((ms.x * k) as i64) as f64));
        dm.set_prop_quiet(mouse, "Y", Val::Num(((ms.y * k) as i64) as f64));
        let mut cd: Option<InstId> = None;
        if let (Some(p), false) = (hit.part, over_gui) {
            cd = dm.find_child_of_class(p, "ClickDetector");
            if cd.is_none() {
                if let Some(par) = dm.get(p).and_then(|i| i.parent) {
                    if dm.get(par).map(|i| i.class == "Model").unwrap_or(false) {
                        cd = dm.find_child_of_class(par, "ClickDetector");
                    }
                }
            }
            if let (Some(c), Some(own)) = (cd, self.own.as_ref()) {
                let t = own.cf;
                let dist = ((hit.point[0] - t[0]).powi(2) + (hit.point[1] - t[1]).powi(2) + (hit.point[2] - t[2]).powi(2)).sqrt();
                let max = match dm.get_prop(c, "MaxActivationDistance") {
                    Some(Val::Num(n)) => *n,
                    _ => 32.0,
                };
                if dist > max {
                    cd = None;
                }
            }
        }
        if cd != ms.hover {
            if let Some(h) = ms.hover {
                self.out.push(serde_json::json!({"t": "hover", "id": h, "on": false}).to_string());
            }
            if let Some(c) = cd {
                self.out.push(serde_json::json!({"t": "hover", "id": c, "on": true}).to_string());
            }
            ms.hover = cd;
        }
    }
}

#[derive(Default)]
pub struct Tools {
    pub equipped: Option<InstId>,
    pub tool_mouse: Option<InstId>,
    pub selected_bin: Option<InstId>,
    pub slot_order: Vec<InstId>,
    pub mouse_send_acc: f64,
}

fn is_class(dm: &DataModel, id: InstId, c: &str) -> bool {
    dm.get(id).map(|i| !i.destroyed && i.class == c).unwrap_or(false)
}

impl ClientChars {
    fn open_tool_mouse(&mut self, dm: &mut DataModel, tools: &mut Tools, mouse: InstId) -> Option<InstId> {
        let m = dm.create_any("PlayerMouse", None).ok()?;
        dm.set_prop_quiet(m, "Name", Val::Str("Mouse".into()));
        dm.get_mut_name(m, "Mouse");
        for k in ["Hit", "Origin", "Target", "X", "Y", "ViewSizeX", "ViewSizeY"] {
            if let Some(v) = dm.get_prop(mouse, k).cloned() {
                dm.set_prop_quiet(m, k, v);
            }
        }
        tools.tool_mouse = Some(m);
        Some(m)
    }

    pub fn tool_moved(&mut self, dm: &mut DataModel, tools: &mut Tools, mouse: InstId, tool: InstId, old: Option<InstId>, local: bool, fired: &mut Vec<Fired>) {
        let own_model = match self.own.as_ref() {
            Some(o) => o.model,
            None => return,
        };
        let parent = dm.get(tool).and_then(|i| i.parent);
        let in_char = parent == Some(own_model);
        let was_char = old == Some(own_model);
        let is_tool = is_class(dm, tool, "Tool");
        if local && is_tool && in_char != was_char {
            let bp = dm.get(dm.local_player.unwrap_or(0)).and_then(|_| dm.find_child_of_class(dm.local_player.unwrap(), "Backpack"));
            if in_char && old == bp && bp.is_some() {
                self.out.push(serde_json::json!({"t": "equip", "id": tool}).to_string());
            } else if was_char && parent == bp && bp.is_some() {
                self.out.push(serde_json::json!({"t": "unequip", "id": tool}).to_string());
            }
        }
        if is_tool {
            if in_char && !was_char {
                tools.equipped = Some(tool);
                if let Some(m) = self.open_tool_mouse(dm, tools, mouse) {
                    fired.push((tool, "Equipped", vec![Wire::I(m)]));
                }
            } else if !in_char && was_char {
                if tools.equipped == Some(tool) {
                    tools.equipped = None;
                    tools.tool_mouse = None;
                }
                fired.push((tool, "Unequipped", vec![]));
            }
        }
    }

    pub fn emit_mouse(&self, tools: &Tools, mouse: InstId, ev: &'static str, args: Vec<Wire>, fired: &mut Vec<Fired>) {
        fired.push((mouse, ev, args.clone()));
        if let Some(t) = tools.tool_mouse {
            fired.push((t, ev, args));
        }
    }

    fn mouse_payload(&self, dm: &DataModel, mouse: InstId, ms: &MouseState) -> serde_json::Value {
        let r2 = |v: f64| (v * 100.0).round() / 100.0;
        let h = match dm.get_prop(mouse, "Hit") {
            Some(Val::Arr(a)) if a.len() >= 3 => vec![r2(a[0]), r2(a[1]), r2(a[2])],
            _ => vec![0.0; 3],
        };
        let o = match dm.get_prop(mouse, "Origin") {
            Some(Val::Arr(a)) if a.len() >= 3 => vec![r2(a[0]), r2(a[1]), r2(a[2])],
            _ => vec![0.0; 3],
        };
        let tg = match dm.get_prop(mouse, "Target") {
            Some(Val::Ref(Some(t))) => serde_json::json!(t),
            _ => serde_json::Value::Null,
        };
        serde_json::json!({"h": h, "o": o, "tg": tg, "x": ms.x as i64, "y": ms.y as i64})
    }

    pub fn send_mouse(&mut self, dm: &DataModel, tools: &mut Tools, mouse: InstId, ms: &MouseState, force: bool) {
        if tools.equipped.is_none() && tools.selected_bin.is_none() {
            return;
        }
        if !force && tools.mouse_send_acc < 0.1 {
            return;
        }
        tools.mouse_send_acc = 0.0;
        let mut p = self.mouse_payload(dm, mouse, ms);
        p["t"] = serde_json::json!("mouse");
        self.out.push(p.to_string());
    }

    pub fn mouse_down(&mut self, dm: &mut DataModel, tools: &mut Tools, mouse: InstId, ms: &mut MouseState, button: i64, fired: &mut Vec<Fired>) {
        if button != 0 {
            return;
        }
        ms.down = true;
        ms.press = ms.hover;
        self.emit_mouse(tools, mouse, "Button1Down", vec![], fired);
        if let Some(t) = tools.equipped {
            if !matches!(dm.get_prop(t, "Enabled"), Some(Val::Bool(false))) {
                fired.push((t, "Activated", vec![]));
            }
            self.send_mouse(dm, tools, mouse, ms, true);
            self.out.push(serde_json::json!({"t": "act", "id": t, "down": true}).to_string());
        } else if tools.selected_bin.is_some() {
            self.send_mouse(dm, tools, mouse, ms, true);
            let mut p = self.mouse_payload(dm, mouse, ms);
            p["t"] = serde_json::json!("mouse");
            p["b"] = serde_json::json!("down");
            self.out.push(p.to_string());
        }
    }

    pub fn mouse_up(&mut self, dm: &mut DataModel, tools: &mut Tools, mouse: InstId, ms: &mut MouseState, button: i64, fired: &mut Vec<Fired>) {
        if button != 0 {
            return;
        }
        ms.down = false;
        self.emit_mouse(tools, mouse, "Button1Up", vec![], fired);
        if let Some(t) = tools.equipped {
            fired.push((t, "Deactivated", vec![]));
            self.out.push(serde_json::json!({"t": "act", "id": t, "down": false}).to_string());
        } else if tools.selected_bin.is_some() {
            let mut p = self.mouse_payload(dm, mouse, ms);
            p["t"] = serde_json::json!("mouse");
            p["b"] = serde_json::json!("up");
            self.out.push(p.to_string());
        }
        if let Some(cd) = ms.press {
            if Some(cd) == ms.hover && dm.get(cd).map(|i| !i.destroyed).unwrap_or(false) {
                self.out.push(serde_json::json!({"t": "click", "id": cd}).to_string());
                if let Some(pl) = dm.local_player {
                    fired.push((cd, "MouseClick", vec![Wire::I(pl)]));
                }
            }
        }
        ms.press = None;
    }

    pub fn key(&mut self, tools: &Tools, mouse: InstId, k: &str, down: bool, fired: &mut Vec<Fired>) {
        self.emit_mouse(tools, mouse, if down { "KeyDown" } else { "KeyUp" }, vec![Wire::S(k.to_string())], fired);
        if (tools.equipped.is_some() || tools.selected_bin.is_some()) && k.chars().count() == 1 {
            self.out.push(serde_json::json!({"t": "key", "k": k, "down": down}).to_string());
        }
    }

    pub fn drop_tool(&mut self, dm: &DataModel, tools: &Tools) {
        if let Some(t) = tools.equipped {
            if matches!(dm.get_prop(t, "CanBeDropped"), Some(Val::Bool(true))) {
                self.out.push(serde_json::json!({"t": "drop", "id": t}).to_string());
            }
        }
    }

    pub fn backpack_items(&mut self, dm: &DataModel, tools: &mut Tools) -> Vec<InstId> {
        let mut items = vec![];
        if let Some(o) = self.own.as_ref() {
            for c in dm.get(o.model).map(|i| i.children.clone()).unwrap_or_default() {
                if is_class(dm, c, "Tool") {
                    items.push(c);
                }
            }
        }
        if let Some(bp) = dm.local_player.and_then(|p| dm.find_child_of_class(p, "Backpack")) {
            for c in dm.get(bp).map(|i| i.children.clone()).unwrap_or_default() {
                if is_class(dm, c, "Tool") || is_class(dm, c, "HopperBin") {
                    items.push(c);
                }
            }
        }
        tools.slot_order.retain(|t| items.contains(t));
        for t in items {
            if !tools.slot_order.contains(&t) {
                tools.slot_order.push(t);
            }
        }
        tools.slot_order.iter().take(10).copied().collect()
    }

    pub fn toggle_slot(&mut self, dm: &mut DataModel, tools: &mut Tools, mouse: InstId, item: InstId, fired: &mut Vec<Fired>) {
        let own_model = match self.own.as_ref() {
            Some(o) => o.model,
            None => return,
        };
        if is_class(dm, item, "HopperBin") {
            if let Some(t) = tools.equipped {
                self.out.push(serde_json::json!({"t": "unequip", "id": t}).to_string());
            }
            let on = tools.selected_bin != Some(item);
            if let Some(sb) = tools.selected_bin {
                tools.tool_mouse = None;
                fired.push((sb, "Deselected", vec![]));
                self.out.push(serde_json::json!({"t": "hop", "id": sb, "on": false}).to_string());
            }
            tools.selected_bin = if on { Some(item) } else { None };
            if on {
                if let Some(m) = self.open_tool_mouse(dm, tools, mouse) {
                    fired.push((item, "Selected", vec![Wire::I(m)]));
                }
                self.out.push(serde_json::json!({"t": "hop", "id": item, "on": true}).to_string());
            }
            return;
        }
        if let Some(sb) = tools.selected_bin {
            self.toggle_slot(dm, tools, mouse, sb, fired);
        }
        if dm.get(item).and_then(|i| i.parent) == Some(own_model) {
            self.out.push(serde_json::json!({"t": "unequip", "id": item}).to_string());
        } else {
            self.out.push(serde_json::json!({"t": "equip", "id": item}).to_string());
        }
    }
}

impl ClientChars {
    pub fn apply_ch(&mut self, rows: &[serde_json::Value], _t: f64) {
        if let Some(o) = self.own.as_mut() {
            for r in rows {
                if let Some(a) = r.as_array() {
                    if a.len() >= 13 && a[0].as_u64().or_else(|| a[0].as_f64().map(|f| f as u64)) == Some(o.model as u64) {
                        let mut t = [0.0; 12];
                        for k in 0..12 {
                            t[k] = a[1 + k].as_f64().unwrap_or(0.0);
                        }
                        o.target = Some(t);
                    }
                }
            }
        }
        for r in rows {
            let a = match r.as_array() {
                Some(a) if a.len() >= 17 => a,
                _ => continue,
            };
            let model = a[0].as_u64().or_else(|| a[0].as_f64().map(|f| f as u64)).unwrap_or(0) as InstId;
            if let Some(rec) = self.remotes.get_mut(&model) {
                for k in 0..12 {
                    rec.target[k] = a[1 + k].as_f64().unwrap_or(0.0);
                }
                rec.vel = [a[13].as_f64().unwrap_or(0.0), a[14].as_f64().unwrap_or(0.0), a[15].as_f64().unwrap_or(0.0)];
                rec.state = a[16].as_u64().unwrap_or(0) as u8;
                rec.net_angles = if a.len() >= 21 { Some(a[17..21].iter().map(|v| v.as_f64().unwrap_or(0.0)).collect()) } else { None };
            }
        }
    }

    pub fn step_remotes(&mut self, dm: &mut DataModel, dt: f64) {
        let dirty = std::mem::take(&mut self.rig_dirty);
        let ids: Vec<InstId> = self.remotes.keys().copied().collect();
        for m in ids {
            let rec = self.remotes.get_mut(&m).unwrap();
            if dirty {
                rec.rig.build(dm);
                rec.proxy.refresh(&mut self.ph, dm);
            }
            rec.cf = swirl_physics::cf::lerp(&rec.cf, &rec.target, (dt * 14.0).min(1.0));
            rec.t += dt;
            let speed = (rec.vel[0] * rec.vel[0] + rec.vel[2] * rec.vel[2]).sqrt();
            let scripted = dm.era == 2018 && dm.get(m).map(|i| i.children.iter().any(|c| dm.name(*c) == "Animate" && matches!(dm.get(*c).map(|x| x.class.as_str()), Some("LocalScript") | Some("Script")))).unwrap_or(false);
            if !scripted {
                swirl_physics::animator::animate_procedural(dm, m, rec.state, speed, rec.t, swirl_physics::animator::holds_tool(dm, m));
            }
            swirl_physics::animator::step_motors(dm, m, dt);
            if let Some(a) = rec.net_angles.clone() {
                swirl_physics::animator::apply_limb_angles(dm, m, &a);
            }
            let poses = rec.rig.pose(&self.ph, dm, &rec.cf);
            for (p, c) in poses.iter() {
                dm.set_prop_quiet(*p, "CFrame", Val::Arr(c.to_vec()));
                self.cf_marks.push(*p);
            }
            rec.proxy.update(&mut self.ph, dm, &rec.cf, &poses);
        }
    }
}

fn seated(dm: &DataModel, model: InstId, hum: InstId) -> Option<InstId> {
    let torso = humanoid::root_of(dm, model)?;
    let ws = dm.services.get("Workspace").copied()?;
    let sit = matches!(dm.get_prop(hum, "Sit"), Some(Val::Bool(true)));
    let stand = matches!(dm.get_prop(hum, "PlatformStand"), Some(Val::Bool(true)));
    if !sit && !stand {
        return None;
    }
    let mut all = vec![];
    dm.descendants(ws, &mut all);
    for d in all {
        let i = dm.get(d)?;
        if i.class == "Weld" && dm.name(d) == "SeatWeld" && matches!(dm.get_prop(d, "Part1"), Some(Val::Ref(Some(p))) if *p == torso) {
            if let Some(Val::Ref(Some(p0))) = dm.get_prop(d, "Part0") {
                if sit || dm.get(*p0).map(|x| x.class == "SkateboardPlatform").unwrap_or(false) {
                    return Some(*p0);
                }
            }
        }
    }
    None
}

use std::cell::RefCell;
use std::rc::Rc;
use swirl_core::instance::{DataModel, Val};
use swirl_lua::Host;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct LuaVm {
    dm: Rc<RefCell<DataModel>>,
    host: Option<Host>,
    run: bool,
    core: Rc<RefCell<swirl_lua::client_api::CoreState>>,
    anim: Rc<RefCell<swirl_lua::client_api::AnimState>>,
    chars: Option<crate::clientchars::ClientChars>,
    mouse_state: crate::clientchars::MouseState,
    t: f64,
}

#[wasm_bindgen]
impl LuaVm {
    #[wasm_bindgen(constructor)]
    pub fn new(wall: f64) -> Result<LuaVm, JsValue> {
        swirl_lua::clock::set_wall(wall);
        swirl_lua::clock::set_now(0.0);
        let mut dm = DataModel::new("client");
        dm.local_alloc = true;
        let dm = Rc::new(RefCell::new(dm));
        Ok(LuaVm { dm, host: None, run: false, core: Rc::new(RefCell::new(Default::default())), anim: Rc::new(RefCell::new(Default::default())), chars: None, mouse_state: Default::default(), t: 0.0 })
    }

    pub fn apply(&mut self, text: &str) -> bool {
        let m: serde_json::Value = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(_) => return false,
        };
        let mut dm = self.dm.borrow_mut();
        match m["t"].as_str() {
            Some("init") => {
                dm.era = m["era"].as_i64().unwrap_or(2012) as i32;
                let snap = m["snap"].as_array().cloned().unwrap_or_default();
                swirl_core::wire::apply_init(&mut dm, &snap);
                dm.local_player = m["you"].as_u64().map(|v| v as u32);
                if self.run {
                    let mouse = boot(&mut dm);
                    self.core.borrow_mut().mouse = mouse;
                    if dm.era != 2018 {
                        let sc = dm.services.get("ScriptContext").copied();
                        self.core.borrow_mut().queue.push((37801172, sc, "StarterScript".to_string()));
                    }
                    drop(dm);
                    self.host = Host::new(self.dm.clone()).ok();
                    if let Some(h) = self.host.as_mut() {
                        h.set_handler(Box::new(swirl_lua::client_api::ClientApi { core: self.core.clone(), anim: self.anim.clone() }));
                        self.chars = Some(crate::clientchars::ClientChars::new(&mut self.dm.borrow_mut()));
                        h.scan_scripts();
                    }
                }
                true
            }
            Some("rep") => {
                let ops = m["o"].as_array().cloned().unwrap_or_default();
                let moved = swirl_core::wire::apply_ops(&mut dm, &ops);
                if let Some(ch) = self.chars.as_mut() {
                    ch.track(&dm);
                    for a in m["after"].as_array().cloned().unwrap_or_default() {
                        let c: Vec<f64> = a["c"].as_array().map(|c| c.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect()).unwrap_or_default();
                        match a["t"].as_str() {
                            Some("own") if c.len() >= 12 => {
                                let mut cf = [0.0; 12];
                                cf.copy_from_slice(&c[..12]);
                                ch.take_own(&dm, a["id"].as_u64().unwrap_or(0) as u32, cf, a["s"].as_i64().unwrap_or(1));
                            }
                            Some("tp") => ch.teleport(&dm, &c, a["s"].as_i64().unwrap_or(0)),
                            _ => {}
                        }
                    }
                }
                drop(dm);
                if let Some(h) = self.host.as_mut() {
                    for (id, old, new) in moved {
                        h.notify_parent(id, old, new);
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub fn instance_count(&self) -> usize {
        let d = self.dm.borrow();
        let mut all = vec![];
        d.descendants(d.root, &mut all);
        all.len()
    }

    pub fn run_script(&mut self, source: &str) -> Result<(), JsValue> {
        if self.host.is_none() {
            self.host = Host::new(self.dm.clone()).ok();
        }
        let (sp, class) = match (self.dm.borrow().local_player, self.dm.borrow().services.get("Workspace")) {
            (Some(p), _) => (p, "LocalScript"),
            (None, Some(w)) => (*w, "Script"),
            _ => return Err(JsValue::from_str("no workspace")),
        };
        let sc = self.dm.borrow_mut().create(class, Some(sp)).map_err(|e| JsValue::from_str(&e))?;
        self.dm.borrow_mut().set_prop(sc, "Source", Val::Str(source.into())).map_err(|e| JsValue::from_str(&e))?;
        if let Some(h) = self.host.as_mut() {
            h.scan_subtree(sc);
        }
        Ok(())
    }

    pub fn provide_core_source(&mut self, id: f64, text: Option<String>) {
        self.core.borrow_mut().sources.insert(id as u64, text);
    }

    pub fn take_wanted(&mut self) -> String {
        let w = std::mem::take(&mut self.core.borrow_mut().wanted);
        serde_json::to_string(&w).unwrap_or_default()
    }

    pub fn take_wanted_anims(&mut self) -> String {
        let w = std::mem::take(&mut self.anim.borrow_mut().an.wanted);
        serde_json::to_string(&w).unwrap_or_default()
    }

    pub fn provide_anim(&mut self, id: f64, bytes: Option<Vec<u8>>) {
        self.anim.borrow_mut().an.provide(id as u64, bytes.as_deref());
    }

    pub fn anim_pose(&self) -> String {
        let d = self.dm.borrow();
        let st = self.anim.borrow();
        let mut m: std::collections::BTreeMap<String, Vec<f64>> = Default::default();
        for (j, c) in &st.an.anim {
            let name = match d.get_prop(*j, "Part1") {
                Some(Val::Ref(Some(p))) => d.name(*p).to_string(),
                _ => continue,
            };
            let model = d.get(*j).and_then(|i| i.parent).unwrap_or(0);
            m.insert(format!("{}:{}", model, name), c.iter().map(|v| (v * 1e4).round() / 1e4).collect());
        }
        serde_json::to_string(&m).unwrap_or_default()
    }

    pub fn motor_angles(&self) -> String {
        let d = self.dm.borrow();
        let mut m: std::collections::BTreeMap<String, f64> = Default::default();
        if let Some(ws) = d.services.get("Workspace") {
            for model in d.get(*ws).map(|i| i.children.clone()).unwrap_or_default() {
                if d.find_child_of_class(model, "Humanoid").is_none() {
                    continue;
                }
                if let Some(t) = d.find_child(model, "Torso", false) {
                    for j in d.get(t).map(|i| i.children.clone()).unwrap_or_default() {
                        if d.is_a(j, "Motor") {
                            if let Some(Val::Num(a)) = d.get_prop(j, "CurrentAngle") {
                                m.insert(format!("{}:{}", model, d.name(j)), *a);
                            }
                        }
                    }
                }
            }
        }
        serde_json::to_string(&m).unwrap_or_default()
    }

    pub fn set_run(&mut self, run: bool) {
        self.run = run;
    }

    fn drain_core(&mut self) {
        let queue = std::mem::take(&mut self.core.borrow_mut().queue);
        let mut keep = vec![];
        for (cid, parent, name) in queue {
            let src = self.core.borrow().sources.get(&cid).cloned();
            match src {
                Some(Some(text)) => {
                    let parent = parent.filter(|p| self.dm.borrow().get(*p).map(|i| !i.destroyed).unwrap_or(false)).or_else(|| self.dm.borrow().services.get("ScriptContext").copied());
                    let sid = {
                        let mut dm = self.dm.borrow_mut();
                        match dm.create_any("CoreScript", None) {
                            Ok(sid) => {
                                dm.set_prop_quiet(sid, "Source", Val::Str(text));
                                dm.set_prop_quiet(sid, "Name", Val::Str(name.clone()));
                                dm.get_mut_name(sid, &name);
                                if let Some(p) = parent {
                                    let _ = dm.set_parent(sid, Some(p));
                                }
                                Some(sid)
                            }
                            Err(_) => None,
                        }
                    };
                    if let (Some(sid), Some(h)) = (sid, self.host.as_mut()) {
                        h.scan_subtree(sid);
                    }
                }
                Some(None) => {}
                None => {
                    let mut c = self.core.borrow_mut();
                    if c.requested.insert(cid) {
                        c.wanted.push(cid);
                    }
                    keep.push((cid, parent, name));
                }
            }
        }
        self.core.borrow_mut().queue.extend(keep);
    }

    pub fn tick(&mut self, dt: f64) {
        self.update(dt, 0.0, 0.0, false);
    }

    pub fn phys_parts(&self) -> String {
        let d = self.dm.borrow();
        match self.chars.as_ref() {
            Some(c) => serde_json::to_string(&c.ph.part_ids().into_iter().map(|i| format!("{}:{}", d.name(i), d.full_name(i))).collect::<Vec<_>>()).unwrap_or_default(),
            None => "[]".into(),
        }
    }

    pub fn update_mouse(&mut self, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, x: f64, y: f64, over_gui: bool, k: f64) {
        let mouse = match self.core.borrow().mouse {
            Some(m) => m,
            None => return,
        };
        self.mouse_state.x = x;
        self.mouse_state.y = y;
        if let Some(ch) = self.chars.as_mut() {
            let mut dm = self.dm.borrow_mut();
            ch.update_mouse(&mut dm, mouse, &mut self.mouse_state, [ox, oy, oz], [dx, dy, dz], over_gui, k);
        }
    }

    pub fn mouse_props(&self) -> String {
        let mouse = match self.core.borrow().mouse {
            Some(m) => m,
            None => return "null".into(),
        };
        let d = self.dm.borrow();
        let g = |k: &str| match d.get_prop(mouse, k) {
            Some(Val::Arr(a)) => serde_json::json!(a),
            Some(Val::Num(n)) => serde_json::json!(n),
            Some(Val::Ref(r)) => serde_json::json!(r),
            _ => serde_json::Value::Null,
        };
        serde_json::json!({"Hit": g("Hit"), "Origin": g("Origin"), "Target": g("Target"), "TargetSurface": g("TargetSurface"), "X": g("X"), "Y": g("Y")}).to_string()
    }

    pub fn take_out(&mut self) -> String {
        match self.chars.as_mut() {
            Some(c) => serde_json::to_string(&std::mem::take(&mut c.out)).unwrap_or_default(),
            None => "[]".into(),
        }
    }

    pub fn own_state(&self) -> String {
        match self.chars.as_ref().and_then(|c| c.own.as_ref()) {
            Some(o) => serde_json::json!({"model": o.model, "cf": o.cf, "state": o.state, "vel": o.vel}).to_string(),
            None => "null".into(),
        }
    }

    pub fn update(&mut self, dt: f64, mx: f64, mz: f64, jump: bool) {
        self.t += dt;
        swirl_lua::clock::set_now(self.t);
        self.drain_core();
        if let Some(h) = self.host.as_mut() {
            h.tick(self.t);
        }
        let fired = match self.chars.as_mut() {
            Some(ch) => {
                ch.input = ([mx, 0.0, mz], jump);
                let mut dm = self.dm.borrow_mut();
                ch.update(&mut dm, dt)
            }
            None => vec![],
        };
        if let Some(h) = self.host.as_mut() {
            for (inst, name, args) in fired {
                h.fire(inst, name, args);
            }
        }
        let events = {
            let mut st = self.anim.borrow_mut();
            let mut dm = self.dm.borrow_mut();
            st.an.step(&mut dm, dt)
        };
        if let Some(h) = self.host.as_mut() {
            for e in events {
                let args = e.arg.map(|a| vec![swirl_lua::host::Wire::S(a)]).unwrap_or_default();
                h.fire(e.inst, e.name, args);
            }
        }
        if self.host.is_some() {
            let mut dm = self.dm.borrow_mut();
            if let Some(ws) = dm.services.get("Workspace").copied() {
                let kids: Vec<u32> = dm.get(ws).map(|i| i.children.clone()).unwrap_or_default();
                for m in kids {
                    if dm.find_child_of_class(m, "Humanoid").is_some() {
                        swirl_physics::animator::step_motors(&mut dm, m, dt);
                    }
                }
            }
        }
        if let Some(ch) = self.chars.as_mut() {
            let mut dm = self.dm.borrow_mut();
            ch.pose(&mut dm);
            ch.step_physics(&mut dm);
        }
    }

    pub fn logs(&mut self) -> String {
        let logs = self.host.as_mut().map(|h| h.take_logs()).unwrap_or_default();
        let v: Vec<serde_json::Value> = logs.into_iter().map(|l| serde_json::json!({"kind": l.kind, "msg": l.msg})).collect();
        serde_json::to_string(&v).unwrap_or_default()
    }

    pub fn workspace_children(&self) -> String {
        let d = self.dm.borrow();
        let ws = d.services["Workspace"];
        let names: Vec<String> = d.get(ws).map(|i| i.children.iter().map(|c| d.name(*c).to_string()).collect()).unwrap_or_default();
        serde_json::to_string(&names).unwrap_or_default()
    }
}

#[no_mangle]
pub extern "C" fn tmpfile() -> *mut u8 {
    std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn clock() -> i64 {
    0
}

#[no_mangle]
pub extern "C" fn system(_command: *const u8) -> i32 {
    -1
}

#[no_mangle]
pub extern "C" fn tmpnam(_buffer: *mut u8) -> *mut u8 {
    std::ptr::null_mut()
}

#[wasm_bindgen]
impl LuaVm {
    pub fn class_counts(&self, root_name: &str) -> String {
        let d = self.dm.borrow();
        let root = match d.services.get(root_name) {
            Some(r) => *r,
            None => return "{}".into(),
        };
        let mut all = vec![];
        d.descendants(root, &mut all);
        let mut m: std::collections::BTreeMap<String, usize> = Default::default();
        for i in all {
            if let Some(x) = d.get(i) {
                *m.entry(x.class.clone()).or_default() += 1;
            }
        }
        serde_json::to_string(&m).unwrap_or_default()
    }
}

fn boot(dm: &mut DataModel) -> Option<u32> {
    let mut mouse_id = None;
    for s in ["ScriptContext", "GuiService", "LogService", "MarketplaceService", "TeleportService", "InsertService", "ChangeHistoryService", "PersonalServerService", "PointsService", "HttpService", "ContextActionService", "NetworkClient", "CollectionService", "LocalBackpack", "CoreGui", "UserInputService", "RunService"] {
        let _ = dm.service(s);
    }
    if let (Some(players), Some(lp)) = (dm.services.get("Players").copied(), dm.local_player) {
        dm.set_prop_quiet(players, "LocalPlayer", Val::Ref(Some(lp)));
    }
    if let Ok(m) = dm.create_any("PlayerMouse", None) {
        mouse_id = Some(m);
        dm.set_prop_quiet(m, "Name", Val::Str("Mouse".into()));
        dm.get_mut_name(m, "Mouse");
    }
    if let Some(ws) = dm.services.get("Workspace").copied() {
        if let Ok(cam) = dm.create_any("Camera", None) {
            dm.set_prop_quiet(cam, "Name", Val::Str("Camera".into()));
            dm.get_mut_name(cam, "Camera");
            dm.set_prop_quiet(cam, "CameraType", Val::Num(5.0));
            dm.set_prop_quiet(cam, "FieldOfView", Val::Num(70.0));
            dm.link_quiet(cam, Some(ws));
            dm.set_prop_quiet(ws, "CurrentCamera", Val::Ref(Some(cam)));
        }
    }
    if let Some(cg) = dm.services.get("CoreGui").copied() {
        if let Ok(g) = dm.create_any("ScreenGui", None) {
            dm.set_prop_quiet(g, "Name", Val::Str("RobloxGui".into()));
            dm.get_mut_name(g, "RobloxGui");
            dm.link_quiet(g, Some(cg));
        }
    }
    mouse_id
}

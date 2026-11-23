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
        Ok(LuaVm { dm, host: None, run: false, t: 0.0 })
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
                    drop(dm);
                    self.host = Host::new(self.dm.clone()).ok();
                    if let Some(h) = self.host.as_mut() {
                        h.scan_scripts();
                    }
                }
                true
            }
            Some("rep") => {
                let ops = m["o"].as_array().cloned().unwrap_or_default();
                let moved = swirl_core::wire::apply_ops(&mut dm, &ops);
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

    pub fn set_run(&mut self, run: bool) {
        self.run = run;
    }

    pub fn tick(&mut self, dt: f64) {
        self.t += dt;
        swirl_lua::clock::set_now(self.t);
        if let Some(h) = self.host.as_mut() {
            h.tick(self.t);
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

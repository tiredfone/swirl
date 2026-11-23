use std::cell::RefCell;
use std::rc::Rc;
use swirl_core::instance::{DataModel, Val};
use swirl_lua::Host;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct LuaVm {
    dm: Rc<RefCell<DataModel>>,
    host: Host,
    t: f64,
}

#[wasm_bindgen]
impl LuaVm {
    #[wasm_bindgen(constructor)]
    pub fn new(wall: f64) -> Result<LuaVm, JsValue> {
        swirl_lua::clock::set_wall(wall);
        swirl_lua::clock::set_now(0.0);
        let mut dm = DataModel::new("client");
        for s in ["Workspace", "Players", "Lighting", "StarterGui"] {
            dm.service(s).map_err(|e| JsValue::from_str(&e))?;
        }
        let dm = Rc::new(RefCell::new(dm));
        let host = Host::new(dm.clone()).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(LuaVm { dm, host, t: 0.0 })
    }

    pub fn run_script(&mut self, source: &str) -> Result<(), JsValue> {
        let sp = self.dm.borrow().services["Workspace"];
        let sc = self.dm.borrow_mut().create("Script", Some(sp)).map_err(|e| JsValue::from_str(&e))?;
        self.dm.borrow_mut().set_prop(sc, "Source", Val::Str(source.into())).map_err(|e| JsValue::from_str(&e))?;
        self.host.scan_subtree(sc);
        Ok(())
    }

    pub fn tick(&mut self, dt: f64) {
        self.t += dt;
        swirl_lua::clock::set_now(self.t);
        self.host.tick(self.t);
    }

    pub fn logs(&mut self) -> String {
        let v: Vec<serde_json::Value> = self.host.take_logs().into_iter().map(|l| serde_json::json!({"kind": l.kind, "msg": l.msg})).collect();
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

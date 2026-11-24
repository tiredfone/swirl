use std::cell::RefCell;
use std::rc::Rc;
use swirl_core::instance::{DataModel, Val};
use swirl_lua::Host;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub(crate) struct WebMeasure {
    ctx: Option<web_sys::OffscreenCanvasRenderingContext2d>,
}

impl WebMeasure {
    pub(crate) fn new() -> WebMeasure {
        let ctx = web_sys::OffscreenCanvas::new(1, 1).ok().and_then(|c| c.get_context("2d").ok().flatten()).and_then(|c| c.dyn_into::<web_sys::OffscreenCanvasRenderingContext2d>().ok());
        WebMeasure { ctx }
    }
}

impl swirl_lua::gui_layout::Measure for WebMeasure {
    fn width(&self, font: &str, text: &str) -> f64 {
        match self.ctx.as_ref() {
            Some(c) => {
                c.set_font(font);
                c.measure_text(text).map(|m| m.width()).unwrap_or(0.0)
            }
            None => text.chars().count() as f64 * 7.0,
        }
    }
}

#[wasm_bindgen]
pub struct LuaVm {
    dm: Rc<RefCell<DataModel>>,
    host: Option<Host>,
    run: bool,
    core: Rc<RefCell<swirl_lua::client_api::CoreState>>,
    anim: Rc<RefCell<swirl_lua::client_api::AnimState>>,
    gui: Rc<RefCell<swirl_lua::client_api::GuiState>>,
    chars: Option<crate::clientchars::ClientChars>,
    mouse_state: crate::clientchars::MouseState,
    screen: [f64; 2],
    gui_dom: Option<crate::gui::Gui>,
    gui_dirty: bool,
    cam: crate::camera::Camera,
    init_terrain: Option<String>,
    terrain_keys: Vec<(i32, i32, i32)>,
    bridge: crate::bridge::Bridge,
    hidden: std::collections::HashSet<u32>,
    now_ms: f64,
    friends: std::collections::HashMap<u32, f64>,
    tags: Option<crate::tags::Tags>,
    tools: crate::clientchars::Tools,
    remote_moved: std::collections::HashSet<u32>,
    fired_later: Vec<(u32, &'static str, Vec<swirl_lua::host::Wire>)>,
    t: f64,
    pack: std::collections::BTreeMap<String, String>,
    io_keys: std::collections::HashMap<i64, u32>,
    io_btn: std::collections::HashMap<i32, u32>,
    io_move: Option<u32>,
    io_wheel: Option<u32>,
    io_destroy: Vec<(u32, f64)>,
    last_behavior: f64,
    behavior_req: Option<i32>,
    touches: std::collections::HashMap<i32, TouchRec>,
    menu: Option<[u32; 5]>,
    exit_btns: Vec<u32>,
    leave_req: bool,
    clicks: Vec<u32>,
    own_cam: Option<u32>,
}

struct TouchRec {
    io: u32,
    x: f64,
    y: f64,
    sx: f64,
    sy: f64,
    t0: f64,
    began: Vec<u32>,
    processed: bool,
    moved: f64,
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
        Ok(LuaVm { dm, host: None, run: false, core: Rc::new(RefCell::new(Default::default())), anim: Rc::new(RefCell::new(Default::default())), gui: Rc::new(RefCell::new(Default::default())), chars: None, mouse_state: Default::default(), screen: [800.0, 600.0], gui_dom: None, gui_dirty: true, cam: Default::default(), init_terrain: None, terrain_keys: vec![], bridge: Default::default(), hidden: Default::default(), now_ms: 0.0, friends: Default::default(), tags: None, tools: Default::default(), remote_moved: Default::default(), fired_later: vec![], t: 0.0, pack: Default::default(), io_keys: Default::default(), io_btn: Default::default(), io_move: None, io_wheel: None, io_destroy: vec![], last_behavior: 0.0, behavior_req: None, touches: Default::default(), menu: None, exit_btns: vec![], leave_req: false, clicks: vec![], own_cam: None })
    }

    fn make_io(&mut self, ty: &str, key: i64) -> Option<u32> {
        let mut dm = self.dm.borrow_mut();
        let io = dm.create_any("InputObject", None).ok()?;
        dm.set_prop_quiet(io, "Name", Val::Str("InputObject".into()));
        dm.set_prop_quiet(io, "UserInputType", Val::Num(swirl_core::classes::enum_value("UserInputType", ty).unwrap_or(0) as f64));
        dm.set_prop_quiet(io, "KeyCode", Val::Num(key as f64));
        dm.set_prop_quiet(io, "UserInputState", Val::Num(4.0));
        dm.set_prop_quiet(io, "Position", Val::Arr(vec![0.0, 0.0, 0.0]));
        dm.set_prop_quiet(io, "Delta", Val::Arr(vec![0.0, 0.0, 0.0]));
        Some(io)
    }

    fn send_input(&mut self, io: u32, processed: bool, kind: &str) {
        let uis = self.dm.borrow().services.get("UserInputService").copied();
        if let (Some(u), Some(h)) = (uis, self.host.as_mut()) {
            use swirl_lua::host::Wire;
            h.fire(u, "__Input", vec![Wire::I(io), Wire::B(processed), Wire::S(kind.into())]);
        }
    }

    fn set_last_input(&mut self, ty: &str) {
        let v = swirl_core::classes::enum_value("UserInputType", ty).unwrap_or(0) as f64;
        let changed = {
            let mut c = self.core.borrow_mut();
            let ch = c.last_input != v;
            c.last_input = v;
            ch
        };
        if changed {
            let uis = self.dm.borrow().services.get("UserInputService").copied();
            if let (Some(u), Some(h)) = (uis, self.host.as_mut()) {
                h.fire(u, "LastInputTypeChanged", vec![swirl_lua::host::Wire::E("UserInputType".into(), v)]);
            }
        }
    }

    fn io_pos(&self) -> Vec<f64> {
        let k = 1.0;
        vec![self.mouse_state.x * k, self.mouse_state.y * k - 36.0, 0.0]
    }

    pub fn input_key(&mut self, code: &str, down: bool, typing: bool, is_repeat: bool) {
        let name = match key_name(code) {
            Some(n) => n,
            None => return,
        };
        let key = swirl_core::classes::enum_value("KeyCode", &name).unwrap_or(0);
        if key == 0 {
            return;
        }
        if down {
            if is_repeat {
                return;
            }
            let io = match self.make_io("Keyboard", key) {
                Some(i) => i,
                None => return,
            };
            self.dm.borrow_mut().set_prop_quiet(io, "UserInputState", Val::Num(0.0));
            self.io_keys.insert(key, io);
            self.core.borrow_mut().keys_code.insert(key);
            self.set_last_input("Keyboard");
            self.send_input(io, typing, "b");
        } else if let Some(io) = self.io_keys.remove(&key) {
            self.core.borrow_mut().keys_code.remove(&key);
            self.dm.borrow_mut().set_prop_quiet(io, "UserInputState", Val::Num(2.0));
            self.send_input(io, typing, "e");
            self.io_destroy.push((io, self.t + 1.0));
        }
    }

    fn handle_clicks(&mut self) {
        let clicks = std::mem::take(&mut self.clicks);
        for c in clicks {
            if self.exit_btns.contains(&c) {
                self.leave_req = true;
            }
            let m = match self.menu {
                Some(m) => m,
                None => continue,
            };
            let [leave, shield, yes, no, toggle] = m;
            let show = |vm: &mut LuaVm, on: bool| {
                let mut dm = vm.dm.borrow_mut();
                let _ = dm.set_prop(shield, "Visible", Val::Bool(on));
                if let Some(u) = dm.services.get("UserInputService").copied() {
                    let _ = dm.set_prop(u, "ModalEnabled", Val::Bool(on));
                }
            };
            if c == leave {
                show(self, true);
            } else if c == no {
                show(self, false);
            } else if c == yes {
                self.leave_req = true;
            } else if c == toggle {
                if let Some(st) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
                    let _ = st.set_item("swirl-touch-menu", "hidden");
                }
                let _ = self.dm.borrow_mut().set_prop(leave, "Visible", Val::Bool(false));
                show(self, false);
            }
        }
    }

    pub fn take_leave(&mut self) -> bool {
        std::mem::take(&mut self.leave_req)
    }

    pub fn set_touch(&mut self, on: bool) {
        {
            let mut c = self.core.borrow_mut();
            c.touch = on;
            if c.last_input == 0.0 {
                c.last_input = swirl_core::classes::enum_value("UserInputType", if on { "Touch" } else { "Keyboard" }).unwrap_or(8) as f64;
            }
        }
        let mut dm = self.dm.borrow_mut();
        if let Some(u) = dm.services.get("UserInputService").copied() {
            dm.set_prop_quiet(u, "TouchEnabled", Val::Bool(on));
            dm.set_prop_quiet(u, "MouseEnabled", Val::Bool(!on));
            dm.set_prop_quiet(u, "KeyboardEnabled", Val::Bool(!on));
        }
    }

    fn touch_xy(&self, x: f64, y: f64, k: f64) -> [f64; 2] {
        let top = if self.dm.borrow().era == 2018 { 36.0 } else { 0.0 };
        [x * k, y * k - top]
    }

    fn gui_hits(&self, x: f64, y: f64) -> Vec<u32> {
        let dm = self.dm.borrow();
        let mut roots: Vec<(u32, f64)> = vec![];
        let order = |dm: &DataModel, sg: u32, core: bool| (if core { 1000.0 } else { 10.0 }) + match dm.get_prop(sg, "DisplayOrder") {
            Some(Val::Num(n)) => *n,
            _ => 0.0,
        };
        if let Some(cg) = dm.services.get("CoreGui") {
            for c in dm.get(*cg).map(|i| i.children.clone()).unwrap_or_default() {
                if dm.is_a(c, "ScreenGui") {
                    roots.push((c, order(&dm, c, true)));
                }
            }
        }
        if let Some(pg) = dm.local_player.and_then(|p| dm.find_child_of_class(p, "PlayerGui")) {
            for c in dm.get(pg).map(|i| i.children.clone()).unwrap_or_default() {
                if dm.is_a(c, "ScreenGui") {
                    roots.push((c, order(&dm, c, false)));
                }
            }
        }
        let mut found: Vec<(u32, [f64; 4])> = vec![];
        let mut seq = 0.0;
        fn walk(dm: &DataModel, inst: u32, base: f64, depth: f64, x: f64, y: f64, seq: &mut f64, out: &mut Vec<(u32, [f64; 4])>) {
            for c in dm.get(inst).map(|i| i.children.clone()).unwrap_or_default() {
                if !dm.is_a(c, "GuiObject") {
                    continue;
                }
                if !matches!(dm.get_prop(c, "Visible"), Some(Val::Bool(true))) {
                    continue;
                }
                let order = *seq;
                *seq += 1.0;
                let z = match dm.get_prop(c, "ZIndex") {
                    Some(Val::Num(n)) => *n,
                    _ => 1.0,
                };
                if let (Some(Val::Arr(p)), Some(Val::Arr(s))) = (dm.get_prop(c, "AbsolutePosition"), dm.get_prop(c, "AbsoluteSize")) {
                    if p.len() >= 2 && s.len() >= 2 && x >= p[0] && y >= p[1] && x < p[0] + s[0] && y < p[1] + s[1] {
                        out.push((c, [base, z, depth, order]));
                    }
                }
                walk(dm, c, base, depth + 1.0, x, y, seq, out);
            }
        }
        for (sg, base) in roots {
            if matches!(dm.get_prop(sg, "Enabled"), Some(Val::Bool(false))) {
                continue;
            }
            walk(&dm, sg, base, 0.0, x, y, &mut seq, &mut found);
        }
        found.sort_by(|a, b| {
            for i in 0..4 {
                if a.1[i] != b.1[i] {
                    return b.1[i].partial_cmp(&a.1[i]).unwrap_or(std::cmp::Ordering::Equal);
                }
            }
            std::cmp::Ordering::Equal
        });
        found.into_iter().map(|f| f.0).collect()
    }

    fn touch_io_prop(&mut self, io: u32, state: f64, pos: [f64; 2], delta: [f64; 2]) {
        let mut dm = self.dm.borrow_mut();
        dm.set_prop_quiet(io, "UserInputState", Val::Num(state));
        dm.set_prop_quiet(io, "Position", Val::Arr(vec![pos[0], pos[1], 0.0]));
        dm.set_prop_quiet(io, "Delta", Val::Arr(vec![delta[0], delta[1], 0.0]));
    }

    fn touch_uis(&mut self, io: u32, processed: bool, kind: &str) {
        if self.dm.borrow().era == 2018 {
            self.send_input(io, processed, kind);
        } else {
            let name = match kind {
                "b" => "InputBegan",
                "e" => "InputEnded",
                _ => "InputChanged",
            };
            self.touch_emit(name, io, processed, &[], true);
        }
    }

    fn touch_emit(&mut self, name: &str, io: u32, processed: bool, targets: &[u32], with_processed: bool) {
        use swirl_lua::host::Wire;
        let uis = self.dm.borrow().services.get("UserInputService").copied();
        if let (Some(u), Some(h)) = (uis, self.host.as_mut()) {
            if with_processed {
                h.fire(u, name, vec![Wire::I(io), Wire::B(processed)]);
            }
        }
        let _ = targets;
    }

    pub fn abs_of(&self, name: &str) -> String {
        let dm = self.dm.borrow();
        let mut all = vec![];
        dm.descendants(dm.root, &mut all);
        all.into_iter().filter(|i| dm.name(*i) == name).map(|id| format!("{}:{:?}:{:?}", id, dm.get_prop(id, "AbsolutePosition"), dm.get_prop(id, "AbsoluteSize"))).collect::<Vec<_>>().join(";")
    }

    pub fn touch_hits(&self, x: f64, y: f64, k: f64) -> String {
        let [x, y] = self.touch_xy(x, y, k);
        let dm = self.dm.borrow();
        serde_json::to_string(&self.gui_hits(x, y).into_iter().take(12).map(|g| format!("{}:{}{}", dm.name(g), dm.get(g).map(|i| i.class.clone()).unwrap_or_default(), if matches!(dm.get_prop(g, "Active"), Some(Val::Bool(true))) { ":A" } else { "" })).collect::<Vec<_>>()).unwrap_or_default()
    }

    pub fn touch_down(&mut self, id: i32, x: f64, y: f64, k: f64) -> bool {
        use swirl_lua::host::Wire;
        if self.touches.contains_key(&id) || self.host.is_none() {
            return false;
        }
        let [x, y] = self.touch_xy(x, y, k);
        let io = match self.make_io("Touch", 0) {
            Some(i) => i,
            None => return false,
        };
        self.touch_io_prop(io, 0.0, [x, y], [0.0, 0.0]);
        let hits = self.gui_hits(x, y);
        let processed = {
            let dm = self.dm.borrow();
            hits.iter().any(|g| dm.is_a(*g, "GuiButton") || dm.get(*g).map(|i| i.class == "TextBox").unwrap_or(false) || matches!(dm.get_prop(*g, "Active"), Some(Val::Bool(true))))
        };
        self.touches.insert(id, TouchRec { io, x, y, sx: x, sy: y, t0: self.now_ms, began: hits.clone(), processed, moved: 0.0 });
        self.set_last_input("Touch");
        self.touch_emit("TouchStarted", io, processed, &[], true);
        self.touch_uis(io, processed, "b");
        if let Some(h) = self.host.as_mut() {
            for g in &hits {
                h.fire(*g, "InputBegan", vec![Wire::I(io)]);
            }
        }
        self.touch_emit("ProcessedEvent", io, processed, &[], true);
        !processed || hits.is_empty()
    }

    pub fn touch_move(&mut self, id: i32, x: f64, y: f64, k: f64) -> bool {
        use swirl_lua::host::Wire;
        let [x, y] = self.touch_xy(x, y, k);
        let (io, prev, processed, targets) = match self.touches.get_mut(&id) {
            None => return false,
            Some(r) => {
                if x == r.x && y == r.y {
                    return true;
                }
                let prev = [r.x, r.y];
                r.x = x;
                r.y = y;
                r.moved = r.moved.max(((x - r.sx).powi(2) + (y - r.sy).powi(2)).sqrt());
                (r.io, prev, r.processed, r.began.clone())
            }
        };
        self.touch_io_prop(io, 1.0, [x, y], [x - prev[0], y - prev[1]]);
        self.touch_emit("TouchMoved", io, processed, &[], true);
        self.touch_uis(io, processed, "c");
        let mut list = self.gui_hits(x, y);
        for g in targets {
            if !list.contains(&g) {
                list.push(g);
            }
        }
        if let Some(h) = self.host.as_mut() {
            for g in &list {
                h.fire(*g, "InputChanged", vec![Wire::I(io)]);
            }
        }
        true
    }

    pub fn touch_up(&mut self, id: i32, x: f64, y: f64, k: f64, cancelled: bool) -> bool {
        use swirl_lua::host::Wire;
        let [x, y] = self.touch_xy(x, y, k);
        let rec = match self.touches.remove(&id) {
            Some(r) => r,
            None => return false,
        };
        self.touch_io_prop(rec.io, 2.0, [x, y], [x - rec.x, y - rec.y]);
        self.touch_emit("TouchEnded", rec.io, rec.processed, &[], true);
        self.touch_uis(rec.io, rec.processed, "e");
        let mut list = self.gui_hits(x, y);
        for g in &rec.began {
            if !list.contains(g) {
                list.push(*g);
            }
        }
        if let Some(h) = self.host.as_mut() {
            for g in &list {
                h.fire(*g, "InputEnded", vec![Wire::I(rec.io)]);
            }
        }
        self.io_destroy.push((rec.io, self.t + 5.0));
        !cancelled && !rec.processed && rec.moved < 20.0 && self.now_ms - rec.t0 < 600.0
    }

    pub fn input_blur(&mut self) {
        let keys: Vec<(i64, u32)> = self.io_keys.drain().collect();
        for (k, io) in keys {
            self.core.borrow_mut().keys_code.remove(&k);
            self.dm.borrow_mut().set_prop_quiet(io, "UserInputState", Val::Num(2.0));
            self.send_input(io, false, "e");
        }
    }

    pub fn input_button(&mut self, button: i32, down: bool, over_gui: bool) {
        let (ty, id): (&str, i32) = match button {
            0 => ("MouseButton1", 0),
            1 => ("MouseButton3", 1),
            2 => ("MouseButton2", 2),
            _ => return,
        };
        let pos = self.io_pos();
        if down {
            let io = match self.make_io(ty, 0) {
                Some(i) => i,
                None => return,
            };
            {
                let mut dm = self.dm.borrow_mut();
                dm.set_prop_quiet(io, "UserInputState", Val::Num(0.0));
                dm.set_prop_quiet(io, "Position", Val::Arr(pos));
            }
            self.io_btn.insert(id, io);
            self.core.borrow_mut().buttons_down.insert(id);
            self.set_last_input("MouseButton1");
            self.send_input(io, over_gui, "b");
        } else if let Some(io) = self.io_btn.remove(&id) {
            self.core.borrow_mut().buttons_down.remove(&id);
            {
                let mut dm = self.dm.borrow_mut();
                dm.set_prop_quiet(io, "UserInputState", Val::Num(2.0));
                dm.set_prop_quiet(io, "Position", Val::Arr(pos));
            }
            self.send_input(io, over_gui, "e");
            self.io_destroy.push((io, self.t + 1.0));
        }
    }

    pub fn input_move(&mut self, dx: f64, dy: f64, over_gui: bool, locked: bool) {
        let sens = 1.0;
        let _ = locked;
        let (dx, dy) = (dx * sens, dy * sens);
        {
            let mut c = self.core.borrow_mut();
            c.mouse_delta[0] += dx;
            c.mouse_delta[1] += dy;
        }
        if self.io_move.is_none() {
            self.io_move = self.make_io("MouseMovement", 0);
        }
        let io = match self.io_move {
            Some(i) => i,
            None => return,
        };
        let pos = self.io_pos();
        {
            let mut dm = self.dm.borrow_mut();
            dm.set_prop_quiet(io, "UserInputState", Val::Num(1.0));
            dm.set_prop_quiet(io, "Position", Val::Arr(pos));
            dm.set_prop_quiet(io, "Delta", Val::Arr(vec![dx, dy, 0.0]));
        }
        self.set_last_input("MouseMovement");
        self.send_input(io, over_gui, "c");
    }

    pub fn input_wheel(&mut self, dir: f64, over_gui: bool) {
        if self.io_wheel.is_none() {
            self.io_wheel = self.make_io("MouseWheel", 0);
        }
        let io = match self.io_wheel {
            Some(i) => i,
            None => return,
        };
        let mut pos = self.io_pos();
        pos[2] = dir / 4.0;
        for _ in 0..4 {
            {
                let mut dm = self.dm.borrow_mut();
                dm.set_prop_quiet(io, "UserInputState", Val::Num(1.0));
                dm.set_prop_quiet(io, "Position", Val::Arr(pos.clone()));
            }
            self.send_input(io, over_gui, "c");
        }
    }

    pub fn take_mouse_behavior(&mut self) -> i32 {
        self.behavior_req.take().unwrap_or(-1)
    }

    pub fn set_core_pack(&mut self, text: &str) {
        if let Ok(serde_json::Value::Object(o)) = serde_json::from_str::<serde_json::Value>(text) {
            self.pack = o.into_iter().filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string()))).collect();
        }
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
                self.init_terrain = m["terrain"].as_str().map(String::from);
                let snap = m["snap"].as_array().cloned().unwrap_or_default();
                swirl_core::wire::apply_init(&mut dm, &snap);
                self.bridge = Default::default();
                {
                    let mut all = vec![];
                    dm.descendants(dm.root, &mut all);
                    for i in all {
                        if dm.get(i).map(|x| x.class == "Sound").unwrap_or(false) && matches!(dm.get_prop(i, "IsPlaying"), Some(Val::Bool(true))) {
                            self.core.borrow_mut().sound_ops.push(serde_json::json!({"op": "play", "id": i}));
                        }
                    }
                }
                dm.local_player = m["you"].as_u64().map(|v| v as u32);
                if dm.era == 2018 {
                    let root = dm.root;
                    dm.set_prop_quiet(root, "PlaceId", Val::Num(m["place"].as_f64().unwrap_or(0.0)));
                }
                if self.run {
                    let touch = self.core.borrow().touch;
                    let (mouse, exits, menu) = boot(&mut dm, self.screen, touch);
                    self.exit_btns = exits;
                    self.menu = menu;
                    self.own_cam = dm.services.get("Workspace").copied().and_then(|w| match dm.get_prop(w, "CurrentCamera") {
                        Some(Val::Ref(Some(c))) => Some(*c),
                        _ => None,
                    });
                    self.core.borrow_mut().mouse = mouse;
                    if dm.era != 2018 {
                        let sc = dm.services.get("ScriptContext").copied();
                        self.core.borrow_mut().queue.push((37801172, sc, "StarterScript".to_string()));
                    }
                    drop(dm);
                    if self.dm.borrow().era == 2018 {
                        self.install_2018();
                    }
                    self.host = Host::new(self.dm.clone()).ok();
                    if let Some(h) = self.host.as_mut() {
                        h.set_handler(Box::new(swirl_lua::client_api::ClientApi { core: self.core.clone(), anim: self.anim.clone(), gui: self.gui.clone(), measure: Some(Rc::new(WebMeasure::new())) }));
                        self.chars = Some(crate::clientchars::ClientChars::new(&mut self.dm.borrow_mut()));
                        self.core.borrow_mut().ph = self.chars.as_ref().map(|c| c.ph.clone());
                        if let Some(ch) = self.chars.as_mut() {
                            let dm = self.dm.borrow();
                            if let Some(t) = dm.services.get("Workspace").and_then(|w| dm.find_child_of_class(*w, "Terrain")) {
                                let mut td = swirl_physics::terrain::TerrainData::new(dm.era);
                                if let Some(raw) = self.init_terrain.as_deref() {
                                    let _ = td.decode(raw);
                                }
                                td.take_dirty();
                                let keys = td.all_keys();
                                ch.ph.borrow_mut().set_terrain(t, td);
                                self.terrain_keys.extend(keys);
                            }
                        }
                        h.scan_scripts();
                    }
                }
                true
            }
            Some("terrain") => {
                drop(dm);
                if let (Some(ch), Some(d)) = (self.chars.as_mut(), m["d"].as_str()) {
                    if let Some(td) = ch.ph.borrow_mut().terrain.as_mut() {
                        let _ = td.decode(d);
                    }
                    let keys = ch.ph.borrow_mut().terrain_flush();
                    self.terrain_keys.extend(keys);
                }
                true
            }
            Some("rep") => {
                let ops = m["o"].as_array().cloned().unwrap_or_default();
                self.bridge.collect(&dm, false);
                let applied = swirl_core::wire::apply_ops(&mut dm, &ops);
                let moved = applied.moved;
                for (sid, op) in &applied.sounds {
                    self.core.borrow_mut().sound_ops.push(serde_json::json!({"op": op, "id": sid}));
                }
                let prop_changes = applied.props;
                self.bridge.collect(&dm, true);
                let mouse = self.core.borrow().mouse;
                if let (Some(ch), Some(mouse)) = (self.chars.as_mut(), mouse) {
                    for (id, old, _) in &moved {
                        if dm.get(*id).map(|i| i.class == "Tool" || i.class == "HopperBin").unwrap_or(false) {
                            self.remote_moved.insert(*id);
                            let mut fired = vec![];
                            ch.tool_moved(&mut dm, &mut self.tools, mouse, *id, *old, false, &mut fired);
                            self.fired_later.extend(fired);
                        }
                    }
                }
                if let Some(ch) = self.chars.as_mut() {
                    if !moved.is_empty() {
                        ch.rig_dirty = true;
                    }
                    if let Some(rows) = m["ph"].as_array() {
                        for r in rows {
                            if let Some(a) = r.as_array() {
                                if a.len() >= 16 {
                                    let id = a[0].as_u64().or_else(|| a[0].as_f64().map(|f| f as u64)).unwrap_or(0) as u32;
                                    if !dm.get(id).map(|i| !i.destroyed).unwrap_or(false) || !dm.is_a(id, "BasePart") {
                                        continue;
                                    }
                                    if let Some(own) = ch.own.as_ref() {
                                        if dm.is_descendant_of(id, own.model) {
                                            continue;
                                        }
                                    }
                                    let cf: Vec<f64> = a[1..13].iter().map(|v| v.as_f64().unwrap_or(0.0)).collect();
                                    let _ = dm.set_prop(id, "CFrame", Val::Arr(cf));
                                    dm.set_prop_quiet(id, "Velocity", Val::Arr(vec![a[13].as_f64().unwrap_or(0.0), a[14].as_f64().unwrap_or(0.0), a[15].as_f64().unwrap_or(0.0)]));
                                }
                            }
                        }
                    }
                    ch.track(&dm);
                    if let Some(rows) = m["ch"].as_array() {
                        ch.apply_ch(rows, self.t);
                    }
                    for a in m["after"].as_array().cloned().unwrap_or_default() {
                        let c: Vec<f64> = a["c"].as_array().map(|c| c.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect()).unwrap_or_default();
                        match a["t"].as_str() {
                            Some("own") if c.len() >= 12 => {
                                let mut cf = [0.0; 12];
                                cf.copy_from_slice(&c[..12]);
                                let mid = a["id"].as_u64().unwrap_or(0) as u32;
                                ch.take_own(&dm, mid, cf, a["s"].as_i64().unwrap_or(1));
                                self.core.borrow_mut().own_model = Some(mid);
                                if let (Some(held), Some(mouse)) = (dm.get(mid).and_then(|i| i.children.iter().copied().find(|c| dm.get(*c).map(|x| x.class == "Tool").unwrap_or(false))), mouse) {
                                    if self.tools.equipped != Some(held) {
                                        self.tools.equipped = Some(held);
                                        let mut fired = vec![];
                                        ch.tool_moved(&mut dm, &mut self.tools, mouse, held, None, false, &mut fired);
                                        self.fired_later.extend(fired);
                                    }
                                }
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
                    let dmr = self.dm.clone();
                    let dm = dmr.borrow();
                    for (id, name, old, new) in prop_changes {
                        let class = dm.get(id).map(|i| i.class.clone()).unwrap_or_default();
                        if swirl_core::classes::is_a(&class, "ValueBase") && name == "Value" {
                            h.fire(id, "Changed", vec![val_to_wire_simple(&new)]);
                        } else {
                            h.fire(id, "Changed", vec![swirl_lua::host::Wire::S(name.clone())]);
                        }
                        let key = intern(&format!("P:{}", name));
                        h.fire(id, key, vec![]);
                        if class == "Player" && name == "Character" {
                            if let (Val::Ref(Some(o)), true) = (old.clone().unwrap_or(Val::Ref(None)), old != Some(new.clone())) {
                                if dm.get(o).map(|i| !i.destroyed).unwrap_or(false) {
                                    h.fire(id, "CharacterRemoving", vec![swirl_lua::host::Wire::I(o)]);
                                }
                            }
                            if let Val::Ref(Some(n)) = new {
                                h.fire(id, "CharacterAdded", vec![swirl_lua::host::Wire::I(n)]);
                            }
                        }
                        if class == "Humanoid" && name == "Health" {
                            if let Val::Num(hp) = new {
                                h.fire(id, "HealthChanged", vec![swirl_lua::host::Wire::N(hp)]);
                            }
                        }
                    }
                }
                true
            }
            Some("anim") => {
                let hum = m["h"].as_u64().unwrap_or(0) as u32;
                if dm.get(hum).map(|i| i.class == "Humanoid").unwrap_or(false) {
                    if let Some(k) = m["k"].as_str() {
                        let key = format!("r{}", k);
                        let f = |n: &str| m[n].as_f64();
                        self.anim.borrow_mut().an.op(&mut dm, hum, &key, m["op"].as_str().unwrap_or(""), m["id"].as_u64(), f("a"), f("b"), f("c"), None, f("st").unwrap_or(0.0));
                    }
                }
                true
            }
            Some("chat") => {
                let players = dm.services.get("Players").copied();
                let speaker = if let Some(p) = m["p"].as_u64() {
                    Some(p as u32)
                } else {
                    let uid = m["u"].as_f64();
                    players.and_then(|pl| dm.get(pl).and_then(|i| i.children.iter().copied().find(|c| dm.get(*c).map(|x| x.class == "Player").unwrap_or(false) && matches!(dm.get_prop(*c, "userId"), Some(Val::Num(n)) if Some(*n) == uid))))
                };
                let kind = match m["k"].as_str() {
                    Some("team") => 1.0,
                    Some("whisper") => 2.0,
                    _ => 0.0,
                };
                let to = m["to"].as_u64().map(|t| t as u32).filter(|t| dm.get(*t).is_some());
                let text = m["m"].as_str().unwrap_or("").to_string();
                let head = m["h"].as_u64().map(|h| h as u32).filter(|h| dm.get(*h).is_some());
                let era = dm.era;
                drop(dm);
                use swirl_lua::host::Wire;
                if let (Some(h), Some(sp), Some(pl)) = (self.host.as_mut(), speaker, players) {
                    h.fire(pl, "PlayerChatted", vec![Wire::E("PlayerChatType".into(), kind), Wire::I(sp), Wire::S(text.clone()), to.map(Wire::I).unwrap_or(Wire::Nil)]);
                    h.fire(sp, "Chatted", vec![Wire::S(text.clone()), to.map(Wire::I).unwrap_or(Wire::Nil)]);
                }
                if let (Some(hd), Some(t)) = (head, self.tags.as_mut()) {
                    if kind != 2.0 && era != 2018 {
                        t.add_bubble(hd, &text, self.now_ms);
                    }
                }
                true
            }
            Some("bubble") => {
                let id = m["id"].as_u64().unwrap_or(0) as u32;
                let text = m["m"].as_str().unwrap_or("").to_string();
                let era = dm.era;
                let chat = dm.services.get("Chat").copied();
                let exists = dm.get(id).is_some();
                drop(dm);
                if exists {
                    if era == 2018 {
                        if let (Some(h), Some(c)) = (self.host.as_mut(), chat) {
                            use swirl_lua::host::Wire;
                            h.fire(c, "Chatted", vec![Wire::I(id), Wire::S(text), Wire::E("ChatColor".into(), m["c"].as_f64().unwrap_or(0.0))]);
                        }
                    } else if let Some(t) = self.tags.as_mut() {
                        t.add_bubble(id, &text, self.now_ms);
                    }
                }
                true
            }
            Some("msg") => {
                drop(dm);
                if let Some(g) = self.gui_dom.as_mut() {
                    g.forced_message = m["m"].as_str().map(String::from);
                }
                self.gui_dirty = true;
                true
            }
            Some("fs") => {
                let lp = dm.local_player;
                let mut changes = vec![];
                if let Some(o) = m["s"].as_object() {
                    for (k, v) in o {
                        let id: u32 = k.parse().unwrap_or(0);
                        let st = v.as_f64().unwrap_or(1.0);
                        let prev = self.friends.get(&id).copied().unwrap_or(1.0);
                        self.friends.insert(id, st);
                        self.core.borrow_mut().friends.insert(id, st);
                        if prev != st && dm.get(id).is_some() {
                            changes.push((id, st));
                        }
                    }
                }
                drop(dm);
                if let (Some(h), Some(p)) = (self.host.as_mut(), lp) {
                    for (id, st) in changes {
                        use swirl_lua::host::Wire;
                        h.fire(p, "FriendStatusChanged", vec![Wire::I(id), Wire::E("FriendStatus".into(), st)]);
                    }
                }
                true
            }
            Some("fre") => {
                let (a, b) = (m["a"].as_u64().unwrap_or(0) as u32, m["b"].as_u64().unwrap_or(0) as u32);
                let players = dm.services.get("Players").copied();
                let ok = dm.get(a).is_some() && dm.get(b).is_some();
                drop(dm);
                if let (true, Some(h), Some(p)) = (ok, self.host.as_mut(), players) {
                    use swirl_lua::host::Wire;
                    h.fire(p, "FriendRequestEvent", vec![Wire::I(a), Wire::I(b), Wire::E("FriendRequestEvent".into(), m["e"].as_f64().unwrap_or(0.0))]);
                }
                true
            }
            Some("re") | Some("rfc") => {
                let id = m["id"].as_u64().unwrap_or(0) as u32;
                let class = dm.get(id).map(|i| i.class.clone());
                let args = m["a"].as_array().map(|a| swirl_lua::wire_from_json(a)).unwrap_or_default();
                drop(dm);
                if let Some(h) = self.host.as_mut() {
                    if m["t"] == "re" && class.as_deref() == Some("RemoteEvent") {
                        h.fire(id, "OnClientEvent", args);
                    } else if m["t"] == "rfc" && class.as_deref() == Some("RemoteFunction") {
                        let mut full = vec![swirl_lua::host::Wire::N(m["k"].as_f64().unwrap_or(0.0))];
                        full.extend(args);
                        h.fire(id, "__Invoke", full);
                    }
                }
                true
            }
            Some("mktr") => {
                let k = m["k"].as_u64().unwrap_or(0) as u32;
                let r = &m["r"];
                drop(dm);
                let tok = self.core.borrow_mut().mkt.remove(&k);
                if let (Some(h), Some(tok)) = (self.host.as_mut(), tok) {
                    let w = if let Some(info) = r.get("info") {
                        json_to_wire(info)
                    } else if let Some(o) = r.get("owns") {
                        swirl_lua::host::Wire::B(o.as_bool().unwrap_or(false))
                    } else if let Some(n) = r.get("robux") {
                        swirl_lua::host::Wire::N(n.as_f64().unwrap_or(0.0))
                    } else if r.is_null() {
                        json_to_wire(&serde_json::json!({"success": false, "status": "Error"}))
                    } else {
                        json_to_wire(r)
                    };
                    h.resolve(tok, Ok(vec![w]));
                }
                true
            }
            Some("prompt") => {
                let kind = m["kind"].as_str().unwrap_or("asset").to_string();
                let id = m["id"].as_u64().unwrap_or(0);
                let equip = m["equip"].as_bool().unwrap_or(false);
                drop(dm);
                self.core.borrow_mut().prompt_req.push((kind, id, equip));
                true
            }
            Some("pver") => {
                let mps = dm.services.get("MarketplaceService").copied();
                let uid = dm.local_player.and_then(|p| match dm.get_prop(p, "userId") {
                    Some(Val::Num(n)) => Some(*n),
                    _ => None,
                });
                drop(dm);
                if let (Some(h), Some(mps), Some(uid)) = (self.host.as_mut(), mps, uid) {
                    h.fire(mps, "ServerPurchaseVerification", vec![swirl_lua::host::Wire::Dict(vec![("playerId".into(), swirl_lua::host::Wire::N(uid)), ("productId".into(), swirl_lua::host::Wire::N(m["product"].as_f64().unwrap_or(0.0))), ("receipt".into(), swirl_lua::host::Wire::S(m["receipt"].as_str().unwrap_or("").to_string()))])]);
                }
                true
            }
            Some("out") => {
                let ls = dm.services.get("LogService").copied();
                let era = dm.era;
                drop(dm);
                let mt = match m["k"].as_str() {
                    Some("error") => 3.0,
                    Some("warn") => 2.0,
                    _ => 0.0,
                };
                let text = m["m"].as_str().unwrap_or("").to_string();
                if let (Some(h), Some(ls)) = (self.host.as_mut(), ls) {
                    use swirl_lua::host::Wire;
                    if era == 2018 {
                        h.fire(ls, "ServerMessageOut", vec![Wire::S(text), Wire::E("MessageType".into(), mt), Wire::N((self.now_ms / 1000.0).floor())]);
                    } else {
                        h.fire(ls, "MessageOut", vec![Wire::S(format!("[server] {}", text)), Wire::E("MessageType".into(), mt)]);
                    }
                }
                true
            }
            Some("rfr") => {
                let k = m["k"].as_u64().unwrap_or(0) as u32;
                let args = m["a"].as_array().map(|a| swirl_lua::wire_from_json(a)).unwrap_or_default();
                drop(dm);
                if let (Some(h), Some(tok)) = (self.host.as_mut(), self.core.borrow_mut().invokes.remove(&k)) {
                    h.resolve(tok, Ok(args));
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

    pub fn take_wanted_models(&mut self) -> String {
        let w = std::mem::take(&mut self.core.borrow_mut().models_wanted);
        serde_json::to_string(&w).unwrap_or_default()
    }

    pub fn provide_model(&mut self, token: u32, bytes: Option<Vec<u8>>) {
        let result: Result<Vec<swirl_lua::host::Wire>, String> = match bytes.as_deref().and_then(|b| swirl_core::rbx::parse(b).ok()) {
            Some(tree) => {
                let mut dm = self.dm.borrow_mut();
                match dm.create_any("Model", None) {
                    Ok(m) => {
                        dm.set_prop_quiet(m, "Name", Val::Str("Model".into()));
                        dm.get_mut_name(m, "Model");
                        dm.load_tree(&tree, m);
                        Ok(vec![swirl_lua::host::Wire::I(m)])
                    }
                    Err(e) => Err(e),
                }
            }
            None => Err("asset is not a model".to_string()),
        };
        if let Some(h) = self.host.as_mut() {
            h.resolve(token, result);
        }
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

    pub fn attach_gui(&mut self, root: web_sys::HtmlElement) {
        self.gui_dom = crate::gui::Gui::new(&root);
        if let (Some(g), Some(doc)) = (self.gui_dom.as_ref(), web_sys::window().and_then(|w| w.document())) {
            self.tags = crate::tags::Tags::new(&doc, &g.overlay);
        }
    }

    pub fn gui_frame(&mut self, vw: f64, vh: f64, k: f64, l: f64, t: f64, r: f64, b: f64, show: bool, now_ms: f64) -> usize {
        self.now_ms = now_ms;
        let mut fired: Vec<(u32, &'static str, Vec<swirl_lua::host::Wire>)> = vec![];
        let mut out: Vec<String> = vec![];
        let mut count = 0;
        if let Some(g) = self.gui_dom.as_mut() {
            g.set_visible(show);
            if !show {
                return 0;
            }
            let evs: Vec<crate::gui::GuiEv> = std::mem::take(&mut *g.events.borrow_mut());
            for e in evs {
                match e {
                    crate::gui::GuiEv::Fire(inst, name, a, rep) => {
                        if name == "MouseButton1Click" {
                            self.clicks.push(inst);
                        }
                        let wire: Vec<swirl_lua::host::Wire> = if name.ends_with("Click") { vec![] } else { vec![swirl_lua::host::Wire::N(a.first().copied().unwrap_or(0.0).trunc()), swirl_lua::host::Wire::N(a.get(1).copied().unwrap_or(0.0).trunc())] };
                        fired.push((inst, name, if name.ends_with("Click") { vec![] } else { wire }));
                        if rep {
                            let (cx, cy) = if name.ends_with("Click") { (a.first().copied().unwrap_or(0.0), a.get(1).copied().unwrap_or(0.0)) } else { (a.get(2).copied().unwrap_or(0.0), a.get(3).copied().unwrap_or(0.0)) };
                            let mut j = serde_json::json!({"t": "gui", "id": inst, "ev": name});
                            if !name.starts_with("MouseEnter") && !name.starts_with("MouseLeave") {
                                j["x"] = serde_json::json!(cx as i64);
                                j["y"] = serde_json::json!(cy as i64);
                            }
                            out.push(j.to_string());
                        }
                    }
                    crate::gui::GuiEv::Focus(inst) => {
                        if flag_true(&self.dm.borrow(), inst, "ClearTextOnFocus") {
                            self.dm.borrow_mut().set_prop_quiet(inst, "Text", Val::Str(String::new()));
                        }
                        fired.push((inst, "Focused", vec![]));
                    }
                    crate::gui::GuiEv::Commit(inst, text, enter) => {
                        self.dm.borrow_mut().set_prop_quiet(inst, "Text", Val::Str(text.clone()));
                        fired.push((inst, "Changed", vec![swirl_lua::host::Wire::S("Text".into())]));
                        fired.push((inst, "FocusLost", vec![swirl_lua::host::Wire::B(enter)]));
                        if inst < swirl_core::instance::LOCAL_BASE {
                            out.push(serde_json::json!({"t": "txt", "id": inst, "text": text, "done": true, "enter": enter}).to_string());
                        }
                    }
                }
            }
            if self.gui_dirty || g.dirty.get() || g.hover_changed() {
                self.gui_dirty = false;
                g.dirty.set(false);
                let mut dm = self.dm.borrow_mut();
                let _ = (l, t, r, b);
                let ins = self.core.borrow().gui_inset;
                g.layout(&mut dm, vw, vh, k, ins, &mut fired);
            }
            g.apply_pending_focus();
            count = g.node_count();
        }
        self.handle_clicks();
        if show {
            if let (Some(t), Some(ch)) = (self.tags.as_mut(), self.chars.as_ref()) {
                let dm = self.dm.borrow();
                let fov = dm.services.get("Workspace").and_then(|w| match dm.get_prop(*w, "CurrentCamera") {
                    Some(Val::Ref(Some(c))) => Some(*c),
                    _ => None,
                }).and_then(|c| match dm.get_prop(c, "FieldOfView") {
                    Some(Val::Num(n)) if *n > 0.0 => Some(*n),
                    _ => None,
                }).unwrap_or(70.0);
                let view = crate::tags::View { cf: self.cam.cf, fov, w: vw, h: vh };
                let own = ch.own.as_ref().map(|o| o.model);
                t.update(&dm, &*ch.ph.borrow(), &view, own, now_ms);
                t.update_bubbles(&dm, &view, now_ms);
                drop(dm);
                if let (Some(g), Some(ch)) = (self.gui_dom.as_mut(), self.chars.as_ref()) {
                    let mut dm = self.dm.borrow_mut();
                    g.update_billboards(&mut dm, &*ch.ph.borrow(), &view, now_ms, own, &mut fired);
                }
            }
        }
        if let Some(h) = self.host.as_mut() {
            for (inst, name, args) in fired {
                h.fire(inst, name, args);
            }
        }
        self.core.borrow_mut().out.extend(out);
        count
    }

    pub fn set_gui_message(&mut self, text: Option<String>) {
        if let Some(g) = self.gui_dom.as_mut() {
            g.forced_message = text;
        }
        self.gui_dirty = true;
    }

    pub fn is_typing(&self) -> bool {
        self.gui_dom.as_ref().map(|g| g.focused.get().is_some()).unwrap_or(false)
    }

    pub fn pointer_over_gui(&self, x: f64, y: f64) -> bool {
        self.gui_dom.as_ref().map(|g| g.pointer_over(x, y)).unwrap_or(false)
    }

    pub fn focus_text_box(&mut self, inst: u32, on: bool) {
        if let Some(g) = self.gui_dom.as_mut() {
            g.focus_box(inst, on);
        }
    }

    pub fn camera_update(&mut self, yaw: f64, pitch: f64, zoom: f64, shift_lock: bool, pointer_locked: bool, key_yaw: f64, dt: f64) -> String {
        self.cam.yaw = yaw;
        self.cam.pitch = pitch;
        self.cam.target_zoom = zoom;
        self.cam.shift_lock = shift_lock;
        self.cam.key_yaw = key_yaw;
        let own = self.chars.as_ref().and_then(|c| c.own.as_ref()).map(|o| o.model);
        let mut dm = self.dm.borrow_mut();
        match self.chars.as_ref() {
            Some(ch) => {
                let cf = self.cam.update(&mut dm, &*ch.ph.borrow(), own, dt, pointer_locked);
                let want: std::collections::HashSet<u32> = match (self.cam.first_person, own) {
                    (true, Some(m)) => {
                        let mut all = vec![m];
                        dm.descendants(m, &mut all);
                        all.into_iter().filter(|i| dm.is_a(*i, "BasePart")).collect()
                    }
                    _ => Default::default(),
                };
                for i in want.difference(&self.hidden) {
                    self.bridge.alpha.push((*i, Some(1.0)));
                }
                for i in self.hidden.difference(&want) {
                    self.bridge.alpha.push((*i, None));
                }
                self.hidden = want;
                serde_json::json!({"cf": cf, "yaw": self.cam.yaw, "zoom": self.cam.zoom, "fp": self.cam.first_person}).to_string()
            }
            None => "null".into(),
        }
    }

    pub fn sync_now(&mut self, now_ms: f64) {
        self.now_ms = now_ms;
    }

    pub fn set_drive_input(&mut self, forward: bool, back: bool, left: bool, right: bool, dismount: bool) {
        if let Some(c) = self.chars.as_mut() {
            c.drive_in = [forward, back, left, right, dismount];
        }
    }

    pub fn take_kicked(&mut self) -> Option<String> {
        self.core.borrow_mut().kicked.take()
    }

    pub fn take_sound_ops(&mut self) -> String {
        let ops = std::mem::take(&mut self.core.borrow_mut().sound_ops);
        let d = self.dm.borrow();
        let out: Vec<serde_json::Value> = ops
            .into_iter()
            .map(|mut o| {
                let id = o["id"].as_u64().unwrap_or(0) as u32;
                if let Some(i) = d.get(id) {
                    if matches!(o["op"].as_str(), Some("play") | Some("resume")) {
                        let url = match d.get_prop(id, "SoundId") {
                            Some(Val::Str(u)) => u.clone(),
                            _ => String::new(),
                        };
                        let num = |k: &str, dflt: f64| match d.get_prop(id, k) {
                            Some(Val::Num(n)) => *n,
                            _ => dflt,
                        };
                        o["url"] = serde_json::json!(url);
                        o["volume"] = serde_json::json!(num("Volume", 0.5));
                        o["pitch"] = serde_json::json!(num("Pitch", 1.0));
                        o["looped"] = serde_json::json!(matches!(d.get_prop(id, "Looped"), Some(Val::Bool(true))));
                        if let Some(par) = i.parent {
                            if d.is_a(par, "BasePart") {
                                if let Some(Val::Arr(a)) = d.get_prop(par, "CFrame") {
                                    o["pos"] = serde_json::json!([a[0], a[1], a[2]]);
                                }
                            }
                        }
                    }
                }
                o
            })
            .collect();
        serde_json::to_string(&out).unwrap_or_default()
    }

    pub fn sound_ended(&mut self, id: u32) {
        let mut dm = self.dm.borrow_mut();
        if dm.get(id).map(|i| !i.destroyed).unwrap_or(false) {
            let _ = dm.set_prop(id, "IsPlaying", Val::Bool(false));
            drop(dm);
            self.core.borrow_mut().fire_later.push((id, "Ended"));
        }
    }

    pub fn client_era(&self) -> i32 {
        self.dm.borrow().era
    }

    pub fn terrain_era(&self) -> i32 {
        self.chars.as_ref().and_then(|c| c.ph.borrow().terrain.as_ref().map(|t| t.era)).unwrap_or(2012)
    }

    pub fn terrain_info(&self) -> String {
        let ph = self.chars.as_ref().map(|c| c.ph.clone());
        let g = ph.as_ref().map(|p| p.borrow());
        match g.as_ref().and_then(|p| p.terrain.as_ref()) {
            Some(t) => format!("{{\"era\":{},\"keys\":{},\"init\":{}}}", t.era, t.all_keys().len(), self.init_terrain.as_ref().map(|s| s.len()).unwrap_or(0)),
            None => format!("{{\"none\":true,\"init\":{}}}", self.init_terrain.as_ref().map(|s| s.len()).unwrap_or(0)),
        }
    }

    pub fn take_terrain_keys(&mut self) -> String {
        let keys = std::mem::take(&mut self.terrain_keys);
        let v: Vec<String> = keys.into_iter().map(|k| format!("{},{},{}", k.0, k.1, k.2)).collect();
        serde_json::to_string(&v).unwrap_or_default()
    }

    pub fn terrain_chunk(&self, key: &str) -> js_sys::Array {
        let out = js_sys::Array::new();
        let p: Vec<i32> = key.split(',').filter_map(|x| x.parse().ok()).collect();
        if p.len() != 3 {
            return out;
        }
        let ph = self.chars.as_ref().map(|c| c.ph.clone());
        let g = ph.as_ref().map(|p| p.borrow());
        if let Some(td) = g.as_ref().and_then(|p| p.terrain.as_ref()) {
            let groups = if td.era == 2018 { td.mesh_smooth((p[0], p[1], p[2])) } else { td.mesh_cells((p[0], p[1], p[2])) };
            for (m, pos, nrm, uv) in groups {
                let g = js_sys::Array::new();
                g.push(&JsValue::from(m as u32));
                g.push(&js_sys::Float32Array::from(&pos[..]));
                g.push(&js_sys::Float32Array::from(&nrm[..]));
                g.push(&js_sys::Float32Array::from(&uv[..]));
                out.push(&g);
            }
        }
        out
    }

    pub fn render_ops(&mut self) -> String {
        self.bridge.take_ops()
    }

    pub fn render_cfs(&mut self) -> Vec<f64> {
        self.bridge.take_cfs()
    }

    pub fn render_alpha(&mut self) -> String {
        self.bridge.take_alpha()
    }

    pub fn render_camera(&self) -> Vec<f32> {
        let d = self.dm.borrow();
        let cam = d.services.get("Workspace").and_then(|w| match d.get_prop(*w, "CurrentCamera") {
            Some(Val::Ref(Some(c))) => Some(*c),
            _ => None,
        });
        let cf: Vec<f32> = match cam.and_then(|c| d.get_prop(c, "CFrame")) {
            Some(Val::Arr(a)) if a.len() >= 12 => a.iter().take(12).map(|v| *v as f32).collect(),
            _ => self.cam.cf.iter().map(|v| *v as f32).collect(),
        };
        let fov = match cam.and_then(|c| d.get_prop(c, "FieldOfView")) {
            Some(Val::Num(n)) if *n > 0.0 => *n as f32,
            _ => 70.0,
        };
        let mut out = cf;
        out.push(fov);
        out
    }

    pub fn cam_drag(&mut self, dx: f64, dy: f64) {
        self.cam.drag(dx, dy);
    }

    pub fn cam_rotate(&mut self, dx: f64, dy: f64) {
        self.cam.rotate_by(dx, dy);
    }

    pub fn cam_wheel(&mut self, dir: i32) {
        let dm = self.dm.borrow();
        self.cam.step_zoom(&dm, dir);
    }

    pub fn cam_zoom_by(&mut self, f: f64) {
        let dm = self.dm.borrow();
        self.cam.zoom_by(&dm, f);
    }

    pub fn cam_key_yaw(&mut self, v: f64) {
        self.cam.key_yaw = v;
    }

    pub fn cam_toggle_shift_lock(&mut self) {
        let dm = self.dm.borrow();
        let allowed = {
            let sp = dm.services.get("StarterPlayer").copied();
            let blocked = sp.map(|s| matches!(dm.get_prop(s, "EnableMouseLockOption"), Some(Val::Bool(false)))).unwrap_or(false);
            let t = dm.services.get("Workspace").and_then(|w| match dm.get_prop(*w, "CurrentCamera") {
                Some(Val::Ref(Some(c))) => Some(*c),
                _ => None,
            }).and_then(|c| match dm.get_prop(c, "CameraType") {
                Some(Val::Num(n)) => Some(*n as i64),
                _ => None,
            }).unwrap_or(5);
            !blocked && t != 6 && t != 0
        };
        if self.cam.shift_lock || allowed {
            self.cam.shift_lock = !self.cam.shift_lock;
        }
    }

    pub fn cam_first_person(&self) -> bool {
        self.cam.first_person
    }

    pub fn cam_state(&self) -> String {
        serde_json::json!({"yaw": self.cam.yaw, "pitch": self.cam.pitch, "zoom": self.cam.target_zoom, "fp": self.cam.first_person, "shift": self.cam.shift_lock}).to_string()
    }

    fn face_dir(&self, pointer_locked: bool) -> Option<[f64; 3]> {
        let dm = self.dm.borrow();
        let ws = dm.services.get("Workspace").copied()?;
        let cam = match dm.get_prop(ws, "CurrentCamera") {
            Some(Val::Ref(Some(c))) => *c,
            _ => return None,
        };
        if dm.era == 2018 {
            let us = self.host.as_ref().and_then(|h| h.user_settings_id())?;
            let gs = dm.find_child(us, "GameSettings", false)?;
            if !matches!(dm.get_prop(gs, "RotationType"), Some(Val::Num(n)) if (*n as i64) == 1) {
                return None;
            }
            return match dm.get_prop(cam, "CFrame") {
                Some(Val::Arr(c)) if c.len() >= 12 && c[5].hypot(c[11]) > 1e-3 => Some([-c[5], 0.0, -c[11]]),
                _ => None,
            };
        }
        if !(pointer_locked && (self.cam.shift_lock || self.cam.first_person)) {
            return None;
        }
        let t = match dm.get_prop(cam, "CameraType") {
            Some(Val::Num(n)) => *n as i64,
            _ => 0,
        };
        if t == 6 || t == 0 {
            return None;
        }
        Some(self.cam.forward_flat())
    }

    pub fn frame(&mut self, dt: f64, fw: f64, sd: f64, jump: bool, dismount: bool, pointer_locked: bool) {
        let (mut fw, mut sd, mut jump) = (fw, sd, jump);
        let (rots, zooms) = {
            let mut c = self.core.borrow_mut();
            if let Some(tm) = c.touch_move {
                if fw == 0.0 && sd == 0.0 {
                    fw = -tm[1];
                    sd = tm[0];
                }
            }
            if c.touch_jump {
                jump = true;
                c.touch_jump = false;
            }
            (std::mem::take(&mut c.cam_rot), std::mem::take(&mut c.cam_zoom))
        };
        for r in rots {
            self.cam.rotate_by(r[0], r[1]);
        }
        for z in zooms {
            let dm = self.dm.borrow();
            self.cam.zoom_by(&dm, (-z * 0.15).exp());
        }
        let f = self.cam.forward_flat();
        let r = self.cam.right_flat();
        let mx = f[0] * fw + r[0] * sd;
        let mz = f[2] * fw + r[2] * sd;
        if let Some(c) = self.chars.as_mut() {
            c.drive_in = [fw > 0.3, fw < -0.3, sd < -0.3, sd > 0.3, dismount];
        }
        let face = self.face_dir(pointer_locked);
        if let Some(c) = self.chars.as_mut() {
            c.face = face;
        }
        self.update(dt, mx, mz, jump);
        let own = self.chars.as_ref().and_then(|c| c.own.as_ref()).map(|o| o.model);
        let mut dm = self.dm.borrow_mut();
        if let Some(ch) = self.chars.as_ref() {
            let cf = self.cam.update(&mut dm, &*ch.ph.borrow(), own, dt, pointer_locked);
            let _ = cf;
            let want: std::collections::HashSet<u32> = match (self.cam.first_person, own) {
                (true, Some(m)) => {
                    let mut all = vec![m];
                    dm.descendants(m, &mut all);
                    all.into_iter().filter(|i| dm.is_a(*i, "BasePart")).collect()
                }
                _ => Default::default(),
            };
            for i in want.difference(&self.hidden) {
                self.bridge.alpha.push((*i, Some(1.0)));
            }
            for i in self.hidden.difference(&want) {
                self.bridge.alpha.push((*i, None));
            }
            self.hidden = want;
        }
    }

    pub fn mouse_frame(&mut self, x: f64, y: f64, w: f64, h: f64, k: f64, over_gui: bool, centered: bool) {
        let fov = match self.dm.borrow().services.get("Workspace").and_then(|ws| match self.dm.borrow().get_prop(*ws, "CurrentCamera") {
            Some(Val::Ref(Some(c))) => Some(*c),
            _ => None,
        }) {
            Some(c) => match self.dm.borrow().get_prop(c, "FieldOfView") {
                Some(Val::Num(n)) if *n > 0.0 => *n,
                _ => 70.0,
            },
            None => 70.0,
        };
        let (px, py) = if centered { (w / 2.0, h / 2.0) } else { (x, y) };
        let (o, d) = self.cam.ray(fov, w, h, px, py);
        self.update_mouse(o[0], o[1], o[2], d[0], d[1], d[2], x, y, over_gui, k);
    }

    pub fn sky_urls(&self) -> String {
        let d = self.dm.borrow();
        let lt = match d.services.get("Lighting") {
            Some(l) => *l,
            None => return "null".into(),
        };
        let sky = d.find_child_of_class(lt, "Sky");
        let keys = ["SkyboxRt", "SkyboxLf", "SkyboxUp", "SkyboxDn", "SkyboxBk", "SkyboxFt"];
        let v: Vec<String> = keys
            .iter()
            .map(|k| match sky.and_then(|s| d.get_prop(s, k)) {
                Some(Val::Str(u)) => u.clone(),
                _ => String::new(),
            })
            .collect();
        serde_json::json!({"urls": v, "era": d.era}).to_string()
    }

    pub fn fire_service(&mut self, service: &str, event: &str, args: &str) {
        let id = match self.dm.borrow().services.get(service) {
            Some(i) => *i,
            None => return,
        };
        let parsed: serde_json::Value = serde_json::from_str(args).unwrap_or(serde_json::Value::Null);
        let list: Vec<swirl_lua::host::Wire> = parsed
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|v| match v {
                        serde_json::Value::Array(e) if e.len() == 3 && e[0] == "e" => swirl_lua::host::Wire::E(e[1].as_str().unwrap_or("").to_string(), e[2].as_f64().unwrap_or(0.0)),
                        other => json_to_wire(other),
                    })
                    .collect()
            })
            .unwrap_or_default();
        if let Some(h) = self.host.as_mut() {
            h.fire(id, intern(event), list);
        }
    }

    pub fn gui_key_claimed(&self, key: &str) -> bool {
        self.core.borrow().gui_keys.contains(key)
    }

    pub fn mouse_event(&mut self, name: &str) {
        let mouse = match self.core.borrow().mouse {
            Some(m) => m,
            None => return,
        };
        if let Some(ch) = self.chars.as_ref() {
            let mut fired = vec![];
            ch.emit_mouse(&self.tools, mouse, intern(name), vec![], &mut fired);
            self.fired_later.extend(fired);
        }
    }

    pub fn mouse_cursor(&self) -> String {
        let mouse = match self.core.borrow().mouse {
            Some(m) => m,
            None => return "arrow".into(),
        };
        let d = self.dm.borrow();
        let icon = |m: u32| match d.get_prop(m, "Icon") {
            Some(Val::Str(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };
        if let Some(i) = self.tools.tool_mouse.and_then(icon).or_else(|| icon(mouse)) {
            return i;
        }
        if self.cam.shift_lock && !self.cam.first_person {
            return "lock".into();
        }
        if self.mouse_state.hover.is_some() {
            return "hand".into();
        }
        match d.get_prop(mouse, "Target") {
            Some(Val::Ref(Some(_))) => "arrow".into(),
            _ => "far".into(),
        }
    }

    pub fn set_screen(&mut self, w: f64, h: f64) {
        self.screen = [w, h];
        self.core.borrow_mut().screen = (w, h);
    }

    pub fn set_run(&mut self, run: bool) {
        self.run = run;
    }

    fn drain_market(&mut self) {
        let (prompts, finished) = {
            let mut c = self.core.borrow_mut();
            (std::mem::take(&mut c.prompt_req), std::mem::take(&mut c.finished))
        };
        let (mps, lp, uid) = {
            let dm = self.dm.borrow();
            let lp = dm.local_player;
            let uid = lp.and_then(|p| match dm.get_prop(p, "userId") {
                Some(Val::Num(n)) => Some(*n),
                _ => None,
            });
            (dm.services.get("MarketplaceService").copied(), lp, uid)
        };
        let (mps, lp) = match (mps, lp) {
            (Some(m), Some(l)) => (m, l),
            _ => return,
        };
        use swirl_lua::host::Wire;
        if let Some(h) = self.host.as_mut() {
            for (kind, id, equip) in prompts {
                if id == 0 {
                    continue;
                }
                if kind == "gamepass" {
                    self.core.borrow_mut().pass_prompts.insert(id);
                    h.fire(mps, "PromptPurchaseRequested", vec![Wire::I(lp), Wire::N(id as f64), Wire::B(false), Wire::E("CurrencyType".into(), 0.0)]);
                } else if kind == "product" {
                    h.fire(mps, "PromptProductPurchaseRequested", vec![Wire::I(lp), Wire::N(id as f64), Wire::B(equip), Wire::E("CurrencyType".into(), 0.0)]);
                } else {
                    h.fire(mps, "PromptPurchaseRequested", vec![Wire::I(lp), Wire::N(id as f64), Wire::B(equip), Wire::E("CurrencyType".into(), 0.0)]);
                }
            }
            for (kind, _who, id, ok) in finished {
                if kind == "gamepass" {
                    h.fire(mps, "PromptGamePassPurchaseFinished", vec![Wire::I(lp), Wire::N(id as f64), Wire::B(ok)]);
                } else if kind == "asset" {
                    h.fire(mps, "PromptPurchaseFinished", vec![Wire::I(lp), Wire::N(id as f64), Wire::B(ok)]);
                } else {
                    h.fire(mps, "PromptProductPurchaseFinished", vec![Wire::N(uid.unwrap_or(0.0)), Wire::N(id as f64), Wire::B(ok)]);
                }
            }
        }
    }

    fn install_2018(&mut self) {
        let mut dm = self.dm.borrow_mut();
        for s in ["StarterPlayer", "TweenService", "TextService", "LocalizationService", "VRService", "RobloxReplicatedStorage"] {
            let _ = dm.service(s);
        }
        let rg = dm.services.get("CoreGui").and_then(|cg| dm.get(*cg).and_then(|i| i.children.iter().copied().find(|c| dm.name(*c) == "RobloxGui")));
        let rg = match rg {
            Some(r) => r,
            None => return,
        };
        crate::core2018::build_modules(&mut dm, &self.pack, rg);
        if let Some(cf) = gui(&mut dm, "Frame", "ControlFrame", Some(rg), &[("BackgroundTransparency", Val::Num(1.0)), ("Size", ud([1.0, 0.0, 1.0, 0.0]))]) {
            gui(&mut dm, "Frame", "BottomLeftControl", Some(cf), &[("BackgroundTransparency", Val::Num(1.0)), ("Position", ud([0.0, 0.0, 1.0, -50.0])), ("Size", ud([0.0, 130.0, 0.0, 50.0]))]);
        }
        crate::core2018::core_script(&mut dm, &self.pack, "LoadingScript", rg);
        if let Some(sc) = dm.services.get("ScriptContext").copied() {
            crate::core2018::starter_script(&mut dm, &self.pack, sc);
        }
    }

    fn drain_core(&mut self) {
        let locals = std::mem::take(&mut self.core.borrow_mut().local_scripts);
        for (name, parent) in locals {
            let parent = parent.filter(|p| self.dm.borrow().get(*p).map(|i| !i.destroyed).unwrap_or(false)).or_else(|| self.dm.borrow().services.get("ScriptContext").copied());
            if let Some(p) = parent {
                let sid = crate::core2018::core_script(&mut self.dm.borrow_mut(), &self.pack, &name, p);
                if let (Some(sid), Some(h)) = (sid, self.host.as_mut()) {
                    h.scan_subtree(sid);
                }
            }
        }
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
            Some(c) => serde_json::to_string(&c.ph.borrow().part_ids().into_iter().map(|i| format!("{}:{}", d.name(i), d.full_name(i))).collect::<Vec<_>>()).unwrap_or_default(),
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
        self.core.borrow_mut().mouse_pos = [x, y];
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
        let mut out: Vec<String> = match self.chars.as_mut() {
            Some(c) => std::mem::take(&mut c.out),
            None => vec![],
        };
        let extra = std::mem::take(&mut self.core.borrow_mut().out);
        for e in extra {
            if e.contains("\"id\":-1") {
                if let Some(t) = self.tools.equipped {
                    out.push(format!("{{\"t\":\"unequip\",\"id\":{}}}", t));
                }
            } else {
                out.push(e);
            }
        }
        serde_json::to_string(&out).unwrap_or_default()
    }

    pub fn mouse_button(&mut self, button: i32, down: bool) {
        let mouse = match self.core.borrow().mouse {
            Some(m) => m,
            None => return,
        };
        if let Some(ch) = self.chars.as_mut() {
            let mut dm = self.dm.borrow_mut();
            let mut fired = vec![];
            if down {
                ch.mouse_down(&mut dm, &mut self.tools, mouse, &mut self.mouse_state, button as i64, &mut fired);
            } else {
                ch.mouse_up(&mut dm, &mut self.tools, mouse, &mut self.mouse_state, button as i64, &mut fired);
            }
            self.fired_later.extend(fired);
        }
    }

    pub fn key_event(&mut self, k: &str, down: bool) {
        {
            let mut c = self.core.borrow_mut();
            if down {
                c.keys.insert(k.to_lowercase());
            } else {
                c.keys.remove(&k.to_lowercase());
            }
        }
        let mouse = match self.core.borrow().mouse {
            Some(m) => m,
            None => return,
        };
        if let Some(ch) = self.chars.as_mut() {
            let mut fired = vec![];
            ch.key(&self.tools, mouse, k, down, &mut fired);
            self.fired_later.extend(fired);
        }
    }

    pub fn toggle_slot(&mut self, item: u32) {
        let mouse = match self.core.borrow().mouse {
            Some(m) => m,
            None => return,
        };
        if let Some(ch) = self.chars.as_mut() {
            let mut dm = self.dm.borrow_mut();
            let mut fired = vec![];
            ch.toggle_slot(&mut dm, &mut self.tools, mouse, item, &mut fired);
            self.fired_later.extend(fired);
        }
    }

    pub fn drop_tool(&mut self) {
        if let Some(ch) = self.chars.as_mut() {
            let dm = self.dm.borrow();
            ch.drop_tool(&dm, &self.tools);
        }
    }

    pub fn names_under(&self, service: &str, class: &str) -> String {
        let d = self.dm.borrow();
        let root = match d.services.get(service) {
            Some(r) => *r,
            None => return "[]".into(),
        };
        let mut all = vec![];
        d.descendants(root, &mut all);
        let v: Vec<String> = all.into_iter().filter(|i| d.get(*i).map(|x| x.class == class).unwrap_or(false)).map(|i| d.full_name(i)).collect();
        serde_json::to_string(&v).unwrap_or_default()
    }

    pub fn script_source(&self, name: &str) -> String {
        let d = self.dm.borrow();
        let mut all = vec![];
        d.descendants(d.root, &mut all);
        for i in all {
            if d.name(i) == name && d.is_a(i, "LuaSourceContainer") {
                if let Some(Val::Str(s)) = d.get_prop(i, "Source") {
                    return s.clone();
                }
            }
        }
        String::new()
    }

    pub fn debug_state(&self) -> String {
        let rs = self.dm.borrow().services.get("RunService").copied();
        let l = |n: &str| match (rs, self.host.as_ref()) {
            (Some(r), Some(h)) => h.has_listener(r, n),
            _ => false,
        };
        serde_json::json!({"removed": self.core.borrow().loading_removed, "lm": self.core.borrow().lua_move, "keys": self.core.borrow().keys_code.len(), "era": self.dm.borrow().era, "rs": l("RenderStepped"), "hb": l("Heartbeat"), "st": l("Stepped"), "t": self.t, "touches": self.touches.len(), "rot": self.core.borrow().cam_rot.len(), "tj": self.core.borrow().touch_jump, "tm": self.core.borrow().touch_move}).to_string()
    }

    pub fn dump_named(&self, name: &str) -> String {
        let d = self.dm.borrow();
        let mut all = vec![];
        d.descendants(d.root, &mut all);
        let v: Vec<serde_json::Value> = all
            .into_iter()
            .filter(|i| d.name(*i) == name)
            .map(|i| {
                let t = match d.get_prop(i, "Text") {
                    Some(Val::Str(s)) => s.clone(),
                    _ => String::new(),
                };
                let pos = match d.get_prop(i, "Position") {
                    Some(Val::Arr(a)) => serde_json::json!(a),
                    _ => serde_json::Value::Null,
                };
                serde_json::json!({"n": d.full_name(i), "c": d.get(i).map(|x| x.class.clone()), "t": t, "p": pos, "s": match d.get_prop(i, "Size") { Some(Val::Arr(a)) => serde_json::json!(a), _ => serde_json::Value::Null }, "v": d.get_prop(i, "Visible").map(|v| matches!(v, Val::Bool(true))), "a": match d.get_prop(i, "AbsoluteSize") { Some(Val::Arr(a)) => serde_json::json!(a), _ => serde_json::Value::Null }, "bt": d.get_prop(i, "BackgroundTransparency").map(|v| matches!(v, Val::Num(n) if *n > 0.5)), "id": i})
            })
            .collect();
        serde_json::to_string(&v).unwrap_or_default()
    }

    pub fn text_bounds_all(&self) -> String {
        let d = self.dm.borrow();
        let m = WebMeasure::new();
        let (w, h) = self.core.borrow().screen;
        let mut all = vec![];
        d.descendants(d.root, &mut all);
        let v: Vec<serde_json::Value> = all
            .into_iter()
            .filter(|i| d.is_a(*i, "TextLabel") || d.is_a(*i, "TextButton") || d.is_a(*i, "TextBox"))
            .map(|i| {
                let b = swirl_lua::gui_layout::text_bounds(&d, i, &m, [w, h]);
                let t = match d.get_prop(i, "Text") {
                    Some(Val::Str(s)) => s.clone(),
                    _ => String::new(),
                };
                serde_json::json!([d.full_name(i), t, b[0], b[1]])
            })
            .collect();
        serde_json::to_string(&v).unwrap_or_default()
    }

    pub fn gui_node_names(&self) -> String {
        let d = self.dm.borrow();
        let v: Vec<String> = match self.gui_dom.as_ref() {
            Some(g) => g.node_ids().into_iter().map(|i| d.full_name(i)).collect(),
            None => vec![],
        };
        serde_json::to_string(&v).unwrap_or_default()
    }

    pub fn children_of(&self, path: &str) -> String {
        let d = self.dm.borrow();
        let mut all = vec![];
        d.descendants(d.root, &mut all);
        let v: Vec<Vec<String>> = all.into_iter().filter(|i| d.full_name(*i) == path).map(|i| d.get(i).map(|x| x.children.iter().map(|c| format!("{}:{}", d.name(*c), d.get(*c).map(|k| k.class.clone()).unwrap_or_default())).collect()).unwrap_or_default()).collect();
        serde_json::to_string(&v).unwrap_or_default()
    }

    pub fn tools_json(&self) -> String {
        serde_json::json!({"equipped": self.tools.equipped, "bin": self.tools.selected_bin, "order": self.tools.slot_order}).to_string()
    }

    pub fn backpack_items(&mut self) -> String {
        match self.chars.as_mut() {
            Some(ch) => {
                let dm = self.dm.borrow();
                serde_json::to_string(&ch.backpack_items(&dm, &mut self.tools)).unwrap_or_default()
            }
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
        if let Some(oc) = self.own_cam {
            let mut dm = self.dm.borrow_mut();
            if let Some(ws) = dm.services.get("Workspace").copied() {
                if !matches!(dm.get_prop(ws, "CurrentCamera"), Some(Val::Ref(Some(c))) if *c == oc) {
                    dm.set_prop_quiet(ws, "CurrentCamera", Val::Ref(Some(oc)));
                }
            }
            if let Some(hum) = self.chars.as_ref().and_then(|c| c.own.as_ref()).map(|o| o.hum) {
                if !matches!(dm.get_prop(oc, "CameraSubject"), Some(Val::Ref(Some(h))) if *h == hum) {
                    let _ = dm.set_prop(oc, "CameraSubject", Val::Ref(Some(hum)));
                }
            }
        }
        self.drain_core();
        self.drain_market();
        {
            let reqs = std::mem::take(&mut self.core.borrow_mut().focus_req);
            if let Some(g) = self.gui_dom.as_mut() {
                for (i, on) in reqs {
                    g.focus_box(i, on);
                }
            }
            let st = self.chars.as_ref().and_then(|c| c.own.as_ref()).map(|o| (o.hum, o.state));
            let seat = self.chars.as_ref().and_then(|c| c.own.as_ref()).and_then(|o| o.seated_on);
            let mut cs = self.core.borrow_mut();
            cs.own_state = st;
            cs.own_seat = seat;
        }
        {
            let mut g = self.gui.borrow_mut();
            g.now = self.t;
            g.step(&mut self.dm.borrow_mut());
        }
        if let Some(h) = self.host.as_mut() {
            h.tick(self.t);
        }
        if let Some(h) = self.host.as_mut() {
            for r in h.take_requests() {
                match r {
                    swirl_lua::host::Req::Remote(id, op, args) if op == "fs" => {
                        let a = swirl_lua::wire_to_json(&args);
                        self.core.borrow_mut().out.push(serde_json::json!({"t": "re", "id": id, "a": a}).to_string());
                    }
                    swirl_lua::host::Req::InvokeResult(_, tok, args) => {
                        let a = swirl_lua::wire_to_json(&args);
                        self.core.borrow_mut().out.push(serde_json::json!({"t": "rfr", "k": tok as i64, "a": a}).to_string());
                    }
                    _ => {}
                }
            }
        }
        if let Some((hum, pos)) = self.core.borrow_mut().move_to.take() {
            if let Some(ch) = self.chars.as_mut() {
                if ch.own.as_ref().map(|o| o.hum == hum).unwrap_or(false) {
                    ch.move_to = pos;
                }
            }
        }
        {
            let now = self.t;
            let dead: Vec<u32> = self.io_destroy.iter().filter(|(_, t)| *t <= now).map(|(i, _)| *i).collect();
            self.io_destroy.retain(|(_, t)| *t > now);
            let mut dm = self.dm.borrow_mut();
            for i in dead {
                dm.destroy(i);
            }
            let uis = dm.services.get("UserInputService").copied();
            if let Some(u) = uis {
                if let Some(Val::Num(b)) = dm.get_prop(u, "MouseBehavior") {
                    if *b != self.last_behavior {
                        self.last_behavior = *b;
                        self.behavior_req = Some(*b as i32);
                    }
                }
            }
        }
        let rs = self.dm.borrow().services.get("RunService").copied();
        if let (Some(rs), Some(h)) = (rs, self.host.as_mut()) {
            if h.has_listener(rs, "Stepped") {
                h.fire(rs, "Stepped", vec![swirl_lua::host::Wire::N(self.t), swirl_lua::host::Wire::N(dt)]);
            }
        }
        let interp = self.core.borrow().cam_interp;
        if let Some((cam, st, goal, focus, t0, dur)) = interp {
            let a = ((self.t - t0) / dur).clamp(0.0, 1.0);
            let mut dm = self.dm.borrow_mut();
            if dm.get(cam).map(|i| i.destroyed).unwrap_or(true) {
                self.core.borrow_mut().cam_interp = None;
            } else {
                let cf = if a >= 1.0 { goal } else { swirl_physics::cf::lerp(&st, &goal, a) };
                let _ = dm.set_prop(cam, "CFrame", Val::Arr(cf.to_vec()));
                if a >= 1.0 {
                    if let Some(f) = focus {
                        let fc = swirl_physics::cf::with_pos(&swirl_physics::cf::rot_only(&goal), f[0], f[1], f[2]);
                        let _ = dm.set_prop(cam, "Focus", Val::Arr(fc.to_vec()));
                    }
                    self.core.borrow_mut().cam_interp = None;
                }
            }
        }
        let era18 = self.dm.borrow().era == 2018;
        let lm = self.core.borrow().lua_move;
        let hum_jump = if era18 {
            let dm = self.dm.borrow();
            let own = self.chars.as_ref().and_then(|c| c.own.as_ref()).map(|o| o.hum);
            own.map(|h| matches!(dm.get_prop(h, "Jump"), Some(Val::Bool(true)))).unwrap_or(false)
        } else {
            false
        };
        let fired = match self.chars.as_mut() {
            Some(ch) => {
                if era18 {
                    ch.input = (lm.unwrap_or([0.0; 3]), jump || hum_jump);
                    if hum_jump {
                        if let Some(o) = ch.own.as_ref() {
                            let _ = self.dm.borrow_mut().set_prop(o.hum, "Jump", Val::Bool(false));
                        }
                    }
                } else {
                    ch.input = ([mx, 0.0, mz], jump);
                }
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
        if let Some(ch) = self.chars.as_ref() {
            ch.ph.borrow_mut().anim_cf = self.anim.borrow().an.anim.clone();
        }
        if let Some(h) = self.host.as_mut() {
            for e in events {
                let args = e.arg.map(|a| vec![swirl_lua::host::Wire::S(a)]).unwrap_or_default();
                h.fire(e.inst, e.name, args);
            }
        }
        if let (Some(rs), Some(h)) = (rs, self.host.as_mut()) {
            for ev in ["Heartbeat", "RenderStepped"] {
                if h.has_listener(rs, ev) {
                    h.fire(rs, ev, vec![swirl_lua::host::Wire::N(dt)]);
                }
            }
        }
        if let Some(h) = self.host.as_mut() {
            h.render_step(dt);
        }
        if self.host.is_some() {
            let mut dm = self.dm.borrow_mut();
            if let Some(ws) = dm.services.get("Workspace").copied() {
                let kids: Vec<u32> = dm.get(ws).map(|i| i.children.clone()).unwrap_or_default();
                for m in kids {
                    if dm.find_child_of_class(m, "Humanoid").is_some() && !self.chars.as_ref().map(|c| c.remotes.contains_key(&m)).unwrap_or(false) {
                        swirl_physics::animator::step_motors(&mut dm, m, dt);
                    }
                }
            }
        }
        if let Some(ch) = self.chars.as_mut() {
            let mut dm = self.dm.borrow_mut();
            ch.pose(&mut dm);
            ch.step_remotes(&mut dm, dt);
            if dm.journal.iter().any(|c| match c {
                swirl_core::instance::Change::Prop(id, _) => dm.get(*id).map(|i| matches!(i.class.as_str(), "Player" | "Humanoid") || dm.is_a(*id, "GuiBase") || dm.is_a(*id, "ValueBase") || dm.is_a(*id, "GuiObject")).unwrap_or(false),
                _ => true,
            }) {
                self.gui_dirty = true;
            }
            let mouse = self.core.borrow().mouse;
            if let Some(mouse) = mouse {
                let moves: Vec<(u32, Option<u32>)> = dm
                    .journal
                    .iter()
                    .filter_map(|c| match c {
                        swirl_core::instance::Change::Parent { id, old, .. } => Some((*id, *old)),
                        _ => None,
                    })
                    .collect();
                let mut fired = vec![];
                for (id, old) in moves {
                    if self.remote_moved.contains(&id) {
                        continue;
                    }
                    if dm.get(id).map(|i| i.class == "Tool" || i.class == "HopperBin").unwrap_or(false) {
                        ch.tool_moved(&mut dm, &mut self.tools, mouse, id, old, true, &mut fired);
                    }
                }
                self.fired_later.extend(fired);
                self.tools.mouse_send_acc += dt;
                ch.send_mouse(&dm, &mut self.tools, mouse, &self.mouse_state, false);
            }
            self.remote_moved.clear();
            {
                let mut ops = vec![];
                for c in dm.journal.iter() {
                    match c {
                        swirl_core::instance::Change::Prop(id, name) if (name == "Volume" || name == "Pitch") && dm.get(*id).map(|i| i.class == "Sound").unwrap_or(false) => {
                            let num = |k: &str, dflt: f64| match dm.get_prop(*id, k) {
                                Some(Val::Num(n)) => *n,
                                _ => dflt,
                            };
                            ops.push(serde_json::json!({"op": "update", "id": id, "volume": num("Volume", 0.5), "pitch": num("Pitch", 1.0)}));
                        }
                        swirl_core::instance::Change::Destroy { id, .. } if dm.get(*id).map(|i| i.class == "Sound").unwrap_or(false) => {
                            ops.push(serde_json::json!({"op": "stop", "id": id}));
                        }
                        _ => {}
                    }
                }
                self.core.borrow_mut().sound_ops.extend(ops);
            }
            self.bridge.cf_marks.extend(std::mem::take(&mut ch.cf_marks));
            self.bridge.collect(&dm, false);
            self.bridge.flush_marks(&dm);
            ch.step_physics(&mut dm);
            self.bridge.render_from = 0;
        }
        {
            let later = std::mem::take(&mut self.core.borrow_mut().fire_later);
            self.fired_later.extend(later.into_iter().map(|(i, n)| (i, n, vec![])));
        }
        if let Some(h) = self.host.as_mut() {
            for (inst, name, args) in std::mem::take(&mut self.fired_later) {
                h.fire(inst, name, args);
            }
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

fn gui(dm: &mut DataModel, class: &str, name: &str, parent: Option<u32>, props: &[(&str, Val)]) -> Option<u32> {
    let g = dm.create_any(class, None).ok()?;
    dm.set_prop_quiet(g, "Name", Val::Str(name.into()));
    dm.get_mut_name(g, name);
    for (k, v) in props {
        dm.set_prop_quiet(g, k, v.clone());
    }
    dm.link_quiet(g, parent);
    Some(g)
}

fn ud(a: [f64; 4]) -> Val {
    Val::Arr(a.to_vec())
}

fn control_button(dm: &mut DataModel, parent: u32, name: &str, image: &str, pos: [f64; 4], size: [f64; 4]) {
    gui(dm, "ImageButton", name, Some(parent), &[("Image", Val::Str(format!("rbxasset://textures/{}.png", image))), ("BackgroundTransparency", Val::Num(1.0)), ("Position", ud(pos)), ("Size", ud(size)), ("ZIndex", Val::Num(2.0))]);
}

fn kid_named(dm: &DataModel, parent: u32, name: &str) -> Option<u32> {
    dm.get(parent).and_then(|i| i.children.iter().copied().find(|c| dm.name(*c) == name))
}

fn build_leave_menu(dm: &mut DataModel, cf: u32, hidden: bool) -> Option<[u32; 5]> {
    let img = |n: &str| Val::Str(format!("rbxasset://textures/ui/{}.png", n));
    let t = |v: f64| ("BackgroundTransparency", Val::Num(v));
    let leave = gui(dm, "ImageButton", "LeaveButton", Some(cf), &[("Image", img("LeaveMenuLeaveBtn")), t(1.0), ("Position", ud([0.0, 6.0, 0.0, 6.0])), ("Size", ud([0.0, 150.0, 0.0, 56.0])), ("ZIndex", Val::Num(8.0))])?;
    let shield = gui(dm, "TextButton", "LeaveMenuShield", Some(cf), &[("Text", Val::Str(String::new())), ("AutoButtonColor", Val::Bool(false)), ("Active", Val::Bool(true)), ("Visible", Val::Bool(false)), ("BackgroundColor3", Val::Arr(vec![0.0, 0.0, 0.0])), t(0.5), ("BorderSizePixel", Val::Num(0.0)), ("Size", ud([1.0, 0.0, 1.0, 0.0])), ("ZIndex", Val::Num(9.0))])?;
    let menu = gui(dm, "ImageLabel", "LeaveMenu", Some(shield), &[("Image", img("LeaveMenuBkgd")), t(1.0), ("Position", ud([0.5, -220.0, 0.5, -160.0])), ("Size", ud([0.0, 440.0, 0.0, 256.0])), ("ZIndex", Val::Num(9.0))])?;
    gui(dm, "TextLabel", "Title", Some(menu), &[("Text", Val::Str("Leave this game?".into())), t(1.0), ("Position", ud([0.0, 0.0, 0.0, 40.0])), ("Size", ud([1.0, 0.0, 0.0, 60.0])), ("Font", Val::Num(2.0)), ("FontSize", Val::Num(7.0)), ("TextColor3", Val::Arr(vec![1.0, 1.0, 1.0])), ("ZIndex", Val::Num(10.0))]);
    let yes = gui(dm, "ImageButton", "Leave", Some(menu), &[("Image", img("LeaveMenuLeaveBtn")), t(1.0), ("Position", ud([0.5, -160.0, 1.0, -86.0])), ("Size", ud([0.0, 150.0, 0.0, 56.0])), ("ZIndex", Val::Num(10.0))])?;
    let no = gui(dm, "ImageButton", "Cancel", Some(menu), &[("Image", img("LeaveMenuCancelBtn")), t(1.0), ("Position", ud([0.5, 10.0, 1.0, -86.0])), ("Size", ud([0.0, 150.0, 0.0, 56.0])), ("ZIndex", Val::Num(10.0))])?;
    let toggle = gui(dm, "TextButton", "HideMenuButton", Some(menu), &[("Text", Val::Str("Hide menu button".into())), ("BackgroundColor3", Val::Arr(vec![0.2, 0.2, 0.2])), t(0.3), ("BorderSizePixel", Val::Num(0.0)), ("Position", ud([0.5, -110.0, 0.0, 120.0])), ("Size", ud([0.0, 220.0, 0.0, 36.0])), ("Font", Val::Num(2.0)), ("FontSize", Val::Num(5.0)), ("TextColor3", Val::Arr(vec![1.0, 1.0, 1.0])), ("ZIndex", Val::Num(10.0))])?;
    if hidden {
        dm.set_prop_quiet(leave, "Visible", Val::Bool(false));
    }
    Some([leave, shield, yes, no, toggle])
}

fn menu_hidden() -> bool {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten()).and_then(|s| s.get_item("swirl-touch-menu").ok().flatten()).map(|v| v == "hidden").unwrap_or(false)
}

fn build_control_frame(dm: &mut DataModel, robloxgui: u32, touch: bool) -> (Vec<u32>, Option<[u32; 5]>) {
    let t = |v: f64| ("BackgroundTransparency", Val::Num(v));
    let mut exits = vec![];
    let cf = match gui(dm, "Frame", "ControlFrame", Some(robloxgui), &[t(1.0), ("Size", ud([1.0, 0.0, 1.0, 0.0]))]) {
        Some(c) => c,
        None => return (exits, None),
    };
    if let Some(bl) = gui(dm, "Frame", "BottomLeftControl", Some(cf), &[t(1.0), ("Position", ud([0.0, 0.0, 1.0, -50.0])), ("Size", ud([0.0, 130.0, 0.0, 50.0]))]) {
        control_button(dm, bl, "Exit", "Exit", [0.0, 0.0, 0.0, -1.0], [0.0, 70.0, 0.0, 51.0]);
    }
    if touch {
    } else if let Some(br) = gui(dm, "Frame", "BottomRightControl", Some(cf), &[t(1.0), ("Position", ud([1.0, -50.0, 1.0, -48.0])), ("Size", ud([0.0, 48.0, 0.0, 46.0]))]) {
        control_button(dm, br, "CameraTiltUp", "CameraTiltUp", [0.0, 2.0, 0.0, 0.0], [0.0, 22.0, 0.0, 22.0]);
        control_button(dm, br, "CameraTiltDown", "CameraTiltDown", [0.0, 2.0, 0.0, 23.0], [0.0, 22.0, 0.0, 22.0]);
        control_button(dm, br, "CameraZoomIn", "CameraZoomIn", [0.0, 25.0, 0.0, 0.0], [0.0, 22.0, 0.0, 22.0]);
        control_button(dm, br, "CameraZoomOut", "CameraZoomOut", [0.0, 25.0, 0.0, 23.0], [0.0, 22.0, 0.0, 22.0]);
    }
    if let Some(tl) = gui(dm, "Frame", "TopLeftControl", Some(cf), &[t(1.0), ("Position", ud([0.0, 0.0, 0.0, 0.0])), ("Size", ud([0.0, 115.0, 0.0, 51.0]))]) {
        control_button(dm, tl, "Help", "Help", [0.0, 0.0, 0.0, 0.0], [0.0, 45.0, 0.0, 51.0]);
        control_button(dm, tl, "Exit", "Exit", [0.0, 45.0, 0.0, 0.0], [0.0, 70.0, 0.0, 51.0]);
    }
    for p in [kid_named(dm, cf, "BottomLeftControl"), kid_named(dm, cf, "TopLeftControl")].into_iter().flatten() {
        if let Some(e) = kid_named(dm, p, "Exit") {
            exits.push(e);
        }
    }
    let menu = if touch { build_leave_menu(dm, cf, menu_hidden()) } else { None };
    (exits, menu)
}

fn boot(dm: &mut DataModel, screen: [f64; 2], touch: bool) -> (Option<u32>, Vec<u32>, Option<[u32; 5]>) {
    let mut exits = vec![];
    let mut menu = None;
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
        if let Some(g) = gui(dm, "ScreenGui", "RobloxGui", Some(cg), &[("AbsoluteSize", Val::Arr(vec![screen[0], screen[1]]))]) {
            if dm.era != 2018 {
                let r = build_control_frame(dm, g, touch);
                exits = r.0;
                menu = r.1;
            }
        }
    }
    (mouse_id, exits, menu)
}

fn flag_true(dm: &DataModel, id: u32, k: &str) -> bool {
    matches!(dm.get_prop(id, k), Some(Val::Bool(true)))
}

fn val_to_wire_simple(v: &Val) -> swirl_lua::host::Wire {
    use swirl_lua::host::Wire;
    match v {
        Val::Nil => Wire::Nil,
        Val::Bool(b) => Wire::B(*b),
        Val::Num(n) => Wire::N(*n),
        Val::Str(s) => Wire::S(s.clone()),
        Val::Ref(Some(i)) => Wire::I(*i),
        Val::Ref(None) => Wire::Nil,
        Val::Arr(a) if a.len() == 3 => Wire::V3([a[0], a[1], a[2]]),
        Val::Arr(a) if a.len() == 2 => Wire::V2([a[0], a[1]]),
        Val::Arr(a) if a.len() == 12 => {
            let mut c = [0.0; 12];
            c.copy_from_slice(a);
            Wire::Cf(c)
        }
        Val::Arr(a) if a.len() == 4 => Wire::Ud2([a[0], a[1], a[2], a[3]]),
        Val::Arr(_) => Wire::Nil,
    }
}

fn intern(s: &str) -> &'static str {
    thread_local! {
        static NAMES: RefCell<std::collections::HashMap<String, &'static str>> = RefCell::new(std::collections::HashMap::new());
    }
    NAMES.with(|n| {
        let mut n = n.borrow_mut();
        if let Some(v) = n.get(s) {
            return *v;
        }
        let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
        n.insert(s.to_string(), leaked);
        leaked
    })
}

fn json_to_wire(j: &serde_json::Value) -> swirl_lua::host::Wire {
    use swirl_lua::host::Wire;
    match j {
        serde_json::Value::Null => Wire::Nil,
        serde_json::Value::Bool(b) => Wire::B(*b),
        serde_json::Value::Number(n) => Wire::N(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Wire::S(s.clone()),
        serde_json::Value::Array(a) => Wire::List(a.iter().map(json_to_wire).collect()),
        serde_json::Value::Object(o) => Wire::Dict(o.iter().map(|(k, v)| (k.clone(), json_to_wire(v))).collect()),
    }
}

fn key_name(code: &str) -> Option<String> {
    let fixed = [
        ("Backspace", "Backspace"), ("Tab", "Tab"), ("Enter", "Return"), ("NumpadEnter", "KeypadEnter"), ("Escape", "Escape"), ("Space", "Space"),
        ("Quote", "Quote"), ("Comma", "Comma"), ("Minus", "Minus"), ("Period", "Period"), ("Slash", "Slash"), ("Semicolon", "Semicolon"), ("Equal", "Equals"),
        ("BracketLeft", "LeftBracket"), ("Backslash", "BackSlash"), ("BracketRight", "RightBracket"), ("Backquote", "Backquote"),
        ("Delete", "Delete"), ("Insert", "Insert"), ("Home", "Home"), ("End", "End"), ("PageUp", "PageUp"), ("PageDown", "PageDown"),
        ("ArrowUp", "Up"), ("ArrowDown", "Down"), ("ArrowLeft", "Left"), ("ArrowRight", "Right"),
        ("ShiftLeft", "LeftShift"), ("ShiftRight", "RightShift"), ("ControlLeft", "LeftControl"), ("ControlRight", "RightControl"),
        ("AltLeft", "LeftAlt"), ("AltRight", "RightAlt"), ("MetaLeft", "LeftSuper"), ("MetaRight", "RightSuper"), ("CapsLock", "CapsLock"),
        ("NumLock", "NumLock"), ("ScrollLock", "ScrollLock"), ("Pause", "Pause"), ("PrintScreen", "Print"),
        ("NumpadDecimal", "KeypadPeriod"), ("NumpadDivide", "KeypadDivide"), ("NumpadMultiply", "KeypadMultiply"), ("NumpadSubtract", "KeypadMinus"), ("NumpadAdd", "KeypadPlus"),
    ];
    if let Some((_, n)) = fixed.iter().find(|(c, _)| *c == code) {
        return Some(n.to_string());
    }
    let digits = ["Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine"];
    if let Some(d) = code.strip_prefix("Digit").and_then(|d| d.parse::<usize>().ok()).filter(|d| *d < 10) {
        return Some(digits[d].to_string());
    }
    if let Some(d) = code.strip_prefix("Numpad").and_then(|d| d.parse::<usize>().ok()).filter(|d| *d < 10) {
        return Some(format!("Keypad{}", digits[d]));
    }
    if let Some(l) = code.strip_prefix("Key") {
        if l.len() == 1 {
            return Some(l.to_string());
        }
    }
    if let Some(n) = code.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()).filter(|n| (1..=15).contains(n)) {
        return Some(format!("F{}", n));
    }
    None
}

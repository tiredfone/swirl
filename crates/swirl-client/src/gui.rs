use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use swirl_core::instance::{DataModel, InstId, Val, LOCAL_BASE};
use swirl_lua::host::Wire;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Event, HtmlElement, HtmlImageElement};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = solarImageUrl, catch)]
    fn solar_image_url(url: &str) -> Result<JsValue, JsValue>;
}

const FONT_SIZES: [f64; 15] = [8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 18.0, 24.0, 36.0, 48.0, 28.0, 32.0, 42.0, 60.0, 96.0];

pub enum GuiEv {
    Fire(InstId, &'static str, Vec<f64>, bool),
    Focus(InstId),
    Commit(InstId, String, bool),
}

struct Node {
    el: HtmlElement,
    kids: Option<HtmlElement>,
    text: Option<HtmlElement>,
    img: Option<HtmlImageElement>,
    clip: Option<HtmlElement>,
    sizer: Option<HtmlElement>,
    state: Rc<Cell<(bool, bool)>>,
    img_url: String,
    img_src: Option<String>,
}

pub struct Gui {
    doc: Document,
    pub overlay: HtmlElement,
    screen: HtmlElement,
    nodes: HashMap<InstId, Node>,
    closures: Vec<Closure<dyn FnMut(Event)>>,
    pub events: Rc<RefCell<Vec<GuiEv>>>,
    pub dirty: Rc<Cell<bool>>,
    scale_key: String,
    pub focused: Rc<Cell<Option<InstId>>>,
    last_state: HashMap<InstId, (bool, bool)>,
    hint: HtmlElement,
    message: HtmlElement,
    pub forced_message: Option<String>,
    pending_focus: Option<(InstId, u32)>,
    billboards: HtmlElement,
    bb_live: HashSet<InstId>,
    bb_sight: HashMap<InstId, (f64, bool, u32)>,
    bb_world: Vec<InstId>,
    bb_frame: u32,
}

struct Kids {
    inner: [f64; 2],
    area: [f64; 2],
    objs: Vec<InstId>,
    sizes: HashMap<InstId, [f64; 2]>,
    placed: Option<HashMap<InstId, [f64; 2]>>,
    scroll: bool,
    canvas: [f64; 2],
}

fn ud(v: Option<&Val>, base: f64) -> f64 {
    match v {
        Some(Val::Arr(a)) if a.len() >= 2 => base * a[0] + a[1],
        _ => 0.0,
    }
}

fn arr4(d: &DataModel, id: InstId, k: &str, dflt: [f64; 4]) -> [f64; 4] {
    match d.get_prop(id, k) {
        Some(Val::Arr(a)) if a.len() >= 4 => [a[0], a[1], a[2], a[3]],
        _ => dflt,
    }
}

fn arr2(d: &DataModel, id: InstId, k: &str, dflt: [f64; 2]) -> [f64; 2] {
    match d.get_prop(id, k) {
        Some(Val::Arr(a)) if a.len() >= 2 => [a[0], a[1]],
        _ => dflt,
    }
}

fn num(d: &DataModel, id: InstId, k: &str, dflt: f64) -> f64 {
    match d.get_prop(id, k) {
        Some(Val::Num(n)) => *n,
        _ => dflt,
    }
}

fn flag(d: &DataModel, id: InstId, k: &str, dflt: bool) -> bool {
    match d.get_prop(id, k) {
        Some(Val::Bool(b)) => *b,
        _ => dflt,
    }
}

fn text(d: &DataModel, id: InstId) -> String {
    match d.get_prop(id, "Text") {
        Some(Val::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

fn rgba(c: &[f64], t: f64) -> String {
    let g = |i: usize| (c.get(i).copied().unwrap_or(0.0) * 255.0).round();
    format!("rgba({},{},{},{})", g(0), g(1), g(2), (1.0 - t).max(0.0).min(1.0))
}

fn color(d: &DataModel, id: InstId, k: &str) -> Vec<f64> {
    match d.get_prop(id, k) {
        Some(Val::Arr(a)) => a.clone(),
        _ => vec![0.0, 0.0, 0.0],
    }
}

fn gui_kids(d: &DataModel, parent: InstId) -> Vec<InstId> {
    let mut out = vec![];
    for c in d.get(parent).map(|i| i.children.clone()).unwrap_or_default() {
        if d.is_a(c, "GuiObject") {
            out.push(c);
        } else if d.get(c).map(|i| i.class == "Folder").unwrap_or(false) {
            out.extend(gui_kids(d, c));
        }
    }
    out
}

fn sort_kids(d: &DataModel, mut kids: Vec<InstId>, order: i64) -> Vec<InstId> {
    if order == 0 {
        kids.sort_by(|a, b| d.name(*a).cmp(d.name(*b)));
    } else if order == 2 {
        kids.sort_by_key(|c| num(d, *c, "LayoutOrder", 0.0) as i64);
    }
    kids
}

fn style(e: &HtmlElement, k: &str, v: &str) {
    let _ = e.style().set_property(k, v);
}

impl Gui {
    pub fn new(root: &HtmlElement) -> Option<Gui> {
        let doc = web_sys::window()?.document()?;
        let mk = |cls: &str, parent: &HtmlElement| -> Option<HtmlElement> {
            let e = doc.create_element("div").ok()?.dyn_into::<HtmlElement>().ok()?;
            e.set_class_name(cls);
            parent.append_child(&e).ok()?;
            Some(e)
        };
        let overlay = mk("ch-gui", root)?;
        let screen = mk("ch-screen", &overlay)?;
        let billboards = mk("ch-billboards", &overlay)?;
        let hint = mk("ch-hint", &overlay)?;
        hint.set_hidden(true);
        let message = mk("ch-message", &overlay)?;
        message.set_hidden(true);
        Some(Gui { doc, overlay, screen, nodes: HashMap::new(), closures: vec![], events: Rc::new(RefCell::new(vec![])), dirty: Rc::new(Cell::new(true)), scale_key: String::new(), focused: Rc::new(Cell::new(None)), last_state: HashMap::new(), hint, message, forced_message: None, pending_focus: None, billboards, bb_live: HashSet::new(), bb_sight: HashMap::new(), bb_world: vec![], bb_frame: 0 })
    }

    fn el(&self, tag: &str, cls: &str, parent: &HtmlElement) -> HtmlElement {
        let e = self.doc.create_element(tag).unwrap().dyn_into::<HtmlElement>().unwrap();
        e.set_class_name(cls);
        let _ = parent.append_child(&e);
        e
    }

    fn listen(&mut self, e: &HtmlElement, ev: &str, f: Box<dyn FnMut(Event)>) {
        let c = Closure::wrap(f);
        let _ = e.add_event_listener_with_callback(ev, c.as_ref().unchecked_ref());
        self.closures.push(c);
    }

    fn node_for(&mut self, d: &DataModel, inst: InstId, parent_el: &HtmlElement) -> Option<()> {
        if let Some(n) = self.nodes.get(&inst) {
            if n.el.parent_element().as_ref() != Some(parent_el) {
                let _ = parent_el.append_child(&n.el);
                if let Some(k) = &n.kids {
                    let _ = parent_el.append_child(k);
                }
            }
            return Some(());
        }
        let class = d.get(inst)?.class.clone();
        let is_box = class == "TextBox";
        let is_btn = d.is_a(inst, "GuiButton");
        let e = if is_box {
            let tag = if flag(d, inst, "MultiLine", false) { "textarea" } else { "input" };
            let e = self.el(tag, "ch-g ch-g-box", parent_el);
            let _ = e.set_attribute("spellcheck", "false");
            e
        } else {
            self.el("div", "ch-g", parent_el)
        };
        let kids = if is_box { Some(self.el("div", "ch-g ch-g-pass", parent_el)) } else { None };
        if !is_btn && !is_box {
            let _ = e.class_list().add_1("ch-g-pass");
        }
        let text_el = if class == "TextLabel" || class == "TextButton" { Some(self.el("span", "ch-g-text", &e)) } else { None };
        let (clip, img) = if class == "ImageLabel" || class == "ImageButton" {
            let clip = self.el("div", "ch-g-clip", &e);
            let img = self.el("img", "ch-g-img", &clip).dyn_into::<HtmlImageElement>().ok()?;
            img.set_alt("");
            img.set_draggable(false);
            (Some(clip), Some(img))
        } else {
            (None, None)
        };
        let state = Rc::new(Cell::new((false, false)));
        let evs = self.events.clone();
        let dirty = self.dirty.clone();
        let replicated = inst < LOCAL_BASE;
        if is_btn {
            let _ = e.class_list().add_1("ch-g-btn");
            for (dom, name) in [("pointerdown", "MouseButton1Down"), ("pointerup", "MouseButton1Up")] {
                let evs2 = evs.clone();
                self.listen(&e, dom, Box::new(move |ev: Event| {
                    ev.stop_propagation();
                    if let Ok(pe) = ev.dyn_into::<web_sys::PointerEvent>() {
                        let (nm, ok) = match (name, pe.button()) {
                            ("MouseButton1Down", 0) => ("MouseButton1Down", true),
                            ("MouseButton1Down", 2) => ("MouseButton2Down", true),
                            ("MouseButton1Up", 0) => ("MouseButton1Up", true),
                            ("MouseButton1Up", 2) => ("MouseButton2Up", true),
                            _ => ("", false),
                        };
                        if ok {
                            evs2.borrow_mut().push(GuiEv::Fire(inst, nm, vec![pe.offset_x() as f64, pe.offset_y() as f64, pe.client_x() as f64, pe.client_y() as f64], replicated));
                        }
                    }
                }));
            }
            let evs2 = evs.clone();
            self.listen(&e, "click", Box::new(move |ev: Event| {
                ev.stop_propagation();
                let (cx, cy) = ev.dyn_ref::<web_sys::MouseEvent>().map(|m| (m.client_x() as f64, m.client_y() as f64)).unwrap_or((0.0, 0.0));
                evs2.borrow_mut().push(GuiEv::Fire(inst, "MouseButton1Click", vec![cx, cy], replicated));
            }));
            let evs2 = evs.clone();
            self.listen(&e, "contextmenu", Box::new(move |ev: Event| {
                ev.prevent_default();
                ev.stop_propagation();
                let (cx, cy) = ev.dyn_ref::<web_sys::MouseEvent>().map(|m| (m.client_x() as f64, m.client_y() as f64)).unwrap_or((0.0, 0.0));
                evs2.borrow_mut().push(GuiEv::Fire(inst, "MouseButton2Click", vec![cx, cy], replicated));
            }));
            let (st, dr) = (state.clone(), dirty.clone());
            self.listen(&e, "pointerenter", Box::new(move |_| {
                st.set((true, st.get().1));
                dr.set(true);
            }));
            let (st, dr) = (state.clone(), dirty.clone());
            self.listen(&e, "pointerleave", Box::new(move |_| {
                st.set((false, false));
                dr.set(true);
            }));
            let (st, dr) = (state.clone(), dirty.clone());
            self.listen(&e, "pointerdown", Box::new(move |_| {
                st.set((st.get().0, true));
                dr.set(true);
            }));
            let (st, dr) = (state.clone(), dirty.clone());
            self.listen(&e, "pointerup", Box::new(move |_| {
                st.set((st.get().0, false));
                dr.set(true);
            }));
        }
        if d.is_a(inst, "GuiObject") {
            for (dom, name) in [("pointerenter", "MouseEnter"), ("pointerleave", "MouseLeave")] {
                let evs2 = evs.clone();
                self.listen(&e, dom, Box::new(move |ev: Event| {
                    let (ox, oy) = ev.dyn_ref::<web_sys::MouseEvent>().map(|m| (m.offset_x() as f64, m.offset_y() as f64)).unwrap_or((0.0, 0.0));
                    evs2.borrow_mut().push(GuiEv::Fire(inst, name, vec![ox, oy], replicated && is_btn));
                }));
            }
        }
        if is_box {
            let evs2 = evs.clone();
            let focused = self.focused.clone();
            self.listen(&e, "focus", Box::new(move |_| {
                focused.set(Some(inst));
                evs2.borrow_mut().push(GuiEv::Focus(inst));
            }));
            let (evs2, focused, e2) = (evs.clone(), self.focused.clone(), e.clone());
            self.listen(&e, "keydown", Box::new(move |ev: Event| {
                ev.stop_propagation();
                if let Some(k) = ev.dyn_ref::<web_sys::KeyboardEvent>() {
                    if k.key() == "Enter" && e2.tag_name().eq_ignore_ascii_case("input") {
                        if focused.get() == Some(inst) {
                            focused.set(None);
                            let v = e2.clone().dyn_into::<web_sys::HtmlInputElement>().map(|i| i.value()).unwrap_or_default();
                            evs2.borrow_mut().push(GuiEv::Commit(inst, v, true));
                        }
                        let _ = e2.blur();
                    }
                }
            }));
            let (evs2, focused, e2) = (evs.clone(), self.focused.clone(), e.clone());
            self.listen(&e, "blur", Box::new(move |_| {
                if focused.get() == Some(inst) {
                    focused.set(None);
                    let v = e2.clone().dyn_into::<web_sys::HtmlInputElement>().map(|i| i.value()).or_else(|_| e2.clone().dyn_into::<web_sys::HtmlTextAreaElement>().map(|t| t.value())).unwrap_or_default();
                    evs2.borrow_mut().push(GuiEv::Commit(inst, v, false));
                }
            }));
        }
        let _ = e.set_attribute("data-ipath", &d.full_name(inst));
        self.nodes.insert(inst, Node { el: e, kids, text: text_el, img, clip, sizer: None, state, img_url: String::new(), img_src: None });
        Some(())
    }

    fn set_abs(d: &mut DataModel, c: InstId, name: &str, v: [f64; 2], fired: &mut Vec<(InstId, &'static str, Vec<Wire>)>) {
        if let Some(Val::Arr(o)) = d.get_prop(c, name) {
            if o.len() >= 2 && o[0] == v[0] && o[1] == v[1] {
                return;
            }
        }
        d.set_prop_quiet(c, name, Val::Arr(v.to_vec()));
        fired.push((c, "Changed", vec![Wire::S(name.to_string())]));
        let p: &'static str = match name {
            "AbsoluteSize" => "P:AbsoluteSize",
            "AbsolutePosition" => "P:AbsolutePosition",
            _ => "P:AbsoluteContentSize",
        };
        fired.push((c, p, vec![]));
    }

    fn constrain(d: &DataModel, c: InstId, mut w: f64, mut h: f64, area: Option<[f64; 2]>) -> [f64; 2] {
        for k in d.get(c).map(|i| i.children.clone()).unwrap_or_default() {
            let n = d.get(k).map(|i| i.class.clone()).unwrap_or_default();
            if n == "UIAspectRatioConstraint" {
                let ar = num(d, k, "AspectRatio", 1.0).max(1e-3);
                if num(d, k, "AspectType", 0.0) as i64 == 1 {
                    if let (Some(a), 2018) = (area, d.era) {
                        w = a[0];
                        h = a[1];
                        if w / h.max(1e-3) > ar {
                            w = h * ar;
                        } else {
                            h = w / ar;
                        }
                    } else if num(d, k, "DominantAxis", 0.0) as i64 == 1 {
                        w = h * ar;
                    } else {
                        h = w / ar;
                    }
                } else if w / h.max(1e-3) > ar {
                    w = h * ar;
                } else {
                    h = w / ar;
                }
            } else if n == "UISizeConstraint" {
                let mn = arr2(d, k, "MinSize", [0.0, 0.0]);
                let mx = arr2(d, k, "MaxSize", [1e9, 1e9]);
                w = mn[0].max(mx[0].min(w));
                h = mn[1].max(mx[1].min(h));
            }
        }
        [w.round(), h.round()]
    }

    fn arrange(d: &mut DataModel, layout: InstId, objs: &[InstId], sizes: &mut HashMap<InstId, [f64; 2]>, area: [f64; 2], fired: &mut Vec<(InstId, &'static str, Vec<Wire>)>) -> HashMap<InstId, [f64; 2]> {
        let mut pos: HashMap<InstId, [f64; 2]> = HashMap::new();
        let vis: Vec<InstId> = objs.iter().copied().filter(|c| flag(d, *c, "Visible", true)).collect();
        let list = sort_kids(d, vis, num(d, layout, "SortOrder", 0.0) as i64);
        let ha = num(d, layout, "HorizontalAlignment", 0.0) as i64;
        let va = num(d, layout, "VerticalAlignment", 0.0) as i64;
        let align_x = |span: f64| if ha == 0 { (area[0] - span) / 2.0 } else if ha == 2 { area[0] - span } else { 0.0 };
        let align_y = |span: f64| if va == 0 { (area[1] - span) / 2.0 } else if va == 2 { area[1] - span } else { 0.0 };
        let content;
        let class = d.get(layout).map(|i| i.class.clone()).unwrap_or_default();
        if class == "UIGridLayout" {
            let cs = arr4(d, layout, "CellSize", [0.0, 100.0, 0.0, 100.0]);
            let cp = arr4(d, layout, "CellPadding", [0.0, 5.0, 0.0, 5.0]);
            let cw = area[0] * cs[0] + cs[1];
            let chh = area[1] * cs[2] + cs[3];
            let px = area[0] * cp[0] + cp[1];
            let py = area[1] * cp[2] + cp[3];
            let vertical = num(d, layout, "FillDirection", 0.0) as i64 == 1;
            let max_cells = num(d, layout, "FillDirectionMaxCells", 0.0) as i64;
            let mut per = if vertical { ((area[1] + py) / (chh + py).max(1.0)).floor() } else { ((area[0] + px) / (cw + px).max(1.0)).floor() };
            if max_cells > 0 {
                per = per.min(max_cells as f64);
            }
            per = per.max(1.0);
            let n = list.len() as f64;
            let major = (n / per).ceil();
            let minor = n.min(per);
            let (cols, rows) = if vertical { (major, minor) } else { (minor, major) };
            content = [(cols * (cw + px) - px).max(0.0), (rows * (chh + py) - py).max(0.0)];
            let (ox, oy) = (align_x(content[0]), align_y(content[1]));
            for (i, c) in list.iter().enumerate() {
                let a = (i as f64 / per).floor();
                let b = i as f64 % per;
                let (col, row) = if vertical { (a, b) } else { (b, a) };
                sizes.insert(*c, [cw.round(), chh.round()]);
                pos.insert(*c, [(ox + col * (cw + px)).round(), (oy + row * (chh + py)).round()]);
            }
        } else {
            let vertical = num(d, layout, "FillDirection", 0.0) as i64 == 1;
            let pad = ud(d.get_prop(layout, "Padding"), if vertical { area[1] } else { area[0] });
            let (mut total, mut cross) = (0.0f64, 0.0f64);
            for c in &list {
                let sz = sizes[c];
                total += if vertical { sz[1] } else { sz[0] };
                cross = cross.max(if vertical { sz[0] } else { sz[1] });
            }
            total += (list.len() as f64 - 1.0).max(0.0) * pad;
            content = if vertical { [cross, total] } else { [total, cross] };
            let mut at = if vertical { align_y(total) } else { align_x(total) };
            for c in &list {
                let sz = sizes[c];
                if vertical {
                    pos.insert(*c, [align_x(sz[0]).round(), at.round()]);
                    at += sz[1] + pad;
                } else {
                    pos.insert(*c, [at.round(), align_y(sz[1]).round()]);
                    at += sz[0] + pad;
                }
            }
        }
        Self::set_abs(d, layout, "AbsoluteContentSize", content, fired);
        pos
    }

    fn measure_kids(d: &mut DataModel, parent: InstId, p_size: [f64; 2], scroll: bool, fired: &mut Vec<(InstId, &'static str, Vec<Wire>)>) -> Kids {
        let mut inner = [0.0, 0.0];
        let mut area = p_size;
        let mut canvas = [0.0, 0.0];
        let is_scroll = scroll && d.get(parent).map(|i| i.class == "ScrollingFrame").unwrap_or(false);
        if is_scroll {
            let cs = arr4(d, parent, "CanvasSize", [0.0, 0.0, 2.0, 0.0]);
            canvas = [p_size[0].max(p_size[0] * cs[0] + cs[1]), p_size[1].max(p_size[1] * cs[2] + cs[3])];
            area = canvas;
        }
        let mut layout = None;
        for c in d.get(parent).map(|i| i.children.clone()).unwrap_or_default() {
            let n = d.get(c).map(|i| i.class.clone()).unwrap_or_default();
            if n == "UIPadding" {
                let l = ud(d.get_prop(c, "PaddingLeft"), area[0]);
                let r = ud(d.get_prop(c, "PaddingRight"), area[0]);
                let t = ud(d.get_prop(c, "PaddingTop"), area[1]);
                let b = ud(d.get_prop(c, "PaddingBottom"), area[1]);
                inner = [l, t];
                area = [(area[0] - l - r).max(0.0), (area[1] - t - b).max(0.0)];
            } else if n == "UIListLayout" || n == "UIGridLayout" {
                layout = Some(c);
            }
        }
        let objs = gui_kids(d, parent);
        let mut sizes = HashMap::new();
        for c in &objs {
            let sz = arr4(d, *c, "Size", [0.0, 0.0, 0.0, 0.0]);
            let cons = num(d, *c, "SizeConstraint", 0.0) as i64;
            let rx = if cons == 2 { area[1] } else { area[0] };
            let ry = if cons == 1 { area[0] } else { area[1] };
            sizes.insert(*c, Self::constrain(d, *c, rx * sz[0] + sz[1], ry * sz[2] + sz[3], Some(area)));
        }
        let placed = layout.map(|l| Self::arrange(d, l, &objs, &mut sizes, area, fired));
        Kids { inner, area, objs, sizes, placed, scroll: is_scroll, canvas }
    }

    fn pos_of(d: &DataModel, c: InstId, k: &Kids) -> [f64; 2] {
        let [w, h] = k.sizes[&c];
        let (x, y) = match k.placed.as_ref().and_then(|p| p.get(&c)) {
            Some(p) => (p[0], p[1]),
            None => {
                let ps = arr4(d, c, "Position", [0.0; 4]);
                let ap = arr2(d, c, "AnchorPoint", [0.0, 0.0]);
                ((k.area[0] * ps[0] + ps[1] - ap[0] * w).round(), (k.area[1] * ps[2] + ps[3] - ap[1] * h).round())
            }
        };
        [x + k.inner[0], y + k.inner[1]]
    }

    pub fn layout(&mut self, d: &mut DataModel, vw: f64, vh: f64, scale: f64, inset: [f64; 4], fired: &mut Vec<(InstId, &'static str, Vec<Wire>)>) {
        let k = scale;
        let (sw, sh) = ((vw * k).round(), (vh * k).round());
        let key = format!("{}x{}x{}", sw, sh, k);
        if self.scale_key != key {
            self.scale_key = key;
            style(&self.screen, "right", "auto");
            style(&self.screen, "bottom", "auto");
            style(&self.screen, "width", &format!("{}px", sw));
            style(&self.screen, "height", &format!("{}px", sh));
            style(&self.screen, "zoom", &format!("{}", 1.0 / k));
            style(&self.screen, "--ch-k", &format!("{}", k));
        }
        let [l, t, r, b] = inset;
        let w = (sw - l - r).max(0.0);
        let h = (sh - t - b).max(0.0);
        let mut live: HashSet<InstId> = HashSet::new();
        let mut guis = vec![];
        if let Some(cg) = d.services.get("CoreGui") {
            for c in d.get(*cg).map(|i| i.children.clone()).unwrap_or_default() {
                if d.is_a(c, "ScreenGui") {
                    guis.push(c);
                }
            }
        }
        if let Some(pg) = d.local_player.and_then(|p| d.find_child_of_class(p, "PlayerGui")) {
            for c in d.get(pg).map(|i| i.children.clone()).unwrap_or_default() {
                if d.is_a(c, "ScreenGui") {
                    guis.push(c);
                }
            }
        }
        let screen = self.screen.clone();
        for (order, sg) in guis.into_iter().enumerate() {
            let _ = order;
            let abs = arr2(d, sg, "AbsoluteSize", [-1.0, -1.0]);
            if abs[0] != w || abs[1] != h {
                d.set_prop_quiet(sg, "AbsoluteSize", Val::Arr(vec![w, h]));
                fired.push((sg, "Changed", vec![Wire::S("AbsoluteSize".into())]));
                fired.push((sg, "P:AbsoluteSize", vec![]));
            }
            if self.node_for(d, sg, &screen).is_none() {
                continue;
            }
            live.insert(sg);
            let in_core = d.get(sg).and_then(|i| i.parent).map(|p| d.get(p).map(|x| x.class == "CoreGui").unwrap_or(false)).unwrap_or(false);
            let z = if in_core { 1000.0 } else { 10.0 } + num(d, sg, "DisplayOrder", 0.0);
            let el = self.nodes[&sg].el.clone();
            style(&el, "z-index", &format!("{}", z));
            style(&el, "display", if flag(d, sg, "Enabled", true) { "" } else { "none" });
            style(&el, "inset", &format!("{}px {}px {}px {}px", t, r, b, l));
            self.layout_children(d, sg, [l, t], [w, h], &mut live, fired);
        }
        let mut hint_text: Option<String> = None;
        let mut msg_text: Option<String> = self.forced_message.clone().filter(|m| !m.is_empty());
        let pg = d.local_player.and_then(|p| d.find_child_of_class(p, "PlayerGui"));
        for src in [d.services.get("Workspace").copied(), pg] {
            if let Some(src) = src {
                for c in d.get(src).map(|i| i.children.clone()).unwrap_or_default() {
                    let cls = d.get(c).map(|i| i.class.clone()).unwrap_or_default();
                    if cls == "Hint" {
                        hint_text = Some(text(d, c));
                    } else if cls == "Message" {
                        msg_text = Some(text(d, c));
                    }
                }
            }
        }
        let hint_text = hint_text.filter(|t| !t.is_empty());
        let msg_text = msg_text.filter(|t| !t.is_empty());
        self.hint.set_hidden(hint_text.is_none());
        if let Some(t) = hint_text {
            if self.hint.text_content().unwrap_or_default() != t {
                self.hint.set_text_content(Some(&t));
            }
        }
        self.message.set_hidden(msg_text.is_none());
        if let Some(t) = msg_text {
            if self.message.text_content().unwrap_or_default() != t {
                self.message.set_text_content(Some(&t));
            }
        }
        let dead: Vec<InstId> = self.nodes.keys().copied().filter(|i| !live.contains(i) && !self.bb_live.contains(i)).collect();
        for i in dead {
            if let Some(n) = self.nodes.remove(&i) {
                n.el.remove();
                if let Some(k) = n.kids {
                    k.remove();
                }
            }
        }
    }

    fn layout_children(&mut self, d: &mut DataModel, parent: InstId, p_pos: [f64; 2], p_size: [f64; 2], live: &mut HashSet<InstId>, fired: &mut Vec<(InstId, &'static str, Vec<Wire>)>) {
        let (pel, pkids) = match self.nodes.get(&parent) {
            Some(n) => (n.el.clone(), n.kids.clone()),
            None => return,
        };
        let kids = Self::measure_kids(d, parent, p_size, true, fired);
        let mut scroll_off = [0.0, 0.0];
        if kids.scroll {
            if self.nodes[&parent].sizer.is_none() {
                let s = self.el("div", "ch-g-sizer", &pel);
                if let Some(n) = self.nodes.get_mut(&parent) {
                    n.sizer = Some(s);
                }
            }
            let sizer = self.nodes[&parent].sizer.clone().unwrap();
            style(&sizer, "width", &format!("{}px", kids.canvas[0]));
            style(&sizer, "height", &format!("{}px", kids.canvas[1]));
            let want = arr2(d, parent, "CanvasPosition", [0.0, 0.0]);
            if (pel.scroll_left() as f64 - want[0]).abs() > 1.0 {
                pel.set_scroll_left(want[0] as i32);
            }
            if (pel.scroll_top() as f64 - want[1]).abs() > 1.0 {
                pel.set_scroll_top(want[1] as i32);
            }
            scroll_off = [pel.scroll_left() as f64, pel.scroll_top() as f64];
        }
        let target = pkids.unwrap_or(pel);
        for c in kids.objs.clone() {
            live.insert(c);
            if self.node_for(d, c, &target).is_none() {
                continue;
            }
            let [w, h] = kids.sizes[&c];
            let [x, y] = Self::pos_of(d, c, &kids);
            let abs = [p_pos[0] + x - scroll_off[0], p_pos[1] + y - scroll_off[1]];
            Self::set_abs(d, c, "AbsolutePosition", abs, fired);
            Self::set_abs(d, c, "AbsoluteSize", [w, h], fired);
            let class = d.get(c).map(|i| i.class.clone()).unwrap_or_default();
            let is_btn = d.is_a(c, "GuiButton");
            let (el, kel, state) = {
                let n = &self.nodes[&c];
                (n.el.clone(), n.kids.clone(), n.state.clone())
            };
            if !is_btn && class != "TextBox" {
                let _ = el.class_list().toggle_with_force("ch-g-pass", !flag(d, c, "Active", false) && class != "ScrollingFrame");
            }
            let visible = flag(d, c, "Visible", true);
            style(&el, "display", if visible { "" } else { "none" });
            if let Some(k) = &kel {
                if !visible {
                    style(k, "display", "none");
                }
            }
            if !visible {
                continue;
            }
            style(&el, "left", &format!("{}px", x));
            style(&el, "top", &format!("{}px", y));
            style(&el, "width", &format!("{}px", w.max(0.0)));
            style(&el, "height", &format!("{}px", h.max(0.0)));
            let z = num(d, c, "ZIndex", 1.0);
            style(&el, "z-index", &format!("{}", if z == 0.0 { 1.0 } else { z }));
            let scale_inst = d.get(c).and_then(|i| i.children.iter().copied().find(|k| d.get(*k).map(|x| x.class == "UIScale").unwrap_or(false)));
            let rot = num(d, c, "Rotation", 0.0);
            let sc = scale_inst.map(|s| num(d, s, "Scale", 1.0)).filter(|v| *v != 0.0).unwrap_or(1.0);
            let transform = if rot != 0.0 || sc != 1.0 { format!("{}{}", if rot != 0.0 { format!("rotate({}deg)", rot) } else { String::new() }, if sc != 1.0 { format!(" scale({})", sc) } else { String::new() }) } else { String::new() };
            style(&el, "transform", &transform);
            if sc != 1.0 {
                let ap = arr2(d, c, "AnchorPoint", [0.0, 0.0]);
                style(&el, "transform-origin", &format!("{}% {}%", ap[0] * 100.0, ap[1] * 100.0));
            }
            let clips = flag(d, c, "ClipsDescendants", false);
            if class == "ScrollingFrame" {
                let cs = arr4(d, c, "CanvasSize", [0.0, 0.0, 2.0, 0.0]);
                let (cw, chh) = (w * cs[0] + cs[1], h * cs[2] + cs[3]);
                let on = flag(d, c, "ScrollingEnabled", true);
                style(&el, "overflow", "");
                style(&el, "overflow-x", if on && cw > w + 1.0 { "auto" } else { "hidden" });
                style(&el, "overflow-y", if on && chh > h + 1.0 { "auto" } else { "hidden" });
                let _ = el.class_list().toggle_with_force("ch-g-pass", !(on && (cw > w + 1.0 || chh > h + 1.0)) && !flag(d, c, "Active", false));
                let _ = el.class_list().add_1("ch-g-scroll");
                let nobar = d.get(c).map(|i| i.props.contains_key("ScrollBarThickness")).unwrap_or(false) && (num(d, c, "ScrollBarThickness", 0.0) as i64) <= 0;
                let _ = el.class_list().toggle_with_force("ch-g-nobar", nobar);
            } else {
                style(&el, "overflow", if clips { "hidden" } else { "visible" });
            }
            let st = num(d, c, "Style", 0.0) as i64;
            let bt = num(d, c, "BackgroundTransparency", 0.0);
            let bc = color(d, c, "BackgroundColor3");
            let style_attr = if class == "Frame" && st > 0 { format!("frame{}", st) } else if is_btn && st > 0 { format!("button{}", st) } else { String::new() };
            let _ = el.dataset().set("style", &style_attr);
            let (hover, pressed) = state.get();
            let shade = if is_btn && flag(d, c, "AutoButtonColor", true) && style_attr.is_empty() { if pressed { 0.7 } else if hover { 0.85 } else { 1.0 } } else { 1.0 };
            if style_attr.is_empty() {
                let col: Vec<f64> = if shade < 1.0 { bc.iter().map(|v| v * shade).collect() } else { bc.clone() };
                style(&el, "background", &rgba(&col, bt));
            } else {
                style(&el, "background", "");
            }
            let bs = num(d, c, "BorderSizePixel", 0.0);
            let bt_border = num(d, c, "BackgroundTransparency", 1.0);
            let shadow = if bs > 0.0 && bt_border < 1.0 && style_attr.is_empty() { format!("0 0 0 {}px {}", bs, rgba(&color(d, c, "BorderColor3"), bt)) } else { "none".to_string() };
            style(&el, "box-shadow", &shadow);
            if is_btn {
                let _ = el.class_list().toggle_with_force("auto", flag(d, c, "AutoButtonColor", true));
            }
            if let Some(k) = &kel {
                for (prop, v) in [("display", String::new()), ("left", format!("{}px", x)), ("top", format!("{}px", y)), ("width", format!("{}px", w.max(0.0))), ("height", format!("{}px", h.max(0.0))), ("z-index", format!("{}", if z == 0.0 { 1.0 } else { z })), ("transform", transform.clone()), ("overflow", (if clips { "hidden" } else { "visible" }).to_string()), ("background", "transparent".to_string())] {
                    style(k, prop, &v);
                }
            }
            if self.nodes[&c].text.is_some() || class == "TextBox" {
                self.style_text(d, c, w, h);
            }
            if self.nodes[&c].img.is_some() {
                self.style_image(d, c, w, h, hover, pressed);
            }
            self.layout_children(d, c, abs, [w, h], live, fired);
        }
    }

    fn style_text(&mut self, d: &mut DataModel, c: InstId, w: f64, h: f64) {
        let (el, text_el) = {
            let n = &self.nodes[&c];
            (n.el.clone(), n.text.clone())
        };
        let is_box = d.get(c).map(|i| i.class == "TextBox").unwrap_or(false);
        let e = if is_box { el.clone() } else { match text_el { Some(t) => t, None => return } };
        let era = d.era;
        let font = num(d, c, "Font", 0.0) as i64;
        let txt = text(d, c);
        let mut px = if era == 2018 && d.get(c).map(|i| i.props.contains_key("TextSize")).unwrap_or(false) { num(d, c, "TextSize", 14.0) } else { FONT_SIZES.get(num(d, c, "FontSize", 0.0) as usize).copied().unwrap_or(12.0) };
        if flag(d, c, "TextScaled", false) {
            px = (h * 0.85).min(w / (txt.chars().count().max(1) as f64) * 1.8).floor().min(100.0).max(6.0);
        }
        style(&e, "font-family", "Arial, Helvetica, sans-serif");
        style(&e, "font-weight", if matches!(font, 0 | 2 | 4) { "700" } else { "400" });
        style(&e, "font-size", &format!("{}px", px));
        let tt = num(d, c, "TextTransparency", 0.0);
        style(&e, "color", &rgba(&color(d, c, "TextColor3"), tt));
        let stroke = num(d, c, "TextStrokeTransparency", 1.0);
        if stroke < 1.0 {
            let sc = rgba(&color(d, c, "TextStrokeColor3"), stroke.max(tt));
            style(&e, "text-shadow", &format!("-1px -1px 0 {s}, 1px -1px 0 {s}, -1px 1px 0 {s}, 1px 1px 0 {s}", s = sc));
        } else {
            style(&e, "text-shadow", "none");
        }
        let wrap = flag(d, c, "TextWrapped", false) || flag(d, c, "TextWrap", false);
        style(&e, "white-space", if wrap { "pre-wrap" } else { "pre" });
        let xa = num(d, c, "TextXAlignment", 2.0) as i64;
        let ya = num(d, c, "TextYAlignment", 1.0) as i64;
        let ta = if xa == 0 { "left" } else if xa == 1 { "right" } else { "center" };
        if is_box {
            style(&e, "text-align", ta);
            if let Ok(i) = e.clone().dyn_into::<web_sys::HtmlInputElement>() {
                let active = self.doc.active_element().map(|a| a == *AsRef::<web_sys::Element>::as_ref(&e)).unwrap_or(false);
                if !active && i.value() != txt {
                    i.set_value(&txt);
                }
            }
            return;
        }
        style(&el, "display", "flex");
        style(&el, "justify-content", if xa == 0 { "flex-start" } else if xa == 1 { "flex-end" } else { "center" });
        style(&el, "align-items", if ya == 0 { "flex-start" } else if ya == 2 { "flex-end" } else { "center" });
        style(&e, "text-align", ta);
        if e.text_content().unwrap_or_default() != txt {
            e.set_text_content(Some(&txt));
        }
    }

    fn style_image(&mut self, d: &DataModel, c: InstId, w: f64, h: f64, hover: bool, pressed: bool) {
        let (img, clip) = {
            let n = &self.nodes[&c];
            (n.img.clone().unwrap(), n.clip.clone().unwrap())
        };
        let get_s = |k: &str| match d.get_prop(c, k) {
            Some(Val::Str(s)) => s.clone(),
            _ => String::new(),
        };
        let hover_img = get_s("HoverImage");
        let pressed_img = get_s("PressedImage");
        let url = if pressed && !pressed_img.is_empty() { pressed_img } else if hover && !hover_img.is_empty() { hover_img } else { get_s("Image") };
        let changed = self.nodes[&c].img_url != url;
        if changed {
            let src = resolve_image(&url);
            if let Some(n) = self.nodes.get_mut(&c) {
                n.img_url = url.clone();
                n.img_src = src.clone();
            }
            img.remove_attribute("src").ok();
            match src {
                Some(s) => {
                    img.set_src(&s);
                    style(&img, "visibility", "");
                    let dr = self.dirty.clone();
                    let img_err = img.clone();
                    let ecb = Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_| {
                        style(img_err.unchecked_ref::<HtmlElement>(), "visibility", "hidden");
                    }));
                    let _ = img.add_event_listener_with_callback("error", ecb.as_ref().unchecked_ref());
                    self.closures.push(ecb);
                    let cb = Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_| dr.set(true)));
                    let _ = img.add_event_listener_with_callback("load", cb.as_ref().unchecked_ref());
                    self.closures.push(cb);
                }
                None => style(&img, "visibility", "hidden"),
            }
        }
        let mode = num(d, c, "ScaleType", 0.0) as i64;
        let src = self.nodes[&c].img_src.clone();
        let (nw, nh) = (img.natural_width() as f64, img.natural_height() as f64);
        let ic = color(d, c, "ImageColor3");
        if mode == 1 && src.is_some() && nw > 0.0 {
            let r = match d.get_prop(c, "SliceCenter") {
                Some(Val::Arr(a)) if a.len() >= 4 => [a[0], a[1], a[2], a[3]],
                _ => [0.0; 4],
            };
            let k = if d.get(c).map(|i| i.props.contains_key("SliceScale")).unwrap_or(false) { num(d, c, "SliceScale", 1.0).max(0.0).max(1e-9).min(1e9) } else { 1.0 };
            let (mut top, mut left, mut right, mut bottom) = (r[1].max(0.0), r[0].max(0.0), (nw - r[2]).max(0.0), (nh - r[3]).max(0.0));
            if left + right >= nw && nw > 1.0 {
                right = (nw - left - 1.0).max(0.0);
            }
            if left + right >= nw && nw > 1.0 {
                left = (nw - right - 1.0).max(0.0);
            }
            if top + bottom >= nh && nh > 1.0 {
                bottom = (nh - top - 1.0).max(0.0);
            }
            if top + bottom >= nh && nh > 1.0 {
                top = (nh - bottom - 1.0).max(0.0);
            }
            style(&img, "display", "none");
            style(&clip, "border-style", "solid");
            style(&clip, "border-width", &format!("{}px {}px {}px {}px", top * k, right * k, bottom * k, left * k));
            style(&clip, "border-image", &format!("url({}) {} {} {} {} fill stretch", src.unwrap(), top, right, bottom, left));
            style(&clip, "box-sizing", "border-box");
            style(&clip, "background", "");
        } else {
            if !clip.style().get_property_value("border-image").unwrap_or_default().is_empty() {
                style(&clip, "border-image", "");
                style(&clip, "border-width", "0");
                style(&clip, "border-style", "");
            }
            if mode == 2 && src.is_some() {
                let ts = arr4(d, c, "TileSize", [1.0, 0.0, 1.0, 0.0]);
                style(&img, "display", "none");
                style(&clip, "background", &format!("url({}) 0 0 / {}px {}px repeat", src.unwrap(), w * ts[0] + ts[1], h * ts[2] + ts[3]));
            } else {
                style(&clip, "background", "");
                style(&img, "display", "");
                style(&img, "object-fit", if mode == 3 { "contain" } else if mode == 4 { "cover" } else { "" });
                self.image_rect(d, c, &img, w, h, nw, nh);
            }
        }
        let it = num(d, c, "ImageTransparency", 0.0);
        style(&img, "opacity", &format!("{}", 1.0 - it));
        style(&clip, "opacity", &format!("{}", 1.0 - it));
        let bright = if ic.len() >= 3 && (ic[0] < 0.99 || ic[1] < 0.99 || ic[2] < 0.99) { format!("brightness({})", (ic[0] + ic[1] + ic[2]) / 3.0) } else { String::new() };
        style(&img, "filter", &bright);
    }

    fn image_rect(&self, d: &DataModel, c: InstId, img: &HtmlImageElement, w: f64, h: f64, nw: f64, nh: f64) {
        let rs = d.get_prop(c, "ImageRectSize");
        let ro = d.get_prop(c, "ImageRectOffset");
        let rs = match rs {
            Some(Val::Arr(a)) if a.len() >= 2 && a[0] > 0.0 && a[1] > 0.0 && nw > 0.0 && nh > 0.0 => [a[0], a[1]],
            _ => {
                if !img.style().get_property_value("width").unwrap_or_default().is_empty() {
                    for k in ["left", "top", "width", "height"] {
                        style(img, k, "");
                    }
                }
                return;
            }
        };
        let ro = match ro {
            Some(Val::Arr(a)) if a.len() >= 2 => [a[0], a[1]],
            _ => [0.0, 0.0],
        };
        let (kx, ky) = (w / rs[0], h / rs[1]);
        style(img, "left", &format!("{}px", -ro[0] * kx));
        style(img, "top", &format!("{}px", -ro[1] * ky));
        style(img, "width", &format!("{}px", nw * kx));
        style(img, "height", &format!("{}px", nh * ky));
    }

    pub fn commit_text(&mut self, d: &mut DataModel, inst: InstId, value: &str) {
        d.set_prop_quiet(inst, "Text", Val::Str(value.to_string()));
    }

    pub fn update_billboards(&mut self, d: &mut DataModel, ph: &swirl_physics::Physics, view: &crate::tags::View, now_ms: f64, own_char: Option<InstId>, fired: &mut Vec<(InstId, &'static str, Vec<Wire>)>) {
        let ws = d.services.get("Workspace").copied();
        let pg = d.local_player.and_then(|p| d.find_child_of_class(p, "PlayerGui"));
        let scan = |d: &DataModel, root: Option<InstId>| -> Vec<InstId> {
            let mut out = vec![];
            if let Some(r) = root {
                let mut all = vec![];
                d.descendants(r, &mut all);
                out.extend(all.into_iter().filter(|i| d.get(*i).map(|x| x.class == "BillboardGui").unwrap_or(false)));
            }
            out
        };
        if self.bb_frame % 15 == 0 || self.bb_world.is_empty() {
            self.bb_world = scan(d, ws);
        }
        self.bb_frame += 1;
        let mut list = self.bb_world.clone();
        list.extend(scan(d, pg));
        let mut live: HashSet<InstId> = HashSet::new();
        let cam = [view.cf[0], view.cf[1], view.cf[2]];
        let mut budget = 12;
        let bb_el = self.billboards.clone();
        for bb in list {
            if d.get(bb).map(|i| i.destroyed).unwrap_or(true) {
                continue;
            }
            let ad = match d.get_prop(bb, "Adornee") {
                Some(Val::Ref(Some(a))) => Some(*a),
                _ => d.get(bb).and_then(|i| i.parent).filter(|p| d.is_a(*p, "BasePart")),
            };
            let ad = match ad.filter(|a| d.is_a(*a, "BasePart")) {
                Some(a) => a,
                None => continue,
            };
            if !flag(d, bb, "Enabled", true) {
                continue;
            }
            let cf = match d.get_prop(ad, "CFrame") {
                Some(Val::Arr(a)) if a.len() >= 12 => [a[0], a[1], a[2]],
                _ => continue,
            };
            let off = match d.get_prop(bb, "StudsOffset") { Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]], _ => [0.0; 3] };
            let wo = match d.get_prop(bb, "StudsOffsetWorldSpace") { Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]], _ => [0.0; 3] };
            let ex = match d.get_prop(bb, "ExtentsOffset") { Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]], _ => [0.0; 3] };
            let asz = match d.get_prop(ad, "Size") { Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]], _ => [0.0; 3] };
            let loc = [off[0] + ex[0] * asz[0] / 2.0, off[1] + ex[1] * asz[1] / 2.0, off[2] + ex[2] * asz[2] / 2.0];
            let mut at = [cf[0] + wo[0], cf[1] + wo[1], cf[2] + wo[2]];
            let basis = [swirl_physics::cf::vector(&view.cf, 1.0, 0.0, 0.0), swirl_physics::cf::vector(&view.cf, 0.0, 1.0, 0.0), swirl_physics::cf::vector(&view.cf, 0.0, 0.0, 1.0)];
            for i in 0..3 {
                at[0] += basis[i][0] * loc[i];
                at[1] += basis[i][1] * loc[i];
                at[2] += basis[i][2] * loc[i];
            }
            if let Some(Val::Arr(e)) = d.get_prop(bb, "ExtentsOffsetWorldSpace") {
                if e.len() >= 3 {
                    at[0] += e[0] * asz[0] / 2.0;
                    at[1] += e[1] * asz[1] / 2.0;
                    at[2] += e[2] * asz[2] / 2.0;
                }
            }
            let p = crate::tags::project(view, at);
            if self.node_for(d, bb, &bb_el).is_none() {
                continue;
            }
            live.insert(bb);
            let el = self.nodes[&bb].el.clone();
            let p = match p {
                Some(p) => p,
                None => {
                    style(&el, "display", "none");
                    continue;
                }
            };
            let maxd = num(d, bb, "MaxDistance", 0.0);
            let dist_at = ((at[0] - cam[0]).powi(2) + (at[1] - cam[1]).powi(2) + (at[2] - cam[2]).powi(2)).sqrt();
            if maxd > 0.0 && maxd.is_finite() && maxd < 1e8 && dist_at > maxd {
                style(&el, "display", "none");
                continue;
            }
            if let Some(Val::Ref(Some(h))) = d.get_prop(bb, "PlayerToHideFrom") {
                if Some(*h) == d.local_player {
                    style(&el, "display", "none");
                    continue;
                }
            }
            if !flag(d, bb, "AlwaysOnTop", false) {
                let model = d.get(ad).and_then(|i| i.parent).filter(|m| d.get(*m).map(|x| x.class == "Model").unwrap_or(false));
                let entry = self.bb_sight.entry(bb).or_insert((-1e9, true, 0));
                if now_ms - entry.0 >= 90.0 && budget > 0 {
                    budget -= 1;
                    entry.0 = now_ms;
                    let dd = [at[0] - cam[0], at[1] - cam[1], at[2] - cam[2]];
                    let len = (dd[0] * dd[0] + dd[1] * dd[1] + dd[2] * dd[2]).sqrt();
                    if len < 0.5 {
                        entry.1 = true;
                        entry.2 = 0;
                    } else {
                        let k = (len - 0.3).max(0.0) / len;
                        let mut ig = vec![ad];
                        if let Some(m) = model {
                            if d.find_child_of_class(m, "Humanoid").is_some() {
                                ig.push(m);
                            }
                        }
                        if let Some(c) = own_char {
                            ig.push(c);
                        }
                        let hit = ph.raycast(d, cam, [dd[0] * k, dd[1] * k, dd[2] * k], &ig, Some(&|q| matches!(d.get_prop(q, "Transparency"), Some(Val::Num(t)) if *t < 0.95) || d.get_prop(q, "Transparency").is_none()));
                        if hit.part.is_some() {
                            entry.2 += 1;
                            if entry.2 >= 2 {
                                entry.1 = false;
                            }
                        } else {
                            entry.2 = 0;
                            entry.1 = true;
                        }
                    }
                }
                if !entry.1 {
                    style(&el, "display", "none");
                    continue;
                }
            }
            let dist = ((cf[0] - cam[0]).powi(2) + (cf[1] - cam[1]).powi(2) + (cf[2] - cam[2]).powi(2)).sqrt().max(0.1);
            let px_per_stud = view.h / (2.0 * dist * (view.fov * std::f64::consts::PI / 360.0).tan());
            let sz = arr4(d, bb, "Size", [0.0; 4]);
            let (w, h) = (sz[0] * px_per_stud + sz[1], sz[2] * px_per_stud + sz[3]);
            style(&el, "display", "");
            style(&el, "left", &format!("{}px", p[0] - w / 2.0));
            style(&el, "top", &format!("{}px", p[1] - h / 2.0));
            style(&el, "width", &format!("{}px", w));
            style(&el, "height", &format!("{}px", h));
            style(&el, "z-index", &format!("{}", (1000.0 - dist).round()));
            let mut lv = std::mem::take(&mut live);
            self.layout_children(d, bb, [p[0] - w / 2.0, p[1] - h / 2.0], [w, h], &mut lv, fired);
            live = lv;
        }
        let dead: Vec<InstId> = self.nodes.keys().copied().filter(|i| !live.contains(i) && self.bb_live.contains(i)).collect();
        for i in dead {
            if let Some(n) = self.nodes.remove(&i) {
                n.el.remove();
            }
        }
        self.bb_live = live;
    }

    pub fn pointer_over(&self, x: f64, y: f64) -> bool {
        match self.doc.element_from_point(x as f32, y as f32) {
            Some(e) => e.closest(".ch-g").ok().flatten().is_some() && e.closest("canvas").ok().flatten().is_none(),
            None => false,
        }
    }

    pub fn focus_box(&mut self, inst: InstId, on: bool) {
        match self.nodes.get(&inst) {
            Some(n) => {
                if on {
                    let _ = n.el.focus();
                } else {
                    let _ = n.el.blur();
                }
            }
            None => {
                if on {
                    self.pending_focus = Some((inst, 30));
                } else if matches!(self.pending_focus, Some((i, _)) if i == inst) {
                    self.pending_focus = None;
                }
            }
        }
    }

    pub fn apply_pending_focus(&mut self) {
        if let Some((inst, tries)) = self.pending_focus {
            let done = match self.nodes.get(&inst) {
                Some(n) => {
                    let _ = n.el.focus();
                    self.doc.active_element().map(|a| a == *AsRef::<web_sys::Element>::as_ref(&n.el)).unwrap_or(false)
                }
                None => false,
            };
            self.pending_focus = if done || tries <= 1 { None } else { Some((inst, tries - 1)) };
        }
    }

    pub fn node_ids(&self) -> Vec<InstId> {
        self.nodes.keys().copied().collect()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn set_visible(&self, on: bool) {
        style(&self.overlay, "display", if on { "" } else { "none" });
    }

    pub fn hover_changed(&mut self) -> bool {
        let mut changed = false;
        for (id, n) in &self.nodes {
            let s = n.state.get();
            if self.last_state.get(id) != Some(&s) {
                changed = true;
            }
        }
        if changed {
            self.last_state = self.nodes.iter().map(|(i, n)| (*i, n.state.get())).collect();
        }
        changed
    }
}

fn resolve_image(url: &str) -> Option<String> {
    let t = url.trim();
    if t.is_empty() {
        return None;
    }
    if let Ok(v) = solar_image_url(t) {
        if let Some(s) = v.as_string() {
            return Some(s);
        }
    }
    None
}

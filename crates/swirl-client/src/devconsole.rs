use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlElement, HtmlInputElement};

const MAX_LINES: usize = 2000;

struct Line {
    kind: u8,
    time: String,
    msg: String,
}

struct Dev {
    doc: Document,
    root: HtmlElement,
    tab_btns: [HtmlElement; 2],
    counts: [HtmlElement; 3],
    list: HtmlElement,
    empty: HtmlElement,
    logs: [Vec<Line>; 2],
    tab: usize,
    filter: [bool; 3],
    query: String,
    open: bool,
    server_seen: bool,
}

fn make(doc: &Document, tag: &str, cls: &str, parent: Option<&Element>, text: Option<&str>) -> HtmlElement {
    let e = doc.create_element(tag).unwrap().dyn_into::<HtmlElement>().unwrap();
    if !cls.is_empty() {
        e.set_class_name(cls);
    }
    if let Some(t) = text {
        e.set_text_content(Some(t));
    }
    if let Some(p) = parent {
        let _ = p.append_child(&e);
    }
    e
}

fn stamp() -> String {
    let d = js_sys::Date::new_0();
    format!("{:02}:{:02}:{:02}", d.get_hours(), d.get_minutes(), d.get_seconds())
}

fn kind_of(k: &str) -> u8 {
    match k {
        "error" => 2,
        "warn" => 1,
        _ => 0,
    }
}

const KIND_CLASS: [&str; 3] = ["print", "warn", "error"];

impl Dev {
    fn visible(&self, l: &Line) -> bool {
        self.filter[l.kind as usize] && (self.query.is_empty() || l.msg.to_lowercase().contains(&self.query))
    }

    fn row(&self, l: &Line) -> HtmlElement {
        let r = make(&self.doc, "div", &format!("ch-dev-line ch-dev-{}", KIND_CLASS[l.kind as usize]), None, None);
        make(&self.doc, "span", "ch-dev-time", Some(&r), Some(&l.time));
        make(&self.doc, "span", "ch-dev-msg", Some(&r), Some(&l.msg));
        r
    }

    fn update_counts(&self) {
        for k in 0..3 {
            let n = self.logs[self.tab].iter().filter(|l| l.kind as usize == k).count();
            self.counts[k].set_text_content(Some(&n.to_string()));
        }
    }

    fn render(&self) {
        self.list.set_text_content(None);
        let mut n = 0;
        for l in &self.logs[self.tab] {
            if self.visible(l) {
                let _ = self.list.append_child(&self.row(l));
                n += 1;
            }
        }
        self.update_counts();
        self.empty.set_hidden(n > 0);
        self.list.set_hidden(n == 0);
        let msg = if !self.logs[self.tab].is_empty() {
            "Nothing matches the current filters."
        } else if self.tab == 1 && !self.server_seen {
            "Server output is shown to the game\u{2019}s owner and developers."
        } else {
            "Nothing has been logged yet."
        };
        self.empty.set_text_content(Some(msg));
        self.list.set_scroll_top(self.list.scroll_height());
    }

    fn show(&mut self, tab: usize) {
        self.tab = tab;
        for (i, b) in self.tab_btns.iter().enumerate() {
            let _ = b.class_list().toggle_with_force("on", i == tab);
            let _ = b.set_attribute("aria-selected", if i == tab { "true" } else { "false" });
        }
        self.render();
    }

    fn toggle(&mut self, on: bool) {
        self.open = on;
        self.root.set_hidden(!on);
        if on {
            if self.doc.pointer_lock_element().is_some() {
                self.doc.exit_pointer_lock();
            }
            self.render();
        } else if let Some(a) = self.doc.active_element() {
            if self.root.contains(Some(&a)) {
                if let Ok(h) = a.dyn_into::<HtmlElement>() {
                    let _ = h.blur();
                }
            }
        }
    }
}

#[wasm_bindgen]
pub struct DevConsole {
    st: Rc<RefCell<Dev>>,
}

fn listen(target: &Element, name: &str, f: impl FnMut(web_sys::Event) + 'static) {
    let c = Closure::<dyn FnMut(web_sys::Event)>::new(f);
    let _ = target.add_event_listener_with_callback(name, c.as_ref().unchecked_ref());
    c.forget();
}

#[wasm_bindgen]
impl DevConsole {
    #[wasm_bindgen(constructor)]
    pub fn new(parent: &Element) -> DevConsole {
        let doc = web_sys::window().unwrap().document().unwrap();
        let root = make(&doc, "section", "ch-dev", Some(parent), None);
        root.set_hidden(true);
        let _ = root.style().set_property("pointer-events", "auto");
        let _ = root.set_attribute("role", "dialog");
        let _ = root.set_attribute("aria-label", "Developer Console");
        let head = make(&doc, "header", "ch-dev-head", Some(&root), None);
        make(&doc, "h2", "ch-dev-title", Some(&head), Some("Developer Console"));
        let tabs = make(&doc, "div", "ch-dev-tabs", Some(&head), None);
        let _ = tabs.set_attribute("role", "tablist");
        let tab_btns = [make(&doc, "button", "ch-dev-tab", Some(&tabs), Some("Client Log")), make(&doc, "button", "ch-dev-tab", Some(&tabs), Some("Server Log"))];
        for b in &tab_btns {
            let _ = b.set_attribute("type", "button");
            let _ = b.set_attribute("role", "tab");
        }
        let close = make(&doc, "button", "ch-dev-close", Some(&head), Some("\u{d7}"));
        let _ = close.set_attribute("type", "button");
        let _ = close.set_attribute("title", "Close (F9)");
        let _ = close.set_attribute("aria-label", "Close developer console");
        let bar = make(&doc, "div", "ch-dev-bar", Some(&root), None);
        let mut checks: Vec<HtmlInputElement> = vec![];
        let mut counts: Vec<HtmlElement> = vec![];
        for (i, label) in ["Output", "Warnings", "Errors"].iter().enumerate() {
            let lab = make(&doc, "label", &format!("ch-dev-filter ch-dev-{}", KIND_CLASS[i]), Some(&bar), None);
            let cb = make(&doc, "input", "", Some(&lab), None).dyn_into::<HtmlInputElement>().unwrap();
            cb.set_type("checkbox");
            cb.set_checked(true);
            make(&doc, "span", "", Some(&lab), Some(label));
            counts.push(make(&doc, "b", "ch-dev-count", Some(&lab), Some("0")));
            checks.push(cb);
        }
        let search = make(&doc, "input", "ch-dev-search", Some(&bar), None).dyn_into::<HtmlInputElement>().unwrap();
        search.set_type("search");
        search.set_placeholder("Search");
        let _ = search.set_attribute("aria-label", "Search the log");
        let clear = make(&doc, "button", "ch-dev-clear", Some(&bar), Some("Clear"));
        let _ = clear.set_attribute("type", "button");
        let list = make(&doc, "div", "ch-dev-list", Some(&root), None);
        let _ = list.set_attribute("role", "log");
        list.set_tab_index(0);
        let empty = make(&doc, "p", "ch-dev-empty", Some(&root), None);
        let st = Rc::new(RefCell::new(Dev {
            doc: doc.clone(),
            root: root.clone(),
            tab_btns: tab_btns.clone(),
            counts: [counts[0].clone(), counts[1].clone(), counts[2].clone()],
            list,
            empty,
            logs: [vec![], vec![]],
            tab: 0,
            filter: [true; 3],
            query: String::new(),
            open: false,
            server_seen: false,
        }));
        for (i, b) in tab_btns.iter().enumerate() {
            let s = st.clone();
            listen(b, "click", move |_| s.borrow_mut().show(i));
        }
        {
            let s = st.clone();
            listen(&close, "click", move |_| s.borrow_mut().toggle(false));
        }
        for (i, cb) in checks.iter().enumerate() {
            let s = st.clone();
            let c2 = cb.clone();
            listen(cb, "change", move |_| {
                let mut d = s.borrow_mut();
                d.filter[i] = c2.checked();
                d.render();
            });
        }
        {
            let s = st.clone();
            let se = search.clone();
            listen(&search, "input", move |_| {
                let mut d = s.borrow_mut();
                d.query = se.value().to_lowercase();
                d.render();
            });
        }
        {
            let s = st.clone();
            listen(&search, "keydown", move |e| {
                e.stop_propagation();
                if let Some(k) = e.dyn_ref::<web_sys::KeyboardEvent>() {
                    if k.key() == "Escape" || k.key() == "F9" {
                        e.prevent_default();
                        s.borrow_mut().toggle(false);
                    }
                }
            });
        }
        {
            let s = st.clone();
            listen(&clear, "click", move |_| {
                let mut d = s.borrow_mut();
                let t = d.tab;
                d.logs[t].clear();
                d.render();
            });
        }
        for ev in ["pointerdown", "wheel", "touchstart", "contextmenu"] {
            listen(&root, ev, |e| e.stop_propagation());
        }
        st.borrow_mut().show(0);
        DevConsole { st }
    }

    pub fn add(&mut self, source: &str, kind: &str, msg: &str) {
        let tab = if source == "server" { 1 } else { 0 };
        let mut d = self.st.borrow_mut();
        if tab == 1 {
            d.server_seen = true;
        }
        let line = Line { kind: kind_of(kind), time: stamp(), msg: msg.to_string() };
        let show = d.visible(&line);
        let row = if d.open && tab == d.tab && show { Some(d.row(&line)) } else { None };
        d.logs[tab].push(line);
        let len = d.logs[tab].len();
        if len > MAX_LINES {
            d.logs[tab].drain(0..len - MAX_LINES);
        }
        if !d.open || tab != d.tab {
            return;
        }
        match row {
            None => d.update_counts(),
            Some(r) => {
                let near = d.list.scroll_height() - d.list.scroll_top() - d.list.client_height() < 40;
                let _ = d.list.append_child(&r);
                while d.list.child_element_count() as usize > MAX_LINES {
                    if let Some(f) = d.list.first_element_child() {
                        f.remove();
                    }
                }
                d.empty.set_hidden(true);
                d.list.set_hidden(false);
                d.update_counts();
                if near {
                    d.list.set_scroll_top(d.list.scroll_height());
                }
            }
        }
    }

    pub fn toggle(&mut self) {
        let mut d = self.st.borrow_mut();
        let on = !d.open;
        d.toggle(on);
    }

    pub fn is_open(&self) -> bool {
        self.st.borrow().open
    }
}

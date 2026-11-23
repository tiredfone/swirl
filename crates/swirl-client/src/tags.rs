use js_sys::Promise;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use swirl_core::instance::{DataModel, InstId, Val};
use swirl_physics::cf;
use swirl_physics::Physics;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{CanvasRenderingContext2d, Document, Event, HtmlCanvasElement, HtmlElement, HtmlImageElement};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = solarImageUrl, catch)]
    fn solar_image_url(url: &str) -> Result<JsValue, JsValue>;
}

pub struct Font {
    pub img: HtmlImageElement,
    pub glyphs: HashMap<char, [f64; 4]>,
    pub h: f64,
}

pub type FontSlot = Rc<RefCell<Option<Rc<Font>>>>;

pub fn load_font(doc: &Document, name: &str, slot: FontSlot) {
    let def_url = solar_image_url(&format!("rbxasset://fonts/{}.fontdef", name)).ok().and_then(|v| v.as_string());
    let png_url = solar_image_url(&format!("rbxasset://fonts/{}.png", name)).ok().and_then(|v| v.as_string());
    let (def_url, png_url) = match (def_url, png_url) {
        (Some(a), Some(b)) => (a, b),
        _ => return,
    };
    let img = match doc.create_element("img").ok().and_then(|e| e.dyn_into::<HtmlImageElement>().ok()) {
        Some(i) => i,
        None => return,
    };
    let (img2, slot2) = (img.clone(), slot.clone());
    let def_cell: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let img_ready = Rc::new(RefCell::new(false));
    let finish = {
        let (def_cell, img_ready) = (def_cell.clone(), img_ready.clone());
        Rc::new(move || {
            if !*img_ready.borrow() {
                return;
            }
            let def = match def_cell.borrow().clone() {
                Some(d) => d,
                None => return,
            };
            let (w, h) = (img2.natural_width() as f64, img2.natural_height() as f64);
            let mut glyphs = HashMap::new();
            let mut fh = 0.0f64;
            for line in def.lines() {
                let t: Vec<&str> = line.split_whitespace().collect();
                if t.len() >= 6 && t[0] == "glyph" {
                    let key = if t[1].chars().count() > 1 && t[1].starts_with('u') { t[1][1..].parse::<u32>().ok().and_then(char::from_u32) } else { t[1].chars().next() };
                    let n: Vec<f64> = t[2..6].iter().map(|v| v.parse().unwrap_or(0.0)).collect();
                    if let Some(k) = key {
                        let r = [n[0] * w, n[1] * h, (n[2] - n[0]) * w, (n[3] - n[1]) * h];
                        fh = fh.max(r[3]);
                        glyphs.insert(k, r);
                    }
                }
            }
            *slot2.borrow_mut() = Some(Rc::new(Font { img: img2.clone(), glyphs, h: fh }));
        })
    };
    let (f1, r1) = (finish.clone(), img_ready.clone());
    let onload = Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_| {
        *r1.borrow_mut() = true;
        f1();
    }));
    img.set_onload(Some(onload.as_ref().unchecked_ref()));
    onload.forget();
    img.set_src(&png_url);
    let f2 = finish.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let win = match web_sys::window() {
            Some(w) => w,
            None => return,
        };
        let p: Promise = win.fetch_with_str(&def_url);
        if let Ok(r) = JsFuture::from(p).await {
            if let Ok(resp) = r.dyn_into::<web_sys::Response>() {
                if let Ok(tp) = resp.text() {
                    if let Ok(t) = JsFuture::from(tp).await {
                        if let Some(s) = t.as_string() {
                            *def_cell.borrow_mut() = Some(s);
                            f2();
                        }
                    }
                }
            }
        }
    });
}

fn glyph_of(font: &Font, ch: char) -> Option<[f64; 4]> {
    font.glyphs.get(&ch).or_else(|| font.glyphs.get(&'?')).copied()
}

pub fn measure(font: &Font, text: &str) -> f64 {
    text.chars().filter_map(|c| glyph_of(font, c)).map(|g| g[2]).sum()
}

pub fn wrap(font: &Font, text: &str, max_w: f64) -> Vec<String> {
    let mut out = vec![];
    for para in text.split('\n') {
        let mut line = String::new();
        let mut words: Vec<String> = vec![];
        let mut cur = String::new();
        let mut ws = None;
        for ch in para.chars() {
            let w = ch.is_whitespace();
            match ws {
                Some(p) if p == w => cur.push(ch),
                _ => {
                    if !cur.is_empty() {
                        words.push(std::mem::take(&mut cur));
                    }
                    cur.push(ch);
                    ws = Some(w);
                }
            }
        }
        if !cur.is_empty() {
            words.push(cur);
        }
        for word in words {
            let next = format!("{}{}", line, word);
            if !line.is_empty() && measure(font, next.trim_end()) > max_w {
                out.push(line.trim_end().to_string());
                line = word.trim_start().to_string();
                while measure(font, &line) > max_w && line.chars().count() > 1 {
                    let chars: Vec<char> = line.chars().collect();
                    let mut cut = chars.len() - 1;
                    while cut > 1 && measure(font, &chars[..cut].iter().collect::<String>()) > max_w {
                        cut -= 1;
                    }
                    out.push(chars[..cut].iter().collect());
                    line = chars[cut..].iter().collect();
                }
            } else {
                line = next;
            }
        }
        out.push(line.trim_end().to_string());
    }
    out
}

fn canvas(doc: &Document, w: f64, h: f64) -> Option<(HtmlCanvasElement, CanvasRenderingContext2d)> {
    let c = doc.create_element("canvas").ok()?.dyn_into::<HtmlCanvasElement>().ok()?;
    c.set_width(w.max(1.0).ceil() as u32);
    c.set_height(h.max(1.0).ceil() as u32);
    let g = c.get_context("2d").ok()??.dyn_into::<CanvasRenderingContext2d>().ok()?;
    Some((c, g))
}

fn tinted(doc: &Document, font: &Font, lines: &[String], color: &str, w: f64, h: f64, center: bool) -> Option<HtmlCanvasElement> {
    let (c, g) = canvas(doc, w, h)?;
    for (i, line) in lines.iter().enumerate() {
        let mut x = if center { (w - measure(font, line)) / 2.0 } else { 0.0 };
        for ch in line.chars() {
            let gl = match glyph_of(font, ch) {
                Some(g) => g,
                None => continue,
            };
            if !ch.is_whitespace() {
                let _ = g.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(&font.img, gl[0], gl[1], gl[2], gl[3], x.round(), i as f64 * font.h, gl[2], gl[3]);
            }
            x += gl[2];
        }
    }
    let _ = g.set_global_composite_operation("source-in");
    g.set_fill_style_str(color);
    g.fill_rect(0.0, 0.0, c.width() as f64, c.height() as f64);
    Some(c)
}

pub fn text_canvas(doc: &Document, font: &Font, text: &str, color: &str, shadow: Option<&str>, max_w: f64, center: bool) -> Option<HtmlCanvasElement> {
    let lines: Vec<String> = if max_w > 0.0 { wrap(font, text, max_w) } else { text.split('\n').map(String::from).collect() };
    let w = lines.iter().map(|l| measure(font, l)).fold(1.0, f64::max);
    let h = lines.len() as f64 * font.h;
    let pad = if shadow.is_some() { 2.0 } else { 0.0 };
    let (out, g) = canvas(doc, w.ceil() + pad, h.ceil() + pad)?;
    if let Some(s) = shadow {
        let t = tinted(doc, font, &lines, s, w, h, center)?;
        let _ = g.draw_image_with_html_canvas_element(&t, 2.0, 2.0);
    }
    let t = tinted(doc, font, &lines, color, w, h, center)?;
    let _ = g.draw_image_with_html_canvas_element(&t, 0.0, 0.0);
    Some(out)
}

struct Tag {
    boxe: HtmlElement,
    name: HtmlElement,
    bar: HtmlElement,
    fill: HtmlElement,
    key: String,
}

struct Sight {
    t: f64,
    clear: bool,
}

pub struct Tags {
    doc: Document,
    pub layer: HtmlElement,
    font: FontSlot,
    font_req: bool,
    tags: HashMap<InstId, Tag>,
    sight: HashMap<InstId, Sight>,
    hums: Vec<InstId>,
    hum_scan: f64,
}

pub struct View {
    pub cf: cf::CF,
    pub fov: f64,
    pub w: f64,
    pub h: f64,
}

pub fn project(v: &View, p: [f64; 3]) -> Option<[f64; 3]> {
    let inv = cf::inv(&v.cf);
    let c = cf::point(&inv, p[0], p[1], p[2]);
    let (n, f) = (0.1, 5000.0);
    let t = (v.fov * std::f64::consts::PI / 360.0).tan();
    let aspect = v.w / v.h.max(1.0);
    let w = -c[2];
    if w == 0.0 {
        return None;
    }
    let x = c[0] / (w * aspect * t);
    let y = c[1] / (w * t);
    let z = (-(f + n) / (f - n) * c[2] - 2.0 * f * n / (f - n)) / w;
    if z > 1.0 || z < -1.0 {
        return None;
    }
    Some([(x * 0.5 + 0.5) * v.w, (-y * 0.5 + 0.5) * v.h, z])
}

impl Tags {
    pub fn new(doc: &Document, overlay: &HtmlElement) -> Option<Tags> {
        let layer = doc.create_element("div").ok()?.dyn_into::<HtmlElement>().ok()?;
        layer.set_class_name("ch-tags");
        overlay.append_child(&layer).ok()?;
        Some(Tags { doc: doc.clone(), layer, font: Rc::new(RefCell::new(None)), font_req: false, tags: HashMap::new(), sight: HashMap::new(), hums: vec![], hum_scan: -1e9 })
    }

    fn div(&self, cls: &str, parent: &HtmlElement) -> HtmlElement {
        let e = self.doc.create_element("div").unwrap().dyn_into::<HtmlElement>().unwrap();
        e.set_class_name(cls);
        let _ = parent.append_child(&e);
        e
    }

    pub fn update(&mut self, d: &DataModel, ph: &Physics, view: &View, own_model: Option<InstId>, now_ms: f64) {
        if !self.font_req {
            self.font_req = true;
            load_font(&self.doc, "ArialBold-18pt", self.font.clone());
        }
        let ws = match d.services.get("Workspace") {
            Some(w) => *w,
            None => return,
        };
        if now_ms - self.hum_scan > 500.0 {
            self.hum_scan = now_ms;
            let mut all = vec![];
            d.descendants(ws, &mut all);
            self.hums = all.into_iter().filter(|i| d.get(*i).map(|x| x.class == "Humanoid").unwrap_or(false)).collect();
        }
        let cam = [view.cf[0], view.cf[1], view.cf[2]];
        let e18 = d.era == 2018;
        let mut seen: std::collections::HashSet<InstId> = std::collections::HashSet::new();
        for hum in self.hums.clone() {
            let model = match d.get(hum).filter(|i| !i.destroyed).and_then(|i| i.parent) {
                Some(m) => m,
                None => continue,
            };
            if Some(model) == own_model || !d.is_descendant_of(model, ws) || seen.contains(&model) {
                continue;
            }
            let head = match d.find_child(model, "Head", false).filter(|h| d.is_a(*h, "BasePart")) {
                Some(h) => h,
                None => continue,
            };
            let hc = match d.get_prop(head, "CFrame") {
                Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
                _ => continue,
            };
            let dist = ((hc[0] - cam[0]).powi(2) + (hc[1] - cam[1]).powi(2) + (hc[2] - cam[2]).powi(2)).sqrt();
            if e18 {
                if matches!(d.get_prop(hum, "DisplayDistanceType"), Some(Val::Num(n)) if *n as i64 == 2) {
                    continue;
                }
                let nd = match d.get_prop(hum, "NameDisplayDistance") {
                    Some(Val::Num(n)) if n.is_finite() => *n,
                    _ => 100.0,
                };
                if dist > nd {
                    continue;
                }
            } else if dist > 60.0 {
                continue;
            }
            let p = match project(view, [hc[0], hc[1] + 2.2, hc[2]]) {
                Some(p) => p,
                None => continue,
            };
            let clear = {
                let stale = self.sight.get(&model).map(|s| now_ms - s.t > 150.0).unwrap_or(true);
                if stale {
                    let dir = [hc[0] - cam[0], hc[1] - cam[1], hc[2] - cam[2]];
                    let ign: Vec<InstId> = [Some(model), own_model].into_iter().flatten().collect();
                    let hit = ph.raycast(d, cam, dir, &ign, Some(&|q| matches!(d.get_prop(q, "Transparency"), Some(Val::Num(t)) if *t < 0.95) || d.get_prop(q, "Transparency").is_none()));
                    self.sight.insert(model, Sight { t: now_ms, clear: hit.part.is_none() });
                }
                self.sight[&model].clear
            };
            if !clear {
                continue;
            }
            seen.insert(model);
            if !self.tags.contains_key(&model) {
                let boxe = self.div(if e18 { "ch-tag ch-tag18" } else { "ch-tag" }, &self.layer);
                let name = self.div("ch-tag-name", &boxe);
                let bar = self.div("ch-tag-bar", &boxe);
                let fill = self.div("ch-tag-fill", &bar);
                self.tags.insert(model, Tag { boxe, name, bar, fill, key: String::new() });
            }
            let name_s = d.name(model).to_string();
            let pl = d.services.get("Players").and_then(|p| d.get(*p)).and_then(|p| p.children.iter().copied().find(|c| d.get(*c).map(|x| x.class == "Player").unwrap_or(false) && matches!(d.get_prop(*c, "Character"), Some(Val::Ref(Some(m))) if *m == model)));
            let mut color = "#fff".to_string();
            if let Some(pl) = pl {
                if !matches!(d.get_prop(pl, "Neutral"), Some(Val::Bool(true))) {
                    let tc = match d.get_prop(pl, "TeamColor") {
                        Some(Val::Num(n)) => *n as i64,
                        _ => 194,
                    };
                    let rgb = swirl_core::brickcolors::rgb(tc);
                    color = format!("rgb({},{},{})", rgb[0], rgb[1], rgb[2]);
                } else if let Some(perk) = d.find_child(pl, "SolarPerks", false) {
                    if let Some(nc) = d.find_child(perk, "NameColor", false).filter(|n| d.get(*n).map(|x| x.class == "Color3Value").unwrap_or(false)) {
                        if let Some(Val::Arr(v)) = d.get_prop(nc, "Value") {
                            if v.len() >= 3 {
                                color = format!("rgb({},{},{})", (v[0] * 255.0).round(), (v[1] * 255.0).round(), (v[2] * 255.0).round());
                            }
                        }
                    }
                }
            }
            let font_ready = self.font.borrow().is_some();
            let key = format!("{}|{}|{}", name_s, color, font_ready);
            let t = self.tags.get_mut(&model).unwrap();
            if t.key != key {
                t.key = key;
                let font = self.font.borrow().clone();
                if e18 || font.is_none() {
                    let _ = t.name.style().set_property("color", &color);
                    t.name.set_text_content(Some(&name_s));
                } else if let Some(f) = font {
                    if let Some(c) = text_canvas(&self.doc, &f, &name_s, &color, Some("rgba(0,0,0,0.8)"), 0.0, true) {
                        let _ = c.style().set_property("width", &format!("{}px", c.width() as f64 * 0.62));
                        let _ = c.style().set_property("height", &format!("{}px", c.height() as f64 * 0.62));
                        t.name.set_text_content(None);
                        let _ = t.name.append_child(&c);
                    }
                }
            }
            let sc = if e18 { (30.0 / dist.max(1.0)).min(1.0).max(0.55) } else { (40.0 / dist.max(1.0)).min(1.0).max(0.6) };
            let h = match d.get_prop(hum, "Health") {
                Some(Val::Num(n)) => *n,
                _ => 100.0,
            };
            let mh = match d.get_prop(hum, "MaxHealth") {
                Some(Val::Num(n)) => *n,
                _ => 100.0,
            };
            let hdt = if e18 { match d.get_prop(hum, "HealthDisplayType") { Some(Val::Num(n)) => *n as i64, _ => 0 } } else { -1 };
            t.bar.set_hidden(!(mh > 0.0) || (e18 && (hdt == 2 || (hdt != 1 && h >= mh))));
            let _ = t.fill.style().set_property("width", &format!("{}%", if mh > 0.0 { (h / mh * 100.0).max(0.0).min(100.0) } else { 0.0 }));
            let _ = t.boxe.style().set_property("transform-origin", "0 0");
            let _ = t.boxe.style().set_property("transform", &format!("translate({}px, {}px) scale({}) translate(-50%, -100%)", p[0], p[1], sc));
        }
        let dead: Vec<InstId> = self.tags.keys().copied().filter(|m| !seen.contains(m)).collect();
        for m in dead {
            if let Some(t) = self.tags.remove(&m) {
                t.boxe.remove();
            }
        }
        self.sight.retain(|m, _| seen.contains(m) || (d.get(*m).map(|i| !i.destroyed).unwrap_or(false) && d.is_descendant_of(*m, ws)));
    }

    pub fn tag_names(&self) -> Vec<(f64, String)> {
        self.tags.values().map(|t| (0.0, t.key.clone())).collect()
    }
}

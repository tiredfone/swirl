use wasm_bindgen::prelude::*;
use web_sys::{Document, HtmlElement};

#[wasm_bindgen]
pub struct Boot {
    root: Option<HtmlElement>,
    status: Option<HtmlElement>,
    bricks: Option<HtmlElement>,
    connectors: Option<HtmlElement>,
    run: Option<Run>,
    done: bool,
}

struct Run {
    bricks: u32,
    connectors: u32,
    t0: f64,
    ms: f64,
    stage: u8,
    at: f64,
}

fn el(doc: &Document, id: &str) -> Option<HtmlElement> {
    doc.get_element_by_id(id).and_then(|e| e.dyn_into::<HtmlElement>().ok())
}

#[wasm_bindgen]
impl Boot {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Boot {
        let doc = web_sys::window().and_then(|w| w.document());
        let get = |id: &str| doc.as_ref().and_then(|d| el(d, id));
        Boot { root: get("swirl-boot"), status: get("swirl-status"), bricks: get("swirl-bricks"), connectors: get("swirl-connectors"), run: None, done: false }
    }

    pub fn present(&self) -> bool {
        self.root.is_some()
    }

    pub fn set_status(&self, text: &str) {
        if let Some(s) = &self.status {
            s.set_text_content(Some(text));
        }
    }

    pub fn finish(&mut self, era: i32, bricks: u32, connectors: u32, now: f64) {
        if self.run.is_some() || self.done {
            return;
        }
        if era == 2018 {
            self.hide();
            return;
        }
        let total = (bricks + connectors) as f64;
        self.run = Some(Run { bricks, connectors, t0: now, ms: (350.0 + total * 1.5).min(2200.0), stage: 0, at: 0.0 });
    }

    pub fn step(&mut self, now: f64) -> bool {
        let (b, c, stage) = match self.run.as_mut() {
            None => return self.done,
            Some(r) => {
                match r.stage {
                    0 => {
                        let k = ((now - r.t0) / r.ms).min(1.0);
                        let out = ((r.bricks as f64 * k).round() as u32, (r.connectors as f64 * k).round() as u32);
                        if k >= 1.0 {
                            r.stage = 1;
                            r.at = now + 250.0;
                        }
                        (Some(out), (), 0)
                    }
                    1 if now >= r.at => {
                        r.stage = 2;
                        r.at = now + 450.0;
                        (None, (), 1)
                    }
                    2 if now >= r.at => (None, (), 2),
                    _ => (None, (), 255),
                }
            }
        };
        if let Some((nb, nc)) = b {
            if let Some(e) = &self.bricks {
                e.set_text_content(Some(&nb.to_string()));
            }
            if let Some(e) = &self.connectors {
                e.set_text_content(Some(&nc.to_string()));
            }
        }
        let _ = c;
        match stage {
            1 => {
                if let Some(r) = &self.root {
                    let _ = r.class_list().add_1("sw-boot-done");
                }
            }
            2 => self.hide(),
            _ => {}
        }
        self.done
    }

    fn hide(&mut self) {
        if let Some(r) = &self.root {
            r.set_hidden(true);
        }
        if let Some(s) = &self.status {
            s.set_hidden(true);
        }
        self.run = None;
        self.done = true;
    }

    pub fn fail(&mut self) {
        self.hide();
    }
}

impl Default for Boot {
    fn default() -> Self {
        Self::new()
    }
}

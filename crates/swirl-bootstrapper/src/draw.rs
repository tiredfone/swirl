use crate::era::{Era, Phase};
use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub buf: Vec<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Btn {
    Cancel,
    Retry,
    Close,
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x as f32 && py >= self.y as f32 && px < (self.x + self.w) as f32 && py < (self.y + self.h) as f32
    }
}

pub struct View {
    pub phase: Phase,
    pub percent: Option<u32>,
    pub error: Option<String>,
    pub installed_only: bool,
    pub tick: f32,
    pub hover: Option<Btn>,
}

pub struct Assets {
    reg: FontRef<'static>,
    bold: FontRef<'static>,
    logo: Vec<u8>,
    logo_w: usize,
    logo_h: usize,
}

impl Assets {
    pub fn load() -> Assets {
        let reg = FontRef::try_from_slice(include_bytes!("../assets/LiberationSans-Regular.ttf")).expect("font");
        let bold = FontRef::try_from_slice(include_bytes!("../assets/LiberationSans-Bold.ttf")).expect("font");
        let mut dec = png::Decoder::new(&include_bytes!("../assets/logo.png")[..]);
        dec.set_transformations(png::Transformations::EXPAND);
        let mut r = dec.read_info().expect("logo");
        let mut buf = vec![0u8; r.output_buffer_size()];
        let info = r.next_frame(&mut buf).expect("logo");
        let ch = info.color_type.samples();
        let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
        for px in buf[..info.buffer_size()].chunks(ch) {
            match ch {
                4 => rgba.extend_from_slice(px),
                3 => rgba.extend_from_slice(&[px[0], px[1], px[2], 255]),
                2 => rgba.extend_from_slice(&[px[0], px[0], px[0], px[1]]),
                _ => rgba.extend_from_slice(&[px[0], px[0], px[0], 255]),
            }
        }
        Assets { reg, bold, logo: rgba, logo_w: info.width as usize, logo_h: info.height as usize }
    }
}

fn rgb(c: u32) -> (f32, f32, f32) {
    (((c >> 16) & 255) as f32, ((c >> 8) & 255) as f32, (c & 255) as f32)
}

fn mix(a: u32, b: u32, t: f32) -> u32 {
    let (ar, ag, ab) = rgb(a);
    let (br, bg, bb) = rgb(b);
    let t = t.clamp(0.0, 1.0);
    (((ar + (br - ar) * t) as u32) << 16) | (((ag + (bg - ag) * t) as u32) << 8) | ((ab + (bb - ab) * t) as u32)
}

impl Canvas {
    pub fn new(w: usize, h: usize, c: u32) -> Canvas {
        Canvas { w, h, buf: vec![c; w * h] }
    }

    fn put(&mut self, x: i32, y: i32, c: u32, a: f32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h || a <= 0.0 {
            return;
        }
        let i = y as usize * self.w + x as usize;
        self.buf[i] = if a >= 1.0 { c } else { mix(self.buf[i], c, a) };
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, c, 1.0);
            }
        }
    }

    pub fn rect_a(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32, a: f32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, c, a);
            }
        }
    }

    pub fn vgrad(&mut self, x: i32, y: i32, w: i32, h: i32, top: u32, bottom: u32) {
        for yy in 0..h {
            let c = mix(top, bottom, yy as f32 / (h - 1).max(1) as f32);
            for xx in 0..w {
                self.put(x + xx, y + yy, c, 1.0);
            }
        }
    }

    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32) {
        self.rect(x, y, w, 1, c);
        self.rect(x, y + h - 1, w, 1, c);
        self.rect(x, y, 1, h, c);
        self.rect(x + w - 1, y, 1, h, c);
    }

    pub fn text_width(font: &FontRef, px: f32, s: &str) -> f32 {
        let f = font.as_scaled(PxScale::from(px));
        let mut w = 0.0;
        for c in s.chars() {
            w += f.h_advance(f.glyph_id(c));
        }
        w
    }

    pub fn text(&mut self, font: &FontRef, px: f32, x: f32, y: f32, c: u32, s: &str) {
        let f = font.as_scaled(PxScale::from(px));
        let mut cx = x;
        let base = y + f.ascent();
        for ch in s.chars() {
            let id = f.glyph_id(ch);
            let g = id.with_scale_and_position(PxScale::from(px), point(cx, base));
            if let Some(o) = font.outline_glyph(g) {
                let b = o.px_bounds();
                o.draw(|gx, gy, cov| {
                    self.put(b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32, c, cov);
                });
            }
            cx += f.h_advance(id);
        }
    }

    fn logo(&mut self, a: &Assets, x: i32, y: i32, size: i32) {
        let (sw, sh) = (a.logo_w, a.logo_h);
        for dy in 0..size {
            for dx in 0..size {
                let (x0, x1) = (dx as usize * sw / size as usize, (((dx + 1) as usize * sw) / size as usize).max(dx as usize * sw / size as usize + 1));
                let (y0, y1) = (dy as usize * sh / size as usize, (((dy + 1) as usize * sh) / size as usize).max(dy as usize * sh / size as usize + 1));
                let (mut r, mut g, mut b, mut al, mut n) = (0f32, 0f32, 0f32, 0f32, 0f32);
                for yy in y0..y1.min(sh) {
                    for xx in x0..x1.min(sw) {
                        let p = &a.logo[(yy * sw + xx) * 4..];
                        let pa = p[3] as f32 / 255.0;
                        r += p[0] as f32 * pa;
                        g += p[1] as f32 * pa;
                        b += p[2] as f32 * pa;
                        al += pa;
                        n += 1.0;
                    }
                }
                if al > 0.0 && n > 0.0 {
                    let col = (((r / al) as u32) << 16) | (((g / al) as u32) << 8) | (b / al) as u32;
                    self.put(x + dx, y + dy, col, al / n);
                }
            }
        }
    }
}

fn wrap(font: &FontRef, px: f32, s: &str, width: f32, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let t = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
        if Canvas::text_width(font, px, &t) > width && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = t;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(l) = lines.last_mut() {
            l.push_str("...");
        }
    }
    lines
}

pub fn buttons(era: Era, v: &View) -> Vec<(Btn, Rect)> {
    let (w, _h) = era.window_size();
    let w = w as i32;
    match era {
        Era::E2012 => {
            if v.error.is_some() {
                vec![(Btn::Retry, Rect { x: w - 190, y: 168, w: 80, h: 26 }), (Btn::Close, Rect { x: w - 100, y: 168, w: 80, h: 26 })]
            } else if v.phase == Phase::Done && v.installed_only {
                vec![(Btn::Close, Rect { x: w - 100, y: 168, w: 80, h: 26 })]
            } else {
                vec![(Btn::Cancel, Rect { x: w - 100, y: 168, w: 80, h: 26 })]
            }
        }
        Era::E2018 => {
            if v.error.is_some() {
                vec![(Btn::Retry, Rect { x: w / 2 - 108, y: 196, w: 100, h: 32 }), (Btn::Close, Rect { x: w / 2 + 8, y: 196, w: 100, h: 32 })]
            } else if v.phase == Phase::Done && v.installed_only {
                vec![(Btn::Close, Rect { x: w / 2 - 50, y: 196, w: 100, h: 32 })]
            } else {
                vec![(Btn::Cancel, Rect { x: w / 2 - 40, y: 200, w: 80, h: 24 })]
            }
        }
    }
}

fn label(b: Btn) -> &'static str {
    match b {
        Btn::Cancel => "Cancel",
        Btn::Retry => "Retry",
        Btn::Close => "Close",
    }
}

pub fn render(era: Era, a: &Assets, v: &View) -> Canvas {
    let (w, h) = era.window_size();
    match era {
        Era::E2012 => render2012(a, v, w, h),
        Era::E2018 => render2018(a, v, w, h),
    }
}

fn marquee(tick: f32, track: i32, block: i32) -> i32 {
    let span = (track + block) as f32;
    let p = (tick * 150.0) % (span * 2.0);
    let p = if p > span { span * 2.0 - p } else { p };
    p as i32 - block
}

fn render2012(a: &Assets, v: &View, w: usize, h: usize) -> Canvas {
    let mut c = Canvas::new(w, h, 0xeef2f5);
    let (wi, hi) = (w as i32, h as i32);
    c.vgrad(0, 0, wi, hi, 0xf6f9fb, 0xdae3e9);
    c.vgrad(0, 0, wi, 64, 0x20506f, 0x0f2b42);
    c.rect_a(0, 0, wi, 32, 0xffffff, 0.07);
    c.rect(0, 64, wi, 1, 0x0a1d2d);
    c.rect(0, 65, wi, 1, 0xffffff);
    c.logo(a, 16, 9, 46);
    c.text(&a.bold, 30.0, 72.0, 11.0, 0xffffff, "Solar");
    c.text(&a.reg, 13.0, 74.0, 43.0, 0x8fd9ee, "Player 2012");
    let err = v.error.is_some();
    let title = v.phase.text(Era::E2012, v.percent, v.installed_only);
    if err {
        c.text(&a.bold, 17.0, 24.0, 80.0, 0x9a2222, &title);
        let lines = wrap(&a.reg, 13.0, v.error.as_deref().unwrap_or(""), (wi - 48) as f32, 3);
        for (i, l) in lines.iter().enumerate() {
            c.text(&a.reg, 13.0, 24.0, 106.0 + i as f32 * 17.0, 0x2c3a45, l);
        }
    } else {
        c.text(&a.reg, 16.0, 24.0, 82.0, 0x1c2b36, &title);
        let (bx, by, bw, bh) = (24, 114, wi - 48, 22);
        c.rect(bx, by, bw, bh, 0x7d8c98);
        c.vgrad(bx + 1, by + 1, bw - 2, bh - 2, 0xc3ced6, 0xeef2f4);
        let (ix, iw) = (bx + 1, bw - 2);
        let fill = |c: &mut Canvas, x: i32, ww: i32| {
            if ww <= 0 {
                return;
            }
            c.vgrad(x, by + 1, ww, bh - 2, 0x7fdcf3, 0x1596ba);
            c.rect_a(x, by + 1, ww, (bh - 2) / 2, 0xffffff, 0.28);
            c.rect(x, by + 1, ww, 1, 0xc9f1fb);
        };
        match (v.phase, v.percent) {
            (Phase::Downloading, Some(p)) => fill(&mut c, ix, iw * p.min(100) as i32 / 100),
            (Phase::Done, _) => fill(&mut c, ix, iw),
            _ => {
                let bl = 110;
                let s = marquee(v.tick, iw, bl);
                let x0 = s.max(0);
                let x1 = (s + bl).min(iw);
                fill(&mut c, ix + x0, x1 - x0);
            }
        }
    }
    c.text(&a.reg, 11.0, 24.0, 175.0, 0x7a8895, &format!("Solar 2012 Player  /  launcher {}", crate::paths::own_version()));
    for (b, r) in buttons(Era::E2012, v) {
        let hot = v.hover == Some(b);
        let (t, bt) = if hot { (0xffffff, 0xd9eaf2) } else { (0xfbfcfd, 0xd3dae0) };
        c.rect(r.x, r.y, r.w, r.h, if hot { 0x1b7fa3 } else { 0x6f7d89 });
        c.vgrad(r.x + 1, r.y + 1, r.w - 2, r.h - 2, t, bt);
        let tw = Canvas::text_width(&a.reg, 13.0, label(b));
        c.text(&a.reg, 13.0, r.x as f32 + (r.w as f32 - tw) / 2.0, r.y as f32 + 5.0, 0x1c2b36, label(b));
    }
    let _ = hi;
    c
}

fn render2018(a: &Assets, v: &View, w: usize, h: usize) -> Canvas {
    let mut c = Canvas::new(w, h, 0xffffff);
    let wi = w as i32;
    let center = |c: &mut Canvas, font: &FontRef, px: f32, y: f32, col: u32, s: &str| {
        let tw = Canvas::text_width(font, px, s);
        c.text(font, px, (wi as f32 - tw) / 2.0, y, col, s);
    };
    c.rect(0, 0, wi, 4, 0x1fa8c9);
    c.logo(a, (wi - 72) / 2, 28, 72);
    let err = v.error.is_some();
    let title = v.phase.text(Era::E2018, v.percent, v.installed_only);
    if err {
        center(&mut c, &a.bold, 18.0, 112.0, 0x2b3138, &title);
        let lines = wrap(&a.reg, 13.0, v.error.as_deref().unwrap_or(""), (wi - 80) as f32, 3);
        for (i, l) in lines.iter().enumerate() {
            center(&mut c, &a.reg, 13.0, 140.0 + i as f32 * 17.0, 0x6b747c, l);
        }
    } else {
        center(&mut c, &a.reg, 18.0, 118.0, 0x2b3138, &title);
        let (bx, by, bw, bh) = (70, 158, wi - 140, 6);
        c.rect(bx, by, bw, bh, 0xe3e7ea);
        match (v.phase, v.percent) {
            (Phase::Downloading, Some(p)) => c.rect(bx, by, bw * p.min(100) as i32 / 100, bh, 0x1fa8c9),
            (Phase::Done, _) => c.rect(bx, by, bw, bh, 0x1fa8c9),
            _ => {
                let bl = 80;
                let s = marquee(v.tick * 0.9, bw, bl);
                let x0 = s.max(0);
                let x1 = (s + bl).min(bw);
                if x1 > x0 {
                    c.rect(bx + x0, by, x1 - x0, bh, 0x1fa8c9);
                }
            }
        }
    }
    for (b, r) in buttons(Era::E2018, v) {
        let hot = v.hover == Some(b);
        match b {
            Btn::Cancel => {
                let col = if hot { 0x2b3138 } else { 0x8a949c };
                let tw = Canvas::text_width(&a.reg, 13.0, "Cancel");
                let tx = r.x as f32 + (r.w as f32 - tw) / 2.0;
                c.text(&a.reg, 13.0, tx, r.y as f32 + 4.0, col, "Cancel");
                if hot {
                    c.rect(tx as i32, r.y + 20, tw as i32, 1, col);
                }
            }
            Btn::Retry | Btn::Close => {
                let primary = b == Btn::Retry || (b == Btn::Close && v.error.is_none());
                if primary {
                    c.rect(r.x, r.y, r.w, r.h, if hot { 0x168eac } else { 0x1fa8c9 });
                } else {
                    c.rect(r.x, r.y, r.w, r.h, if hot { 0x8a949c } else { 0xc4cad0 });
                    c.rect(r.x + 1, r.y + 1, r.w - 2, r.h - 2, 0xffffff);
                }
                let col = if primary { 0xffffff } else { 0x2b3138 };
                let tw = Canvas::text_width(&a.bold, 14.0, label(b));
                c.text(&a.bold, 14.0, r.x as f32 + (r.w as f32 - tw) / 2.0, r.y as f32 + 7.0, col, label(b));
            }
        }
    }
    c.text(&a.reg, 10.0, 8.0, h as f32 - 16.0, 0xb4bcc2, &format!("Solar {}  v{}", 2018, crate::paths::own_version()));
    c
}

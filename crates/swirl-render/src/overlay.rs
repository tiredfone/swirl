use std::collections::HashMap;

pub struct Font {
    img: Vec<u32>,
    iw: usize,
    glyphs: HashMap<char, [i64; 4]>,
    h: i64,
}

pub fn blend(dst: u32, src: u32) -> u32 {
    let sa = ((src >> 24) & 127) as i64;
    if sa == 0 {
        return src;
    }
    let da = ((dst >> 24) & 127) as i64;
    if sa == 127 {
        return dst;
    }
    if da == 127 {
        return src;
    }
    let alpha = sa * da / 127;
    let sw = 127 - sa;
    let dw = (127 - da) * sa / 127;
    let tw = sw + dw;
    let ch = |s: u32| -> u32 { ((((src >> s) & 255) as i64 * sw + ((dst >> s) & 255) as i64 * dw) / tw) as u32 };
    ((alpha as u32) << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

fn load_font(dir: &str, name: &str) -> Option<Font> {
    let def = std::fs::read_to_string(format!("{}/fonts/{}.fontdef", dir, name)).ok()?;
    let f = std::fs::File::open(format!("{}/fonts/{}.png", dir, name)).ok()?;
    let mut dec = png::Decoder::new(f);
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut rd = dec.read_info().ok()?;
    let mut buf = vec![0; rd.output_buffer_size()];
    let info = rd.next_frame(&mut buf).ok()?;
    let (iw, ih) = (info.width as usize, info.height as usize);
    let ch = info.color_type.samples();
    let mut img = Vec::with_capacity(iw * ih);
    for i in 0..iw * ih {
        let s = &buf[i * ch..i * ch + ch];
        let (r, g, b, a) = match info.color_type {
            png::ColorType::Grayscale => (s[0], s[0], s[0], 255),
            png::ColorType::GrayscaleAlpha => (s[0], s[0], s[0], s[1]),
            png::ColorType::Rgb => (s[0], s[1], s[2], 255),
            _ => (s[0], s[1], s[2], s[3]),
        };
        let a7 = 127 - (a as u32 >> 1);
        img.push((a7 << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32);
    }
    let re = regex::Regex::new(r"(?m)^\s*glyph\s+(\S+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)").ok()?;
    let mut glyphs = HashMap::new();
    let mut h = 0;
    for c in re.captures_iter(&def) {
        let key = &c[1];
        let ch = if key.chars().count() > 1 && key.starts_with('u') {
            match key[1..].parse::<u32>().ok().and_then(char::from_u32) {
                Some(c) => c,
                None => continue,
            }
        } else {
            match key.chars().next() {
                Some(c) => c,
                None => continue,
            }
        };
        let f = |i: usize| c[i].parse::<f64>().unwrap_or(0.0);
        let r = [(f(2) * iw as f64).round() as i64, (f(3) * ih as f64).round() as i64, ((f(4) - f(2)) * iw as f64).round() as i64, ((f(5) - f(3)) * ih as f64).round() as i64];
        h = h.max(r[3]);
        glyphs.insert(ch, r);
    }
    if glyphs.is_empty() || h == 0 {
        return None;
    }
    Some(Font { img, iw, glyphs, h })
}

impl Font {
    fn glyph(&self, ch: char) -> Option<[i64; 4]> {
        self.glyphs.get(&ch).or_else(|| self.glyphs.get(&'?')).copied()
    }

    fn measure(&self, text: &str) -> i64 {
        text.chars().map(|c| self.glyph(c).map(|g| g[2]).unwrap_or(0)).sum()
    }
}

fn php_trim(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\x0b'))
}

fn php_rtrim(s: &str) -> &str {
    s.trim_end_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\x0b'))
}

fn php_ltrim(s: &str) -> &str {
    s.trim_start_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\x0b'))
}

fn decode_entities(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let end = tail.find(';').filter(|&e| e <= 10);
        let rep = end.and_then(|e| {
            let name = &tail[1..e];
            let r: Option<String> = match name {
                "amp" => Some("&".into()),
                "lt" => Some("<".into()),
                "gt" => Some(">".into()),
                "quot" => Some("\"".into()),
                "apos" => Some("'".into()),
                "nbsp" => Some("\u{a0}".into()),
                n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16).ok().and_then(char::from_u32).map(|c| c.to_string()),
                n if n.starts_with('#') => n[1..].parse::<u32>().ok().and_then(char::from_u32).map(|c| c.to_string()),
                _ => None,
            };
            r.map(|r| (r, e))
        });
        match rep {
            Some((r, e)) => {
                out.push_str(&r);
                rest = &tail[e + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn clean(text: &str) -> String {
    let t = decode_entities(text).replace("\r\n", "\n").replace('\r', "\n");
    let t: String = t
        .chars()
        .filter(|&c| c == '\n' || !(c.is_control() || matches!(c as u32, 0xad | 0x200b..=0x200f | 0x202a..=0x202e | 0x2060..=0x2064 | 0xfeff)))
        .collect();
    php_trim(&t).to_string()
}

fn wrap(font: &Font, text: &str, max: i64) -> Vec<String> {
    let ws = regex::Regex::new(r"\s+").unwrap();
    let mut out = vec![];
    for para in text.split('\n') {
        let mut pieces: Vec<&str> = vec![];
        let mut last = 0;
        for m in ws.find_iter(para) {
            pieces.push(&para[last..m.start()]);
            pieces.push(m.as_str());
            last = m.end();
        }
        pieces.push(&para[last..]);
        let mut line = String::new();
        for word in pieces {
            let next = format!("{}{}", line, word);
            if !line.is_empty() && font.measure(php_rtrim(&next)) > max {
                out.push(php_rtrim(&line).to_string());
                line = php_ltrim(word).to_string();
            } else {
                line = next;
            }
        }
        out.push(php_rtrim(&line).to_string());
    }
    out
}

fn text_image(font: &Font, text: &str, px: f64, max_width: i64) -> (Vec<u32>, usize, usize) {
    let k = px / font.h as f64;
    let lines = wrap(font, text, ((max_width as f64 / k).floor() as i64).max(1));
    let mut w = 1i64;
    for l in &lines {
        w = w.max(font.measure(l));
    }
    let h = lines.len() as i64 * font.h;
    let mut c = vec![(127u32 << 24) | 0xffffff; (w * h) as usize];
    for (i, line) in lines.iter().enumerate() {
        let mut x = ((w - font.measure(line)) as f64 / 2.0).round() as i64;
        for ch in line.chars() {
            let g = match font.glyph(ch) {
                Some(g) => g,
                None => continue,
            };
            if !ch.is_whitespace() {
                for gy in 0..g[3] {
                    for gx in 0..g[2] {
                        let sx = g[0] + gx;
                        let sy = g[1] + gy;
                        if sx < 0 || sy < 0 || sx as usize >= font.iw {
                            continue;
                        }
                        let idx = sy as usize * font.iw + sx as usize;
                        if idx >= font.img.len() {
                            continue;
                        }
                        let a = (font.img[idx] >> 24) & 127;
                        let (dx, dy) = (x + gx, i as i64 * font.h + gy);
                        if a < 127 && dx >= 0 && dx < w && dy >= 0 && dy < h {
                            c[(dy * w + dx) as usize] = (a << 24) | 0xffffff;
                        }
                    }
                }
            }
            x += g[2];
        }
    }
    let ow = ((w as f64 * k).round() as i64).max(1) as usize;
    let oh = ((h as f64 * k).round() as i64).max(1) as usize;
    let o = crate::place::gd_resample_pub(&c, w as usize, h as usize, ow, oh);
    (o, ow, oh)
}

fn copy_blend(im: &mut [u32], w: usize, h: usize, src: &[u32], sw: usize, sh: usize, dx: i64, dy: i64) {
    for y in 0..sh as i64 {
        for x in 0..sw as i64 {
            let (tx, ty) = (dx + x, dy + y);
            if tx < 0 || ty < 0 || tx >= w as i64 || ty >= h as i64 {
                continue;
            }
            let i = ty as usize * w + tx as usize;
            im[i] = blend(im[i], src[y as usize * sw + x as usize]);
        }
    }
}

pub struct Overlays {
    pub message: Option<String>,
    pub hint: Option<String>,
}

pub fn apply(im: &mut [u32], w: usize, h: usize, ov: &Overlays, data_dir: &str) {
    if let Some(m) = &ov.message {
        let dim = 64u32 << 24;
        for p in im.iter_mut() {
            *p = blend(*p, dim);
        }
        if let Some(font) = load_font(data_dir, "ArialBold-18pt") {
            let (t, tw, th) = text_image(&font, &clean(m), font.h as f64, (w as f64 * 0.8) as i64);
            copy_blend(im, w, h, &t, tw, th, ((w as f64 - tw as f64) / 2.0) as i64, ((h as f64 - th as f64) / 2.0) as i64);
        }
    }
    if let Some(m) = &ov.hint {
        let font = load_font(data_dir, "Arial-18pt");
        let t = font.as_ref().map(|f| text_image(f, &clean(m), 15.0, w as i64 - 16));
        let band = (16i64).max(t.as_ref().map(|t| t.2 as i64).unwrap_or(0));
        for y in 0..band.min(h as i64) {
            for x in 0..w {
                im[y as usize * w + x] = 0;
            }
        }
        if let Some((t, tw, th)) = t {
            copy_blend(im, w, h, &t, tw, th, ((w as f64 - tw as f64) / 2.0) as i64, ((band as f64 - th as f64) / 2.0) as i64);
        }
    }
}

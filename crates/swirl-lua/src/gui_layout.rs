use swirl_core::instance::{DataModel, InstId, Val};

pub trait Measure {
    fn width(&self, font: &str, text: &str) -> f64;
}

const FONT_SIZES: [f64; 15] = [8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 18.0, 24.0, 36.0, 48.0, 28.0, 32.0, 42.0, 60.0, 96.0];

pub fn font_family(font: i64) -> &'static str {
    match font {
        1 => "ch-arial",
        2 | 0 => "ch-arialbd",
        4 => "ch-ssb",
        16 => "ch-sssb",
        5 => "ch-ssl",
        6 => "ch-ssi",
        7 => "ch-bodoni",
        8 => "ch-garamond",
        9 => "ch-cartoon",
        10 => "ch-code",
        11 => "ch-highway",
        12 => "ch-scifi",
        13 => "ch-arcade",
        14 => "ch-fantasy",
        15 => "ch-antique",
        _ => "ch-ss",
    }
}

pub fn font_family_css(era: i32, font: i64) -> (String, &'static str) {
    if era != 2018 {
        let weight = if matches!(font, 0 | 2 | 4) { "700" } else { "400" };
        return ("Arial, Helvetica, sans-serif".to_string(), weight);
    }
    (format!("\"{}\", \"Source Sans 3\", Arial, sans-serif", font_family(font)), "400")
}

pub fn font_css(era: i32, font: i64, px: f64) -> String {
    let (family, weight) = font_family_css(era, font);
    format!("{} {}px {}", weight, px, family)
}

pub fn scaled_px_2018(m: &dyn Measure, text: &str, font: i64, w: f64, h: f64, lo: f64, hi: f64) -> f64 {
    let mut a = lo.max(1.0);
    let mut b = hi.min(100.0);
    let lines = text.split('\n').count() as f64;
    while b - a > 1.0 {
        let mid = ((a + b) / 2.0).floor();
        let css = font_css(2018, font, mid);
        let tw = text.split('\n').map(|l| m.width(&css, l)).fold(0.0, f64::max);
        if tw <= w && mid * 1.15 * lines <= h {
            a = mid;
        } else {
            b = mid;
        }
    }
    a
}

fn num(d: &DataModel, id: InstId, k: &str) -> Option<f64> {
    match d.get_prop(id, k) {
        Some(Val::Num(n)) => Some(*n),
        _ => None,
    }
}

fn flag(d: &DataModel, id: InstId, k: &str) -> bool {
    matches!(d.get_prop(id, k), Some(Val::Bool(true)))
}

pub fn abs_size(d: &DataModel, id: InstId, fallback: [f64; 2]) -> [f64; 2] {
    let mut chain = vec![];
    let mut x = Some(id);
    while let Some(c) = x {
        if d.is_a(c, "LayerCollector") {
            break;
        }
        chain.push(c);
        x = d.get(c).and_then(|i| i.parent);
    }
    let mut base = match x.and_then(|c| d.get_prop(c, "AbsoluteSize")) {
        Some(Val::Arr(a)) if a.len() >= 2 => [a[0], a[1]],
        _ => fallback,
    };
    for c in chain.into_iter().rev() {
        if let Some(Val::Arr(s)) = d.get_prop(c, "Size") {
            if s.len() == 4 {
                base = [s[0] * base[0] + s[1], s[2] * base[1] + s[3]];
            }
        }
    }
    base
}

pub fn text_bounds(d: &DataModel, id: InstId, m: &dyn Measure, fallback: [f64; 2]) -> [f64; 2] {
    let era = d.era;
    let text = match d.get_prop(id, "Text") {
        Some(Val::Str(s)) => s.clone(),
        _ => String::new(),
    };
    let font = num(d, id, "Font").unwrap_or(0.0) as i64;
    let mut px = if era == 2018 && d.get(id).map(|i| i.props.contains_key("TextSize")).unwrap_or(false) {
        match num(d, id, "TextSize") {
            Some(n) if n != 0.0 => n,
            _ => 14.0,
        }
    } else {
        FONT_SIZES.get(num(d, id, "FontSize").unwrap_or(0.0) as usize).copied().unwrap_or(12.0)
    };
    let line_h = if era == 2018 {
        let lh = if d.get(id).map(|i| i.props.contains_key("LineHeight")).unwrap_or(false) { num(d, id, "LineHeight").filter(|v| *v != 0.0).unwrap_or(1.0) } else { 1.0 };
        (px * lh).round()
    } else {
        (px * 1.15).round()
    };
    if text.is_empty() {
        return if era == 2018 { [0.0, line_h] } else { [0.0, 0.0] };
    }
    let wrap_on = flag(d, id, "TextWrapped") || flag(d, id, "TextWrap");
    let scaled = flag(d, id, "TextScaled");
    let size = if wrap_on || scaled { Some(abs_size(d, id, fallback)) } else { None };
    if scaled {
        let s = size.unwrap();
        let n = text.chars().count().max(1) as f64;
        px = if era == 2018 { scaled_limits_px(d, id, m, &text, font, s) } else { (s[1] * 0.85).min(s[0] / n * 1.8).floor().min(100.0).max(6.0) };
    }
    let css = font_css(era, font, px);
    let mut lines: Vec<String> = vec![];
    for para in text.split('\n') {
        if !wrap_on {
            lines.push(para.to_string());
            continue;
        }
        let w = size.unwrap()[0];
        let mut line = String::new();
        let mut words: Vec<String> = vec![];
        let mut cur = String::new();
        let mut in_ws = None;
        for ch in para.chars() {
            let ws = ch.is_whitespace();
            match in_ws {
                Some(prev) if prev == ws => cur.push(ch),
                _ => {
                    if !cur.is_empty() {
                        words.push(std::mem::take(&mut cur));
                    }
                    cur.push(ch);
                    in_ws = Some(ws);
                }
            }
        }
        if !cur.is_empty() {
            words.push(cur);
        }
        for word in words {
            let next = format!("{}{}", line, word);
            if !line.is_empty() && m.width(&css, next.trim_end()) > w {
                lines.push(line.trim_end().to_string());
                line = word.trim_start().to_string();
            } else {
                line = next;
            }
        }
        lines.push(line);
    }
    let maxw = lines.iter().map(|l| m.width(&css, l)).fold(0.0, f64::max);
    let lh = if scaled { (px * 1.15).round() } else { line_h };
    [maxw, lines.len() as f64 * lh]
}

pub fn scaled_limits(d: &DataModel, id: InstId) -> (f64, f64) {
    let lim = d.get(id).and_then(|i| i.children.iter().copied().find(|k| d.get(*k).map(|x| x.class == "UITextSizeConstraint").unwrap_or(false)));
    match lim {
        Some(k) => (num(d, k, "MinTextSize").filter(|v| *v != 0.0).unwrap_or(1.0), num(d, k, "MaxTextSize").filter(|v| *v != 0.0).unwrap_or(100.0)),
        None => (1.0, 100.0),
    }
}

fn scaled_limits_px(d: &DataModel, id: InstId, m: &dyn Measure, text: &str, font: i64, s: [f64; 2]) -> f64 {
    let (lo, hi) = scaled_limits(d, id);
    scaled_px_2018(m, text, font, s[0], s[1], lo, hi)
}

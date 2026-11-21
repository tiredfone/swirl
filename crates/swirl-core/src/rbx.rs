use crate::brickcolors;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Bool(bool),
    Num(f64),
    Brick(i64),
    Color3(f64, f64, f64),
    V3(f64, f64, f64),
    V2(f64, f64),
    CFrame([f64; 3], [f64; 9]),
    Ref(String),
    Url(String),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Brick(b) => Some(*b as f64),
            _ => None,
        }
    }
    pub fn as_url(&self) -> Option<&str> {
        match self {
            Value::Url(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_v3(&self) -> Option<[f64; 3]> {
        match self {
            Value::V3(x, y, z) => Some([*x, *y, *z]),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Inst {
    pub cls: String,
    pub name: String,
    pub referent: Option<String>,
    pub props: HashMap<String, Value>,
    pub children: Vec<Inst>,
}

impl Inst {
    pub fn prop(&self, k: &str) -> Option<&Value> {
        self.props.get(k)
    }
    pub fn url(&self, k: &str) -> Option<&str> {
        self.props.get(k).and_then(|v| v.as_url())
    }
}

#[derive(Debug)]
pub struct Tree {
    pub roots: Vec<Inst>,
}

pub fn brick_rgb01(n: i64) -> [f64; 3] {
    let c = brickcolors::rgb(n);
    [c[0] as f64 / 255.0, c[1] as f64 / 255.0, c[2] as f64 / 255.0]
}

struct El {
    tag: String,
    attrs: HashMap<String, String>,
    kids: Vec<El>,
    text: String,
}

fn unesc(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'&' {
            if let Some(end) = s[i + 1..].find(';') {
                let e = &s[i + 1..i + 1 + end];
                let valid = !e.is_empty()
                    && (e.starts_with('#')
                        && e.len() > 1
                        && (e[1..].chars().all(|c| c.is_ascii_digit())
                            || (e[1..].starts_with('x')
                                && e.len() > 2
                                && e[2..].chars().all(|c| c.is_ascii_hexdigit())))
                        || e.chars().all(|c| c.is_ascii_lowercase()));
                if valid {
                    if e.starts_with('#') {
                        if e.len() <= 9 {
                            let cp = if e[1..].starts_with('x') {
                                u32::from_str_radix(&e[2..], 16).ok()
                            } else {
                                e[1..].parse::<u32>().ok()
                            };
                            if let Some(cp) = cp {
                                if let Some(ch) = char::from_u32(cp) {
                                    if cp >= 1 {
                                        out.push(ch);
                                    }
                                }
                            }
                        }
                        i += end + 2;
                        continue;
                    }
                    let rep = match e {
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "amp" => Some('&'),
                        "quot" => Some('"'),
                        "apos" => Some('\''),
                        _ => None,
                    };
                    if let Some(c) = rep {
                        out.push(c);
                        i += end + 2;
                        continue;
                    }
                }
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn parse_attrs(s: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let st = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let name = &s[st..i];
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'=' {
            break;
        }
        i += 1;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let q = b[i];
        if q != b'"' && q != b'\'' {
            break;
        }
        i += 1;
        let vs = i;
        while i < b.len() && b[i] != q {
            i += 1;
        }
        m.insert(name.to_string(), unesc(&s[vs..i]));
        i += 1;
    }
    m
}

fn xml_tree(src: &str) -> Result<El, String> {
    let low = src.to_ascii_lowercase();
    if low.contains("<!doctype") || low.contains("<!entity") {
        return Err("xml with a doctype is not accepted".into());
    }
    let mut stack: Vec<El> = vec![El { tag: "#root".into(), attrs: HashMap::new(), kids: vec![], text: String::new() }];
    let b = src.as_bytes();
    let mut i = 0;
    let mut count = 0usize;
    while i < b.len() {
        if b[i] != b'<' {
            let e = src[i..].find('<').map(|x| i + x).unwrap_or(b.len());
            let t = unesc(&src[i..e]);
            stack.last_mut().unwrap().text.push_str(&t);
            i = e;
            continue;
        }
        if src[i..].starts_with("<![CDATA[") {
            let e = src[i + 9..].find("]]>").map(|x| i + 9 + x).ok_or("bad cdata")?;
            stack.last_mut().unwrap().text.push_str(&src[i + 9..e]);
            i = e + 3;
            continue;
        }
        if src[i..].starts_with("<!--") {
            let e = src[i + 4..].find("-->").map(|x| i + 4 + x).ok_or("bad comment")?;
            i = e + 3;
            continue;
        }
        if src[i..].starts_with("<?") {
            let e = src[i + 2..].find("?>").map(|x| i + 2 + x).ok_or("bad pi")?;
            i = e + 2;
            continue;
        }
        let close = src[i..].starts_with("</");
        let ns = if close { i + 2 } else { i + 1 };
        let mut e = ns;
        let mut quote = 0u8;
        while e < b.len() {
            if quote != 0 {
                if b[e] == quote {
                    quote = 0;
                }
            } else if b[e] == b'"' || b[e] == b'\'' {
                quote = b[e];
            } else if b[e] == b'>' {
                break;
            }
            e += 1;
        }
        if e >= b.len() {
            return Err("xml could not be read".into());
        }
        let mut inner = &src[ns..e];
        i = e + 1;
        if close {
            if stack.len() > 1 {
                let el = stack.pop().unwrap();
                stack.last_mut().unwrap().kids.push(el);
            }
            continue;
        }
        let selfc = inner.ends_with('/');
        if selfc {
            inner = &inner[..inner.len() - 1];
        }
        let tend = inner.find(|c: char| c.is_whitespace()).unwrap_or(inner.len());
        let tag = &inner[..tend];
        if tag.is_empty() {
            continue;
        }
        count += 1;
        if count > 500000 {
            return Err("xml has too many elements".into());
        }
        let el = El { tag: tag.to_string(), attrs: parse_attrs(&inner[tend..]), kids: vec![], text: String::new() };
        if selfc {
            stack.last_mut().unwrap().kids.push(el);
        } else {
            if stack.len() > 200 {
                return Err("xml is nested too deeply".into());
            }
            stack.push(el);
        }
    }
    while stack.len() > 1 {
        let el = stack.pop().unwrap();
        stack.last_mut().unwrap().kids.push(el);
    }
    Ok(stack.pop().unwrap())
}

fn child<'a>(el: &'a El, tag: &str) -> Option<&'a El> {
    el.kids.iter().find(|k| k.tag == tag)
}

fn pf(t: &str) -> f64 {
    let t = t.trim();
    let l = t.to_ascii_lowercase();
    if l.starts_with("-inf") {
        return f64::NEG_INFINITY;
    }
    if l.starts_with("inf") {
        return f64::INFINITY;
    }
    if l.starts_with("-nan") || l.starts_with("nan") {
        return f64::NAN;
    }
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let ds = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i > ds;
    if i < b.len() && b[i] == b'.' {
        let save = i;
        i += 1;
        let fs = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i > fs {
            digits = true;
        } else if !digits {
            i = save;
        }
    }
    if !digits {
        return f64::NAN;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'-' || b[j] == b'+') {
            j += 1;
        }
        let es = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > es {
            i = j;
        }
    }
    t[..i].parse::<f64>().unwrap_or(f64::NAN)
}

fn php_int(s: &str) -> i64 {
    let t = s.trim_start();
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    t[..i].parse::<i64>().unwrap_or(0)
}

fn num(el: &El, tag: &str, d: f64) -> f64 {
    match child(el, tag) {
        Some(c) => pf(&c.text),
        None => d,
    }
}

fn xml_prop(el: &El) -> Option<Value> {
    let txt = el.text.trim();
    match el.tag.as_str() {
        "string" | "ProtectedString" => Some(Value::Str(el.text.clone())),
        "bool" => Some(Value::Bool(txt == "true")),
        "int" | "int64" | "token" | "float" | "double" => Some(Value::Num(pf(txt))),
        "BrickColor" => Some(Value::Brick(php_int(txt))),
        "Color3" => {
            if child(el, "R").is_some() {
                return Some(Value::Color3(num(el, "R", 0.0), num(el, "G", 0.0), num(el, "B", 0.0)));
            }
            let u = (php_int(txt) as u64 & 0xffffffff) as u32;
            Some(Value::Color3(((u >> 16) & 255) as f64 / 255.0, ((u >> 8) & 255) as f64 / 255.0, (u & 255) as f64 / 255.0))
        }
        "Color3uint8" => {
            let u = (php_int(txt) as u64 & 0xffffffff) as u32;
            Some(Value::Color3(((u >> 16) & 255) as f64 / 255.0, ((u >> 8) & 255) as f64 / 255.0, (u & 255) as f64 / 255.0))
        }
        "Vector3" => Some(Value::V3(num(el, "X", 0.0), num(el, "Y", 0.0), num(el, "Z", 0.0))),
        "Vector2" => Some(Value::V2(num(el, "X", 0.0), num(el, "Y", 0.0))),
        "CoordinateFrame" | "CFrame" => {
            let mut m = [0.0; 9];
            for (i, k) in ["R00", "R01", "R02", "R10", "R11", "R12", "R20", "R21", "R22"].iter().enumerate() {
                m[i] = num(el, k, if i % 4 == 0 { 1.0 } else { 0.0 });
            }
            Some(Value::CFrame([num(el, "X", 0.0), num(el, "Y", 0.0), num(el, "Z", 0.0)], m))
        }
        "Ref" => Some(Value::Ref(txt.to_string())),
        "Content" | "url" => {
            let u2 = child(el, "url").or_else(|| child(el, "uri"));
            Some(Value::Url(match u2 {
                Some(u) => u.text.trim().to_string(),
                None => {
                    if el.kids.is_empty() {
                        txt.to_string()
                    } else {
                        String::new()
                    }
                }
            }))
        }
        _ => None,
    }
}

fn alias_content(props: &mut HashMap<String, Value>) {
    let keys: Vec<String> = props.keys().cloned().collect();
    for k in keys {
        if k.len() > 7 && k.ends_with("Content") {
            if let Some(Value::Url(u)) = props.get(&k).cloned() {
                let base = k[..k.len() - 7].to_string();
                let has_base = matches!(props.get(&base), Some(Value::Url(_)));
                if !has_base {
                    props.insert(base.clone(), Value::Url(u.clone()));
                }
                if base == "Texture" && !props.contains_key("TextureID") {
                    props.insert("TextureID".into(), Value::Url(u.clone()));
                }
                if base == "Texture" && !props.contains_key("TextureId") {
                    props.insert("TextureId".into(), Value::Url(u));
                }
            }
        }
    }
}

fn item(el: &El) -> Inst {
    let mut inst = Inst {
        cls: el.attrs.get("class").cloned().unwrap_or_default(),
        referent: el.attrs.get("referent").cloned(),
        ..Default::default()
    };
    if let Some(pr) = child(el, "Properties") {
        for p in &pr.kids {
            if let (Some(v), Some(n)) = (xml_prop(p), p.attrs.get("name")) {
                inst.props.insert(n.clone(), v);
            }
        }
    }
    alias_content(&mut inst.props);
    inst.name = match inst.props.get("Name") {
        Some(Value::Str(s)) => s.clone(),
        _ => inst.cls.clone(),
    };
    for k in &el.kids {
        if k.tag == "Item" {
            inst.children.push(item(k));
        }
    }
    inst
}

pub fn parse(bytes: &[u8]) -> Result<Tree, String> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("model too large".into());
    }
    if bytes.len() > 8 && &bytes[..8] == b"<roblox!" {
        return Err("binary models are not supported yet".into());
    }
    let src = String::from_utf8_lossy(bytes);
    let doc = xml_tree(&src)?;
    let rt = child(&doc, "roblox").ok_or("not a Roblox XML model")?;
    let roots = rt.kids.iter().filter(|k| k.tag == "Item").map(item).collect();
    Ok(Tree { roots })
}

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
        if count > 6000000 {
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
        "BinaryString" if el.attrs.get("name").map(|n| n == "ChocolateTerrain").unwrap_or(false) => Some(Value::Str(el.text.clone())),
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
    parse_limit(bytes, 16 * 1024 * 1024)
}

pub fn parse_limit(bytes: &[u8], max: usize) -> Result<Tree, String> {
    if bytes.len() > max {
        return Err("model too large".into());
    }
    if bytes.len() > 8 && &bytes[..8] == b"<roblox!" {
        return parse_binary(bytes);
    }
    let src = String::from_utf8_lossy(bytes);
    let doc = xml_tree(&src)?;
    let rt = child(&doc, "roblox").ok_or("not a Roblox XML model")?;
    let roots = rt.kids.iter().filter(|k| k.tag == "Item").map(item).collect();
    Ok(Tree { roots })
}

fn lz4(src: &[u8], out_len: usize) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::with_capacity(out_len);
    let mut i = 0;
    let n = src.len();
    let bad = || "bad LZ4 data".to_string();
    while i < n {
        let tok = src[i] as usize;
        i += 1;
        let mut lit = tok >> 4;
        if lit == 15 {
            loop {
                let k = *src.get(i).ok_or_else(bad)? as usize;
                i += 1;
                lit += k;
                if k != 255 {
                    break;
                }
            }
        }
        out.extend_from_slice(src.get(i..i + lit).ok_or_else(bad)?);
        i += lit;
        if i >= n {
            break;
        }
        let off = *src.get(i).ok_or_else(bad)? as usize | ((*src.get(i + 1).ok_or_else(bad)? as usize) << 8);
        i += 2;
        let mut ml = tok & 15;
        if ml == 15 {
            loop {
                let k = *src.get(i).ok_or_else(bad)? as usize;
                i += 1;
                ml += k;
                if k != 255 {
                    break;
                }
            }
        }
        ml += 4;
        let o = out.len();
        if off == 0 || off > o {
            return Err("bad LZ4 offset".into());
        }
        for k in 0..ml {
            let b = out[o - off + k];
            out.push(b);
        }
        if out.len() > out_len {
            return Err("LZ4 size mismatch".into());
        }
    }
    if out.len() != out_len {
        return Err("LZ4 size mismatch".into());
    }
    Ok(out)
}

fn zstd_decode(src: &[u8], out_len: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut d = ruzstd::StreamingDecoder::new(src).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(out_len);
    d.read_to_end(&mut out).map_err(|e| e.to_string())?;
    if out.len() != out_len {
        return Err("bad ZSTD chunk".into());
    }
    Ok(out)
}

const ROT: [(u8, [f64; 9]); 24] = [
    (0x02, [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
    (0x03, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0]),
    (0x05, [1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0]),
    (0x06, [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0]),
    (0x07, [0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0]),
    (0x09, [0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
    (0x0a, [0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
    (0x0c, [0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 0.0, -1.0, 0.0]),
    (0x0d, [0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0]),
    (0x0e, [0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0]),
    (0x10, [0.0, -1.0, 0.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0]),
    (0x11, [0.0, 0.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0]),
    (0x14, [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0]),
    (0x15, [-1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0]),
    (0x17, [-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0]),
    (0x18, [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, -1.0, 0.0]),
    (0x19, [0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
    (0x1b, [0.0, 0.0, -1.0, -1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
    (0x1c, [0.0, -1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0]),
    (0x1e, [0.0, 0.0, 1.0, -1.0, 0.0, 0.0, 0.0, -1.0, 0.0]),
    (0x1f, [0.0, 1.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0]),
    (0x20, [0.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0]),
    (0x22, [0.0, -1.0, 0.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0]),
    (0x23, [0.0, 0.0, -1.0, 0.0, -1.0, 0.0, -1.0, 0.0, 0.0]),
];

const CONTENT_PROPS: [&str; 24] = [
    "MeshId", "TextureId", "TextureID", "Texture", "ColorMap", "NormalMap", "MetalnessMap", "RoughnessMap", "ShirtTemplate", "PantsTemplate", "Graphic",
    "SoundId", "Image", "SkyboxBk", "SkyboxDn", "SkyboxFt", "SkyboxLf", "SkyboxRt", "SkyboxUp", "MoonTextureId", "SunTextureId", "AnimationId", "Face", "",
];

fn u32le(d: &[u8], q: usize) -> Result<usize, String> {
    d.get(q..q + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize).ok_or_else(|| "truncated chunk".to_string())
}

fn read_raw(d: &[u8], q: usize, n: usize, size: usize) -> Result<Vec<u64>, String> {
    if q + n * size > d.len() {
        return Err("truncated property".into());
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut u = 0u64;
        for b in 0..size {
            u = (u << 8) | d[q + b * n + i] as u64;
        }
        out.push(u);
    }
    Ok(out)
}

fn read_ints(d: &[u8], q: usize, n: usize) -> Result<Vec<i64>, String> {
    Ok(read_raw(d, q, n, 4)?.into_iter().map(|u| ((u >> 1) as i64) ^ -((u & 1) as i64)).collect())
}

fn read_floats(d: &[u8], q: usize, n: usize) -> Result<Vec<f64>, String> {
    Ok(read_raw(d, q, n, 4)?
        .into_iter()
        .map(|u| {
            let u = u as u32;
            f32::from_bits((u >> 1) | ((u & 1) << 31)) as f64
        })
        .collect())
}

fn read_prop(data: &[u8], mut q: usize, n: usize, ty: u8) -> Result<Option<Vec<Value>>, String> {
    let mut out: Vec<Value> = Vec::with_capacity(n);
    match ty {
        1 => {
            for _ in 0..n {
                let l = u32le(data, q)?;
                q += 4;
                let s = data.get(q..q + l).ok_or("truncated string")?;
                out.push(Value::Str(String::from_utf8_lossy(s).to_string()));
                q += l;
            }
        }
        2 => {
            for i in 0..n {
                out.push(Value::Bool(*data.get(q + i).ok_or("truncated")? != 0));
            }
        }
        3 => out.extend(read_ints(data, q, n)?.into_iter().map(|v| Value::Num(v as f64))),
        4 => out.extend(read_floats(data, q, n)?.into_iter().map(Value::Num)),
        5 => {
            for i in 0..n {
                let s = data.get(q + i * 8..q + i * 8 + 8).ok_or("truncated")?;
                out.push(Value::Num(f64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]])));
            }
        }
        11 => out.extend(read_raw(data, q, n, 4)?.into_iter().map(|u| Value::Brick(u as i64))),
        12 => {
            let r = read_floats(data, q, n)?;
            let g = read_floats(data, q + n * 4, n)?;
            let b = read_floats(data, q + n * 8, n)?;
            for k in 0..n {
                out.push(Value::Color3(r[k], g[k], b[k]));
            }
        }
        13 => {
            let x = read_floats(data, q, n)?;
            let y = read_floats(data, q + n * 4, n)?;
            for k in 0..n {
                out.push(Value::V2(x[k], y[k]));
            }
        }
        14 => {
            let x = read_floats(data, q, n)?;
            let y = read_floats(data, q + n * 4, n)?;
            let z = read_floats(data, q + n * 8, n)?;
            for k in 0..n {
                out.push(Value::V3(x[k], y[k], z[k]));
            }
        }
        16 => {
            let mut ms: Vec<[f64; 9]> = vec![];
            for _ in 0..n {
                let rid = *data.get(q).ok_or("truncated")?;
                q += 1;
                if rid == 0 {
                    let mut m = [0.0; 9];
                    for (k, slot) in m.iter_mut().enumerate() {
                        let s = data.get(q + k * 4..q + k * 4 + 4).ok_or("truncated")?;
                        *slot = f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64;
                    }
                    q += 36;
                    ms.push(m);
                } else {
                    ms.push(ROT.iter().find(|r| r.0 == rid).map(|r| r.1).unwrap_or(ROT[0].1));
                }
            }
            let px = read_floats(data, q, n)?;
            let py = read_floats(data, q + n * 4, n)?;
            let pz = read_floats(data, q + n * 8, n)?;
            for (k, m) in ms.into_iter().enumerate() {
                out.push(Value::CFrame([px[k], py[k], pz[k]], m));
            }
        }
        18 => out.extend(read_raw(data, q, n, 4)?.into_iter().map(|u| Value::Num(u as f64))),
        19 => {
            let mut a = 0i64;
            for v in read_ints(data, q, n)? {
                a += v;
                out.push(Value::Ref(a.to_string()));
            }
        }
        27 => {
            for u in read_raw(data, q, n, 8)? {
                out.push(Value::Num((((u >> 1) as i64) ^ -((u & 1) as i64)) as f64));
            }
        }
        26 => {
            if q + n * 3 > data.len() {
                return Err("truncated".into());
            }
            for i in 0..n {
                out.push(Value::Color3(data[q + i] as f64 / 255.0, data[q + n + i] as f64 / 255.0, data[q + 2 * n + i] as f64 / 255.0));
            }
        }
        0x22 => {
            let st = read_raw(data, q, n, 4)?;
            q += n * 4 + 4;
            for i in 0..n {
                if st[i] == 1 {
                    let l = u32le(data, q)?;
                    q += 4;
                    let s = data.get(q..q + l).ok_or("truncated")?;
                    out.push(Value::Url(String::from_utf8_lossy(s).to_string()));
                    q += l;
                } else {
                    out.push(Value::Url(String::new()));
                }
            }
        }
        _ => return Ok(None),
    }
    Ok(Some(out))
}

pub fn parse_binary(bytes: &[u8]) -> Result<Tree, String> {
    let mut p = 14 + 10 + 8;
    let len = bytes.len();
    let mut classes: Vec<(u32, String, Vec<usize>)> = vec![];
    let mut insts: Vec<Inst> = vec![];
    let mut ref_of: Vec<i64> = vec![];
    let mut prnt: Vec<(i64, i64)> = vec![];
    while p + 16 <= len {
        let name: Vec<u8> = bytes[p..p + 4].iter().copied().take_while(|&b| b != 0).collect();
        let cl = u32le(bytes, p + 4)?;
        let ul = u32le(bytes, p + 8)?;
        p += 16;
        if ul > 64 * 1024 * 1024 {
            return Err("chunk too large".into());
        }
        let data: Vec<u8> = if cl == 0 {
            bytes.get(p..p + ul).ok_or("truncated chunk")?.to_vec()
        } else {
            let raw = bytes.get(p..p + cl).ok_or("truncated chunk")?;
            if raw.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
                zstd_decode(raw, ul)?
            } else {
                lz4(raw, ul)?
            }
        };
        p += if cl == 0 { ul } else { cl };
        if name == b"END" {
            break;
        }
        let mut q = 0usize;
        if name == b"INST" {
            let id = u32le(&data, q)? as u32;
            q += 4;
            let ln = u32le(&data, q)?;
            q += 4;
            let cn = String::from_utf8_lossy(data.get(q..q + ln).ok_or("bad inst")?).to_string();
            q += ln + 1;
            let n = u32le(&data, q)?;
            q += 4;
            if n > 100000 || q + n * 4 > data.len() || insts.len() + n > 100000 {
                return Err("bad instance chunk".into());
            }
            let refs = read_ints(&data, q, n)?;
            let mut acc = 0i64;
            let mut list = vec![];
            for r in refs {
                acc += r;
                insts.push(Inst { cls: cn.clone(), referent: Some(acc.to_string()), ..Default::default() });
                ref_of.push(acc);
                list.push(insts.len() - 1);
            }
            classes.push((id, cn, list));
        } else if name == b"PROP" {
            let cid = u32le(&data, q)? as u32;
            q += 4;
            let pl = u32le(&data, q)?;
            q += 4;
            let pn = String::from_utf8_lossy(data.get(q..q + pl).ok_or("bad prop")?).to_string();
            q += pl;
            let ty = *data.get(q).ok_or("bad prop")?;
            q += 1;
            let list = match classes.iter().find(|c| c.0 == cid) {
                Some(c) => c.2.clone(),
                None => continue,
            };
            if let Some(vals) = read_prop(&data, q, list.len(), ty)? {
                for (i, v) in vals.into_iter().enumerate() {
                    let v = match v {
                        Value::Str(s) if CONTENT_PROPS.contains(&pn.as_str()) || pn.ends_with("Content") => Value::Url(s),
                        v => v,
                    };
                    insts[list[i]].props.insert(pn.clone(), v);
                }
            }
        } else if name == b"PRNT" {
            q += 1;
            let n = u32le(&data, q)?;
            q += 4;
            if n > 100000 || q + n * 8 > data.len() {
                return Err("bad parent chunk".into());
            }
            let a = read_ints(&data, q, n)?;
            let b = read_ints(&data, q + n * 4, n)?;
            let (mut ca, mut pa) = (0i64, 0i64);
            prnt.clear();
            for i in 0..n {
                ca += a[i];
                pa += b[i];
                prnt.push((ca, pa));
            }
        }
    }
    let idx_of = |r: i64| ref_of.iter().position(|&x| x == r);
    let mut parent: Vec<Option<usize>> = vec![None; insts.len()];
    let mut placed = vec![false; insts.len()];
    let mut order: Vec<(usize, Option<usize>)> = vec![];
    for (c, pr) in &prnt {
        let ci = match idx_of(*c) {
            Some(i) => i,
            None => continue,
        };
        if placed[ci] {
            continue;
        }
        placed[ci] = true;
        let mut par = if *pr < 0 { None } else { idx_of(*pr) };
        if let Some(mut x) = par {
            let mut d = 0;
            loop {
                if x == ci || d > 200 {
                    par = None;
                    break;
                }
                match parent[x] {
                    Some(n) => x = n,
                    None => break,
                }
                d += 1;
            }
        }
        parent[ci] = par;
        order.push((ci, par));
    }
    for inst in insts.iter_mut() {
        inst.name = match inst.props.get("Name") {
            Some(Value::Str(s)) => s.clone(),
            _ => inst.cls.clone(),
        };
        let keys: Vec<String> = inst.props.keys().cloned().collect();
        let _ = keys;
        alias_content(&mut inst.props);
    }
    let mut kids: Vec<Vec<usize>> = vec![vec![]; insts.len()];
    let mut roots_idx = vec![];
    for (ci, par) in &order {
        match par {
            Some(p) => kids[*p].push(*ci),
            None => roots_idx.push(*ci),
        }
    }
    fn build(i: usize, insts: &mut Vec<Inst>, kids: &Vec<Vec<usize>>) -> Inst {
        let mut x = std::mem::take(&mut insts[i]);
        for &k in &kids[i] {
            let c = build(k, insts, kids);
            x.children.push(c);
        }
        x
    }
    let roots = roots_idx.into_iter().map(|i| build(i, &mut insts, &kids)).collect();
    Ok(Tree { roots })
}

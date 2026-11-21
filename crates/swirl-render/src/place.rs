use crate::img::Img;
use crate::items::{cf_from, part_color, part_size};
use crate::look::{asset_id, Ctx};
use crate::mesh::*;
use crate::scene::{Base, ThumbCam};
use std::collections::HashMap;
use std::rc::Rc;
use swirl_core::math::*;
use swirl_core::rbx::{self, Inst, Value};

const SCENE_PARTS: [&str; 11] = ["Part", "WedgePart", "CornerWedgePart", "TrussPart", "Seat", "VehicleSeat", "SpawnLocation", "SkateboardPlatform", "FlagStand", "MeshPart", "UnionOperation"];

const DROP_RE: &str = r#"<Item class="(?:Script|LocalScript|ModuleScript|Sound|[A-Za-z]*SoundEffect|SoundGroup|KeyframeSequence|Animation|Animator|AnimationController|BoolValue|IntValue|NumberValue|StringValue|Color3Value|Vector3Value|CFrameValue|ObjectValue|BrickColorValue|RemoteEvent|RemoteFunction|BindableEvent|BindableFunction|ClickDetector|ScreenGui|BillboardGui|SurfaceGui|PlayerGui|StarterGui|StarterPack|StarterPlayer|Players|Teams|ReplicatedStorage|ReplicatedFirst|ServerStorage|ServerScriptService|SoundService|Chat|TextChatService|VRService|VoiceChatService|VirtualInputManager|VideoService|Humanoid|CharacterMesh|Weld|WeldConstraint|ParticleEmitter|PointLight|SpotLight|SurfaceLight|UserInputService|GuiService)""#;

fn drop_items_text(xml: &str) -> String {
    let re = regex::Regex::new(DROP_RE).unwrap();
    let starts: Vec<usize> = re.find_iter(xml).map(|m| m.start()).collect();
    if starts.is_empty() {
        return xml.to_string();
    }
    let mut out = String::with_capacity(xml.len());
    let mut pos = 0usize;
    for start in starts {
        if start < pos {
            continue;
        }
        let mut depth = 1;
        let mut i = start + 5;
        while depth > 0 {
            let o = xml[i..].find("<Item ").map(|x| x + i);
            let c = xml[i..].find("</Item>").map(|x| x + i);
            let c = match c {
                Some(c) => c,
                None => break,
            };
            match o {
                Some(o) if o < c => {
                    depth += 1;
                    i = o + 6;
                }
                _ => {
                    depth -= 1;
                    i = c + 7;
                }
            }
        }
        if depth > 0 {
            break;
        }
        out.push_str(&xml[pos..start]);
        pos = i;
    }
    out.push_str(&xml[pos..]);
    out
}

fn strip_text(bytes: &[u8]) -> String {
    let src = String::from_utf8_lossy(bytes).to_string();
    let n = src.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        let a = src[i..].find("<ProtectedString").map(|x| x + i);
        let b = src[i..].find("<BinaryString").map(|x| x + i);
        let s = match (a, b) {
            (None, None) => {
                out.push_str(&src[i..]);
                break;
            }
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (Some(a), Some(b)) => a.min(b),
        };
        let close = if src.as_bytes()[s + 1] == b'P' { "</ProtectedString>" } else { "</BinaryString>" };
        let e = match src[s..].find(close) {
            Some(e) => e + s,
            None => {
                out.push_str(&src[i..]);
                break;
            }
        };
        out.push_str(&src[i..s]);
        i = e + close.len();
    }
    let re1 = regex::Regex::new(r#"<token name="(?:Back|Bottom|Front|Left|Right|Top)Surface">0</token>\s*"#).unwrap();
    let out = re1.replace_all(&out, "").to_string();
    let unused = r"(?:Back|Bottom|Front|Left|Right|Top)(?:ParamA|ParamB|SurfaceInput)|Anchored|Archivable|CanCollide|CollisionGroupId|Locked|Elasticity|Friction|formFactorRaw|FormFactor|ControllerFlagShown|DraggingV1|PrimaryPart|LinkedSource|Disabled|ClassName";
    let re2 = regex::Regex::new(&format!(r#"<([A-Za-z0-9]+) name="(?:{})">[^<]*</([A-Za-z0-9]+)>\s*"#, unused)).unwrap();
    let out = re2.replace_all(&out, |c: &regex::Captures| if c[1] == c[2] { String::new() } else { c[0].to_string() }).to_string();
    let re3 = regex::Regex::new(r#"(?s)<Vector3 name="(?:Velocity|RotVelocity)">.*?</Vector3>\s*"#).unwrap();
    let out = re3.replace_all(&out, "").to_string();
    drop_items_text(&out)
}

#[derive(Clone)]
pub struct SItem {
    pub cf: CF,
    pub scale: V3,
    pub off: V3,
    pub color: [f64; 3],
    pub alpha: f64,
    pub tex: Option<Rc<Img>>,
    pub tex_url: Option<String>,
    pub smooth: bool,
    pub size: Option<V3>,
    pub refl: f64,
    pub mesh: Rc<Mesh>,
    pub tiles: Option<Vec<Option<Rc<Vec<Vec<i64>>>>>>,
    pub unlit: bool,
    pub decal: bool,
}

#[derive(Clone, Copy)]
pub struct Light {
    pub amb: f64,
    pub key: f64,
    pub fill: f64,
    pub tint: [f64; 3],
}

pub fn lighting_from(props: &HashMap<String, Value>, era: i32) -> Light {
    let c3 = |k: &str, d: [f64; 3]| match props.get(k) {
        Some(Value::Color3(r, g, b)) => [*r, *g, *b],
        _ => d,
    };
    let mut amb = c3("Ambient", [0.5; 3]);
    let top = c3("ColorShift_Top", [0.0; 3]);
    let bot = c3("ColorShift_Bottom", [0.0; 3]);
    let bright = props.get("Brightness").and_then(|v| v.as_f64()).map(|b| b.clamp(0.0, 5.0)).unwrap_or(1.0);
    let out = c3("OutdoorAmbient", [0.5; 3]);
    let e2018 = era == 2018;
    if e2018 && !props.contains_key("Ambient") {
        amb = [0.0; 3];
    }
    let mut amb_c = [0.0; 3];
    let mut key_c = [0.0; 3];
    let mut fill_c = [0.0; 3];
    for i in 0..3 {
        amb_c[i] = if e2018 { (0.5 * (amb[i] + out[i] * 0.55) / 0.275).clamp(0.0, 1.2) } else { amb[i].clamp(0.0, 1.2) };
        key_c[i] = 0.55 * bright * (1.0 + top[i] * 0.6 / 0.9);
        fill_c[i] = 0.12 * (0.28 + bot[i] * 0.6) / 0.28;
    }
    let avg = |v: [f64; 3]| (v[0] + v[1] + v[2]) / 3.0;
    let a = avg(amb_c);
    let k = avg(key_c);
    let mix = |i: usize| amb_c[i] + key_c[i] * 0.7;
    let base = (a + k * 0.7).max(1e-3);
    Light { amb: a, key: k, fill: avg(fill_c), tint: [mix(0) / base, mix(1) / base, mix(2) / base] }
}

struct Sparse {
    mt: [u32; 624],
    idx: usize,
}

impl Sparse {
    fn new(seed: u32) -> Sparse {
        let mut mt = [0u32; 624];
        mt[0] = seed;
        for i in 1..624 {
            mt[i] = 1812433253u32.wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_add(i as u32);
        }
        Sparse { mt, idx: 624 }
    }
    fn next(&mut self) -> u32 {
        if self.idx >= 624 {
            for i in 0..624 {
                let y = (self.mt[i] & 0x80000000) | (self.mt[(i + 1) % 624] & 0x7fffffff);
                let mut v = self.mt[(i + 397) % 624] ^ (y >> 1);
                if y & 1 != 0 {
                    v ^= 0x9908b0df;
                }
                self.mt[i] = v;
            }
            self.idx = 0;
        }
        let mut y = self.mt[self.idx];
        self.idx += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c5680;
        y ^= (y << 15) & 0xefc60000;
        y ^= y >> 18;
        y
    }
    fn rand(&mut self) -> f64 {
        (self.next() >> 1) as f64 / 2147483647.0
    }
}

fn crc32(b: &[u8]) -> u32 {
    let mut c = 0xFFFFFFFFu32;
    for &x in b {
        c ^= x as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB88320 } else { c >> 1 };
        }
    }
    !c
}

fn noise(cells: usize, seed: u64) -> Vec<f64> {
    let mut grid = vec![];
    let mut v = (seed.wrapping_mul(2654435761)) % 4294967296;
    for _ in 0..cells * cells {
        v = (v.wrapping_mul(1664525).wrapping_add(1013904223)) % 4294967296;
        grid.push(((v >> 8) & 65535) as f64 / 65535.0);
    }
    let mut out = Vec::with_capacity(128 * 128);
    for y in 0..128 {
        let fy = y as f64 / 128.0 * cells as f64;
        let iy = fy as usize;
        let mut ty = fy - iy as f64;
        ty = ty * ty * (3.0 - 2.0 * ty);
        for x in 0..128 {
            let fx = x as f64 / 128.0 * cells as f64;
            let ix = fx as usize;
            let mut tx = fx - ix as f64;
            tx = tx * tx * (3.0 - 2.0 * tx);
            let a = grid[iy * cells + ix];
            let b = grid[iy * cells + (ix + 1) % cells];
            let c = grid[((iy + 1) % cells) * cells + ix];
            let d = grid[((iy + 1) % cells) * cells + (ix + 1) % cells];
            out.push((a + (b - a) * tx) * (1.0 - ty) + (c + (d - c) * tx) * ty);
        }
    }
    out
}

fn cells(count: usize, seed: u64) -> Vec<f64> {
    let mut pts = vec![];
    let mut v = (seed.wrapping_mul(40503)) % 4294967296;
    for _ in 0..count {
        v = (v.wrapping_mul(1664525).wrapping_add(1013904223)) % 4294967296;
        let x = ((v >> 8) & 65535) as f64 / 65535.0 * 128.0;
        v = (v.wrapping_mul(1664525).wrapping_add(1013904223)) % 4294967296;
        let y = ((v >> 8) & 65535) as f64 / 65535.0 * 128.0;
        pts.push((x, y));
    }
    let mut out = Vec::with_capacity(128 * 128);
    for y in 0..128 {
        for x in 0..128 {
            let mut d1 = 1e9f64;
            let mut d2 = 1e9f64;
            for p in &pts {
                let mut dx = (x as f64 - p.0).abs();
                let mut dy = (y as f64 - p.1).abs();
                dx = dx.min(128.0 - dx);
                dy = dy.min(128.0 - dy);
                let d = dx * dx + dy * dy;
                if d < d1 {
                    d2 = d1;
                    d1 = d;
                } else if d < d2 {
                    d2 = d;
                }
            }
            out.push(d2.sqrt() - d1.sqrt());
        }
    }
    out
}

fn php_round(v: f64) -> i64 {
    v.round() as i64
}

fn material_px(material: i64) -> Option<Vec<i64>> {
    let known = [512, 528, 784, 800, 816, 832, 848, 864, 880, 1040, 1056, 1072, 1088, 1280, 1296, 1312, 1536];
    if !known.contains(&material) {
        return None;
    }
    let m = material as u64;
    let fine = noise(64, m + 1);
    let mid = noise(16, m + 2);
    let big = noise(4, m + 3);
    let cl = if material == 880 {
        Some(cells(14, 880))
    } else if material == 864 {
        Some(cells(40, 864))
    } else {
        None
    };
    let pi = std::f64::consts::PI;
    let mut px = Vec::with_capacity(16384);
    for y in 0..128i64 {
        for x in 0..128i64 {
            let i = (y * 128 + x) as usize;
            let f = fine[i] - 0.5;
            let mm = mid[i] - 0.5;
            let b = big[i] - 0.5;
            let xf = x as f64;
            let yf = y as f64;
            let v: f64 = match material {
                512 => 100.0 + 13.0 * (xf / 128.0 * 2.0 * pi * 5.0 + 7.0 * b + 3.0 * mm).sin() + 8.0 * f,
                528 => {
                    let row = y / 32;
                    let edge = if y % 32 < 2 { -32.0 } else { 0.0 };
                    100.0 + 10.0 * (xf / 128.0 * 2.0 * pi * 3.0 + row as f64 * 2.1 + 5.0 * mm).sin() + 7.0 * f + edge + if row % 2 != 0 { 4.0 } else { -4.0 }
                }
                784 => {
                    let vein = (((xf + yf) / 128.0 * 2.0 * pi * 2.0 + 7.0 * b + 4.0 * mm).sin()).abs();
                    108.0 - (if vein < 0.08 { 30.0 } else if vein < 0.2 { 10.0 } else { 0.0 }) + 4.0 * f
                }
                800 => 100.0 + 34.0 * mm + 22.0 * b + 14.0 * f,
                816 => 100.0 + 18.0 * f + 8.0 * mm,
                832 => 100.0 + 42.0 * f + 6.0 * mm,
                848 => {
                    let row = y / 16;
                    let bx = (x + if row % 2 != 0 { 16 } else { 0 }) % 32;
                    let mortar = y % 16 < 2 || bx < 2;
                    if mortar {
                        128.0
                    } else {
                        94.0 + 12.0 * mm + 8.0 * f
                    }
                }
                864 | 880 => {
                    let e = cl.as_ref().unwrap()[i];
                    (if e < 1.5 { 62.0 } else if e < 3.5 { 84.0 } else { 104.0 }) + 8.0 * f + 8.0 * mm
                }
                1040 => 92.0 + 56.0 * mm + 28.0 * b + 18.0 * f,
                1056 => {
                    let tx = x % 32;
                    let ty = y % 32;
                    let lift = (tx - ty).abs() < 2 && tx > 7 && tx < 25;
                    let tx2 = (x + 16) % 32;
                    let ty2 = (y + 16) % 32;
                    let lift2 = ((tx2 + ty2) - 32).abs() < 2 && tx2 > 7 && tx2 < 25;
                    if lift || lift2 {
                        128.0
                    } else {
                        96.0 + 3.0 * f
                    }
                }
                1072 => 104.0 + 46.0 * mm + 24.0 * f,
                1088 => 102.0 + 14.0 * (fine[(y * 128) as usize] - 0.5) + 6.0 * f,
                1280 => 98.0 + 32.0 * f + 16.0 * mm + 10.0 * b,
                1296 => 101.0 + 22.0 * f + 6.0 * b,
                1312 => 100.0 + (if ((x >> 2) + (y >> 2)) % 2 != 0 { 7.0 } else { -7.0 }) + 6.0 * f,
                _ => 108.0 + 12.0 * mm + 10.0 * b,
            };
            px.push(php_round(v));
        }
    }
    Some(px)
}

fn mips(px: Vec<i64>) -> Vec<Vec<i64>> {
    let mut levels = vec![px.clone()];
    let mut px = px;
    let mut size = 128usize;
    while size > 2 {
        let half = size >> 1;
        let mut next = Vec::with_capacity(half * half);
        for y in 0..half {
            for x in 0..half {
                let (x2, y2) = (x * 2, y * 2);
                next.push((px[y2 * size + x2] + px[y2 * size + x2 + 1] + px[(y2 + 1) * size + x2] + px[(y2 + 1) * size + x2 + 1]) / 4);
            }
        }
        levels.push(next.clone());
        px = next;
        size >>= 1;
    }
    levels
}

fn load_png_rgba(path: &str) -> Option<(usize, usize, Vec<u8>)> {
    let b = std::fs::read(path).ok()?;
    let mut dec = png::Decoder::new(std::io::Cursor::new(b));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut rd = dec.read_info().ok()?;
    let mut buf = vec![0; rd.output_buffer_size()];
    let info = rd.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    Some((info.width as usize, info.height as usize, buf))
}

fn tile(data: &str, era: i32, material: i64, surface: i64, cache: &mut HashMap<(i64, i64, i32), Option<Rc<Vec<Vec<i64>>>>>) -> Option<Rc<Vec<Vec<i64>>>> {
    let surface = if surface == 3 || surface == 4 { surface } else { 0 };
    let key = (material, surface, era);
    if let Some(c) = cache.get(&key) {
        return c.clone();
    }
    let mut px = material_px(material);
    if surface != 0 && era == 2018 {
        if let Some((w, h, buf)) = load_png_rgba(&format!("{}/studs2018.png", data)) {
            if w >= 128 && h >= 2048 {
                let mut p = px.unwrap_or_else(|| vec![100; 16384]);
                let base = if surface == 3 { 0 } else { 1024 };
                for y in 0..128usize {
                    for x in 0..128usize {
                        let s = buf[((base + (y % 32) * 2) * w + (x % 32) * 2) * 4] as f64;
                        p[y * 128 + x] = php_round(p[y * 128 + x] as f64 * s / 128.0);
                    }
                }
                px = Some(p);
            }
        }
    } else if surface != 0 {
        let file = format!("{}/{}.png", data, if surface == 3 { "stud" } else { "inlet" });
        if let Some((gw, gh, buf)) = load_png_rgba(&file) {
            let mut p = px.unwrap_or_else(|| vec![100; 16384]);
            for y in 0..128usize {
                for x in 0..128usize {
                    let sx = (((x % 32) as f64) / 32.0 * gw as f64) as usize;
                    let sy = (((y % 32) as f64) / 32.0 * gh as f64) as usize;
                    let o = (sy * gw + sx) * 4;
                    let a7 = 127 - (buf[o + 3] as u32 >> 1);
                    let a = (127 - a7) as f64 / 127.0;
                    let k = if buf[o] > 127 { 1.0 + 0.55 * a } else { 1.0 - 0.55 * a };
                    p[y * 128 + x] = php_round(p[y * 128 + x] as f64 * k);
                }
            }
            px = Some(p);
        }
    }
    let r = px.map(|p| Rc::new(mips(p)));
    cache.insert(key, r.clone());
    r
}

fn scene_faces(f: &[Vec<V3>]) -> Mesh {
    scene_faces_pub(f)
}

fn wedge(x: f64, y: f64, z: f64) -> Mesh {
    let a = [-x, -y, -z];
    let b = [x, -y, -z];
    let c = [x, -y, z];
    let d = [-x, -y, z];
    let e = [-x, y, z];
    let f = [x, y, z];
    scene_faces(&[vec![a, b, c, d], vec![d, c, f, e], vec![a, e, f, b], vec![a, d, e], vec![b, f, c]])
}

fn corner(x: f64, y: f64, z: f64) -> Mesh {
    let a = [-x, -y, -z];
    let b = [x, -y, -z];
    let c = [x, -y, z];
    let d = [-x, -y, z];
    let t = [x, y, -z];
    scene_faces(&[vec![a, b, c, d], vec![b, t, c], vec![a, t, b], vec![c, t, d], vec![d, t, a]])
}

fn face_id(v: Option<&Value>) -> i64 {
    match v {
        Some(Value::Str(s)) if s.parse::<f64>().is_err() => match s.to_lowercase().as_str() {
            "right" => 0,
            "top" => 1,
            "back" => 2,
            "left" => 3,
            "bottom" => 4,
            _ => 5,
        },
        Some(v) => v.as_f64().map(|f| f as i64).unwrap_or(5),
        None => 5,
    }
}

fn decals(h: &Inst, cf: CF, size: V3) -> Vec<SItem> {
    let mut out = vec![];
    for c in &h.children {
        if c.cls != "Decal" {
            continue;
        }
        let url = match c.url("Texture") {
            Some(u) if !u.is_empty() => u.to_string(),
            _ => continue,
        };
        let dt = c.props.get("Transparency").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let da = (1.0 - dt).clamp(0.0, 1.0);
        if da < 0.05 {
            continue;
        }
        out.push(SItem {
            cf,
            scale: [1.0; 3],
            off: [0.0; 3],
            color: [1.0; 3],
            alpha: da,
            tex: None,
            tex_url: Some(url),
            smooth: false,
            size: Some(size),
            refl: 0.0,
            mesh: Rc::new(scene_decal(size, face_id(c.props.get("Face")))),
            tiles: None,
            unlit: false,
            decal: true,
        });
    }
    out
}

pub struct SceneReq {
    pub kind: String,
    pub bytes: Vec<u8>,
    pub w: usize,
    pub h: usize,
    pub out_w: usize,
    pub out_h: usize,
    pub era: i32,
    pub content_dir: Option<String>,
}

fn in_wearable(path: &[&Inst]) -> bool {
    path.iter().any(|p| matches!(p.cls.as_str(), "Tool" | "HopperBin" | "Hat" | "Accessory" | "Accoutrement"))
}

fn thumb_pose(thumb: Option<&Inst>) -> Option<(V3, V3, f64)> {
    let t = thumb?;
    let frame = t.props.get("CFrame").or(t.props.get("CoordinateFrame"))?;
    let cf = match frame {
        Value::CFrame(p, m) => CF::new(*p, *m),
        _ => return None,
    };
    let look = rmul(&cf.r, [0.0, 0.0, -1.0]);
    if dot(look, look) < 0.5 {
        return None;
    }
    let fov = t.props.get("FieldOfView").and_then(|v| v.as_f64()).unwrap_or(70.0);
    let fov = if fov > 1.0 && fov < 150.0 { fov } else { 70.0 };
    let eye = cf.p;
    Some((eye, [eye[0] + look[0], eye[1] + look[1], eye[2] + look[2]], fov))
}

fn camera_fit(items: &[SItem], kind: &str, cam: Option<&Inst>, aspect: f64) -> (V3, V3, f64) {
    let mut centres: Vec<V3> = vec![];
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for it in items {
        let mut p = it.cf.p;
        let ext = match it.size {
            Some(s) => s,
            None => {
                p = [0.0; 3];
                [2.0, 2.0, 2.0]
            }
        };
        let big = ext[0].max(ext[1]).max(ext[2]);
        if kind == "place" && big > 400.0 {
            continue;
        }
        centres.push(p);
        for k in 0..3 {
            lo[k] = lo[k].min(p[k] - ext[k] / 2.0);
            hi[k] = hi[k].max(p[k] + ext[k] / 2.0);
        }
    }
    if centres.is_empty() {
        for it in items {
            centres.push(it.cf.p);
        }
        lo = [-64.0, 0.0, -64.0];
        hi = [64.0, 32.0, 64.0];
    }
    if kind == "place" && centres.len() > 10 {
        for k in 0..3 {
            let mut v: Vec<f64> = centres.iter().map(|c| c[k]).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let n = v.len();
            lo[k] = lo[k].max(v[(n as f64 * 0.03) as usize] - 8.0);
            hi[k] = hi[k].min(v[(n as f64 * 0.97) as usize] + 8.0);
        }
    }
    let diag = |lo: &V3, hi: &V3| ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
    if kind == "place" && centres.len() > 10 && diag(&lo, &hi) > 1200.0 {
        let mut med = [0.0; 3];
        for k in 0..3 {
            let mut v: Vec<f64> = centres.iter().map(|c| c[k]).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            med[k] = v[v.len() / 2];
        }
        let dist: Vec<f64> = centres.iter().map(|q| ((q[0] - med[0]).powi(2) + (q[1] - med[1]).powi(2) + (q[2] - med[2]).powi(2)).sqrt()).collect();
        let mut sorted = dist.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let reach = (sorted[(sorted.len() as f64 * 0.8) as usize] * 1.5).max(60.0);
        lo = [f64::INFINITY; 3];
        hi = [f64::NEG_INFINITY; 3];
        for (i, q) in centres.iter().enumerate() {
            if dist[i] > reach {
                continue;
            }
            for k in 0..3 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        for k in 0..3 {
            lo[k] -= 8.0;
            hi[k] += 8.0;
        }
    }
    let pose = if kind == "place" { thumb_pose(cam) } else { None };
    if let Some((eye, t, fov)) = pose {
        if eye[0].abs() + eye[1].abs() + eye[2].abs() > 0.01 {
            let dir = norm(sub(t, eye));
            let tan_y = (fov.to_radians() / 2.0).tan();
            let tan_x = tan_y * aspect;
            let up = if dir[1].abs() > 0.99 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
            let right = norm(cross(dir, up));
            let upv = cross(right, dir);
            let mut seen = 0;
            let mut total = 0;
            for it in items {
                let v = sub(it.cf.p, eye);
                let z = dot(v, dir);
                let span = it.size.map(|s| s[0].max(s[1]).max(s[2])).unwrap_or(4.0);
                if span > 150.0 {
                    continue;
                }
                total += 1;
                if z > 0.2 && span > z * 0.006 && dot(v, right).abs() < z * tan_x && dot(v, upv).abs() < z * tan_y {
                    seen += 1;
                }
            }
            if total == 0 || (seen > 0 && seen as f64 >= total as f64 * 0.25) {
                return (eye, t, fov);
            }
        }
    }
    let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let r = (diag(&lo, &hi) / 2.0).max(1.0);
    let fov: f64 = if kind == "place" { 50.0 } else { 30.0 };
    let dir = if kind == "place" { norm([-0.62, 0.5, -0.6]) } else { norm([-0.5, 0.35, -0.8]) };
    let fit = r / (fov.to_radians() / 2.0 * aspect.min(1.0)).sin();
    let dist = if kind == "place" { fit * 0.8 } else { fit * 1.05 };
    ([c[0] + dir[0] * dist, c[1] + dir[1] * dist, c[2] + dir[2] * dist], c, fov)
}

fn reflect(col: [f64; 3], refl: f64, v: V3, n: V3) -> [f64; 3] {
    let vl = norm(v);
    let nn = norm(n);
    let d = vl[0] * nn[0] + vl[1] * nn[1] + vl[2] * nn[2];
    let ry = vl[1] - 2.0 * d * nn[1];
    let env = if ry >= 0.0 {
        let t = (ry * 1.4).min(1.0);
        [0.80 + (0.42 - 0.80) * t, 0.87 + (0.62 - 0.87) * t, 0.96 + (0.92 - 0.96) * t]
    } else {
        let t = (-ry * 2.0).min(1.0);
        [0.55 + (0.33 - 0.55) * t, 0.55 + (0.32 - 0.55) * t, 0.53 + (0.30 - 0.53) * t]
    };
    let k = refl * 0.85;
    [col[0] * (1.0 - k) + env[0] * k, col[1] * (1.0 - k) + env[1] * k, col[2] * (1.0 - k) + env[2] * k]
}

struct Tri {
    a: [f64; 6],
    b: [f64; 6],
    c: [f64; 6],
    area: f64,
    col: [f64; 3],
    tex: Option<Rc<Img>>,
    smooth: bool,
    alpha: f64,
    decal: bool,
    tile: Option<Rc<Vec<Vec<i64>>>>,
}

fn triangles(items: &mut Vec<SItem>, eye: V3, target: V3, fov: f64, w: usize, h: usize, sl: Light) -> (Vec<Tri>, Vec<Tri>) {
    let dir = norm(sub(target, eye));
    let up_ref = if dir[1].abs() > 0.99 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
    let right = norm(cross(dir, up_ref));
    let up = cross(right, dir);
    let f = (h as f64 / 2.0) / (fov.to_radians() / 2.0).tan();
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let near = 0.2;
    let sun = norm([0.45, 0.8, -0.4]);
    let fill = norm([-0.5, 0.2, 0.6]);
    let mut solid = vec![];
    let mut glass = vec![];
    let mut count = 0usize;
    items.sort_by(|p, q| {
        if p.decal != q.decal {
            return (p.decal as i32).cmp(&(q.decal as i32));
        }
        let ps = p.size.map(|s| s[0].max(s[1]).max(s[2])).unwrap_or(0.0);
        let qs = q.size.map(|s| s[0].max(s[1]).max(s[2])).unwrap_or(0.0);
        qs.partial_cmp(&ps).unwrap_or(std::cmp::Ordering::Equal)
    });
    for it in items.iter() {
        if count > 700000 {
            break;
        }
        let r = it.cf;
        let m = &it.mesh;
        let [sx, sy, sz] = it.scale;
        let [ox, oy, oz] = it.off;
        let part_size = it.size.unwrap_or([0.0; 3]);
        let refl = if it.decal { 0.0 } else { it.refl };
        let halfs = [part_size[0] / 2.0, part_size[1] / 2.0, part_size[2] / 2.0];
        let n = m.pos.len();
        let non_uniform = (sx - sy).abs() > 1e-3 || (sy - sz).abs() > 1e-3;
        let mut i = 0;
        while i + 2 < n {
            let mut vs: Vec<[f64; 6]> = vec![];
            let mut n0 = [0.0; 3];
            let mut v0 = [0.0; 3];
            let mut axis = 0usize;
            let mut face_tile: Option<Rc<Vec<Vec<i64>>>> = None;
            for j in 0..3 {
                let q = m.pos[i + j];
                let wv = r.apply([q[0] * sx + ox, q[1] * sy + oy, q[2] * sz + oz]);
                let v = [wv[0] - eye[0], wv[1] - eye[1], wv[2] - eye[2]];
                let zc = dot(v, dir);
                let xc = dot(v, right);
                let yc = dot(v, up);
                let mut nl = m.nrm.get(i + j).copied().unwrap_or([0.0, 1.0, 0.0]);
                if non_uniform {
                    nl = norm([nl[0] / sx.max(1e-4), nl[1] / sy.max(1e-4), nl[2] / sz.max(1e-4)]);
                }
                let nw = rmul(&r.r, nl);
                if j == 0 {
                    n0 = nw;
                    v0 = v;
                }
                let d1 = (nw[0] * sun[0] + nw[1] * sun[1] + nw[2] * sun[2]).max(0.0);
                let d2 = (nw[0] * fill[0] + nw[1] * fill[1] + nw[2] * fill[2]).max(0.0);
                let mut uv = m.uv.get(i + j).copied().unwrap_or([0.0, 0.0]);
                if let Some(tiles) = &it.tiles {
                    if j == 0 {
                        let nf = m.nrm.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
                        let (ax, ay, az) = (nf[0].abs(), nf[1].abs(), nf[2].abs());
                        axis = if ax >= ay && ax >= az { 0 } else if ay >= az { 1 } else { 2 };
                        face_tile = tiles[if nf[axis] >= 0.0 { axis } else { axis + 3 }].clone();
                    }
                    let lx = q[0] + halfs[0];
                    let ly = q[1] + halfs[1];
                    let lz = q[2] + halfs[2];
                    uv = if axis == 0 { [lz / 4.0, ly / 4.0] } else if axis == 1 { [lx / 4.0, lz / 4.0] } else { [lx / 4.0, ly / 4.0] };
                }
                let lit = if it.unlit { 1.15 } else { (sl.amb + sl.key * d1 + sl.fill * d2).min(1.17) };
                vs.push([xc, yc, zc, lit, uv[0], uv[1]]);
            }
            let mut poly = vs.clone();
            if vs[0][2] < near || vs[1][2] < near || vs[2][2] < near {
                poly = vec![];
                for j in 0..3 {
                    let a = vs[j];
                    let b = vs[(j + 1) % 3];
                    let ai = a[2] >= near;
                    let bi = b[2] >= near;
                    if ai {
                        poly.push(a);
                    }
                    if ai != bi {
                        let t = (near - a[2]) / (b[2] - a[2]);
                        let mut c = [0.0; 6];
                        for k in 0..6 {
                            c[k] = a[k] + (b[k] - a[k]) * t;
                        }
                        poly.push(c);
                    }
                }
                if poly.len() < 3 {
                    i += 3;
                    continue;
                }
            }
            let pc: Vec<[f64; 6]> = poly
                .iter()
                .map(|q| {
                    let iz = 1.0 / q[2];
                    [cx + q[0] * f * iz, cy - q[1] * f * iz, iz, q[3], q[4], q[5]]
                })
                .collect();
            for t in 1..pc.len() - 1 {
                let (a, b, c) = (pc[0], pc[t], pc[t + 1]);
                let area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
                if area > -0.01 {
                    continue;
                }
                if a[0].max(b[0]).max(c[0]) < 0.0 || a[0].min(b[0]).min(c[0]) >= w as f64 || a[1].max(b[1]).max(c[1]) < 0.0 || a[1].min(b[1]).min(c[1]) >= h as f64 {
                    continue;
                }
                let tcol = if refl > 0.01 { reflect(it.color, refl, v0, n0) } else { it.color };
                let tri = Tri { a, b, c, area, col: tcol, tex: it.tex.clone(), smooth: it.smooth, alpha: it.alpha, decal: it.decal, tile: if it.tiles.is_some() { face_tile.clone() } else { None } };
                if it.alpha < 0.99 || it.decal {
                    glass.push(tri);
                } else {
                    solid.push(tri);
                }
                count += 1;
            }
            i += 3;
        }
    }
    glass.sort_by(|p, q| (p.a[2] + p.b[2] + p.c[2]).partial_cmp(&(q.a[2] + q.b[2] + q.c[2])).unwrap_or(std::cmp::Ordering::Equal));
    (solid, glass)
}

fn raster(tris: &(Vec<Tri>, Vec<Tri>), w: usize, h: usize, sky: &Sky, sl: Light) -> Vec<u32> {
    let n = w * h;
    let [t_r, t_g, t_b] = sl.tint;
    let mut z = vec![0.0f64; n];
    let mut c: Vec<u32> = match sky {
        Sky::Pixels(p) if p.len() == n => p.clone(),
        Sky::Gradient => {
            let mut v = Vec::with_capacity(n);
            for y in 0..h {
                let t = y as f64 / (h as i64 - 1).max(1) as f64;
                let r = (96.0 + 110.0 * t) as u32;
                let g = (158.0 + 62.0 * t) as u32;
                let b = (232.0 + 16.0 * t) as u32;
                let rgb = (r << 16) | (g << 8) | b;
                for _ in 0..w {
                    v.push(rgb);
                }
            }
            v
        }
        _ => vec![127u32 << 24; n],
    };
    for pass in 0..2 {
        let list = if pass == 0 { &tris.0 } else { &tris.1 };
        for t in list {
            let (a, b, cc) = (&t.a, &t.b, &t.c);
            let x0 = (a[0].min(b[0]).min(cc[0]).floor() as i64).max(0);
            let x1 = (a[0].max(b[0]).max(cc[0]).ceil() as i64).min(w as i64 - 1);
            let y0 = (a[1].min(b[1]).min(cc[1]).floor() as i64).max(0);
            let y1 = (a[1].max(b[1]).max(cc[1]).ceil() as i64).min(h as i64 - 1);
            if x1 < x0 || y1 < y0 {
                continue;
            }
            let ia = -1.0 / t.area;
            let e0x = (cc[1] - b[1]) * ia;
            let e0y = (b[0] - cc[0]) * ia;
            let e1x = (a[1] - cc[1]) * ia;
            let e1y = (cc[0] - a[0]) * ia;
            let e2x = (b[1] - a[1]) * ia;
            let e2y = (a[0] - b[0]) * ia;
            let px = x0 as f64 + 0.5;
            let py = y0 as f64 + 0.5;
            let mut r0 = ((px - b[0]) * (cc[1] - b[1]) - (py - b[1]) * (cc[0] - b[0])) * ia;
            let mut r1 = ((px - cc[0]) * (a[1] - cc[1]) - (py - cc[1]) * (a[0] - cc[0])) * ia;
            let mut r2 = ((px - a[0]) * (b[1] - a[1]) - (py - a[1]) * (b[0] - a[0])) * ia;
            let (za, zb, zc) = (a[2], b[2], cc[2]);
            let (mut la, mut lb, mut lc) = (a[3], b[3], cc[3]);
            if !t.smooth {
                let m = (la + lb + lc) / 3.0;
                la = m;
                lb = m;
                lc = m;
            }
            let cr = t.col[0] * 255.0;
            let cg = t.col[1] * 255.0;
            let cb = t.col[2] * 255.0;
            let (tw, th) = t.tex.as_ref().map(|i| (i.w, i.h)).unwrap_or((0, 0));
            let (ua, ub, uc) = (a[4] * za, b[4] * zb, cc[4] * zc);
            let (va, vb, vc) = (a[5] * za, b[5] * zb, cc[5] * zc);
            let d_ux = e0x * ua + e1x * ub + e2x * uc;
            let d_uy = e0y * ua + e1y * ub + e2y * uc;
            let d_vx = e0x * va + e1x * vb + e2x * vc;
            let d_vy = e0y * va + e1y * vb + e2y * vc;
            let d_zx = e0x * za + e1x * zb + e2x * zc;
            let d_zy = e0y * za + e1y * zb + e2y * zc;
            for y in y0..=y1 {
                let (mut w0, mut w1, mut w2) = (r0, r1, r2);
                let row = y as usize * w;
                for x in x0..=x1 {
                    if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                        let iz = w0 * za + w1 * zb + w2 * zc;
                        let k = row + x as usize;
                        if iz > z[k] {
                            let mut l = w0 * la + w1 * lb + w2 * lc;
                            let (mut r, mut g, mut bl) = (cr, cg, cb);
                            let mut over = t.alpha;
                            if let Some(tile) = &t.tile {
                                let zz = 1.0 / iz;
                                let u = (w0 * ua + w1 * ub + w2 * uc) * zz;
                                let v = (w0 * va + w1 * vb + w2 * vc) * zz;
                                let dux = (d_ux - u * d_zx) * zz;
                                let dvx = (d_vx - v * d_zx) * zz;
                                let duy = (d_uy - u * d_zy) * zz;
                                let dvy = (d_vy - v * d_zy) * zz;
                                let rho = 128.0 * ((dux * dux + dvx * dvx).max(duy * duy + dvy * dvy)).sqrt();
                                let (lv, ts) = if rho <= 1.5 { (0, 128) } else if rho <= 3.0 { (1, 64) } else if rho <= 6.0 { (2, 32) } else if rho <= 12.0 { (3, 16) } else if rho <= 24.0 { (4, 8) } else { (5, 4) };
                                let iv = (((v - v.floor()) * ts as f64) as i64 & (ts as i64 - 1)) as usize;
                                let iu = (((u - u.floor()) * ts as f64) as i64 & (ts as i64 - 1)) as usize;
                                l *= tile[lv][iv * ts + iu] as f64 / 100.0;
                            }
                            if let Some(tx) = &t.tex {
                                let zz = 1.0 / iz;
                                let u = (w0 * ua + w1 * ub + w2 * uc) * zz;
                                let v = (w0 * va + w1 * vb + w2 * vc) * zz;
                                let txx = (((u - u.floor()) * tw as f64).floor() as i64 % tw as i64) as usize;
                                let tyy = (((v - v.floor()) * th as f64).floor() as i64 % th as i64) as usize;
                                let s = tx.px[tyy * tw + txx];
                                let ta = (s >> 24) & 255;
                                if ta < 8 {
                                    w0 += e0x;
                                    w1 += e1x;
                                    w2 += e2x;
                                    continue;
                                }
                                let f = ta as f64 / 255.0;
                                if t.decal {
                                    r = ((s >> 16) & 255) as f64;
                                    g = ((s >> 8) & 255) as f64;
                                    bl = (s & 255) as f64;
                                    over = t.alpha * f;
                                    l = 1.0;
                                } else {
                                    r = ((s >> 16) & 255) as f64 * f + r * (1.0 - f);
                                    g = ((s >> 8) & 255) as f64 * f + g * (1.0 - f);
                                    bl = (s & 255) as f64 * f + bl * (1.0 - f);
                                }
                            }
                            r = (r * l * t_r).min(255.0);
                            g = (g * l * t_g).min(255.0);
                            bl = (bl * l * t_b).min(255.0);
                            if over < 0.99 {
                                let d = c[k];
                                let da = (d >> 24) & 127;
                                if da == 127 {
                                    c[k] = (((127.0 - 127.0 * over) as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | bl as u32;
                                } else {
                                    let r2 = r * over + ((d >> 16) & 255) as f64 * (1.0 - over);
                                    let g2 = g * over + ((d >> 8) & 255) as f64 * (1.0 - over);
                                    let b2 = bl * over + (d & 255) as f64 * (1.0 - over);
                                    c[k] = (da << 24) | ((r2 as u32) << 16) | ((g2 as u32) << 8) | b2 as u32;
                                }
                            } else {
                                z[k] = iz;
                                c[k] = ((r as u32) << 16) | ((g as u32) << 8) | bl as u32;
                            }
                        }
                    }
                    w0 += e0x;
                    w1 += e1x;
                    w2 += e2x;
                }
                r0 += e0y;
                r1 += e1y;
                r2 += e2y;
            }
        }
    }
    c
}

enum Sky {
    None,
    Gradient,
    Pixels(Vec<u32>),
}

fn sky_pixels(faces: &[Img], eye: V3, target: V3, fov: f64, w: usize, h: usize) -> Vec<u32> {
    let dir = norm(sub(target, eye));
    let up_ref = if dir[1].abs() > 0.99 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
    let right = norm(cross(dir, up_ref));
    let up = cross(right, dir);
    let f = (h as f64 / 2.0) / (fov.to_radians() / 2.0).tan();
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        let v = (cy - y as f64 - 0.5) / f;
        for x in 0..w {
            let u = (x as f64 + 0.5 - cx) / f;
            let rx = -(dir[0] + u * right[0] + v * up[0]);
            let ry = dir[1] + u * right[1] + v * up[1];
            let rz = dir[2] + u * right[2] + v * up[2];
            let (ax, ay, az) = (rx.abs(), ry.abs(), rz.abs());
            let (face, sc, tc, ma) = if ax >= ay && ax >= az {
                if rx > 0.0 { (0, -rz, -ry, ax) } else { (1, rz, -ry, ax) }
            } else if ay >= az {
                if ry > 0.0 { (2, rx, rz, ay) } else { (3, rx, -rz, ay) }
            } else if rz > 0.0 {
                (4, rx, -ry, az)
            } else {
                (5, -rx, -ry, az)
            };
            let img = &faces[face];
            let (fw, fh) = (img.w as i64, img.h as i64);
            let mut s = (sc / ma + 1.0) / 2.0;
            let mut t = (tc / ma + 1.0) / 2.0;
            if face == 2 {
                let (ns, nt) = (1.0 - t, s);
                s = ns;
                t = nt;
            } else if face == 3 {
                let (ns, nt) = (t, 1.0 - s);
                s = ns;
                t = nt;
            }
            let sx = s * fw as f64 - 0.5;
            let sy = t * fh as f64 - 0.5;
            let x0 = sx.floor() as i64;
            let y0 = sy.floor() as i64;
            let fx = sx - x0 as f64;
            let fy = sy - y0 as f64;
            let xa = x0.clamp(0, fw - 1);
            let xb = (x0 + 1).clamp(0, fw - 1);
            let ya = y0.clamp(0, fh - 1);
            let yb = (y0 + 1).clamp(0, fh - 1);
            let p = &img.px;
            let p00 = p[(ya * fw + xa) as usize];
            let p10 = p[(ya * fw + xb) as usize];
            let p01 = p[(yb * fw + xa) as usize];
            let p11 = p[(yb * fw + xb) as usize];
            let (w00, w10, w01, w11) = ((1.0 - fx) * (1.0 - fy), fx * (1.0 - fy), (1.0 - fx) * fy, fx * fy);
            let ch = |s: u32| (((p00 >> s) & 255) as f64 * w00 + ((p10 >> s) & 255) as f64 * w10 + ((p01 >> s) & 255) as f64 * w01 + ((p11 >> s) & 255) as f64 * w11) as u32;
            out.push((ch(16) << 16) | (ch(8) << 8) | ch(0));
        }
    }
    out
}

fn gd_resample(src: &[u32], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u32> {
    let mut out = Vec::with_capacity(dw * dh);
    for y in 0..dh {
        let sy1 = (y as f32) * sh as f32 / dh as f32;
        let sy2 = ((y + 1) as f32) * sh as f32 / dh as f32;
        for x in 0..dw {
            let sx1 = (x as f32) * sw as f32 / dw as f32;
            let sx2 = ((x + 1) as f32) * sw as f32 / dw as f32;
            let mut spixels = 0.0f32;
            let (mut red, mut green, mut blue, mut alpha) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            let mut alpha_sum = 0.0f32;
            let mut contrib_sum = 0.0f32;
            let mut sy = sy1;
            loop {
                let yportion;
                if sy.floor() == sy1.floor() {
                    let mut yp = 1.0 - (sy - sy.floor());
                    if yp > sy2 - sy1 {
                        yp = sy2 - sy1;
                    }
                    yportion = yp;
                    sy = sy.floor();
                } else if sy == sy2.floor() {
                    yportion = sy2 - sy2.floor();
                } else {
                    yportion = 1.0;
                }
                let mut sx = sx1;
                loop {
                    let xportion;
                    if sx.floor() == sx1.floor() {
                        let mut xp = 1.0 - (sx - sx.floor());
                        if xp > sx2 - sx1 {
                            xp = sx2 - sx1;
                        }
                        xportion = xp;
                        sx = sx.floor();
                    } else if sx == sx2.floor() {
                        xportion = sx2 - sx2.floor();
                    } else {
                        xportion = 1.0;
                    }
                    let pcontribution = xportion * yportion;
                    let ix = (sx as usize).min(sw - 1);
                    let iy = (sy as usize).min(sh - 1);
                    let p = src[iy * sw + ix];
                    let a7 = ((p >> 24) & 127) as f32;
                    let alpha_factor = (127.0 - a7) * pcontribution;
                    red += ((p >> 16) & 255) as f32 * alpha_factor;
                    green += ((p >> 8) & 255) as f32 * alpha_factor;
                    blue += (p & 255) as f32 * alpha_factor;
                    alpha += a7 * pcontribution;
                    alpha_sum += alpha_factor;
                    contrib_sum += pcontribution;
                    spixels += xportion * yportion;
                    sx += 1.0;
                    if sx >= sx2 {
                        break;
                    }
                }
                sy += 1.0;
                if sy >= sy2 {
                    break;
                }
            }
            if spixels != 0.0 {
                red /= spixels;
                green /= spixels;
                blue /= spixels;
                alpha /= spixels;
            }
            if alpha_sum != 0.0 {
                if contrib_sum != 0.0 {
                    alpha_sum /= contrib_sum;
                }
                red /= alpha_sum;
                green /= alpha_sum;
                blue /= alpha_sum;
            }
            let cl = |v: f32| if v > 255.0 { 255u32 } else { v as u32 };
            let a = if alpha > 127.0 { 127u32 } else { alpha as u32 };
            out.push((a << 24) | (cl(red) << 16) | (cl(green) << 8) | cl(blue));
        }
    }
    out
}

fn encode_gd_png(c: &[u32], w: usize, h: usize) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(w * h * 4);
    for &p in c {
        let a7 = (p >> 24) & 127;
        let a = 255 - ((a7 << 1) + (a7 >> 6));
        rgba.extend_from_slice(&[((p >> 16) & 255) as u8, ((p >> 8) & 255) as u8, (p & 255) as u8, a as u8]);
    }
    crate::img::encode_png(w, h, &rgba)
}

fn load_img_file(path: &str) -> Option<Img> {
    let b = std::fs::read(path).ok()?;
    Img::decode_cap(&b, 512, 2048)
}

fn mesh_bytes_scene(ctx: &Ctx, _content_dir: &Option<String>, r: &str) -> Option<Vec<u8>> {
    crate::items::mesh_bytes_pub(ctx, r)
}

pub fn render_scene(ctx: &Ctx, base: &Base, req: &SceneReq) -> Result<(Vec<u8>, Vec<String>), String> {
    let (w, h) = (req.w, req.h);
    if req.bytes.is_empty() || w < 16 || h < 16 || w > 1920 || h > 1080 {
        return Err("scene needs bytes and a size".into());
    }
    let era = if req.era >= 2017 { 2018 } else { 2012 };
    let kind = req.kind.as_str();
    let mut notes: Vec<String> = vec![];
    let mut sl = lighting_from(&HashMap::new(), era);
    let mut sky_inst: Option<Inst> = None;
    let mut items: Vec<SItem>;
    let mut parts_for_sparkles: Vec<(CF, V3, Vec<Inst>)> = vec![];
    let mut thumb: Option<Inst> = None;
    let mut cam: Option<Inst> = None;
    let mut overlays = false;
    if kind == "mesh" {
        let mesh = clean_mesh(&parse_mesh2016(&req.bytes)?);
        items = vec![SItem { cf: CF::identity(), scale: [1.0; 3], off: [0.0; 3], color: [0.64, 0.64, 0.66], alpha: 1.0, tex: None, tex_url: None, smooth: true, size: None, refl: 0.0, mesh: Rc::new(mesh), tiles: None, unlit: false, decal: false }];
    } else {
        let stripped: Vec<u8>;
        let src: &[u8] = if req.bytes.starts_with(b"<roblox!") {
            &req.bytes
        } else {
            stripped = strip_text(&req.bytes).into_bytes();
            &stripped
        };
        let tree = rbx::parse_limit(src, 67108864)?;
        let mut roots: Vec<Inst> = tree.roots;
        if kind == "place" {
            for r in &roots {
                if r.cls == "Lighting" {
                    sl = lighting_from(&r.props, era);
                    for c in &r.children {
                        if c.cls == "Sky" {
                            sky_inst = Some(c.clone());
                            break;
                        }
                    }
                    if sky_inst.is_some() {
                        break;
                    }
                }
            }
            let world: Vec<Inst> = roots.iter().filter(|r| r.cls == "Workspace").cloned().collect();
            if !world.is_empty() {
                roots = world;
            }
        }
        let mut saved_ref: Option<String> = None;
        for r in &roots {
            if let Some(Value::Ref(s)) = r.props.get("CurrentCamera") {
                saved_ref = Some(s.clone());
            }
        }
        let mut parts: Vec<(Inst, bool)> = vec![];
        let mut saved: Option<Inst> = None;
        let mut stack: Vec<(&Inst, Vec<&Inst>)> = roots.iter().map(|r| (r, vec![])).collect();
        while let Some((x, path)) = stack.pop() {
            if x.cls == "Camera" && (kind != "place" || !in_wearable(&path)) {
                if x.name.eq_ignore_ascii_case("ThumbnailCamera") {
                    if thumb.is_none() {
                        thumb = Some(x.clone());
                    }
                } else if cam.is_none() {
                    cam = Some(x.clone());
                }
            }
            if let (Some(rf), Some(r)) = (&saved_ref, &x.referent) {
                if r == rf && x.cls == "Camera" {
                    saved = Some(x.clone());
                }
            }
            if SCENE_PARTS.contains(&x.cls.as_str()) {
                parts.push((x.clone(), true));
            }
            if kind == "place" && (x.cls == "Hint" || x.cls == "Message") {
                if let Some(Value::Str(t)) = x.props.get("Text") {
                    if !t.trim().is_empty() {
                        overlays = true;
                    }
                }
            }
            for c in &x.children {
                let mut p = path.clone();
                p.push(x);
                stack.push((c, p));
            }
        }
        if parts.is_empty() {
            return Err(format!("nothing to draw in this {}", kind));
        }
        if overlays {
            return Err("not ported: scene overlays".into());
        }
        cam = saved.or(cam);
        let part_list: Vec<Inst> = parts.into_iter().map(|p| p.0).collect();
        items = scene_items(ctx, base, &part_list, era, &req.content_dir, &mut notes);
        for h in &part_list {
            if h.props.contains_key("CFrame") {
                parts_for_sparkles.push((cf_from(h.props.get("CFrame")), part_size(h), h.children.iter().filter(|c| c.cls == "Sparkles").cloned().collect()));
            }
        }
    }
    if items.is_empty() {
        return Err(format!("nothing visible in this {}", kind));
    }
    let aspect = w as f64 / h as f64;
    let pose = thumb_pose(thumb.as_ref());
    let fitted = pose.is_none();
    let (mut eye, mut target, mut fov) = pose.unwrap_or_else(|| camera_fit(&items, kind, cam.as_ref(), aspect));
    if !parts_for_sparkles.is_empty() {
        let sp = sparkles(base, &parts_for_sparkles, eye, target);
        items.extend(sp);
    }
    let mut items = scene_textures(ctx, items, eye, target, fov, aspect, &mut notes);
    let mut tris = triangles(&mut items, eye, target, fov, w, h, sl);
    if !fitted && tris.0.is_empty() && tris.1.is_empty() {
        let c = camera_fit(&items, kind, cam.as_ref(), aspect);
        eye = c.0;
        target = c.1;
        fov = c.2;
        tris = triangles(&mut items, eye, target, fov, w, h, sl);
    }
    if tris.0.is_empty() && tris.1.is_empty() {
        return Err(format!("nothing in view in this {}", kind));
    }
    let mut sky = Sky::None;
    if kind == "place" {
        sky = Sky::Gradient;
        if let Some(faces) = sky_faces(ctx, sky_inst.as_ref(), era, &mut notes) {
            sky = Sky::Pixels(sky_pixels(&faces, eye, target, fov, w, h));
        }
    }
    let mut c = raster(&tris, w, h, &sky, sl);
    let (mut ow, mut oh) = (w, h);
    if req.out_w != w || req.out_h != h {
        c = gd_resample(&c, w, h, req.out_w, req.out_h);
        ow = req.out_w;
        oh = req.out_h;
    }
    Ok((encode_gd_png(&c, ow, oh), notes))
}

fn sky_faces(ctx: &Ctx, sky: Option<&Inst>, era: i32, notes: &mut Vec<String>) -> Option<Vec<Img>> {
    let mut faces = vec![];
    let mut missing = 0;
    for k in ["Rt", "Lf", "Up", "Dn", "Bk", "Ft"] {
        let mut img: Option<Img> = None;
        let url = sky.and_then(|s| s.url(&format!("Skybox{}", k))).map(|s| s.to_string());
        if let Some(u) = url {
            if !u.trim().is_empty() {
                img = crate::items::mesh_bytes_pub(ctx, &u).and_then(|b| if Img::is_image_bytes(&b) { Img::decode_cap(&b, 512, 2048) } else { ctx.image(&u, 0) });
                if img.is_none() {
                    missing += 1;
                }
            }
        }
        if img.is_none() {
            let lk = k.to_lowercase();
            let file = if era == 2018 { format!("{}/sky2018/sky512_{}.png", ctx.data, lk) } else { format!("{}/sky/null_plainsky512_{}.jpg", ctx.data, lk) };
            img = load_img_file(&file);
        }
        match img {
            Some(i) if i.w >= 1 && i.h >= 1 => faces.push(i),
            _ => return None,
        }
    }
    if missing > 0 {
        notes.push(format!("{} skybox face{} could not be loaded, so the default sky was used for {}", missing, if missing == 1 { "" } else { "s" }, if missing == 1 { "it" } else { "them" }));
    }
    Some(faces)
}

fn scene_items(ctx: &Ctx, base: &Base, parts: &[Inst], era: i32, content: &Option<String>, notes: &mut Vec<String>) -> Vec<SItem> {
    let mut items = vec![];
    let mut meshes: Vec<(String, Option<Rc<Mesh>>)> = vec![];
    let mut tile_cache: HashMap<(i64, i64, i32), Option<Rc<Vec<Vec<i64>>>>> = HashMap::new();
    let mut missing = 0;
    for h in parts {
        let p = &h.props;
        let tr = p.get("Transparency").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let mut alpha = (1.0 - tr).clamp(0.0, 1.0);
        if !p.contains_key("CFrame") {
            continue;
        }
        let size = part_size(h);
        let cf = cf_from(p.get("CFrame"));
        if alpha < 0.05 {
            items.extend(decals(h, cf, size));
            continue;
        }
        let rf = p.get("Reflectance").and_then(|v| v.as_f64()).unwrap_or(0.0).clamp(0.0, 1.0);
        let mut scale = [1.0; 3];
        let mut off = [0.0; 3];
        let sm = h.children.iter().find(|c| matches!(c.cls.as_str(), "SpecialMesh" | "FileMesh" | "BlockMesh" | "CylinderMesh"));
        let (hx, hy, hz) = (size[0] / 2.0, size[1] / 2.0, size[2] / 2.0);
        let mut mesh_url: Option<String> = None;
        let mut tex_url: Option<String> = None;
        let mut typ: Option<i64> = None;
        if h.cls == "MeshPart" {
            mesh_url = h.url("MeshId").or(h.url("MeshID")).map(|s| s.to_string());
            tex_url = h.url("TextureID").or(h.url("TextureId")).map(|s| s.to_string());
        } else if let Some(s) = sm {
            scale = s.props.get("Scale").and_then(|v| v.as_v3()).unwrap_or([1.0; 3]);
            off = s.props.get("Offset").and_then(|v| v.as_v3()).unwrap_or([0.0; 3]);
            let t = if s.cls == "BlockMesh" {
                6
            } else if s.cls == "CylinderMesh" {
                4
            } else {
                match s.props.get("MeshType") {
                    Some(Value::Num(n)) => *n as i64,
                    Some(Value::Str(n)) => match n.to_lowercase().as_str() {
                        "head" => 0,
                        "torso" => 1,
                        "wedge" => 2,
                        "sphere" => 3,
                        "cylinder" => 4,
                        "filemesh" => 5,
                        "brick" => 6,
                        _ => n.parse::<f64>().map(|f| f as i64).unwrap_or(6),
                    },
                    _ => 5,
                }
            };
            typ = Some(t);
            if t == 5 {
                mesh_url = s.url("MeshId").map(|s| s.to_string());
                tex_url = s.url("TextureId").map(|s| s.to_string());
            }
        }
        let mesh_url = mesh_url.filter(|s| !s.is_empty());
        let tex_url = tex_url.filter(|s| !s.is_empty());
        let mut mesh: Option<Rc<Mesh>> = None;
        let mut smooth = false;
        let mut item_tex_url: Option<String> = None;
        if let Some(u) = &mesh_url {
            if !meshes.iter().any(|m| &m.0 == u) {
                meshes.push((u.clone(), None));
                if meshes.len() <= 80 {
                    let parsed = mesh_bytes_scene(ctx, content, u).and_then(|b| match parse_mesh2016(&b) {
                        Ok(m) => Some(Rc::new(clean_mesh(&m))),
                        Err(e) => {
                            if e.contains("not ported") {
                                ctx.unsupported.set(true);
                            }
                            None
                        }
                    });
                    let last = meshes.len() - 1;
                    meshes[last].1 = parsed;
                }
            }
            mesh = meshes.iter().find(|m| &m.0 == u).and_then(|m| m.1.clone());
            if mesh.is_some() {
                smooth = true;
                if h.cls == "MeshPart" {
                    scale = [size[0] / 2.0f64.max(0.001), size[1] / 2.0f64.max(0.001), size[2] / 2.0f64.max(0.001)];
                }
                item_tex_url = tex_url.clone();
            } else {
                missing += 1;
            }
        }
        let mut decal_size = size;
        if mesh.is_none() && typ == Some(0) {
            let kk = (size[0] / 2.0).min(size[1]).min(size[2]) / 1.25;
            scale = [scale[0] * kk, scale[1] * kk, scale[2] * kk];
            let hm = base.head_raw.clone();
            smooth = true;
            let (lo, hi) = mesh_bounds(&hm);
            decal_size = [size[0].max((hi[0] - lo[0]) * scale[0] * 1.02), size[1].max((hi[1] - lo[1]) * scale[1] * 1.02), size[2].max((hi[2] - lo[2]) * scale[2] * 1.02)];
            mesh = Some(Rc::new(hm));
        }
        let mesh = match mesh {
            Some(m) => m,
            None => {
                let k = if sm.is_some() { [hx * scale[0], hy * scale[1], hz * scale[2]] } else { [hx, hy, hz] };
                scale = [1.0; 3];
                let shape = p.get("shape").or(p.get("Shape")).and_then(|v| v.as_f64()).unwrap_or(1.0) as i64;
                if h.cls == "WedgePart" || typ == Some(2) {
                    Rc::new(wedge(k[0], k[1], k[2]))
                } else if h.cls == "CornerWedgePart" {
                    Rc::new(corner(k[0], k[1], k[2]))
                } else if typ == Some(3) || (typ.is_none() && shape == 0) {
                    smooth = true;
                    Rc::new(sphere_mesh(k[0], k[1], k[2]))
                } else if typ == Some(4) || (typ.is_none() && shape == 2) {
                    smooth = true;
                    Rc::new(scene_cylinder(k[0], k[1], k[2], typ == Some(4)))
                } else {
                    Rc::new(plain_box(k[0], k[1], k[2]))
                }
            }
        };
        let material = match p.get("Material") {
            Some(v) => v.as_f64().map(|f| f as i64).unwrap_or(256),
            None => 256,
        };
        let mut tiles = None;
        if item_tex_url.is_none() && mesh_url.is_none() {
            let shape = p.get("shape").or(p.get("Shape")).and_then(|v| v.as_f64()).unwrap_or(1.0) as i64;
            let is_box = sm.is_none() && h.cls != "WedgePart" && h.cls != "CornerWedgePart" && shape == 1;
            let names = ["RightSurface", "TopSurface", "BackSurface", "LeftSurface", "BottomSurface", "FrontSurface"];
            let mut out = vec![];
            let mut any = false;
            for n in names {
                let surface = if is_box { p.get(n).and_then(|v| v.as_f64()).unwrap_or(0.0) as i64 } else { 0 };
                let t = tile(&ctx.data, era, material, surface, &mut tile_cache);
                any = any || t.is_some();
                out.push(t);
            }
            if any {
                tiles = Some(out);
            }
        }
        let mut unlit = false;
        if material == 288 {
            unlit = true;
        } else if material == 1568 && alpha > 0.6 {
            alpha = 0.6;
        }
        items.push(SItem { cf, scale, off, color: part_color(h), alpha, tex: None, tex_url: item_tex_url, smooth, size: Some(size), refl: rf, mesh, tiles, unlit, decal: false });
        items.extend(decals(h, cf, decal_size));
    }
    if missing > 0 {
        notes.push(format!("{} meshes could not be loaded and were drawn as plain parts", missing));
    }
    items
}

fn scene_textures(ctx: &Ctx, items: Vec<SItem>, eye: V3, target: V3, fov: f64, aspect: f64, notes: &mut Vec<String>) -> Vec<SItem> {
    let dir = norm(sub(target, eye));
    let up_ref = if dir[1].abs() > 0.99 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
    let right = norm(cross(dir, up_ref));
    let up = cross(right, dir);
    let tan_y = (fov.to_radians() / 2.0).tan();
    let tan_x = tan_y * aspect;
    let mut near: Vec<(String, f64)> = vec![];
    for it in &items {
        let url = match &it.tex_url {
            Some(u) => u,
            None => continue,
        };
        let v = sub(it.cf.p, eye);
        let z = dot(v, dir);
        let s = it.size.unwrap_or([1.0; 3]);
        let r = s[0].max(s[1]).max(s[2]) * 0.87;
        if z < -r || dot(v, right).abs() > (z + r) * tan_x + r || dot(v, up).abs() > (z + r) * tan_y + r {
            continue;
        }
        let d = z.max(0.0);
        match near.iter_mut().find(|n| &n.0 == url) {
            Some(n) => {
                if d < n.1 {
                    n.1 = d;
                }
            }
            None => near.push((url.clone(), d)),
        }
    }
    near.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut textures: HashMap<String, Option<Rc<Img>>> = HashMap::new();
    for (u, _) in &near {
        textures.insert(u.clone(), ctx.image(u, 0).map(Rc::new));
    }
    let mut out = vec![];
    let mut missing = 0;
    for mut it in items {
        let tex = match &it.tex_url {
            Some(u) => textures.get(u).cloned().flatten(),
            None => it.tex.clone(),
        };
        it.tex = tex;
        if it.decal && it.tex.is_none() {
            if let Some(u) = &it.tex_url {
                if textures.contains_key(u) {
                    missing += 1;
                }
            }
            continue;
        }
        out.push(it);
    }
    if missing > 0 {
        notes.push(format!("{} decals in view could not be loaded", missing));
    }
    out
}

fn sparkles(base: &Base, parts: &[(CF, V3, Vec<Inst>)], eye: V3, target: V3) -> Vec<SItem> {
    let fx = match crate::scene::fx_sparkle(base) {
        Some(f) => f,
        None => return vec![],
    };
    let dir = norm(sub(target, eye));
    let up_ref = if dir[1].abs() > 0.99 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
    let right = norm(cross(dir, up_ref));
    let up = cross(right, dir);
    let mut tinted: HashMap<String, Rc<Img>> = HashMap::new();
    let mut out = vec![];
    let mut count = 0;
    for (cf, size, sparks) in parts {
        for c in sparks {
            if matches!(c.props.get("Enabled"), Some(Value::Bool(false))) || count >= 60 {
                continue;
            }
            count += 1;
            let rgb = match c.props.get("SparkleColor") {
                Some(Value::Color3(r, g, b)) => [*r, *g, *b],
                _ => [144.0 / 255.0, 25.0 / 255.0, 1.0],
            };
            let key = format!("{:.3},{:.3},{:.3}", rgb[0], rgb[1], rgb[2]);
            let img = tinted
                .entry(key)
                .or_insert_with(|| {
                    let px = fx
                        .px
                        .iter()
                        .map(|&p| {
                            let a = (((p >> 24) & 255) as f64 * ((((p >> 16) & 255).max((p >> 8) & 255).max(p & 255)) as f64 / 255.0).sqrt()) as u32;
                            (a << 24)
                                | ((((p >> 16) & 255) as f64 * rgb[0] * 1.3).min(255.0) as u32) << 16
                                | ((((p >> 8) & 255) as f64 * rgb[1] * 1.3).min(255.0) as u32) << 8
                                | ((p & 255) as f64 * rgb[2] * 1.3).min(255.0) as u32
                        })
                        .collect();
                    Rc::new(Img { w: fx.w, h: fx.h, px })
                })
                .clone();
            let ctr = cf.p;
            let mut rng = Sparse::new(crc32(format!("{:.2},{:.2},{:.2}", ctr[0], ctr[1], ctr[2]).as_bytes()));
            let reach = [size[0] / 2.0 + 0.8, size[1] / 2.0 + 1.6, size[2] / 2.0 + 0.8];
            let mut mesh = Mesh::default();
            for _ in 0..12 {
                let q = [ctr[0] + (rng.rand() * 2.0 - 1.0) * reach[0], ctr[1] + (rng.rand() * 1.6 - 0.6) * reach[1], ctr[2] + (rng.rand() * 2.0 - 1.0) * reach[2]];
                let hs = 0.45 + rng.rand() * 0.45;
                let corner = |sx: f64, sy: f64| [q[0] + right[0] * sx * hs + up[0] * sy * hs, q[1] + right[1] * sx * hs + up[1] * sy * hs, q[2] + right[2] * sx * hs + up[2] * sy * hs];
                let (tl, tr, bl, br) = (corner(-1.0, 1.0), corner(1.0, 1.0), corner(-1.0, -1.0), corner(1.0, -1.0));
                mesh.pos.extend_from_slice(&[tl, bl, br, tl, br, tr]);
                let n = [-dir[0], -dir[1], -dir[2]];
                mesh.nrm.extend_from_slice(&[n; 6]);
                mesh.uv.extend_from_slice(&[[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 1.0], [1.0, 0.0]]);
            }
            out.push(SItem { cf: CF::identity(), scale: [1.0; 3], off: [0.0; 3], color: [1.0; 3], alpha: 1.0, tex: Some(img), tex_url: None, smooth: false, size: Some([0.5; 3]), refl: 0.0, mesh: Rc::new(mesh), tiles: None, unlit: false, decal: true });
        }
    }
    out
}

pub fn unused(_: ThumbCam, _: &str) -> Option<String> {
    asset_id("")
}

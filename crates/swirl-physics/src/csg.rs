const CSG_KEY: [u8; 31] = [86, 46, 110, 88, 49, 32, 48, 4, 52, 105, 12, 119, 12, 1, 94, 0, 26, 96, 55, 105, 29, 82, 43, 7, 79, 36, 89, 101, 83, 4, 122];

pub struct UnionMesh {
    pub pos: Vec<f32>,
    pub idx: Vec<u32>,
    pub bounds: [f64; 3],
}

fn rd_i32(d: &[u8], o: usize) -> Option<i32> {
    d.get(o..o + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn rd_f32(d: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

pub fn parse_csg(raw: &[u8]) -> Option<UnionMesh> {
    let d: Vec<u8> = raw.iter().enumerate().map(|(i, b)| b ^ CSG_KEY[i % CSG_KEY.len()]).collect();
    if d.len() < 50 || &d[..6] != b"CSGMDL" {
        return None;
    }
    let version = rd_i32(&d, 6)?;
    if !(2..=5).contains(&version) {
        return None;
    }
    let mut o = 42usize;
    let nv = rd_i32(&d, o)?;
    let stride = rd_i32(&d, o + 4)?;
    o += 8;
    if nv <= 0 || nv as f64 > 1e6 || stride < 24 || o as i64 + nv as i64 * stride as i64 + 4 > d.len() as i64 {
        return None;
    }
    let (nv, stride) = (nv as usize, stride as usize);
    let mut pos = vec![0f32; nv * 3];
    for i in 0..nv {
        let b = o + i * stride;
        for k in 0..3 {
            pos[i * 3 + k] = rd_f32(&d, b + k * 4);
        }
    }
    o += nv * stride;
    let ni = rd_i32(&d, o)?;
    o += 4;
    if ni <= 0 || o as i64 + ni as i64 * 4 > d.len() as i64 {
        return None;
    }
    let mut idx = Vec::with_capacity(ni as usize);
    for i in 0..ni as usize {
        let v = rd_i32(&d, o + i * 4)?;
        idx.push(if v >= 0 && (v as usize) < nv { v as u32 } else { 0 });
    }
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for i in 0..nv {
        for k in 0..3 {
            let v = pos[i * 3 + k] as f64;
            if v < lo[k] {
                lo[k] = v;
            }
            if v > hi[k] {
                hi[k] = v;
            }
        }
    }
    let bounds = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    Some(UnionMesh { pos, idx, bounds })
}

pub fn base64_bytes(s: &str) -> Option<Vec<u8>> {
    let mut cleaned = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
            continue;
        }
        if c == '>' {
            in_tag = false;
            continue;
        }
        if !in_tag && !c.is_whitespace() {
            cleaned.push(c);
        }
    }
    let mut out = Vec::with_capacity(cleaned.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in cleaned.bytes() {
        if c == b'=' {
            break;
        }
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 255) as u8);
        }
    }
    Some(out)
}

pub fn mesh_from_inline(s: &str) -> Option<UnionMesh> {
    parse_csg(&base64_bytes(s)?)
}

pub fn mesh_data_from_asset(bytes: &[u8]) -> Option<UnionMesh> {
    let text = String::from_utf8_lossy(bytes);
    let start = text.find("<BinaryString name=\"MeshData\">")? + "<BinaryString name=\"MeshData\">".len();
    let end = text[start..].find("</BinaryString>")? + start;
    let body = text[start..end].trim();
    if body.is_empty() {
        return None;
    }
    mesh_from_inline(body)
}

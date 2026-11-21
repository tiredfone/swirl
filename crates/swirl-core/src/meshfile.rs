use crate::math::V3;

#[derive(Clone, Default)]
pub struct Mesh {
    pub pos: Vec<V3>,
    pub nrm: Vec<V3>,
    pub uv: Vec<[f64; 2]>,
}

pub const MAX_MESH_VERTS: usize = 150000;

fn php_float(s: &str) -> f64 {
    let t = s.trim();
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let mut digits = false;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits = true;
    }
    if i < b.len() && b[i] == b'.' {
        let mut j = i + 1;
        let mut fd = false;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
            fd = true;
        }
        if fd || digits {
            i = j;
            digits = true;
        }
    }
    if !digits {
        return 0.0;
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
    t[..i].parse().unwrap_or(0.0)
}

fn rd_u16(b: &[u8], o: usize) -> Result<usize, String> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize).ok_or_else(|| "truncated mesh".to_string())
}

fn rd_u32(b: &[u8], o: usize) -> Result<usize, String> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize).ok_or_else(|| "truncated mesh".to_string())
}

fn rd_f32(b: &[u8], o: usize) -> Result<f64, String> {
    b.get(o..o + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64).ok_or_else(|| "truncated mesh".to_string())
}

pub fn parse_mesh(bytes: &[u8]) -> Result<Mesh, String> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(12)]).to_string();
    let rest = head.strip_prefix("version ").ok_or("not a mesh")?;
    let major: usize = rest.split('.').next().and_then(|s| s.parse().ok()).ok_or("not a mesh")?;
    let mut m = Mesh::default();
    if major == 1 {
        let sc = if head.starts_with("version 1.00") { 0.5 } else { 1.0 };
        let text = String::from_utf8_lossy(bytes);
        let mut vs: Vec<Vec<f64>> = Vec::new();
        let mut rest = &text[..];
        while let Some(a) = rest.find('[') {
            match rest[a + 1..].find(']') {
                Some(e) if e > 0 => {
                    vs.push(rest[a + 1..a + 1 + e].split(',').map(php_float).collect());
                    rest = &rest[a + 1 + e + 1..];
                }
                Some(_) => {
                    rest = &rest[a + 1..];
                }
                None => break,
            }
        }
        let mut i = 0;
        while i + 2 < vs.len() {
            let g = |v: &Vec<f64>, k: usize| v.get(k).copied().unwrap_or(0.0);
            m.pos.push([g(&vs[i], 0) * sc, g(&vs[i], 1) * sc, g(&vs[i], 2) * sc]);
            m.nrm.push([g(&vs[i + 1], 0), g(&vs[i + 1], 1), g(&vs[i + 1], 2)]);
            m.uv.push([g(&vs[i + 2], 0), 1.0 - g(&vs[i + 2], 1)]);
            i += 3;
        }
        return Ok(m);
    }
    if major == 6 || major == 7 {
        return parse_chunked(bytes);
    }
    let at = 13usize;
    let len = bytes.len();
    let hs = rd_u16(bytes, at)?;
    let read_v = |o: usize| -> Result<(V3, V3, [f64; 2]), String> {
        let f: Result<Vec<f64>, String> = (0..8).map(|k| rd_f32(bytes, o + k * 4)).collect();
        let f = f?;
        Ok(([f[0], f[1], f[2]], [f[3], f[4], f[5]], [f[6], f[7]]))
    };
    let vs: Vec<(V3, V3, [f64; 2])>;
    let fo: usize;
    let fsz: usize;
    let mut last: usize;
    if major == 2 || major == 3 {
        let vsz = *bytes.get(at + 2).ok_or("truncated mesh")? as usize;
        fsz = *bytes.get(at + 3).ok_or("truncated mesh")? as usize;
        let (nl, nv, nf);
        if major == 3 {
            nl = rd_u16(bytes, at + 6)?;
            nv = rd_u32(bytes, at + 8)?;
            nf = rd_u32(bytes, at + 12)?;
        } else {
            nl = 0;
            nv = rd_u32(bytes, at + 4)?;
            nf = rd_u32(bytes, at + 8)?;
        }
        let vo = at + hs;
        fo = vo + nv * vsz;
        if fo + nf * fsz > len || vsz < 32 {
            return Err("truncated mesh".into());
        }
        vs = (0..nv).map(|i| read_v(vo + i * vsz)).collect::<Result<_, _>>()?;
        last = nf;
        if major == 3 && nl > 1 {
            last = rd_u32(bytes, fo + nf * fsz + 4)?;
        }
    } else if major == 4 || major == 5 {
        let nv = rd_u32(bytes, at + 4)?;
        let nf = rd_u32(bytes, at + 8)?;
        let nlod = rd_u16(bytes, at + 12)?;
        let nbones = rd_u16(bytes, at + 14)?;
        let vo = at + hs;
        fsz = 12;
        fo = vo + nv * 40 + if nbones > 0 { nv * 8 } else { 0 };
        if fo + nf * 12 > len {
            return Err("truncated mesh".into());
        }
        vs = (0..nv).map(|i| read_v(vo + i * 40)).collect::<Result<_, _>>()?;
        last = nf;
        if nlod > 1 {
            let l1 = rd_u32(bytes, fo + nf * 12 + 4)?;
            if (l1 as f64) >= nf as f64 * 0.25 && l1 <= nf {
                last = l1;
            }
        }
    } else {
        return Err(format!("mesh version {} is not supported here", major));
    }
    if last * 3 > MAX_MESH_VERTS {
        return Err("a mesh is too large".into());
    }
    for i in 0..last {
        for k in 0..3 {
            let idx = rd_u32(bytes, fo + i * fsz + k * 4)?;
            let v = vs.get(idx).ok_or("bad mesh index")?;
            m.pos.push(v.0);
            m.nrm.push(v.1);
            m.uv.push([v.2[0], 1.0 - v.2[1]]);
        }
    }
    Ok(m)
}


fn parse_chunked(bytes: &[u8]) -> Result<Mesh, String> {
    let mut o = 13usize;
    let n = bytes.len();
    let mut core: Option<(usize, usize, usize)> = None;
    let mut lods: Vec<usize> = vec![];
    while o + 16 <= n {
        let name: Vec<u8> = bytes[o..o + 8].iter().copied().take_while(|&b| b != 0).collect();
        let ver = rd_u32(bytes, o + 8)?;
        let size = rd_u32(bytes, o + 12)?;
        o += 16;
        if o + size > n {
            return Err("truncated mesh".into());
        }
        if name == b"COREMESH" {
            core = Some((ver, o, size));
        } else if name == b"LODS" && size >= 7 {
            let cnt = rd_u32(bytes, o + 3)?;
            if cnt < 64 && 7 + cnt * 4 <= size {
                for i in 0..cnt {
                    lods.push(rd_u32(bytes, o + 7 + i * 4)?);
                }
            }
        }
        o += size;
    }
    let (ver, o, size) = core.ok_or("mesh has no geometry")?;
    if size < 8 {
        return Err("truncated mesh".into());
    }
    if ver != 1 {
        return Err("Draco meshes are not ported yet".into());
    }
    let nv = rd_u32(bytes, o)?;
    let fo = o + 4 + nv * 40 + 4;
    if fo > o + size {
        return Err("truncated mesh".into());
    }
    let nf = rd_u32(bytes, fo - 4)?;
    if fo + nf * 12 > o + size {
        return Err("truncated mesh".into());
    }
    let mut pv = vec![];
    let mut nv_ = vec![];
    let mut uv_ = vec![];
    for i in 0..nv {
        let b = o + 4 + i * 40;
        let f: Result<Vec<f64>, String> = (0..8).map(|k| rd_f32(bytes, b + k * 4)).collect();
        let f = f?;
        pv.push([f[0], f[1], f[2]]);
        nv_.push([f[3], f[4], f[5]]);
        uv_.push([f[6], f[7]]);
    }
    let mut faces = vec![];
    for i in 0..nf {
        faces.push([rd_u32(bytes, fo + i * 12)?, rd_u32(bytes, fo + i * 12 + 4)?, rd_u32(bytes, fo + i * 12 + 8)?]);
    }
    let mut from = 0;
    let mut to = faces.len();
    if lods.len() >= 2 && lods[0] < lods[1] && lods[1] <= to {
        from = lods[0];
        to = lods[1];
    }
    if (to - from) * 3 > MAX_MESH_VERTS {
        return Err("a mesh is too large".into());
    }
    let mut m = Mesh::default();
    for t in from..to {
        for &i in &faces[t] {
            let p = pv.get(i).ok_or("bad mesh index")?;
            m.pos.push(*p);
            m.nrm.push(nv_[i]);
            m.uv.push([uv_[i][0], 1.0 - uv_[i][1]]);
        }
    }
    Ok(m)
}

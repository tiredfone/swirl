use crate::meshfile::php_float;

pub struct Conv {
    pub verts: Vec<[f64; 9]>,
    pub faces: Vec<[usize; 3]>,
}

const MAX_FACES: usize = 400000;

type R<T> = Result<T, String>;

fn u32_at(b: &[u8], o: usize) -> R<usize> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize).ok_or_else(|| "truncated mesh".to_string())
}

fn u16_at(b: &[u8], o: usize) -> R<usize> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize).ok_or_else(|| "truncated mesh".to_string())
}

fn f32_at(b: &[u8], o: usize) -> R<f64> {
    b.get(o..o + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64).ok_or_else(|| "truncated mesh".to_string())
}

pub fn detect_version(d: &[u8]) -> i32 {
    let s = match std::str::from_utf8(&d[..d.len().min(32)]) {
        Ok(s) => s.to_string(),
        Err(e) => String::from_utf8_lossy(&d[..e.valid_up_to()]).to_string(),
    };
    let rest = match s.strip_prefix("version ") {
        Some(r) => r,
        None => return -1,
    };
    let (maj, after) = match rest.find(|c: char| !c.is_ascii_digit()) {
        Some(i) if i > 0 => (&rest[..i], &rest[i..]),
        _ => return -1,
    };
    let after = match after.strip_prefix('.') {
        Some(a) => a,
        None => return -1,
    };
    let mn: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    if mn.is_empty() {
        return -1;
    }
    let major: i32 = maj.parse().unwrap_or(0);
    if major == 1 {
        return if mn == "00" { 0 } else { 1 };
    }
    if (2..=7).contains(&major) {
        major
    } else {
        -1
    }
}

fn lod_range(lods: &[usize], nf: usize) -> (usize, usize) {
    if lods.len() >= 2 && lods[0] < lods[1] && lods[1] <= nf {
        (lods[0], lods[1])
    } else {
        (0, nf)
    }
}

fn check_faces(faces: &[[usize; 3]], nv: usize) -> R<()> {
    for f in faces {
        if f[0] >= nv || f[1] >= nv || f[2] >= nv {
            return Err("bad mesh index".into());
        }
    }
    Ok(())
}

fn read_vertices(d: &[u8], at: usize, count: usize, size: usize) -> R<Vec<[f64; 9]>> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = at + i * size;
        let mut v = [0.0; 9];
        let n = if size >= 32 { 8 } else { 6 };
        for k in 0..n {
            v[k] = f32_at(d, o + k * 4)?;
        }
        out.push(v);
    }
    Ok(out)
}

fn read_faces(d: &[u8], at: usize, from: usize, to: usize, size: usize) -> R<Vec<[usize; 3]>> {
    let mut out = Vec::with_capacity(to - from);
    for i in from..to {
        let o = at + i * size;
        out.push(if size >= 12 { [u32_at(d, o)?, u32_at(d, o + 4)?, u32_at(d, o + 8)?] } else { [u16_at(d, o)?, u16_at(d, o + 2)?, u16_at(d, o + 4)?] });
    }
    Ok(out)
}

fn parse_ascii(d: &[u8], half: bool) -> Option<Conv> {
    let sc = if half { 0.5 } else { 1.0 };
    let text = String::from_utf8_lossy(d);
    let mut vecs: Vec<Vec<f64>> = vec![];
    let mut rest = &text[..];
    while let Some(a) = rest.find('[') {
        match rest[a + 1..].find(']') {
            Some(e) => {
                vecs.push(rest[a + 1..a + 1 + e].split(',').map(php_float).collect());
                rest = &rest[a + 1 + e + 1..];
            }
            None => break,
        }
    }
    if vecs.is_empty() {
        return None;
    }
    let n = vecs.len() / 3;
    let g = |v: &Vec<f64>, k: usize| v.get(k).copied().unwrap_or(0.0);
    let mut verts = vec![];
    for i in 0..n {
        let (p, q, t) = (&vecs[i * 3], &vecs[i * 3 + 1], &vecs[i * 3 + 2]);
        verts.push([g(p, 0) * sc, g(p, 1) * sc, g(p, 2) * sc, g(q, 0), g(q, 1), g(q, 2), g(t, 0), 1.0 - g(t, 1), 0.0]);
    }
    let mut faces = vec![];
    let mut i = 0;
    while i + 2 < n {
        faces.push([i, i + 1, i + 2]);
        i += 3;
    }
    Some(Conv { verts, faces })
}

fn parse_binary(d: &[u8], major: i32) -> R<Option<Conv>> {
    let nl = match d.iter().position(|&b| b == b'\n') {
        Some(n) => n,
        None => return Ok(None),
    };
    let at = nl + 1;
    let len = d.len();
    if len < at + 12 {
        return Ok(None);
    }
    let hs = u16_at(d, at)?;
    let mut lods = vec![];
    let (nv, nf, vsz, fsz, vo, fo);
    if major == 2 || major == 3 {
        vsz = d[at + 2] as usize;
        fsz = d[at + 3] as usize;
        let (lsz, nlod);
        if major == 3 {
            lsz = u16_at(d, at + 4)?;
            nlod = u16_at(d, at + 6)?;
            nv = u32_at(d, at + 8)?;
            nf = u32_at(d, at + 12)?;
        } else {
            lsz = 4;
            nlod = 0;
            nv = u32_at(d, at + 4)?;
            nf = u32_at(d, at + 8)?;
        }
        if vsz < 24 || fsz < 6 {
            return Ok(None);
        }
        vo = at + hs;
        fo = vo + nv * vsz;
        let lo = fo + nf * fsz;
        if lo > len {
            return Err("truncated mesh".into());
        }
        if nlod > 0 && lsz >= 4 && lo + nlod * lsz <= len {
            for i in 0..nlod {
                lods.push(u32_at(d, lo + i * lsz)?);
            }
        }
    } else {
        nv = u32_at(d, at + 4)?;
        nf = u32_at(d, at + 8)?;
        let nlod = u16_at(d, at + 12)?;
        let nb = u16_at(d, at + 14)?;
        vsz = 40;
        fsz = 12;
        vo = at + hs;
        fo = vo + nv * vsz + if nb > 0 { nv * 8 } else { 0 };
        let lo = fo + nf * fsz;
        if lo > len {
            return Err("truncated mesh".into());
        }
        if nlod > 0 && lo + nlod * 4 <= len {
            for i in 0..nlod {
                lods.push(u32_at(d, lo + i * 4)?);
            }
        }
    }
    if nf > MAX_FACES * 8 || nv > MAX_FACES * 8 {
        return Err("mesh is too large".into());
    }
    let (from, to) = lod_range(&lods, nf);
    let faces = read_faces(d, fo, from, to, fsz)?;
    check_faces(&faces, nv)?;
    Ok(Some(Conv { verts: read_vertices(d, vo, nv, vsz)?, faces }))
}

fn parse_chunked(d: &[u8]) -> R<Option<Conv>> {
    let nl = match d.iter().position(|&b| b == b'\n') {
        Some(n) => n,
        None => return Ok(None),
    };
    let mut o = nl + 1;
    let n = d.len();
    let mut core: Option<(usize, usize, usize)> = None;
    let mut lods: Vec<usize> = vec![];
    while o + 16 <= n {
        let name: Vec<u8> = d[o..o + 8].iter().copied().take_while(|&b| b != 0).collect();
        let ver = u32_at(d, o + 8)?;
        let size = u32_at(d, o + 12)?;
        o += 16;
        if o + size > n {
            return Err("truncated mesh".into());
        }
        if name == b"COREMESH" {
            core = Some((ver, o, size));
        } else if name == b"LODS" && size >= 7 {
            let cnt = u32_at(d, o + 3)?;
            if cnt < 64 && o + 7 + cnt * 4 <= o + size {
                for i in 0..cnt {
                    lods.push(u32_at(d, o + 7 + i * 4)?);
                }
            }
        }
        o += size;
    }
    let (ver, o, size) = core.ok_or("mesh has no geometry")?;
    if ver == 1 {
        let nv = u32_at(d, o)?;
        let vo = o + 4;
        let fo = vo + nv * 40 + 4;
        if fo > o + size {
            return Err("truncated mesh".into());
        }
        let nf = u32_at(d, fo - 4)?;
        if fo + nf * 12 > o + size {
            return Err("truncated mesh".into());
        }
        let (from, to) = lod_range(&lods, nf);
        let faces = read_faces(d, fo, from, to, 12)?;
        check_faces(&faces, nv)?;
        return Ok(Some(Conv { verts: read_vertices(d, vo, nv, 40)?, faces }));
    }
    let g = crate::draco::geometry(d, o, size)?;
    let mut verts = vec![];
    for (i, p) in g.pos.iter().enumerate() {
        let q = g.nrm.as_ref().and_then(|v| v.get(i)).copied().unwrap_or([0.0, 1.0, 0.0]);
        let t = g.uv.as_ref().and_then(|v| v.get(i)).copied().unwrap_or([0.0, 0.0]);
        verts.push([p[0], p[1], p[2], q[0], q[1], q[2], t[0], t[1], 0.0]);
    }
    let (from, to) = lod_range(&lods, g.faces.len());
    let faces: Vec<[usize; 3]> = g.faces[from..to].to_vec();
    check_faces(&faces, verts.len())?;
    Ok(Some(Conv { verts, faces }))
}

pub fn parse(d: &[u8]) -> R<Option<Conv>> {
    let src = detect_version(d);
    let m = match src {
        -1 => None,
        0 | 1 => parse_ascii(d, src == 0),
        2..=5 => parse_binary(d, src)?,
        _ => parse_chunked(d)?,
    };
    if let Some(c) = &m {
        if c.faces.len() > MAX_FACES {
            return Err("mesh is too large".into());
        }
    }
    Ok(m)
}

fn num(x: f64) -> String {
    if !x.is_finite() {
        return "0".into();
    }
    let s = format!("{:.6}", x);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.to_string()
    }
}

pub fn serialize_v100(c: &Conv) -> Vec<u8> {
    let mut out = String::from("version 1.00\n");
    out.push_str(&c.faces.len().to_string());
    out.push('\n');
    for f in &c.faces {
        for &vi in f {
            let v = &c.verts[vi];
            out.push_str(&format!("[{},{},{}][{},{},{}][{},{},0]", num(v[0] * 2.0), num(v[1] * 2.0), num(v[2] * 2.0), num(v[3]), num(v[4]), num(v[5]), num(v[6]), num(1.0 - v[7])));
        }
    }
    out.into_bytes()
}

pub fn convert_v100(d: &[u8]) -> Vec<u8> {
    let src = detect_version(d);
    if src == -1 || src == 0 {
        return d.to_vec();
    }
    match parse(d) {
        Ok(Some(c)) if !c.faces.is_empty() => serialize_v100(&c),
        _ => d.to_vec(),
    }
}

pub fn to_json(c: &Conv) -> String {
    let mut s = String::from("{\"vertices\":[");
    for (i, v) in c.verts.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push('[');
        for k in 0..9 {
            if k > 0 {
                s.push(',');
            }
            let x = if v[k].is_finite() { v[k] } else { 0.0 };
            s.push_str(&format!("{:?}", x));
        }
        s.push(']');
    }
    s.push_str("],\"faces\":[");
    for (i, f) in c.faces.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("[{},{},{}]", f[0], f[1], f[2]));
    }
    s.push_str("]}");
    s
}

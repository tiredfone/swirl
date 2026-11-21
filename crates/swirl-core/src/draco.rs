pub struct Attr {
    pub ty: u8,
    pub nc: usize,
    pub v: Vec<Vec<f64>>,
}

pub struct Decoded {
    pub faces: Vec<[usize; 3]>,
    pub attrs: Vec<Attr>,
}

pub struct Geometry {
    pub pos: Vec<[f64; 3]>,
    pub nrm: Option<Vec<[f64; 3]>>,
    pub uv: Option<Vec<[f64; 2]>>,
    pub faces: Vec<[usize; 3]>,
}

struct D<'a> {
    b: &'a [u8],
    p: usize,
    end: usize,
}

struct Rans {
    buf: Vec<u8>,
    state: i64,
    off: usize,
    base: i64,
    prec: i64,
    lut: Vec<usize>,
    prob: Vec<i64>,
    cum: Vec<i64>,
}

impl Rans {
    fn next(&mut self) -> usize {
        while self.state < self.base && self.off > 0 {
            self.off -= 1;
            self.state = self.state * 256 + self.buf[self.off] as i64;
        }
        let rem = self.state % self.prec;
        let quo = self.state / self.prec;
        let s = self.lut[rem as usize];
        self.state = quo * self.prob[s] + rem - self.cum[s];
        s
    }
}

type R<T> = Result<T, String>;

fn err<T>(m: &str) -> R<T> {
    Err(format!("draco: {}", m))
}

impl<'a> D<'a> {
    fn u8(&mut self) -> R<u8> {
        if self.p >= self.end {
            return err("ran out of data");
        }
        let v = self.b[self.p];
        self.p += 1;
        Ok(v)
    }

    fn i8(&mut self) -> R<i64> {
        let v = self.u8()? as i64;
        Ok(if v > 127 { v - 256 } else { v })
    }

    fn u16(&mut self) -> R<i64> {
        let a = self.u8()? as i64;
        let b = self.u8()? as i64;
        Ok(a | (b << 8))
    }

    fn u32(&mut self) -> R<i64> {
        let a = self.u16()?;
        let b = self.u16()?;
        Ok(a | (b << 16))
    }

    fn i32(&mut self) -> R<i64> {
        let v = self.u32()?;
        Ok(if v >= 0x80000000 { v - 0x100000000 } else { v })
    }

    fn f32(&mut self) -> R<f64> {
        if self.p + 4 > self.end {
            return err("ran out of data");
        }
        let s = &self.b[self.p..self.p + 4];
        let v = f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64;
        self.p += 4;
        Ok(v)
    }

    fn varint(&mut self) -> R<i64> {
        let mut v: i64 = 0;
        let mut s = 0;
        loop {
            let c = self.u8()? as i64;
            v |= (c & 127) << s;
            if c & 128 == 0 {
                return Ok(v);
            }
            s += 7;
            if s > 56 {
                return err("bad varint");
            }
        }
    }

    fn rans(&mut self, bits: i64) -> R<Rans> {
        let ns = self.varint()?;
        if ns < 1 || ns > 1 << 20 {
            return err("bad symbol table");
        }
        let ns = ns as usize;
        let mut prob = vec![0i64; ns];
        let mut i = 0;
        while i < ns {
            let pd = self.u8()? as i64;
            let token = pd & 3;
            if token == 3 {
                let off = (pd >> 2) as usize;
                if i + off >= ns {
                    return err("bad symbol table");
                }
                i += off;
            } else {
                let mut pr = pd >> 2;
                for k in 0..token {
                    pr |= (self.u8()? as i64) << (8 * (k + 1) - 2);
                }
                prob[i] = pr;
            }
            i += 1;
        }
        let pb = (3 * bits / 2).clamp(12, 20);
        let prec = 1i64 << pb;
        let mut lut: Vec<usize> = vec![0; prec as usize];
        let mut cum = vec![0i64; ns];
        let mut acc = 0i64;
        for i in 0..ns {
            cum[i] = acc;
            acc += prob[i];
            if acc > prec {
                return err("bad symbol table");
            }
            for j in cum[i]..acc {
                lut[j as usize] = i;
            }
        }
        if acc != prec {
            return err("bad symbol table");
        }
        let nbytes = self.varint()?;
        if nbytes < 1 || self.p + nbytes as usize > self.end {
            return err("bad rans size");
        }
        let nbytes = nbytes as usize;
        let buf = self.b[self.p..self.p + nbytes].to_vec();
        self.p += nbytes;
        let base = prec * 4;
        let x = (buf[nbytes - 1] >> 6) as usize;
        if nbytes < 1 + x {
            return err("bad rans data");
        }
        let off = nbytes - 1 - x;
        let mut state: i64 = 0;
        for k in (0..=x).rev() {
            state = (state << 8) | buf[off + k] as i64;
        }
        state &= (1i64 << (6 + 8 * x)) - 1;
        state += base;
        Ok(Rans { buf, state, off, base, prec, lut, prob, cum })
    }

    fn integers(&mut self, n: usize, nc: usize) -> R<Vec<i64>> {
        let compressed = self.u8()?;
        if compressed == 0 {
            let nb = self.u8()? as usize;
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let mut v: i64 = 0;
                for k in 0..nb {
                    v |= (self.u8()? as i64) << (8 * k);
                }
                out.push(v);
            }
            return Ok(out);
        }
        let scheme = self.u8()?;
        if scheme == 0 {
            let mut tags = self.rans(5)?;
            let bit_start = self.p;
            let mut bit = 0usize;
            let mut out = Vec::with_capacity(n);
            let mut i = 0;
            while i < n {
                let len = tags.next();
                for _ in 0..nc {
                    let mut val: i64 = 0;
                    for k in 0..len {
                        let o = bit;
                        bit += 1;
                        let byte = bit_start + (o >> 3);
                        if byte < self.end {
                            val |= (((self.b[byte] >> (o & 7)) & 1) as i64) << k;
                        }
                    }
                    out.push(val);
                }
                i += nc;
            }
            self.p = bit_start + ((bit + 7) >> 3);
            return Ok(out);
        }
        if scheme == 1 {
            let max_bits = self.u8()? as i64;
            if !(1..=18).contains(&max_bits) {
                return err("bad symbol width");
            }
            let mut r = self.rans(max_bits)?;
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                out.push(r.next() as i64);
            }
            return Ok(out);
        }
        err("unknown symbol coding")
    }

    fn portable(&mut self, ty_dt: u8, dec: u8, nc: usize, np: usize) -> R<Vec<f64>> {
        let n = np * nc;
        if dec == 0 {
            let sz = match ty_dt {
                1 | 2 | 11 => 1,
                3 | 4 => 2,
                5 | 6 | 9 => 4,
                7 | 8 | 10 => 8,
                _ => 0,
            };
            if sz == 0 || self.p + n * sz > self.end {
                return err("bad generic attribute");
            }
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let s = &self.b[self.p + i * sz..self.p + (i + 1) * sz];
                let v = match ty_dt {
                    1 => s[0] as i8 as f64,
                    2 => s[0] as f64,
                    3 => i16::from_le_bytes([s[0], s[1]]) as f64,
                    4 => u16::from_le_bytes([s[0], s[1]]) as f64,
                    5 => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64,
                    6 => u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64,
                    9 => f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64,
                    10 => f64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]),
                    _ => return err("unsupported attribute type"),
                };
                out.push(v);
            }
            self.p += n * sz;
            return Ok(out);
        }
        if dec != 1 && dec != 2 {
            return err("unsupported attribute encoding");
        }
        let method = self.i8()?;
        let mut wrap = false;
        if method != -2 {
            let tr = self.i8()?;
            if !(-1..=6).contains(&method) {
                return err("bad prediction method");
            }
            if tr == 1 {
                if method != 0 {
                    return err("mesh prediction is not supported");
                }
                wrap = true;
            }
        }
        let mut v = self.integers(n, nc)?;
        for s in v.iter_mut() {
            *s = if *s & 1 != 0 { -(*s >> 1) - 1 } else { *s >> 1 };
        }
        if wrap {
            let min = self.i32()?;
            let max = self.i32()?;
            if min > max {
                return err("bad wrap range");
            }
            let dif = 1 + max - min;
            for i in 0..n {
                let pr = if i < nc { 0 } else { v[i - nc] };
                let pr = if pr > max { max } else if pr < min { min } else { pr };
                let mut x = pr + v[i];
                if x > max {
                    x -= dif;
                } else if x < min {
                    x += dif;
                }
                v[i] = x;
            }
        }
        Ok(v.into_iter().map(|x| x as f64).collect())
    }
}

pub fn decode(bytes: &[u8], off: usize, len: usize) -> R<Decoded> {
    let end = (off + len).min(bytes.len());
    let mut d = D { b: bytes, p: off, end };
    if off + 5 > bytes.len() || &bytes[off..off + 5] != b"DRACO" {
        return err("bad header");
    }
    d.p += 5;
    let major = d.u8()?;
    let minor = d.u8()?;
    let ty = d.u8()?;
    let method = d.u8()?;
    let flags = d.u16()?;
    if major != 2 || minor < 2 {
        return Err(format!("draco: version {}.{} is not supported", major, minor));
    }
    if ty != 1 {
        return err("not a triangle mesh");
    }
    if method != 0 {
        return err("only sequential meshes are supported");
    }
    if flags & 0x8000 != 0 {
        return err("metadata is not supported");
    }
    let nf = d.varint()?;
    let np = d.varint()?;
    if nf > 2000000 || np > 2000000 {
        return err("a mesh is too large");
    }
    let (nf, np) = (nf as usize, np as usize);
    if d.u8()? != 1 {
        return err("only raw indices are supported");
    }
    let mut faces = Vec::with_capacity(nf);
    for _ in 0..nf {
        let mut f = [0usize; 3];
        for k in 0..3 {
            f[k] = if np < 256 {
                d.u8()? as usize
            } else if np < 65536 {
                d.u16()? as usize
            } else if np < (1 << 21) {
                d.varint()? as usize
            } else {
                d.u32()? as usize
            };
        }
        faces.push(f);
    }
    if d.u8()? != 1 {
        return err("unexpected attribute decoders");
    }
    let na = d.varint()?;
    if !(1..=32).contains(&na) {
        return err("bad attribute count");
    }
    struct A {
        ty: u8,
        dt: u8,
        nc: usize,
        dec: u8,
    }
    let mut a: Vec<A> = vec![];
    for _ in 0..na {
        let ty = d.u8()?;
        let dt = d.u8()?;
        let nc = d.u8()? as usize;
        let _norm = d.u8()?;
        let _id = d.varint()?;
        a.push(A { ty, dt, nc, dec: 0 });
    }
    for x in a.iter_mut() {
        x.dec = d.u8()?;
    }
    let mut q: Vec<Vec<f64>> = vec![];
    for x in a.iter() {
        q.push(d.portable(x.dt, x.dec, x.nc, np)?);
    }
    let mut attrs = vec![];
    for (x, qv) in a.iter().zip(q.into_iter()) {
        let rows: Vec<Vec<f64>> = if x.dec == 2 {
            let mut min = vec![];
            for _ in 0..x.nc {
                min.push(d.f32()?);
            }
            let range = d.f32()?;
            let bits = d.u8()? as u32;
            let mx = ((1u64 << bits) - 1) as f64;
            let delta = if mx > 0.0 { range / mx } else { 0.0 };
            (0..np).map(|i| (0..x.nc).map(|c| qv[i * x.nc + c] * delta + min[c]).collect()).collect()
        } else {
            (0..np).map(|i| qv[i * x.nc..(i + 1) * x.nc].to_vec()).collect()
        };
        attrs.push(Attr { ty: x.ty, nc: x.nc, v: rows });
    }
    Ok(Decoded { faces, attrs })
}

pub fn geometry(bytes: &[u8], o: usize, size: usize) -> R<Geometry> {
    if o + 4 > bytes.len() {
        return err("ran out of data");
    }
    let dl = u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]) as usize;
    let m = decode(bytes, o + 4, dl.min(size.saturating_sub(4)))?;
    let mut p: Option<Vec<[f64; 3]>> = None;
    let mut n: Option<Vec<[f64; 3]>> = None;
    let mut u: Option<Vec<[f64; 2]>> = None;
    for a in &m.attrs {
        if a.ty == 0 && a.nc == 3 && p.is_none() {
            p = Some(a.v.iter().map(|r| [r[0], r[1], r[2]]).collect());
        } else if a.ty == 3 && a.nc == 2 && u.is_none() {
            u = Some(a.v.iter().map(|r| [r[0], r[1]]).collect());
        } else if (a.ty == 1 || a.ty == 4) && a.nc == 3 && n.is_none() && !a.v.is_empty() {
            let s = &a.v[0];
            if (s[0] * s[0] + s[1] * s[1] + s[2] * s[2] - 1.0).abs() < 0.1 {
                n = Some(a.v.iter().map(|r| [r[0], r[1], r[2]]).collect());
            }
        }
    }
    let pos = p.ok_or("mesh has no positions")?;
    Ok(Geometry { pos, nrm: n, uv: u, faces: m.faces })
}

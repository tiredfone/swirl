use indexmap::{IndexMap, IndexSet};

pub const VOXEL: i32 = 4;
pub const CHUNK: i32 = 16;
const N: usize = 4096;
pub const MAX_CHUNKS: usize = 6000;
pub const CELL_WATER: u8 = 17;
pub const SMOOTH_WATER: u8 = 1;
pub const SMOOTH_COUNT: u8 = 23;
pub const CELL_COUNT: u8 = 18;

pub type Key = (i32, i32, i32);

pub struct Chunk {
    pub mat: Vec<u8>,
    pub occ: Vec<u8>,
    pub block: Vec<u8>,
    pub orient: Vec<u8>,
    pub count: i32,
}

pub struct TerrainData {
    pub era: i32,
    pub chunks: IndexMap<Key, Chunk>,
    pub dirty: IndexSet<Key>,
    pub material_colors: IndexMap<String, [f64; 3]>,
}

fn div(a: i32) -> i32 {
    a.div_euclid(CHUNK)
}

fn modc(a: i32) -> usize {
    a.rem_euclid(CHUNK) as usize
}

fn idx(x: usize, y: usize, z: usize) -> usize {
    (y * CHUNK as usize + z) * CHUNK as usize + x
}

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(b: &[u8]) -> String {
    let mut out = String::with_capacity(b.len() * 4 / 3 + 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn b64_decode(s: &str) -> Vec<u8> {
    let mut clean = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag && !ch.is_whitespace() {
            clean.push(ch);
        }
    }
    let mut out = vec![];
    let mut buf = 0u32;
    let mut bits = 0;
    for ch in clean.bytes() {
        if ch == b'=' {
            break;
        }
        let v = match B64.iter().position(|c| *c == ch) {
            Some(v) => v as u32,
            None => continue,
        };
        buf = buf << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    out
}

struct Writer(Vec<u8>);

impl Writer {
    fn varint(&mut self, v: i32) {
        let mut z: u32 = if v < 0 { (((-(v as i64)) as u32) << 1).wrapping_sub(1) } else { (v as u32) << 1 };
        loop {
            let mut b = (z & 127) as u8;
            z >>= 7;
            if z != 0 {
                b |= 128;
            }
            self.0.push(b);
            if z == 0 {
                break;
            }
        }
    }

    fn rle(&mut self, arr: &[u8]) {
        let mut i = 0;
        while i < arr.len() {
            let v = arr[i];
            let mut run = 1;
            while i + run < arr.len() && arr[i + run] == v && run < 65535 {
                run += 1;
            }
            self.varint(run as i32);
            self.0.push(v);
            i += run;
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Result<u8, String> {
        let v = *self.b.get(self.i).ok_or("terrain data ends early")?;
        self.i += 1;
        Ok(v)
    }

    fn varint(&mut self) -> Result<i32, String> {
        let mut z: u32 = 0;
        let mut shift = 0;
        loop {
            let b = self.byte()?;
            z |= ((b & 127) as u32).checked_shl(shift).unwrap_or(0);
            shift += 7;
            if !(b & 128 != 0 && shift < 35) {
                break;
            }
        }
        Ok(if z & 1 != 0 { -((((z as u64) + 1) >> 1) as i64) as i32 } else { (z >> 1) as i32 })
    }

    fn rle(&mut self, arr: &mut [u8]) -> Result<(), String> {
        let mut i = 0;
        while i < arr.len() {
            let run = self.varint()?;
            let v = self.byte()?;
            if run <= 0 || i + run as usize > arr.len() {
                return Err("bad terrain run".into());
            }
            for x in &mut arr[i..i + run as usize] {
                *x = v;
            }
            i += run as usize;
        }
        Ok(())
    }
}

impl TerrainData {
    pub fn new(era: i32) -> TerrainData {
        TerrainData { era: if era == 2018 { 2018 } else { 2012 }, chunks: IndexMap::new(), dirty: IndexSet::new(), material_colors: IndexMap::new() }
    }

    fn new_chunk(&self) -> Chunk {
        let cells = self.era != 2018;
        Chunk { mat: vec![0; N], occ: vec![0; N], block: if cells { vec![0; N] } else { vec![] }, orient: if cells { vec![0; N] } else { vec![] }, count: 0 }
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.chunks.get(&(div(x), div(y), div(z))) {
            Some(c) => c.mat[idx(modc(x), modc(y), modc(z))],
            None => 0,
        }
    }

    pub fn occ(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.chunks.get(&(div(x), div(y), div(z))) {
            Some(c) => {
                let i = idx(modc(x), modc(y), modc(z));
                if c.mat[i] != 0 {
                    c.occ[i]
                } else {
                    0
                }
            }
            None => 0,
        }
    }

    pub fn cell(&self, x: i32, y: i32, z: i32) -> [u8; 4] {
        match self.chunks.get(&(div(x), div(y), div(z))) {
            Some(c) => {
                let i = idx(modc(x), modc(y), modc(z));
                [c.mat[i], c.occ[i], c.block.get(i).copied().unwrap_or(0), c.orient.get(i).copied().unwrap_or(0)]
            }
            None => [0; 4],
        }
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, mat: i32, occ: i32, block: i32, orient: i32) -> bool {
        let key = (div(x), div(y), div(z));
        if !self.chunks.contains_key(&key) {
            if mat <= 0 || self.chunks.len() >= MAX_CHUNKS {
                return false;
            }
            let c = self.new_chunk();
            self.chunks.insert(key, c);
        }
        let cells = self.era != 2018;
        let i = idx(modc(x), modc(y), modc(z));
        let m = if mat > 0 && occ > 0 { mat as u8 } else { 0 };
        let o = if m != 0 { occ.clamp(1, 255) as u8 } else { 0 };
        let b = if m != 0 && cells { (block & 7) as u8 } else { 0 };
        let r = if m != 0 && cells { (orient & 3) as u8 } else { 0 };
        let c = self.chunks.get_mut(&key).unwrap();
        let cb = if cells { c.block[i] } else { 0 };
        let cr = if cells { c.orient[i] } else { 0 };
        if c.mat[i] == m && c.occ[i] == o && cb == b && cr == r {
            return false;
        }
        if c.mat[i] == 0 && m != 0 {
            c.count += 1;
        } else if c.mat[i] != 0 && m == 0 {
            c.count -= 1;
        }
        c.mat[i] = m;
        c.occ[i] = o;
        if cells {
            c.block[i] = b;
            c.orient[i] = r;
        }
        let empty = c.count <= 0;
        self.touch(x, y, z);
        if empty {
            self.chunks.shift_remove(&key);
        }
        true
    }

    pub fn touch(&mut self, x: i32, y: i32, z: i32) {
        let (cx, cy, cz) = (div(x), div(y), div(z));
        let r = |l: usize| -> (i32, i32) { (if l == 0 { -1 } else { 0 }, if l == CHUNK as usize - 1 { 1 } else { 0 }) };
        let (ax, bx) = r(modc(x));
        let (ay, by) = r(modc(y));
        let (az, bz) = r(modc(z));
        for a in ax..=bx {
            for b in ay..=by {
                for d in az..=bz {
                    self.dirty.insert((cx + a, cy + b, cz + d));
                }
            }
        }
    }

    pub fn take_dirty(&mut self) -> Vec<Key> {
        let d: Vec<Key> = self.dirty.iter().copied().collect();
        self.dirty.clear();
        d
    }

    pub fn all_keys(&self) -> Vec<Key> {
        let mut out: IndexSet<Key> = IndexSet::new();
        for k in self.chunks.keys() {
            for a in -1..=1 {
                for b in -1..=1 {
                    for d in -1..=1 {
                        out.insert((k.0 + a, k.1 + b, k.2 + d));
                    }
                }
            }
        }
        out.into_iter().collect()
    }

    pub fn clear(&mut self) {
        let keys: Vec<Key> = self.chunks.keys().copied().collect();
        for k in &keys {
            self.dirty.insert(*k);
        }
        for k in &keys {
            for a in -1..=1 {
                for b in -1..=1 {
                    for d in -1..=1 {
                        self.dirty.insert((k.0 + a, k.1 + b, k.2 + d));
                    }
                }
            }
        }
        self.chunks.clear();
    }

    pub fn count(&self) -> i64 {
        self.chunks.values().map(|c| c.count as i64).sum()
    }

    pub fn encode(&self, keys: Option<&[Key]>) -> String {
        let mut w = Writer(vec![67, if self.era == 2018 { 2 } else { 1 }]);
        let list: Vec<(Key, Option<&Chunk>)> = match keys {
            Some(ks) => ks.iter().map(|k| (*k, self.chunks.get(k))).collect(),
            None => self.chunks.iter().map(|(k, c)| (*k, Some(c))).collect(),
        };
        w.varint(list.len() as i32);
        for (k, c) in list {
            w.varint(k.0);
            w.varint(k.1);
            w.varint(k.2);
            match c {
                None => w.0.push(0),
                Some(c) => {
                    w.0.push(1);
                    w.rle(&c.mat);
                    w.rle(&c.occ);
                    if self.era != 2018 {
                        w.rle(&c.block);
                        w.rle(&c.orient);
                    }
                }
            }
        }
        b64_encode(&w.0)
    }

    pub fn decode(&mut self, text: &str) -> Result<Vec<Key>, String> {
        let bytes = b64_decode(text);
        let mut r = Reader { b: &bytes, i: 0 };
        if r.byte()? != 67 {
            return Err("not chocolate terrain data".into());
        }
        let era = if r.byte()? == 2 { 2018 } else { 2012 };
        if era != self.era && self.chunks.is_empty() {
            self.era = era;
        }
        let n = r.varint()?;
        let mut changed: Vec<Key> = vec![];
        for _ in 0..n {
            let (cx, cy, cz) = (r.varint()?, r.varint()?, r.varint()?);
            let k = (cx, cy, cz);
            let has = r.byte()?;
            if has == 0 {
                if self.chunks.shift_remove(&k).is_some() {
                    changed.push(k);
                }
                continue;
            }
            if !self.chunks.contains_key(&k) && self.chunks.len() >= MAX_CHUNKS {
                return Err("too much terrain".into());
            }
            let mut c = self.new_chunk();
            r.rle(&mut c.mat)?;
            r.rle(&mut c.occ)?;
            if era != 2018 {
                let mut bl = vec![0u8; N];
                let mut or = vec![0u8; N];
                r.rle(&mut bl)?;
                r.rle(&mut or)?;
                if self.era != 2018 {
                    c.block = bl;
                    c.orient = or;
                }
            }
            let maxm = if self.era == 2018 { SMOOTH_COUNT - 1 } else { CELL_COUNT - 1 };
            for j in 0..N {
                if c.mat[j] > maxm {
                    c.mat[j] = 0;
                }
                if c.mat[j] != 0 {
                    c.count += 1;
                }
            }
            if c.count > 0 {
                self.chunks.insert(k, c);
            } else {
                self.chunks.shift_remove(&k);
            }
            changed.push(k);
        }
        for k in &changed {
            for a in -1..=1 {
                for b in -1..=1 {
                    for d in -1..=1 {
                        self.dirty.insert((k.0 + a, k.1 + b, k.2 + d));
                    }
                }
            }
        }
        Ok(changed)
    }

    pub fn fill_box(&mut self, cf: &[f64], size: [f64; 3], mat: i32, subtract: bool) -> Result<i64, String> {
        let (hx, hy, hz) = (size[0] / 2.0, size[1] / 2.0, size[2] / 2.0);
        let ext = [cf[3].abs() * hx + cf[4].abs() * hy + cf[5].abs() * hz, cf[6].abs() * hx + cf[7].abs() * hy + cf[8].abs() * hz, cf[9].abs() * hx + cf[10].abs() * hy + cf[11].abs() * hz];
        let v = VOXEL as f64;
        let lo: Vec<i32> = (0..3).map(|k| ((cf[k] - ext[k]) / v).floor() as i32 - 1).collect();
        let hi: Vec<i32> = (0..3).map(|k| ((cf[k] + ext[k]) / v).floor() as i32 + 1).collect();
        if ((hi[0] - lo[0]) as i64) * ((hi[1] - lo[1]) as i64) * ((hi[2] - lo[2]) as i64) > 4_000_000 {
            return Err("Region is too big".into());
        }
        let mut n = 0;
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    let occ = self.box_cover(cf, hx, hy, hz, x, y, z);
                    if occ <= 0.0 {
                        continue;
                    }
                    if self.write_voxel(x, y, z, mat, occ, false, subtract, false, false) {
                        n += 1;
                    }
                }
            }
        }
        Ok(n)
    }

    fn box_cover(&self, cf: &[f64], hx: f64, hy: f64, hz: f64, x: i32, y: i32, z: i32) -> f64 {
        let v = VOXEL as f64;
        let mut inside = 0;
        for s in 0..8 {
            let px = (x as f64 + 0.25 + 0.5 * (s & 1) as f64) * v;
            let py = (y as f64 + 0.25 + 0.5 * ((s >> 1) & 1) as f64) * v;
            let pz = (z as f64 + 0.25 + 0.5 * ((s >> 2) & 1) as f64) * v;
            let (dx, dy, dz) = (px - cf[0], py - cf[1], pz - cf[2]);
            let lx = cf[3] * dx + cf[6] * dy + cf[9] * dz;
            let ly = cf[4] * dx + cf[7] * dy + cf[10] * dz;
            let lz = cf[5] * dx + cf[8] * dy + cf[11] * dz;
            if lx.abs() <= hx && ly.abs() <= hy && lz.abs() <= hz {
                inside += 1;
            }
        }
        inside as f64 / 8.0
    }

    pub fn fill_ball(&mut self, center: [f64; 3], radius: f64, mat: i32, subtract: bool) -> Result<i64, String> {
        let r = radius.max(0.0);
        let v = VOXEL as f64;
        let lo: Vec<i32> = center.iter().map(|c| ((c - r) / v).floor() as i32 - 1).collect();
        let hi: Vec<i32> = center.iter().map(|c| ((c + r) / v).floor() as i32 + 1).collect();
        if ((hi[0] - lo[0]) as i64) * ((hi[1] - lo[1]) as i64) * ((hi[2] - lo[2]) as i64) > 4_000_000 {
            return Err("Region is too big".into());
        }
        let mut n = 0;
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    let d = (((x as f64 + 0.5) * v - center[0]).powi(2) + ((y as f64 + 0.5) * v - center[1]).powi(2) + ((z as f64 + 0.5) * v - center[2]).powi(2)).sqrt();
                    let occ = ((r - d) / v + 0.5).clamp(0.0, 1.0);
                    if occ <= 0.0 {
                        continue;
                    }
                    if self.write_voxel(x, y, z, mat, occ, false, subtract, false, false) {
                        n += 1;
                    }
                }
            }
        }
        Ok(n)
    }

    pub fn write_voxel(&mut self, x: i32, y: i32, z: i32, mat: i32, occ: f64, max: bool, subtract: bool, paint: bool, only: bool) -> bool {
        let [m0, o0, _, _] = self.cell(x, y, z);
        let o0f = o0 as f64;
        if subtract {
            if m0 == 0 {
                return false;
            }
            let left = (o0f / 255.0 - occ).max(0.0);
            return self.set(x, y, z, if left > 0.02 { m0 as i32 } else { 0 }, js_round(left * 255.0) as i32, 0, 0);
        }
        if paint {
            if m0 == 0 {
                return false;
            }
            return self.set(x, y, z, mat, o0 as i32, 0, 0);
        }
        if only && m0 != 0 {
            return false;
        }
        let target = if max { (o0f / 255.0 * if m0 != 0 { 1.0 } else { 0.0 }).max(occ) } else { occ };
        let keep = if m0 != 0 && m0 as i32 == mat { o0f / 255.0 } else { 0.0 };
        self.set(x, y, z, mat, js_round(target.max(keep) * 255.0) as i32, 0, 0)
    }

    pub fn autowedge(&mut self, x: i32, y: i32, z: i32) -> bool {
        let [m, _, _, _] = self.cell(x, y, z);
        if m == 0 || m == CELL_WATER || self.get(x, y + 1, z) != 0 || self.get(x, y - 1, z) == 0 {
            return false;
        }
        let open: Vec<bool> = [(0, -1), (1, 0), (0, 1), (-1, 0)]
            .iter()
            .map(|(a, c)| {
                let n = self.get(x + a, y, z + c);
                n == 0 || n == CELL_WATER
            })
            .collect();
        let n_open = open.iter().filter(|b| **b).count();
        if n_open == 1 {
            let i = open.iter().position(|b| *b).unwrap();
            return self.set(x, y, z, m as i32, 255, 1, [2, 1, 0, 3][(i + 2) % 4]);
        }
        if n_open == 2 {
            let i = open.iter().position(|b| *b).unwrap();
            let j = open.iter().rposition(|b| *b).unwrap();
            if j - i == 2 {
                return false;
            }
            return self.set(x, y, z, m as i32, 255, 2, [3, 2, 1, 0][if j - i == 1 { i } else { 3 }]);
        }
        false
    }

    pub fn water_surface(&self, x: f64, y: f64, z: f64) -> Option<f64> {
        if self.chunks.is_empty() {
            return None;
        }
        let water = if self.era == 2018 { SMOOTH_WATER } else { CELL_WATER };
        let v = VOXEL as f64;
        let cx = (x / v).floor() as i32;
        let cz = (z / v).floor() as i32;
        let mut cy = (y / v).floor() as i32;
        if self.get(cx, cy, cz) != water {
            return None;
        }
        let mut i = 0;
        while i < 64 && self.get(cx, cy + 1, cz) == water {
            cy += 1;
            i += 1;
        }
        let top = cy as f64 * v + if self.era == 2018 { v * self.occ(cx, cy, cz) as f64 / 255.0 } else { v };
        if y <= top {
            Some(top)
        } else {
            None
        }
    }

    pub fn solid_mesh(&self, key: Key) -> Vec<f32> {
        let pos = if self.era == 2018 { self.solid_smooth(key) } else { self.solid_cells(key) };
        pos.into_iter().map(|v| v as f32).collect()
    }

    pub fn mesh_cells(&self, key: Key) -> Vec<(u8, Vec<f32>, Vec<f32>, Vec<f32>)> {
        let (cx, cy, cz) = key;
        let c = match self.chunks.get(&key) {
            Some(c) => c,
            None => return vec![],
        };
        let mut groups: IndexMap<u8, (Vec<f32>, Vec<f32>, Vec<f32>)> = IndexMap::new();
        let h = VOXEL as f64;
        let faces: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
        let push = |g: &mut (Vec<f32>, Vec<f32>, Vec<f32>), pts: &[[f64; 3]], n: [f64; 3]| {
            let mut i = 1;
            while i + 1 < pts.len() {
                for p in [pts[0], pts[i], pts[i + 1]] {
                    g.0.extend_from_slice(&[p[0] as f32, p[1] as f32, p[2] as f32]);
                    g.1.extend_from_slice(&[n[0] as f32, n[1] as f32, n[2] as f32]);
                    let ax = if n[0].abs() > 0.6 { 0 } else if n[1].abs() > 0.6 { 1 } else { 2 };
                    let u = if ax == 0 { p[2] } else { p[0] };
                    let v = if ax == 1 { p[2] } else { p[1] };
                    g.2.extend_from_slice(&[u as f32, v as f32]);
                }
                i += 1;
            }
        };
        for ly in 0..CHUNK as usize {
            for lz in 0..CHUNK as usize {
                for lx in 0..CHUNK as usize {
                    let i = idx(lx, ly, lz);
                    let m = c.mat[i];
                    if m == 0 {
                        continue;
                    }
                    let x = cx * CHUNK + lx as i32;
                    let y = cy * CHUNK + ly as i32;
                    let z = cz * CHUNK + lz as i32;
                    let (ox, oy, oz) = ((x * VOXEL) as f64, (y * VOXEL) as f64, (z * VOXEL) as f64);
                    let water = m == CELL_WATER;
                    let g = groups.entry(m).or_default();
                    if c.block[i] != 0 && !water {
                        for f in cell_shape(c.block[i] as usize, c.orient[i] as usize) {
                            let pts: Vec<[f64; 3]> = f.iter().map(|p| [ox + p[0] * h, oy + p[1] * h, oz + p[2] * h]).collect();
                            if pts.len() >= 3 {
                                let (a, b, d) = (pts[0], pts[1], pts[2]);
                                let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [d[0] - a[0], d[1] - a[1], d[2] - a[2]]);
                                let mut n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
                                n = [n[0] / l, n[1] / l, n[2] / l];
                                push(g, &pts, n);
                            }
                        }
                        continue;
                    }
                    for [dx, dy, dz] in faces {
                        let [nm, _, nb, _] = self.cell(x + dx, y + dy, z + dz);
                        if nm != 0 && nb == 0 && (nm != CELL_WATER || water) {
                            continue;
                        }
                        let pts: Vec<[f64; 3]> = if dx != 0 {
                            let fx = if dx > 0 { ox + h } else { ox };
                            if dx > 0 {
                                vec![[fx, oy, oz], [fx, oy + h, oz], [fx, oy + h, oz + h], [fx, oy, oz + h]]
                            } else {
                                vec![[fx, oy, oz], [fx, oy, oz + h], [fx, oy + h, oz + h], [fx, oy + h, oz]]
                            }
                        } else if dy != 0 {
                            let fy = if dy > 0 { oy + h } else { oy };
                            if dy > 0 {
                                vec![[ox, fy, oz], [ox, fy, oz + h], [ox + h, fy, oz + h], [ox + h, fy, oz]]
                            } else {
                                vec![[ox, fy, oz], [ox + h, fy, oz], [ox + h, fy, oz + h], [ox, fy, oz + h]]
                            }
                        } else {
                            let fz = if dz > 0 { oz + h } else { oz };
                            if dz > 0 {
                                vec![[ox, oy, fz], [ox + h, oy, fz], [ox + h, oy + h, fz], [ox, oy + h, fz]]
                            } else {
                                vec![[ox, oy, fz], [ox, oy + h, fz], [ox + h, oy + h, fz], [ox + h, oy, fz]]
                            }
                        };
                        push(g, &pts, [dx as f64, dy as f64, dz as f64]);
                    }
                }
            }
        }
        groups.into_iter().map(|(m, g)| (m, g.0, g.1, g.2)).collect()
    }

    pub fn mesh_smooth(&self, key: Key) -> Vec<(u8, Vec<f32>, Vec<f32>, Vec<f32>)> {
        let (x0, y0, z0) = (key.0 * CHUNK, key.1 * CHUNK, key.2 * CHUNK);
        let v = VOXEL as f64;
        let mut groups: IndexMap<u8, (Vec<f32>, Vec<f32>, Vec<f32>)> = IndexMap::new();
        for want_water in [false, true] {
            let dens = |x: i32, y: i32, z: i32| -> f64 {
                let m = self.get(x, y, z);
                if m == 0 {
                    return 0.0;
                }
                if (m == SMOOTH_WATER) != want_water {
                    return 0.0;
                }
                self.occ(x, y, z) as f64 / 255.0
            };
            let mut verts: std::collections::HashMap<Key, Option<([f64; 3], [f64; 3])>> = std::collections::HashMap::new();
            let mut vert = |i: i32, j: i32, k: i32| -> Option<([f64; 3], [f64; 3])> {
                if let Some(r) = verts.get(&(i, j, k)) {
                    return *r;
                }
                let (mut sx, mut sy, mut sz, mut n) = (0.0, 0.0, 0.0, 0.0);
                let mut d = [0.0f64; 8];
                for c in 0..8 {
                    d[c] = dens(i + (c & 1) as i32, j + ((c >> 1) & 1) as i32, k + ((c >> 2) & 1) as i32);
                }
                let edges: [[usize; 2]; 12] = [[0, 1], [2, 3], [4, 5], [6, 7], [0, 2], [1, 3], [4, 6], [5, 7], [0, 4], [1, 5], [2, 6], [3, 7]];
                for [a, b] in edges {
                    let (da, db) = (d[a] - 0.5, d[b] - 0.5);
                    if (da >= 0.0) == (db >= 0.0) {
                        continue;
                    }
                    let f = da / (da - db);
                    let pa = [(a & 1) as f64, ((a >> 1) & 1) as f64, ((a >> 2) & 1) as f64];
                    let pb = [(b & 1) as f64, ((b >> 1) & 1) as f64, ((b >> 2) & 1) as f64];
                    sx += pa[0] + (pb[0] - pa[0]) * f;
                    sy += pa[1] + (pb[1] - pa[1]) * f;
                    sz += pa[2] + (pb[2] - pa[2]) * f;
                    n += 1.0;
                }
                let r = if n == 0.0 {
                    None
                } else {
                    let gx = (d[1] + d[3] + d[5] + d[7]) - (d[0] + d[2] + d[4] + d[6]);
                    let gy = (d[2] + d[3] + d[6] + d[7]) - (d[0] + d[1] + d[4] + d[5]);
                    let gz = (d[4] + d[5] + d[6] + d[7]) - (d[0] + d[1] + d[2] + d[3]);
                    let gl = (gx * gx + gy * gy + gz * gz).sqrt();
                    let gl = if gl == 0.0 { 1.0 } else { gl };
                    Some(([(i as f64 + 0.5 + sx / n) * v, (j as f64 + 0.5 + sy / n) * v, (k as f64 + 0.5 + sz / n) * v], [-gx / gl, -gy / gl, -gz / gl]))
                };
                verts.insert((i, j, k), r);
                r
            };
            for y in y0..y0 + CHUNK {
                for z in z0..z0 + CHUNK {
                    for x in x0..x0 + CHUNK {
                        let d0 = dens(x, y, z);
                        for ax in 0..3 {
                            let (nx, ny, nz) = (x + (ax == 0) as i32, y + (ax == 1) as i32, z + (ax == 2) as i32);
                            let d1 = dens(nx, ny, nz);
                            if (d0 >= 0.5) == (d1 >= 0.5) {
                                continue;
                            }
                            if want_water && d0 < 0.5 && self.get(x, y, z) != 0 && self.get(x, y, z) != SMOOTH_WATER {
                                continue;
                            }
                            if want_water && d1 < 0.5 && self.get(nx, ny, nz) != 0 && self.get(nx, ny, nz) != SMOOTH_WATER {
                                continue;
                            }
                            let inside = if d0 >= 0.5 { (x, y, z) } else { (nx, ny, nz) };
                            let m = self.get(inside.0, inside.1, inside.2);
                            let cells: [[i32; 3]; 4] = if ax == 0 {
                                [[x, y - 1, z - 1], [x, y, z - 1], [x, y, z], [x, y - 1, z]]
                            } else if ax == 1 {
                                [[x - 1, y, z - 1], [x - 1, y, z], [x, y, z], [x, y, z - 1]]
                            } else {
                                [[x - 1, y - 1, z], [x, y - 1, z], [x, y, z], [x - 1, y, z]]
                            };
                            let mut vs: Vec<([f64; 3], [f64; 3])> = vec![];
                            let mut ok = true;
                            for c in cells {
                                match vert(c[0], c[1], c[2]) {
                                    Some(p) => vs.push(p),
                                    None => {
                                        ok = false;
                                        break;
                                    }
                                }
                            }
                            if !ok {
                                continue;
                            }
                            if d0 < 0.5 {
                                vs.reverse();
                            }
                            let g = groups.entry(m).or_default();
                            for tri in [[0usize, 1, 2], [0, 2, 3]] {
                                for q in tri {
                                    let (p, nn) = vs[q];
                                    g.0.extend_from_slice(&[p[0] as f32, p[1] as f32, p[2] as f32]);
                                    g.1.extend_from_slice(&[nn[0] as f32, nn[1] as f32, nn[2] as f32]);
                                    g.2.extend_from_slice(&[0.0, 0.0]);
                                }
                            }
                        }
                    }
                }
            }
        }
        groups.into_iter().map(|(m, g)| (m, g.0, g.1, g.2)).collect()
    }

    fn solid_cells(&self, key: Key) -> Vec<f64> {
        let (cx, cy, cz) = key;
        let c = match self.chunks.get(&key) {
            Some(c) => c,
            None => return vec![],
        };
        let mut groups: IndexMap<u8, Vec<f64>> = IndexMap::new();
        let h = VOXEL as f64;
        let faces: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
        for ly in 0..CHUNK as usize {
            for lz in 0..CHUNK as usize {
                for lx in 0..CHUNK as usize {
                    let i = idx(lx, ly, lz);
                    let m = c.mat[i];
                    if m == 0 {
                        continue;
                    }
                    let x = cx * CHUNK + lx as i32;
                    let y = cy * CHUNK + ly as i32;
                    let z = cz * CHUNK + lz as i32;
                    let (ox, oy, oz) = ((x * VOXEL) as f64, (y * VOXEL) as f64, (z * VOXEL) as f64);
                    let g = groups.entry(m).or_default();
                    let water = m == CELL_WATER;
                    if c.block[i] != 0 && !water {
                        for f in cell_shape(c.block[i] as usize, c.orient[i] as usize) {
                            let pts: Vec<[f64; 3]> = f.iter().map(|p| [ox + p[0] * h, oy + p[1] * h, oz + p[2] * h]).collect();
                            push_face(g, &pts);
                        }
                        continue;
                    }
                    for [dx, dy, dz] in faces {
                        let [nm, _, nb, _] = self.cell(x + dx, y + dy, z + dz);
                        if nm != 0 && nb == 0 && (nm != CELL_WATER || water) {
                            continue;
                        }
                        let pts: Vec<[f64; 3]> = if dx != 0 {
                            let fx = if dx > 0 { ox + h } else { ox };
                            if dx > 0 {
                                vec![[fx, oy, oz], [fx, oy + h, oz], [fx, oy + h, oz + h], [fx, oy, oz + h]]
                            } else {
                                vec![[fx, oy, oz], [fx, oy, oz + h], [fx, oy + h, oz + h], [fx, oy + h, oz]]
                            }
                        } else if dy != 0 {
                            let fy = if dy > 0 { oy + h } else { oy };
                            if dy > 0 {
                                vec![[ox, fy, oz], [ox, fy, oz + h], [ox + h, fy, oz + h], [ox + h, fy, oz]]
                            } else {
                                vec![[ox, fy, oz], [ox + h, fy, oz], [ox + h, fy, oz + h], [ox, fy, oz + h]]
                            }
                        } else {
                            let fz = if dz > 0 { oz + h } else { oz };
                            if dz > 0 {
                                vec![[ox, oy, fz], [ox + h, oy, fz], [ox + h, oy + h, fz], [ox, oy + h, fz]]
                            } else {
                                vec![[ox, oy, fz], [ox, oy + h, fz], [ox + h, oy + h, fz], [ox + h, oy, fz]]
                            }
                        };
                        push_face(g, &pts);
                    }
                }
            }
        }
        let mut solid = vec![];
        for (m, g) in groups {
            if m != CELL_WATER {
                solid.extend(g);
            }
        }
        solid
    }

    fn solid_smooth(&self, key: Key) -> Vec<f64> {
        let (x0, y0, z0) = (key.0 * CHUNK, key.1 * CHUNK, key.2 * CHUNK);
        let dens = |x: i32, y: i32, z: i32| -> f64 {
            let m = self.get(x, y, z);
            if m == 0 || m == SMOOTH_WATER {
                return 0.0;
            }
            self.occ(x, y, z) as f64 / 255.0
        };
        let v = VOXEL as f64;
        let mut verts: std::collections::HashMap<Key, Option<[f64; 3]>> = std::collections::HashMap::new();
        let mut vert = |i: i32, j: i32, k: i32| -> Option<[f64; 3]> {
            if let Some(r) = verts.get(&(i, j, k)) {
                return *r;
            }
            let (mut sx, mut sy, mut sz, mut n) = (0.0, 0.0, 0.0, 0.0);
            let mut d = [0.0f64; 8];
            for c in 0..8 {
                d[c] = dens(i + (c & 1) as i32, j + ((c >> 1) & 1) as i32, k + ((c >> 2) & 1) as i32);
            }
            let edges: [[usize; 2]; 12] = [[0, 1], [2, 3], [4, 5], [6, 7], [0, 2], [1, 3], [4, 6], [5, 7], [0, 4], [1, 5], [2, 6], [3, 7]];
            for [a, b] in edges {
                let (da, db) = (d[a] - 0.5, d[b] - 0.5);
                if (da >= 0.0) == (db >= 0.0) {
                    continue;
                }
                let f = da / (da - db);
                let pa = [(a & 1) as f64, ((a >> 1) & 1) as f64, ((a >> 2) & 1) as f64];
                let pb = [(b & 1) as f64, ((b >> 1) & 1) as f64, ((b >> 2) & 1) as f64];
                sx += pa[0] + (pb[0] - pa[0]) * f;
                sy += pa[1] + (pb[1] - pa[1]) * f;
                sz += pa[2] + (pb[2] - pa[2]) * f;
                n += 1.0;
            }
            let r = if n == 0.0 { None } else { Some([(i as f64 + 0.5 + sx / n) * v, (j as f64 + 0.5 + sy / n) * v, (k as f64 + 0.5 + sz / n) * v]) };
            verts.insert((i, j, k), r);
            r
        };
        let mut solid = vec![];
        for y in y0..y0 + CHUNK {
            for z in z0..z0 + CHUNK {
                for x in x0..x0 + CHUNK {
                    let d0 = dens(x, y, z);
                    for ax in 0..3 {
                        let (nx, ny, nz) = (x + (ax == 0) as i32, y + (ax == 1) as i32, z + (ax == 2) as i32);
                        let d1 = dens(nx, ny, nz);
                        if (d0 >= 0.5) == (d1 >= 0.5) {
                            continue;
                        }
                        let cells: [[i32; 3]; 4] = if ax == 0 {
                            [[x, y - 1, z - 1], [x, y, z - 1], [x, y, z], [x, y - 1, z]]
                        } else if ax == 1 {
                            [[x - 1, y, z - 1], [x - 1, y, z], [x, y, z], [x, y, z - 1]]
                        } else {
                            [[x - 1, y - 1, z], [x, y - 1, z], [x, y, z], [x - 1, y, z]]
                        };
                        let mut vs: Vec<[f64; 3]> = vec![];
                        let mut ok = true;
                        for c in cells {
                            match vert(c[0], c[1], c[2]) {
                                Some(p) => vs.push(p),
                                None => {
                                    ok = false;
                                    break;
                                }
                            }
                        }
                        if !ok {
                            continue;
                        }
                        if d0 < 0.5 {
                            vs.reverse();
                        }
                        for tri in [[0usize, 1, 2], [0, 2, 3]] {
                            for q in tri {
                                solid.extend_from_slice(&vs[q]);
                            }
                        }
                    }
                }
            }
        }
        solid
    }
}

pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn push_face(out: &mut Vec<f64>, pts: &[[f64; 3]]) {
    let mut i = 1;
    while i + 1 < pts.len() {
        for p in [pts[0], pts[i], pts[i + 1]] {
            out.extend_from_slice(&p);
        }
        i += 1;
    }
}

fn wedge(block: usize) -> Option<Vec<[f64; 3]>> {
    Some(match block {
        1 => vec![[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.], [0., 1., 1.], [1., 1., 1.]],
        2 => vec![[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.], [1., 1., 1.]],
        3 => vec![[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.], [0., 1., 0.], [1., 1., 1.], [0., 1., 1.]],
        4 => vec![[0., 0., 0.], [1., 0., 0.], [0., 0., 1.], [0., 1., 0.], [1., 1., 0.], [0., 1., 1.]],
        _ => return None,
    })
}

fn cross(u: [f64; 3], v: [f64; 3]) -> [f64; 3] {
    [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
}

fn fixed4(x: f64) -> String {
    let y = if x == 0.0 { 0.0 } else { x };
    format!("{:.4}", y)
}

fn hull_faces(pts: &[[f64; 3]]) -> Vec<Vec<[f64; 3]>> {
    struct F {
        key: String,
        n: [f64; 3],
        pts: Vec<[f64; 3]>,
    }
    let mut faces: Vec<F> = vec![];
    let n = pts.len();
    let c: Vec<f64> = (0..3).map(|k| pts.iter().map(|p| p[k]).sum::<f64>() / n as f64).collect();
    let _ = c;
    for i in 0..n {
        for j in i + 1..n {
            for k in j + 1..n {
                let (a, b, d) = (pts[i], pts[j], pts[k]);
                let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let v = [d[0] - a[0], d[1] - a[1], d[2] - a[2]];
                let mut nn = cross(u, v);
                let len = (nn[0] * nn[0] + nn[1] * nn[1] + nn[2] * nn[2]).sqrt();
                if len < 1e-9 {
                    continue;
                }
                nn = [nn[0] / len, nn[1] / len, nn[2] / len];
                let mut side = 0.0f64;
                let mut ok = true;
                for m in 0..n {
                    if !ok {
                        break;
                    }
                    let t = (pts[m][0] - a[0]) * nn[0] + (pts[m][1] - a[1]) * nn[1] + (pts[m][2] - a[2]) * nn[2];
                    if t.abs() < 1e-9 {
                        continue;
                    }
                    if side == 0.0 {
                        side = t.signum();
                    } else if t.signum() != side {
                        ok = false;
                    }
                }
                if !ok {
                    continue;
                }
                if side > 0.0 {
                    nn = [-nn[0], -nn[1], -nn[2]];
                }
                let key = format!("{},{},{}", fixed4(nn[0]), fixed4(nn[1]), fixed4(nn[2]));
                if faces.iter().any(|f| f.key == key) {
                    continue;
                }
                let mut on: Vec<[f64; 3]> = pts.iter().copied().filter(|p| ((p[0] - a[0]) * nn[0] + (p[1] - a[1]) * nn[1] + (p[2] - a[2]) * nn[2]).abs() < 1e-9).collect();
                let fc: Vec<f64> = (0..3).map(|q| on.iter().map(|p| p[q]).sum::<f64>() / on.len() as f64).collect();
                let ax = if nn[0].abs() > 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
                let t1 = cross(nn, ax);
                let t2 = cross(nn, t1);
                let ang = |p: &[f64; 3]| -> f64 {
                    let d = [p[0] - fc[0], p[1] - fc[1], p[2] - fc[2]];
                    (d[0] * t2[0] + d[1] * t2[1] + d[2] * t2[2]).atan2(d[0] * t1[0] + d[1] * t1[1] + d[2] * t1[2])
                };
                on.sort_by(|p, q| (ang(p) - ang(q)).partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal));
                faces.push(F { key, n: nn, pts: on });
            }
        }
    }
    let mut out = vec![];
    for mut f in faces {
        if f.pts.len() >= 3 {
            let (a, b, d) = (f.pts[0], f.pts[1], f.pts[2]);
            let cr = cross([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [d[0] - a[0], d[1] - a[1], d[2] - a[2]]);
            if cr[0] * f.n[0] + cr[1] * f.n[1] + cr[2] * f.n[2] < 0.0 {
                f.pts.reverse();
            }
        }
        out.push(f.pts);
    }
    out
}

fn cell_shape(block: usize, orient: usize) -> Vec<Vec<[f64; 3]>> {
    let base = match wedge(block) {
        Some(b) => b,
        None => return vec![],
    };
    let rot = |p: &[f64; 3]| -> [f64; 3] {
        let (mut x, mut z) = (p[0] - 0.5, p[2] - 0.5);
        for _ in 0..orient {
            let t = x;
            x = z;
            z = -t;
        }
        [x + 0.5, p[1], z + 0.5]
    };
    let rotated: Vec<[f64; 3]> = base.iter().map(rot).collect();
    hull_faces(&rotated)
}

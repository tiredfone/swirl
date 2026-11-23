use swirl_core::classes;
use swirl_core::instance::{DataModel, InstId};
use swirl_lua::host::{Res, Wire};
use swirl_physics::terrain::{self, TerrainData, CELL_WATER, VOXEL};
use swirl_physics::Physics;

pub const METHODS: &[&str] = &[
    "SetCell", "GetCell", "SetCells", "GetWaterCell", "SetWaterCell", "AutowedgeCell", "AutowedgeCells", "CellCenterToWorld", "CellCornerToWorld", "WorldToCell",
    "WorldToCellPreferSolid", "WorldToCellPreferEmpty", "CountCells", "Clear", "ConvertToSmooth", "FillBlock", "FillBall", "FillCylinder", "FillWedge", "FillRegion",
    "ReplaceMaterial", "__ReadVoxels", "__WriteVoxels", "__MaterialIndex", "__MaterialEnum", "GetMaterialColor", "SetMaterialColor", "CopyRegion", "PasteRegion",
];

const SMOOTH: [&str; 23] = [
    "Air", "Water", "Grass", "Slate", "Concrete", "Brick", "Sand", "WoodPlanks", "Rock", "Glacier", "Snow", "Sandstone", "Mud", "Basalt", "Ground", "CrackedLava", "Asphalt", "Cobblestone", "Ice", "LeafyGrass", "Salt", "Limestone", "Pavement",
];

fn n(a: &[Wire], i: usize) -> f64 {
    match a.get(i) {
        Some(Wire::N(v)) if v.is_finite() => *v,
        Some(Wire::E(_, v)) => *v,
        _ => 0.0,
    }
}

fn fl(a: &[Wire], i: usize) -> i32 {
    n(a, i).floor() as i32
}

fn v3(a: &[Wire], i: usize) -> Option<[f64; 3]> {
    match a.get(i) {
        Some(Wire::V3(v)) => Some(*v),
        _ => None,
    }
}

fn smooth_material(w: Option<&Wire>) -> i32 {
    let name: String = match w {
        Some(Wire::E(t, v)) => classes::enum_name(t, *v as i64).unwrap_or("").to_string(),
        Some(Wire::S(s)) => s.clone(),
        Some(Wire::N(v)) => classes::enum_name("Material", *v as i64).unwrap_or("").to_string(),
        _ => String::new(),
    };
    if name == "Air" {
        return 0;
    }
    SMOOTH.iter().position(|m| *m == name).filter(|i| *i > 0).map(|i| i as i32).unwrap_or(2)
}

fn cell_mat(w: Option<&Wire>) -> i32 {
    match w {
        Some(Wire::E(_, v)) | Some(Wire::N(v)) => (*v as i32).clamp(0, 17),
        Some(Wire::S(s)) => classes::enum_value("CellMaterial", s).map(|v| (v as i32).clamp(0, 17)).unwrap_or(0),
        _ => 0,
    }
}

fn region(a: &[Wire], i: usize, res_i: usize) -> Result<([i32; 3], [i32; 3]), String> {
    let r: Vec<f64> = match a.get(i) {
        Some(Wire::List(l)) if l.len() >= 6 => l.iter().map(|w| if let Wire::N(v) = w { *v } else { 0.0 }).collect(),
        _ => return Err("Region3 expected".into()),
    };
    let res = if a.get(res_i).is_some() { n(a, res_i) } else { VOXEL as f64 };
    let res = if res == 0.0 { VOXEL as f64 } else { res };
    if (res - VOXEL as f64).abs() > 1e-6 {
        return Err("Resolution must be 4".into());
    }
    let v = VOXEL as f64;
    let rnd = |x: f64| (x + 0.5).floor() as i32;
    let lo = [rnd(r[0].min(r[3]) / v), rnd(r[1].min(r[4]) / v), rnd(r[2].min(r[5]) / v)];
    let hi = [rnd(r[0].max(r[3]) / v), rnd(r[1].max(r[4]) / v), rnd(r[2].max(r[5]) / v)];
    if ((hi[0] - lo[0]) as i64) * ((hi[1] - lo[1]) as i64) * ((hi[2] - lo[2]) as i64) > 4 * 1024 * 1024 {
        return Err("Region is too large".into());
    }
    Ok((lo, hi))
}

fn cf12(a: &[Wire], i: usize) -> Option<[f64; 12]> {
    match a.get(i) {
        Some(Wire::Cf(c)) => Some(*c),
        _ => None,
    }
}

fn ens(t: &mut Option<TerrainData>, era: i32) -> &mut TerrainData {
    t.get_or_insert_with(|| TerrainData::new(era))
}

pub fn call(ph: &mut Physics, d: &mut DataModel, id: InstId, method: &str, a: &[Wire]) -> Option<Res> {
    if !METHODS.contains(&method) || d.get(id).map(|i| i.class != "Terrain").unwrap_or(true) {
        return None;
    }
    let era = d.era;
    if ph.terrain.is_none() {
        ph.terrain = Some(TerrainData::new(era));
        ph.terrain_inst = Some(id);
    }
    let t = ens(&mut ph.terrain, era);
    let ok = |v: Vec<Wire>| Some(Res::Ok(v));
    let en = |ty: &str, v: f64| Wire::E(ty.into(), v);
    let v = VOXEL as f64;
    match method {
        "SetCell" => {
            t.set(fl(a, 0), fl(a, 1), fl(a, 2), cell_mat(a.get(3)), 255, n(a, 4) as i32, n(a, 5) as i32);
            ok(vec![])
        }
        "GetCell" => {
            let [m, _, b, o] = t.cell(fl(a, 0), fl(a, 1), fl(a, 2));
            let m = if era == 2018 { (m != 0) as u8 } else { m };
            ok(vec![en("CellMaterial", m as f64), en("CellBlock", b as f64), en("CellOrientation", o as f64)])
        }
        "SetCells" => {
            let (lo, hi) = match (a.first(), a.get(1)) {
                (Some(Wire::List(l)), _) if l.len() >= 6 => {
                    let g = |i: usize| if let Wire::N(x) = &l[i] { x.floor() as i32 } else { 0 };
                    ([g(0), g(1), g(2)], [g(3), g(4), g(5)])
                }
                _ => return Some(Res::Err("Region3int16 expected".into())),
            };
            if ((hi[0] - lo[0] + 1) as i64) * ((hi[1] - lo[1] + 1) as i64) * ((hi[2] - lo[2] + 1) as i64) > 4 * 1024 * 1024 {
                return Some(Res::Err("Region is too large".into()));
            }
            let m = cell_mat(a.get(1));
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    for x in lo[0]..=hi[0] {
                        t.set(x, y, z, m, 255, n(a, 2) as i32, n(a, 3) as i32);
                    }
                }
            }
            ok(vec![])
        }
        "GetWaterCell" => {
            let [m, _, b, o] = t.cell(fl(a, 0), fl(a, 1), fl(a, 2));
            let w = m == CELL_WATER;
            ok(vec![Wire::B(w), en("WaterForce", if w { b as f64 } else { 0.0 }), en("WaterDirection", if w { o as f64 } else { 0.0 })])
        }
        "SetWaterCell" => {
            t.set(fl(a, 0), fl(a, 1), fl(a, 2), CELL_WATER as i32, 255, (n(a, 3) as i32) & 7, (n(a, 4) as i32) & 3);
            ok(vec![])
        }
        "AutowedgeCell" => ok(vec![Wire::B(t.autowedge(fl(a, 0), fl(a, 1), fl(a, 2)))]),
        "AutowedgeCells" => {
            let (lo, hi) = match a.first() {
                Some(Wire::List(l)) if l.len() >= 6 => {
                    let g = |i: usize| if let Wire::N(x) = &l[i] { x.floor() as i32 } else { 0 };
                    ([g(0), g(1), g(2)], [g(3), g(4), g(5)])
                }
                _ => return Some(Res::Err("Region3int16 expected".into())),
            };
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    for x in lo[0]..=hi[0] {
                        t.autowedge(x, y, z);
                    }
                }
            }
            ok(vec![])
        }
        "CellCenterToWorld" => ok(vec![Wire::V3([fl(a, 0) as f64 * v + v / 2.0, fl(a, 1) as f64 * v + v / 2.0, fl(a, 2) as f64 * v + v / 2.0])]),
        "CellCornerToWorld" => ok(vec![Wire::V3([fl(a, 0) as f64 * v, fl(a, 1) as f64 * v, fl(a, 2) as f64 * v])]),
        "WorldToCell" | "WorldToCellPreferSolid" | "WorldToCellPreferEmpty" => {
            let p = v3(a, 0).unwrap_or([0.0; 3]);
            let c = [(p[0] / v).floor() as i32, (p[1] / v).floor() as i32, (p[2] / v).floor() as i32];
            let mut out = c;
            let dirs_solid = [[0, -1, 0], [0, 1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
            let dirs_empty = [[0, 1, 0], [0, -1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
            if method == "WorldToCellPreferSolid" && t.get(c[0], c[1], c[2]) == 0 {
                for d in dirs_solid {
                    if t.get(c[0] + d[0], c[1] + d[1], c[2] + d[2]) != 0 {
                        out = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                        break;
                    }
                }
            } else if method == "WorldToCellPreferEmpty" && t.get(c[0], c[1], c[2]) != 0 {
                for d in dirs_empty {
                    if t.get(c[0] + d[0], c[1] + d[1], c[2] + d[2]) == 0 {
                        out = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                        break;
                    }
                }
            }
            ok(vec![Wire::V3([out[0] as f64, out[1] as f64, out[2] as f64])])
        }
        "CountCells" => ok(vec![Wire::N(t.count() as f64)]),
        "Clear" => {
            t.clear();
            ok(vec![])
        }
        "ConvertToSmooth" | "CopyRegion" | "PasteRegion" => ok(vec![]),
        "FillBlock" => {
            let (cf, size) = match (cf12(a, 0), v3(a, 1)) {
                (Some(c), Some(s)) => (c, s),
                _ => return Some(Res::Err("FillBlock expects a CFrame and a Vector3".into())),
            };
            let m = smooth_material(a.get(2));
            match t.fill_box(&cf, size, m, m == 0) {
                Ok(_) => ok(vec![]),
                Err(e) => Some(Res::Err(e)),
            }
        }
        "FillBall" => {
            let c = match v3(a, 0) {
                Some(c) => c,
                None => return Some(Res::Err("FillBall expects a Vector3".into())),
            };
            let m = smooth_material(a.get(2));
            match t.fill_ball(c, n(a, 1), m, m == 0) {
                Ok(_) => ok(vec![]),
                Err(e) => Some(Res::Err(e)),
            }
        }
        "FillCylinder" => {
            let cf = match cf12(a, 0) {
                Some(c) => c,
                None => return Some(Res::Err("FillCylinder expects a CFrame".into())),
            };
            let m = smooth_material(a.get(3));
            let (r, h) = (n(a, 2), n(a, 1) / 2.0);
            let ext = r + h;
            let lo: Vec<i32> = (0..3).map(|k| ((cf[k] - ext) / v).floor() as i32 - 1).collect();
            let hi: Vec<i32> = (0..3).map(|k| ((cf[k] + ext) / v).floor() as i32 + 1).collect();
            if ((hi[0] - lo[0]) as i64) * ((hi[1] - lo[1]) as i64) * ((hi[2] - lo[2]) as i64) > 4 * 1024 * 1024 {
                return Some(Res::Err("Region is too large".into()));
            }
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    for x in lo[0]..=hi[0] {
                        let (dx, dy, dz) = ((x as f64 + 0.5) * v - cf[0], (y as f64 + 0.5) * v - cf[1], (z as f64 + 0.5) * v - cf[2]);
                        let lx = cf[3] * dx + cf[6] * dy + cf[9] * dz;
                        let ly = cf[4] * dx + cf[7] * dy + cf[10] * dz;
                        let lz = cf[5] * dx + cf[8] * dy + cf[11] * dz;
                        let occ = ((r - (lx * lx + lz * lz).sqrt()) / v + 0.5).clamp(0.0, 1.0).min(((h - ly.abs()) / v + 0.5).clamp(0.0, 1.0));
                        if occ > 0.0 {
                            t.write_voxel(x, y, z, m, occ, false, m == 0, false, false);
                        }
                    }
                }
            }
            ok(vec![])
        }
        "FillWedge" => {
            let (cf, size) = match (cf12(a, 0), v3(a, 1)) {
                (Some(c), Some(s)) => (c, s),
                _ => return Some(Res::Err("FillWedge expects a CFrame and a Vector3".into())),
            };
            let m = smooth_material(a.get(2));
            let (hx, hy, hz) = (size[0] / 2.0, size[1] / 2.0, size[2] / 2.0);
            let ext = (hx * hx + hy * hy + hz * hz).sqrt();
            let lo: Vec<i32> = (0..3).map(|k| ((cf[k] - ext) / v).floor() as i32 - 1).collect();
            let hi: Vec<i32> = (0..3).map(|k| ((cf[k] + ext) / v).floor() as i32 + 1).collect();
            if ((hi[0] - lo[0]) as i64) * ((hi[1] - lo[1]) as i64) * ((hi[2] - lo[2]) as i64) > 4 * 1024 * 1024 {
                return Some(Res::Err("Region is too large".into()));
            }
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    for x in lo[0]..=hi[0] {
                        let mut inside = 0;
                        for s in 0..8 {
                            let px = (x as f64 + 0.25 + 0.5 * (s & 1) as f64) * v - cf[0];
                            let py = (y as f64 + 0.25 + 0.5 * ((s >> 1) & 1) as f64) * v - cf[1];
                            let pz = (z as f64 + 0.25 + 0.5 * ((s >> 2) & 1) as f64) * v - cf[2];
                            let lx = cf[3] * px + cf[6] * py + cf[9] * pz;
                            let ly = cf[4] * px + cf[7] * py + cf[10] * pz;
                            let lz = cf[5] * px + cf[8] * py + cf[11] * pz;
                            let a1 = (ly + hy) / (if 2.0 * hy == 0.0 { 1.0 } else { 2.0 * hy });
                            let a2 = (lz + hz) / (if 2.0 * hz == 0.0 { 1.0 } else { 2.0 * hz });
                            if lx.abs() <= hx && ly.abs() <= hy && lz.abs() <= hz && a1 <= a2 {
                                inside += 1;
                            }
                        }
                        if inside > 0 {
                            t.write_voxel(x, y, z, m, inside as f64 / 8.0, false, m == 0, false, false);
                        }
                    }
                }
            }
            ok(vec![])
        }
        "FillRegion" => {
            let (lo, hi) = match region(a, 0, 1) {
                Ok(r) => r,
                Err(e) => return Some(Res::Err(e)),
            };
            let m = smooth_material(a.get(2));
            for y in lo[1]..hi[1] {
                for z in lo[2]..hi[2] {
                    for x in lo[0]..hi[0] {
                        t.set(x, y, z, m, 255, 0, 0);
                    }
                }
            }
            ok(vec![])
        }
        "ReplaceMaterial" => {
            let (lo, hi) = match region(a, 0, 1) {
                Ok(r) => r,
                Err(e) => return Some(Res::Err(e)),
            };
            let (from, to) = (smooth_material(a.get(2)), smooth_material(a.get(3)));
            for y in lo[1]..hi[1] {
                for z in lo[2]..hi[2] {
                    for x in lo[0]..hi[0] {
                        let [m, o, _, _] = t.cell(x, y, z);
                        if m as i32 == from {
                            t.set(x, y, z, to, if to != 0 { o as i32 } else { 0 }, 0, 0);
                        }
                    }
                }
            }
            ok(vec![])
        }
        "__ReadVoxels" => {
            let (lo, hi) = match region(a, 0, 1) {
                Ok(r) => r,
                Err(e) => return Some(Res::Err(e)),
            };
            let mut mats = vec![];
            let mut occs = vec![];
            for x in lo[0]..hi[0] {
                for y in lo[1]..hi[1] {
                    for z in lo[2]..hi[2] {
                        let [m, o, _, _] = t.cell(x, y, z);
                        mats.push(Wire::N(m as f64));
                        occs.push(Wire::N(if m != 0 { (o as f64 / 255.0 * 1000.0 + 0.5).floor() / 1000.0 } else { 0.0 }));
                    }
                }
            }
            ok(vec![Wire::List(mats), Wire::List(occs), Wire::V3([(hi[0] - lo[0]) as f64, (hi[1] - lo[1]) as f64, (hi[2] - lo[2]) as f64])])
        }
        "__WriteVoxels" => {
            let (lo, hi) = match region(a, 0, 1) {
                Ok(r) => r,
                Err(e) => return Some(Res::Err(e)),
            };
            let (mats, occs) = match (a.get(2), a.get(3)) {
                (Some(Wire::List(m)), Some(Wire::List(o))) => (m.as_slice(), o.as_slice()),
                _ => (&[][..], &[][..]),
            };
            let mut i = 0;
            for x in lo[0]..hi[0] {
                for y in lo[1]..hi[1] {
                    for z in lo[2]..hi[2] {
                        let m = match mats.get(i) {
                            Some(Wire::N(m)) => *m as i32,
                            _ => 0,
                        };
                        let o = match occs.get(i) {
                            Some(Wire::N(o)) => o.clamp(0.0, 1.0),
                            _ => 0.0,
                        };
                        let ok_m = m > 0 && (m as usize) < SMOOTH.len() && o > 0.0;
                        t.set(x, y, z, if ok_m { m } else { 0 }, terrain::js_round(o * 255.0) as i32, 0, 0);
                        i += 1;
                    }
                }
            }
            ok(vec![])
        }
        "__MaterialIndex" => ok(vec![Wire::N(smooth_material(a.first()) as f64)]),
        "__MaterialEnum" => {
            let i = n(a, 0) as usize;
            let name = SMOOTH.get(i).copied().unwrap_or("Air");
            let val = classes::enum_value("Material", name).unwrap_or(0) as f64;
            ok(vec![Wire::E("Material".into(), val)])
        }
        "GetMaterialColor" => {
            let name = match a.first() {
                Some(Wire::E(t, v)) => classes::enum_name(t, *v as i64).unwrap_or("").to_string(),
                _ => String::new(),
            };
            let c = t.material_colors.get(&name).copied().unwrap_or_else(|| smooth_color(&name));
            ok(vec![Wire::C3(c)])
        }
        "SetMaterialColor" => {
            let name = match a.first() {
                Some(Wire::E(t, v)) => classes::enum_name(t, *v as i64).unwrap_or("").to_string(),
                _ => String::new(),
            };
            if let Some(Wire::C3(c)) = a.get(1) {
                t.material_colors.insert(name, *c);
            }
            let keys: Vec<_> = t.chunks.keys().copied().collect();
            for k in keys {
                t.dirty.insert(k);
            }
            ok(vec![])
        }
        _ => None,
    }
}

fn smooth_color(name: &str) -> [f64; 3] {
    let c: [f64; 3] = match name {
        "Grass" => [106.0, 127.0, 63.0],
        "Slate" => [63.0, 127.0, 107.0],
        "Concrete" => [127.0, 102.0, 63.0],
        "Brick" => [138.0, 86.0, 62.0],
        "Sand" => [143.0, 126.0, 95.0],
        "WoodPlanks" => [139.0, 109.0, 79.0],
        "Rock" => [102.0, 108.0, 111.0],
        "Glacier" => [101.0, 176.0, 234.0],
        "Snow" => [195.0, 199.0, 218.0],
        "Sandstone" => [137.0, 90.0, 71.0],
        "Mud" => [58.0, 46.0, 36.0],
        "Basalt" => [30.0, 30.0, 37.0],
        "Ground" => [102.0, 92.0, 59.0],
        "CrackedLava" => [232.0, 156.0, 74.0],
        "Asphalt" => [115.0, 123.0, 107.0],
        "Cobblestone" => [132.0, 123.0, 90.0],
        "Ice" => [129.0, 194.0, 224.0],
        "LeafyGrass" => [115.0, 132.0, 74.0],
        "Salt" => [198.0, 189.0, 181.0],
        "Limestone" => [206.0, 173.0, 148.0],
        "Pavement" => [148.0, 148.0, 140.0],
        "Water" => [12.0, 84.0, 92.0],
        _ => [127.0, 127.0, 127.0],
    };
    [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0]
}

use swirl_core::meshfile::parse_mesh;

pub fn parse(bytes: &[u8]) -> Option<Vec<f32>> {
    let m = parse_mesh(bytes).ok()?;
    let flip = !bytes.starts_with(b"version 1.");
    let mut out = Vec::with_capacity(m.pos.len() * 8);
    let n = m.pos.len();
    let mut i = 0;
    while i + 2 < n {
        let (a, b, c) = (m.pos[i], m.pos[i + 1], m.pos[i + 2]);
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let fnv = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let fl = (fnv[0] * fnv[0] + fnv[1] * fnv[1] + fnv[2] * fnv[2]).sqrt();
        if !(fl > 1e-12) || !fl.is_finite() {
            i += 3;
            continue;
        }
        let fnn = [(fnv[0] / fl) as f32, (fnv[1] / fl) as f32, (fnv[2] / fl) as f32];
        for k in 0..3 {
            let p = m.pos[i + k];
            let mut nrm = m.nrm[i + k];
            let l = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
            let nv = if l > 0.1 && l.is_finite() {
                nrm = [nrm[0] / l, nrm[1] / l, nrm[2] / l];
                [nrm[0] as f32, nrm[1] as f32, nrm[2] as f32]
            } else {
                fnn
            };
            let uv = m.uv[i + k];
            let v = if flip { 1.0 - uv[1] } else { uv[1] };
            let u = if uv[0].is_finite() && uv[0].abs() < 1e4 { uv[0] } else { 0.0 };
            let v = if v.is_finite() && v.abs() < 1e4 { v } else { 0.0 };
            out.extend_from_slice(&[p[0] as f32, p[1] as f32, p[2] as f32, nv[0], nv[1], nv[2], u as f32, v as f32]);
        }
        i += 3;
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

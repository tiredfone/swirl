pub fn cube() -> Vec<f32> {
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([1.0, 0.0, 0.0], [[0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5], [0.5, -0.5, 0.5]]),
        ([-1.0, 0.0, 0.0], [[-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5], [-0.5, -0.5, -0.5]]),
        ([0.0, 1.0, 0.0], [[-0.5, 0.5, -0.5], [-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5]]),
        ([0.0, -1.0, 0.0], [[-0.5, -0.5, 0.5], [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5]]),
        ([0.0, 0.0, 1.0], [[0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, -0.5, 0.5]]),
        ([0.0, 0.0, -1.0], [[-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5], [0.5, -0.5, -0.5]]),
    ];
    let mut v = vec![];
    for (n, q) in faces.iter() {
        for i in [0usize, 1, 2, 0, 2, 3] {
            v.extend_from_slice(&q[i]);
            v.extend_from_slice(n);
        }
    }
    v
}

pub fn sphere() -> Vec<f32> {
    let (lon, lat) = (24usize, 16usize);
    let pt = |i: usize, j: usize| -> [f32; 3] {
        let th = i as f32 / lon as f32 * std::f32::consts::TAU;
        let ph = j as f32 / lat as f32 * std::f32::consts::PI;
        [ph.sin() * th.cos(), ph.cos(), ph.sin() * th.sin()]
    };
    let mut v = vec![];
    for i in 0..lon {
        for j in 0..lat {
            let (a, b, c, d) = (pt(i, j), pt(i + 1, j), pt(i + 1, j + 1), pt(i, j + 1));
            for q in [a, c, b, a, d, c] {
                v.extend_from_slice(&[q[0] * 0.5, q[1] * 0.5, q[2] * 0.5, q[0], q[1], q[2]]);
            }
        }
    }
    v
}

pub fn cylinder() -> Vec<f32> {
    let n = 24usize;
    let mut v = vec![];
    for i in 0..n {
        let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let (y0, z0, y1, z1) = (a0.cos(), a0.sin(), a1.cos(), a1.sin());
        let p = |x: f32, y: f32, z: f32, n: [f32; 3]| [x, y * 0.5, z * 0.5, n[0], n[1], n[2]];
        let quad = [p(-0.5, y0, z0, [0.0, y0, z0]), p(0.5, y0, z0, [0.0, y0, z0]), p(0.5, y1, z1, [0.0, y1, z1]), p(-0.5, y1, z1, [0.0, y1, z1])];
        for k in [0usize, 2, 1, 0, 3, 2] {
            v.extend_from_slice(&quad[k]);
        }
        for (x, nx) in [(0.5f32, 1.0f32), (-0.5, -1.0)] {
            let c = [x, 0.0, 0.0, nx, 0.0, 0.0];
            let (a, b) = (p(x, y0, z0, [nx, 0.0, 0.0]), p(x, y1, z1, [nx, 0.0, 0.0]));
            if nx > 0.0 {
                v.extend_from_slice(&c);
                v.extend_from_slice(&b);
                v.extend_from_slice(&a);
            } else {
                v.extend_from_slice(&c);
                v.extend_from_slice(&a);
                v.extend_from_slice(&b);
            }
        }
    }
    v
}

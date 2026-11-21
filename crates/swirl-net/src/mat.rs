pub type M4 = [f32; 16];

pub fn identity() -> M4 {
    [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
}

pub fn mul(a: &M4, b: &M4) -> M4 {
    let mut o = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[k * 4 + r] * b[c * 4 + k];
            }
            o[c * 4 + r] = s;
        }
    }
    o
}

pub fn perspective(fovy: f32, aspect: f32, near: f32, far: f32) -> M4 {
    let f = 1.0 / (fovy / 2.0).tan();
    let nf = 1.0 / (near - far);
    [f / aspect, 0.0, 0.0, 0.0, 0.0, f, 0.0, 0.0, 0.0, 0.0, (far + near) * nf, -1.0, 0.0, 0.0, 2.0 * far * near * nf, 0.0]
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
    [v[0] / l, v[1] / l, v[2] / l]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> M4 {
    let z = norm([eye[0] - target[0], eye[1] - target[1], eye[2] - target[2]]);
    let x = norm(cross(up, z));
    let y = cross(z, x);
    [x[0], y[0], z[0], 0.0, x[1], y[1], z[1], 0.0, x[2], y[2], z[2], 0.0, -dot(x, eye), -dot(y, eye), -dot(z, eye), 1.0]
}

pub fn from_cframe(cf: &[f32; 12], size: [f32; 3]) -> M4 {
    [
        cf[3] * size[0], cf[6] * size[0], cf[9] * size[0], 0.0, cf[4] * size[1], cf[7] * size[1], cf[10] * size[1], 0.0, cf[5] * size[2], cf[8] * size[2], cf[11] * size[2], 0.0, cf[0], cf[1], cf[2], 1.0,
    ]
}

pub type Cf = [f32; 12];

pub const CF_ID: Cf = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

pub fn cf_mul(a: &Cf, b: &Cf) -> Cf {
    let ra = [a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11]];
    let rb = [b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11]];
    let mut r = [0.0f32; 9];
    for i in 0..3 {
        for j in 0..3 {
            r[i * 3 + j] = ra[i * 3] * rb[j] + ra[i * 3 + 1] * rb[3 + j] + ra[i * 3 + 2] * rb[6 + j];
        }
    }
    let p = [
        ra[0] * b[0] + ra[1] * b[1] + ra[2] * b[2] + a[0],
        ra[3] * b[0] + ra[4] * b[1] + ra[5] * b[2] + a[1],
        ra[6] * b[0] + ra[7] * b[1] + ra[8] * b[2] + a[2],
    ];
    [p[0], p[1], p[2], r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8]]
}

pub fn cf_inv(a: &Cf) -> Cf {
    let rt = [a[3], a[6], a[9], a[4], a[7], a[10], a[5], a[8], a[11]];
    let p = [
        -(rt[0] * a[0] + rt[1] * a[1] + rt[2] * a[2]),
        -(rt[3] * a[0] + rt[4] * a[1] + rt[5] * a[2]),
        -(rt[6] * a[0] + rt[7] * a[1] + rt[8] * a[2]),
    ];
    [p[0], p[1], p[2], rt[0], rt[1], rt[2], rt[3], rt[4], rt[5], rt[6], rt[7], rt[8]]
}

pub fn cf_yaw(pos: [f32; 3], theta: f32) -> Cf {
    let (s, c) = theta.sin_cos();
    [pos[0], pos[1], pos[2], c, 0.0, s, 0.0, 1.0, 0.0, -s, 0.0, c]
}

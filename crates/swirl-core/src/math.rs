pub type V3 = [f64; 3];
pub type M3 = [f64; 9];
pub type M4 = [f64; 16];

pub const I3: M3 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CF {
    pub p: V3,
    pub r: M3,
}

impl CF {
    pub fn new(p: V3, r: M3) -> CF {
        CF { p, r }
    }
    pub fn at(p: V3) -> CF {
        CF { p, r: I3 }
    }
    pub fn identity() -> CF {
        CF::at([0.0, 0.0, 0.0])
    }
    pub fn mul(&self, b: &CF) -> CF {
        let q = rmul(&self.r, b.p);
        CF {
            p: [q[0] + self.p[0], q[1] + self.p[1], q[2] + self.p[2]],
            r: mm(&self.r, &b.r),
        }
    }
    pub fn inv(&self) -> CF {
        let rt = tr(&self.r);
        let q = rmul(&rt, self.p);
        CF { p: [-q[0], -q[1], -q[2]], r: rt }
    }
    pub fn apply(&self, v: V3) -> V3 {
        let q = rmul(&self.r, v);
        [q[0] + self.p[0], q[1] + self.p[1], q[2] + self.p[2]]
    }
}

pub fn norm(v: V3) -> V3 {
    let mut l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 || l.is_nan() {
        l = 1.0;
    }
    [v[0] / l, v[1] / l, v[2] / l]
}

pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn rmul(a: &M3, v: V3) -> V3 {
    [
        a[0] * v[0] + a[1] * v[1] + a[2] * v[2],
        a[3] * v[0] + a[4] * v[1] + a[5] * v[2],
        a[6] * v[0] + a[7] * v[1] + a[8] * v[2],
    ]
}

pub fn mm(a: &M3, b: &M3) -> M3 {
    let mut o = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            o[r * 3 + c] = a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c];
        }
    }
    o
}

pub fn tr(a: &M3) -> M3 {
    [a[0], a[3], a[6], a[1], a[4], a[7], a[2], a[5], a[8]]
}

pub fn rot_z(a: f64) -> M3 {
    let c = a.cos();
    let s = a.sin();
    [c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0]
}

pub fn rot_x(a: f64) -> M3 {
    let c = a.cos();
    let s = a.sin();
    [1.0, 0.0, 0.0, 0.0, c, -s, 0.0, s, c]
}

pub fn perspective(fovy: f64, aspect: f64, near: f64, far: f64) -> M4 {
    let f = 1.0 / (fovy / 2.0).tan();
    let nf = 1.0 / (near - far);
    [
        f / aspect, 0.0, 0.0, 0.0, 0.0, f, 0.0, 0.0, 0.0, 0.0, (far + near) * nf, -1.0, 0.0, 0.0,
        2.0 * far * near * nf, 0.0,
    ]
}

pub fn look_at(eye: V3, target: V3, up: V3) -> M4 {
    let z = norm(sub(eye, target));
    let x = norm(cross(up, z));
    let y = cross(z, x);
    [
        x[0], y[0], z[0], 0.0, x[1], y[1], z[1], 0.0, x[2], y[2], z[2], 0.0, -dot(x, eye),
        -dot(y, eye), -dot(z, eye), 1.0,
    ]
}

pub fn mul4(a: &M4, b: &M4) -> M4 {
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

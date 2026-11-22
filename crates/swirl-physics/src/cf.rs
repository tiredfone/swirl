pub type CF = [f64; 12];

pub const IDENTITY: CF = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

pub fn mul(a: &CF, b: &CF) -> CF {
    let [ax, ay, az, a0, a1, a2, a3, a4, a5, a6, a7, a8] = *a;
    let [bx, by, bz, b0, b1, b2, b3, b4, b5, b6, b7, b8] = *b;
    [
        a0 * bx + a1 * by + a2 * bz + ax,
        a3 * bx + a4 * by + a5 * bz + ay,
        a6 * bx + a7 * by + a8 * bz + az,
        a0 * b0 + a1 * b3 + a2 * b6,
        a0 * b1 + a1 * b4 + a2 * b7,
        a0 * b2 + a1 * b5 + a2 * b8,
        a3 * b0 + a4 * b3 + a5 * b6,
        a3 * b1 + a4 * b4 + a5 * b7,
        a3 * b2 + a4 * b5 + a5 * b8,
        a6 * b0 + a7 * b3 + a8 * b6,
        a6 * b1 + a7 * b4 + a8 * b7,
        a6 * b2 + a7 * b5 + a8 * b8,
    ]
}

pub fn inv(a: &CF) -> CF {
    let [x, y, z, r0, r1, r2, r3, r4, r5, r6, r7, r8] = *a;
    [-(r0 * x + r3 * y + r6 * z), -(r1 * x + r4 * y + r7 * z), -(r2 * x + r5 * y + r8 * z), r0, r3, r6, r1, r4, r7, r2, r5, r8]
}

pub fn point(a: &CF, x: f64, y: f64, z: f64) -> [f64; 3] {
    [a[3] * x + a[4] * y + a[5] * z + a[0], a[6] * x + a[7] * y + a[8] * z + a[1], a[9] * x + a[10] * y + a[11] * z + a[2]]
}

pub fn vector(a: &CF, x: f64, y: f64, z: f64) -> [f64; 3] {
    [a[3] * x + a[4] * y + a[5] * z, a[6] * x + a[7] * y + a[8] * z, a[9] * x + a[10] * y + a[11] * z]
}

pub fn from_quat(x: f64, y: f64, z: f64, qx: f64, qy: f64, qz: f64, qw: f64) -> CF {
    let (xx, yy, zz) = (qx * qx, qy * qy, qz * qz);
    let (xy, xz, yz) = (qx * qy, qx * qz, qy * qz);
    let (wx, wy, wz) = (qw * qx, qw * qy, qw * qz);
    [x, y, z, 1.0 - 2.0 * (yy + zz), 2.0 * (xy - wz), 2.0 * (xz + wy), 2.0 * (xy + wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz - wx), 2.0 * (xz - wy), 2.0 * (yz + wx), 1.0 - 2.0 * (xx + yy)]
}

pub fn to_quat(a: &CF) -> [f64; 4] {
    let (m00, m01, m02, m10, m11, m12, m20, m21, m22) = (a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11]);
    let tr = m00 + m11 + m22;
    let (qx, qy, qz, qw);
    if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        qw = 0.25 * s;
        qx = (m21 - m12) / s;
        qy = (m02 - m20) / s;
        qz = (m10 - m01) / s;
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        qw = (m21 - m12) / s;
        qx = 0.25 * s;
        qy = (m01 + m10) / s;
        qz = (m02 + m20) / s;
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        qw = (m02 - m20) / s;
        qx = (m01 + m10) / s;
        qy = 0.25 * s;
        qz = (m12 + m21) / s;
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        qw = (m10 - m01) / s;
        qx = (m02 + m20) / s;
        qy = (m12 + m21) / s;
        qz = 0.25 * s;
    }
    let mut l = (qx * qx + qy * qy + qz * qz + qw * qw).sqrt();
    if l == 0.0 {
        l = 1.0;
    }
    [qx / l, qy / l, qz / l, qw / l]
}

pub fn axis_angle(ax: f64, ay: f64, az: f64, t: f64) -> CF {
    let mut l = (ax * ax + ay * ay + az * az).sqrt();
    if l == 0.0 {
        l = 1.0;
    }
    let s = (t / 2.0).sin();
    from_quat(0.0, 0.0, 0.0, (ax / l) * s, (ay / l) * s, (az / l) * s, (t / 2.0).cos())
}

pub fn is_finite(a: &CF) -> bool {
    a.iter().all(|v| v.is_finite())
}

pub fn arr(v: &[f64]) -> Option<CF> {
    if v.len() >= 12 {
        let mut o = [0.0; 12];
        o.copy_from_slice(&v[..12]);
        Some(o)
    } else {
        None
    }
}

pub fn with_pos(a: &CF, x: f64, y: f64, z: f64) -> CF {
    let mut r = *a;
    r[0] = x;
    r[1] = y;
    r[2] = z;
    r
}

pub fn rot_only(a: &CF) -> CF {
    with_pos(a, 0.0, 0.0, 0.0)
}

pub fn look_vector(a: &CF) -> [f64; 3] {
    [-a[5], -a[8], -a[11]]
}

pub fn look_at(px: f64, py: f64, pz: f64, tx: f64, ty: f64, tz: f64) -> CF {
    let (mut fx, mut fy, mut fz) = (tx - px, ty - py, tz - pz);
    let fl = (fx * fx + fy * fy + fz * fz).sqrt();
    if fl < 1e-9 {
        return [px, py, pz, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    }
    fx /= fl;
    fy /= fl;
    fz /= fl;
    let (mut rx, ry, mut rz) = (-fz, 0.0, fx);
    let rl = (rx * rx + rz * rz).sqrt();
    let (rx2, rz2);
    if rl < 1e-6 {
        rx2 = 1.0;
        rz2 = 0.0;
        rx = 1.0;
        rz = 0.0;
    } else {
        rx /= rl;
        rz /= rl;
        rx2 = rx;
        rz2 = rz;
    }
    let (bx, by, bz) = (-fx, -fy, -fz);
    let (ux, uy, uz) = (by * rz2 - bz * ry, bz * rx2 - bx * rz2, bx * ry - by * rx2);
    [px, py, pz, rx, ux, -fx, ry, uy, -fy, rz, uz, -fz]
}

pub fn lerp(a: &CF, b: &CF, t: f64) -> CF {
    let qa = to_quat(a);
    let mut qb = to_quat(b);
    let mut d = qa[0] * qb[0] + qa[1] * qb[1] + qa[2] * qb[2] + qa[3] * qb[3];
    if d < 0.0 {
        for v in qb.iter_mut() {
            *v = -*v;
        }
        d = -d;
    }
    let mut q = [0.0; 4];
    if d > 0.9995 {
        for i in 0..4 {
            q[i] = qa[i] + (qb[i] - qa[i]) * t;
        }
    } else {
        let th = d.min(1.0).acos();
        let s = th.sin();
        let wa = ((1.0 - t) * th).sin() / s;
        let wb = (t * th).sin() / s;
        for i in 0..4 {
            q[i] = qa[i] * wa + qb[i] * wb;
        }
    }
    let mut l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if l == 0.0 {
        l = 1.0;
    }
    from_quat(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, q[0] / l, q[1] / l, q[2] / l, q[3] / l)
}

pub fn orthonormalize(a: &CF) -> CF {
    let (mut r0, mut r1, r2, mut r3, mut r4, r5, mut r6, mut r7, r8) = (a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11]);
    let _ = (r2, r5, r8);
    let mut xl = (r0 * r0 + r3 * r3 + r6 * r6).sqrt();
    if xl == 0.0 {
        xl = 1.0;
    }
    r0 /= xl;
    r3 /= xl;
    r6 /= xl;
    let d = r0 * r1 + r3 * r4 + r6 * r7;
    r1 -= d * r0;
    r4 -= d * r3;
    r7 -= d * r6;
    let mut yl = (r1 * r1 + r4 * r4 + r7 * r7).sqrt();
    if yl == 0.0 {
        yl = 1.0;
    }
    r1 /= yl;
    r4 /= yl;
    r7 /= yl;
    let zx = r3 * r7 - r6 * r4;
    let zy = r6 * r1 - r0 * r7;
    let zz = r0 * r4 - r3 * r1;
    [a[0], a[1], a[2], r0, r1, zx, r3, r4, zy, r6, r7, zz]
}

pub fn angles(rx: f64, ry: f64, rz: f64) -> CF {
    let (cx, sx, cy, sy, cz, sz) = (rx.cos(), rx.sin(), ry.cos(), ry.sin(), rz.cos(), rz.sin());
    [0.0, 0.0, 0.0, cy * cz, -cy * sz, sy, cz * sx * sy + cx * sz, cx * cz - sx * sy * sz, -cy * sx, sx * sz - cx * cz * sy, cz * sx + cx * sy * sz, cx * cy]
}

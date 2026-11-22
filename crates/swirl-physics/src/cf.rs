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

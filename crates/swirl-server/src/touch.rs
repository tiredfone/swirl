pub struct Box3 {
    pub c: [f64; 3],
    pub r: [f64; 9],
    pub h: [f64; 3],
}

fn col(r: &[f64; 9], k: usize) -> [f64; 3] {
    [r[k], r[3 + k], r[6 + k]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn overlap(a: &Box3, b: &Box3) -> bool {
    let ax: Vec<[f64; 3]> = (0..3).map(|k| col(&a.r, k)).collect();
    let bx: Vec<[f64; 3]> = (0..3).map(|k| col(&b.r, k)).collect();
    let t = [b.c[0] - a.c[0], b.c[1] - a.c[1], b.c[2] - a.c[2]];
    let mut axes: Vec<[f64; 3]> = vec![];
    axes.extend(ax.iter().copied());
    axes.extend(bx.iter().copied());
    for i in 0..3 {
        for j in 0..3 {
            let c = cross(ax[i], bx[j]);
            if dot(c, c) > 1e-9 {
                axes.push(c);
            }
        }
    }
    for axis in axes {
        let ra: f64 = (0..3).map(|k| a.h[k] * dot(ax[k], axis).abs()).sum();
        let rb: f64 = (0..3).map(|k| b.h[k] * dot(bx[k], axis).abs()).sum();
        if dot(t, axis).abs() > ra + rb + 0.2 * dot(axis, axis).sqrt() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes() {
        let id = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let a = Box3 { c: [0.0; 3], r: id, h: [1.0, 1.0, 1.0] };
        let b = Box3 { c: [1.5, 0.0, 0.0], r: id, h: [1.0, 1.0, 1.0] };
        let c = Box3 { c: [3.5, 0.0, 0.0], r: id, h: [1.0, 1.0, 1.0] };
        assert!(overlap(&a, &b));
        assert!(!overlap(&a, &c));
    }
}

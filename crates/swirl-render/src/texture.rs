use crate::img::Img;
use std::cell::RefCell;

pub struct Texture {
    levels: RefCell<Vec<Option<(usize, usize, Vec<u32>)>>>,
    pub repeat: bool,
    pub mip: bool,
    pub lod: bool,
    pub max_level: usize,
    pub cut: bool,
    pub has_alpha: bool,
    w0: usize,
    h0: usize,
}

impl Texture {
    pub fn new(im: Img, hat: bool) -> Texture {
        let mut im = im;
        let mut has_alpha = false;
        if hat {
            let mut tw = 1;
            while tw < im.w {
                tw *= 2;
            }
            let mut th = 1;
            while th < im.h {
                th *= 2;
            }
            tw = tw.min(2048);
            th = th.min(2048);
            if tw != im.w || th != im.h {
                im = im.resized(tw, th);
            }
            has_alpha = im.px.iter().any(|&c| c < 0xFF000000);
        }
        let (w, h) = (im.w, im.h);
        let pot = (w & (w - 1)) == 0 && (h & (h - 1)) == 0;
        let mut m = 0;
        let (mut a, mut b) = (w, h);
        while a > 1 || b > 1 {
            a = (a >> 1).max(1);
            b = (b >> 1).max(1);
            m += 1;
        }
        let mut levels = vec![None; m + 1];
        levels[0] = Some((w, h, im.px));
        Texture { levels: RefCell::new(levels), repeat: pot, mip: pot, lod: false, max_level: m, cut: true, has_alpha, w0: w, h0: h }
    }


    fn ensure(&self, l: usize) {
        if self.levels.borrow()[l].is_some() {
            return;
        }
        self.ensure(l - 1);
        let (w, h, n) = {
            let lv = self.levels.borrow();
            let (w, h, p) = lv[l - 1].as_ref().unwrap();
            let (w, h) = (*w, *h);
            let nw = (w >> 1).max(1);
            let nh = (h >> 1).max(1);
            let w1 = w > 1;
            let h1 = h > 1;
            let mut n = Vec::with_capacity(nw * nh);
            for y in 0..nh {
                let ra = 2 * y * w;
                let rb = if h1 { ra + w } else { ra };
                for x in 0..nw {
                    let xa = 2 * x;
                    let xb = if w1 { xa + 1 } else { xa };
                    let c1 = p[ra + xa];
                    let c2 = p[ra + xb];
                    let c3 = p[rb + xa];
                    let c4 = p[rb + xb];
                    let e = ((c1 & 0x00FF00FF) + (c2 & 0x00FF00FF) + (c3 & 0x00FF00FF) + (c4 & 0x00FF00FF) + 0x00020002) >> 2 & 0x00FF00FF;
                    let o = (((c1 >> 8) & 0x00FF00FF) + ((c2 >> 8) & 0x00FF00FF) + ((c3 >> 8) & 0x00FF00FF) + ((c4 >> 8) & 0x00FF00FF) + 0x00020002) >> 2 & 0x00FF00FF;
                    n.push(e | (o << 8));
                }
            }
            (nw, nh, n)
        };
        self.levels.borrow_mut()[l] = Some((w, h, n));
    }

    pub fn sample(&self, u: f64, v: f64, dudx: f64, dvdx: f64, dudy: f64, dvdy: f64) -> [f64; 4] {
        if !self.mip && !self.lod {
            return self.bilinear(0, u, v);
        }
        let w = self.w0 as f64;
        let h = self.h0 as f64;
        let rx = (dudx * w).powi(2) + (dvdx * h).powi(2);
        let ry = (dudy * w).powi(2) + (dvdy * h).powi(2);
        let rho2 = rx.max(ry);
        if rho2 <= 1.0 {
            return self.bilinear(0, u, v);
        }
        let lambda = 0.5 * rho2.log2();
        let max = self.max_level;
        if lambda >= max as f64 {
            return self.bilinear(max, u, v);
        }
        let l0 = lambda.floor() as usize;
        let f = lambda - l0 as f64;
        let a = self.bilinear(l0, u, v);
        if f <= 0.0 {
            return a;
        }
        let b = self.bilinear(l0 + 1, u, v);
        [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f, a[3] + (b[3] - a[3]) * f]
    }

    pub fn sample0(&self, u: f64, v: f64) -> [f64; 4] {
        self.sample(u, v, 0.0, 0.0, 0.0, 0.0)
    }

    pub fn bilinear(&self, lvl: usize, u: f64, v: f64) -> [f64; 4] {
        self.ensure(lvl);
        let lv = self.levels.borrow();
        let (w, h, p) = lv[lvl].as_ref().unwrap();
        let (w, h) = (*w as i64, *h as i64);
        let x = u * w as f64 - 0.5;
        let y = v * h as f64 - 0.5;
        let x0 = x.floor() as i64;
        let y0 = y.floor() as i64;
        let fx = x - x0 as f64;
        let fy = y - y0 as f64;
        let (xa, xb, ya, yb);
        if self.repeat {
            xa = ((x0 % w) + w) % w;
            xb = (xa + 1) % w;
            ya = ((y0 % h) + h) % h;
            yb = (ya + 1) % h;
        } else {
            xa = x0.clamp(0, w - 1);
            xb = (x0 + 1).clamp(0, w - 1);
            ya = y0.clamp(0, h - 1);
            yb = (y0 + 1).clamp(0, h - 1);
        }
        let c00 = p[(ya * w + xa) as usize];
        let c10 = p[(ya * w + xb) as usize];
        let c01 = p[(yb * w + xa) as usize];
        let c11 = p[(yb * w + xb) as usize];
        let w00 = (1.0 - fx) * (1.0 - fy) / 255.0;
        let w10 = fx * (1.0 - fy) / 255.0;
        let w01 = (1.0 - fx) * fy / 255.0;
        let w11 = fx * fy / 255.0;
        let ch = |s: u32| -> f64 {
            ((c00 >> s) & 255) as f64 * w00 + ((c10 >> s) & 255) as f64 * w10 + ((c01 >> s) & 255) as f64 * w01 + ((c11 >> s) & 255) as f64 * w11
        };
        [ch(16), ch(8), ch(0), ch(24)]
    }
}

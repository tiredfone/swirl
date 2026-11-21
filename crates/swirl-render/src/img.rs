#[derive(Clone)]
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u32>,
}

fn over(s: u32, d: u32) -> u32 {
    let sa = (s >> 24) & 255;
    if sa == 255 {
        return s;
    }
    if sa == 0 {
        return d;
    }
    let ia = 255 - sa;
    let f = |sh: u32| -> u32 { ((((s >> sh) & 255) * sa + 127) / 255) + ((((d >> sh) & 255) * ia + 127) / 255) };
    let r = f(16).min(255);
    let g = f(8).min(255);
    let b = f(0).min(255);
    (255 << 24) | (r << 16) | (g << 8) | b
}

impl Img {
    pub fn new(w: usize, h: usize) -> Img {
        Img { w, h, px: vec![0; w * h] }
    }

    pub fn is_image_bytes(b: &[u8]) -> bool {
        b.len() >= 12
            && (b.starts_with(b"\x89PNG")
                || b.starts_with(b"\xff\xd8\xff")
                || (b.starts_with(b"RIFF") && &b[8..12] == b"WEBP")
                || b.starts_with(b"GIF8")
                || b.starts_with(b"BM"))
    }

    pub fn decode(b: &[u8], max_side: usize) -> Option<Img> {
        if !Img::is_image_bytes(b) || !b.starts_with(b"\x89PNG") {
            return None;
        }
        let mut dec = png::Decoder::new(std::io::Cursor::new(b));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut rd = dec.read_info().ok()?;
        let mut buf = vec![0; rd.output_buffer_size()];
        let info = rd.next_frame(&mut buf).ok()?;
        let (w, h) = (info.width as usize, info.height as usize);
        if w < 1 || h < 1 || w > max_side || h > max_side {
            return None;
        }
        let ch = info.color_type.samples();
        let mut px = Vec::with_capacity(w * h);
        for i in 0..w * h {
            let s = &buf[i * ch..i * ch + ch];
            let (r, g, bl, a) = match info.color_type {
                png::ColorType::Grayscale => (s[0], s[0], s[0], 255),
                png::ColorType::GrayscaleAlpha => (s[0], s[0], s[0], s[1]),
                png::ColorType::Rgb => (s[0], s[1], s[2], 255),
                _ => (s[0], s[1], s[2], s[3]),
            };
            let a7 = 127 - (a as u32 >> 1);
            let a8 = if a7 == 0 {
                255
            } else if a7 == 127 {
                0
            } else {
                (((127 - a7) as f64) * 255.0 / 127.0).round() as u32
            };
            px.push((a8 << 24) | ((r as u32) << 16) | ((g as u32) << 8) | bl as u32);
        }
        Some(Img { w, h, px })
    }

    pub fn decode_hat(b: &[u8], cap: usize) -> Option<Img> {
        let im = Img::decode(b, 4096)?;
        let mut tw = 1;
        while tw < im.w {
            tw *= 2;
        }
        let mut th = 1;
        while th < im.h {
            th *= 2;
        }
        tw = tw.min(cap);
        th = th.min(cap);
        if tw != im.w || th != im.h {
            Some(im.resized(tw, th))
        } else {
            Some(im)
        }
    }

    pub fn fill_rect(&mut self, x: i64, y: i64, w: i64, h: i64, rgb: [u32; 3]) {
        let v = (255u32 << 24) | (rgb[0] << 16) | (rgb[1] << 8) | rgb[2];
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (self.w as i64).min(x + w);
        let y1 = (self.h as i64).min(y + h);
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.px[yy as usize * self.w + xx as usize] = v;
            }
        }
    }

    pub fn draw_over(&mut self, src: &Img, clip: Option<&[[i64; 4]]>) {
        let w = self.w;
        let h = self.h;
        match clip {
            None => {
                for i in 0..w * h {
                    self.px[i] = over(src.px[i], self.px[i]);
                }
            }
            Some(rects) => {
                let mut done = vec![false; w * h];
                for r in rects {
                    for y in r[1].max(0)..(h as i64).min(r[1] + r[3]) {
                        for x in r[0].max(0)..(w as i64).min(r[0] + r[2]) {
                            let i = y as usize * w + x as usize;
                            if done[i] {
                                continue;
                            }
                            done[i] = true;
                            self.px[i] = over(src.px[i], self.px[i]);
                        }
                    }
                }
            }
        }
    }

    pub fn draw_scaled(&mut self, src: &Img, dx: i64, dy: i64, dw: i64, dh: i64, clip: Option<[i64; 4]>) {
        let sw = src.w as i64;
        let sh = src.h as i64;
        for y in 0..dh {
            let ty = dy + y;
            if ty < 0 || ty >= self.h as i64 {
                continue;
            }
            if let Some(c) = clip {
                if ty < c[1] || ty >= c[1] + c[3] {
                    continue;
                }
            }
            let fy = (y as f64 + 0.5) * sh as f64 / dh as f64 - 0.5;
            let y0 = fy.floor() as i64;
            let wy = fy - y0 as f64;
            let ya = y0.clamp(0, sh - 1);
            let yb = (y0 + 1).clamp(0, sh - 1);
            for x in 0..dw {
                let tx = dx + x;
                if tx < 0 || tx >= self.w as i64 {
                    continue;
                }
                if let Some(c) = clip {
                    if tx < c[0] || tx >= c[0] + c[2] {
                        continue;
                    }
                }
                let fx = (x as f64 + 0.5) * sw as f64 / dw as f64 - 0.5;
                let x0 = fx.floor() as i64;
                let wx = fx - x0 as f64;
                let xa = x0.clamp(0, sw - 1);
                let xb = (x0 + 1).clamp(0, sw - 1);
                let mut acc = [0.0f64; 4];
                for (px_, py_, k) in [
                    (xa, ya, (1.0 - wx) * (1.0 - wy)),
                    (xb, ya, wx * (1.0 - wy)),
                    (xa, yb, (1.0 - wx) * wy),
                    (xb, yb, wx * wy),
                ] {
                    let c = src.px[(py_ * sw + px_) as usize];
                    let a = ((c >> 24) & 255) as f64;
                    acc[0] += ((c >> 16) & 255) as f64 * a / 255.0 * k;
                    acc[1] += ((c >> 8) & 255) as f64 * a / 255.0 * k;
                    acc[2] += (c & 255) as f64 * a / 255.0 * k;
                    acc[3] += a * k;
                }
                let a = acc[3].round() as i64;
                let v = if a <= 0 {
                    0
                } else {
                    let q = |v: f64| ((v * 255.0 / acc[3]).round() as i64).min(255) as u32;
                    ((a as u32) << 24) | (q(acc[0]) << 16) | (q(acc[1]) << 8) | q(acc[2])
                };
                let i = ty as usize * self.w + tx as usize;
                self.px[i] = over(v, self.px[i]);
            }
        }
    }

    pub fn resized(&self, tw: usize, th: usize) -> Img {
        let sw = self.w as i64;
        let sh = self.h as i64;
        let mut out = Vec::with_capacity(tw * th);
        let mut xa = vec![0usize; tw];
        let mut xb = vec![0usize; tw];
        let mut xw = vec![0f64; tw];
        for x in 0..tw {
            let f = (x as f64 + 0.5) * sw as f64 / tw as f64 - 0.5;
            let x0 = f.floor() as i64;
            xw[x] = f - x0 as f64;
            xa[x] = x0.clamp(0, sw - 1) as usize;
            xb[x] = (x0 + 1).clamp(0, sw - 1) as usize;
        }
        for y in 0..th {
            let f = (y as f64 + 0.5) * sh as f64 / th as f64 - 0.5;
            let y0 = f.floor() as i64;
            let wy = f - y0 as f64;
            let ra = y0.clamp(0, sh - 1) as usize * self.w;
            let rb = (y0 + 1).clamp(0, sh - 1) as usize * self.w;
            for x in 0..tw {
                let fx = xw[x];
                let c00 = self.px[ra + xa[x]];
                let c10 = self.px[ra + xb[x]];
                let c01 = self.px[rb + xa[x]];
                let c11 = self.px[rb + xb[x]];
                let w00 = (1.0 - fx) * (1.0 - wy);
                let w10 = fx * (1.0 - wy);
                let w01 = (1.0 - fx) * wy;
                let w11 = fx * wy;
                let ch = |sh: u32| -> u32 {
                    (((c00 >> sh) & 255) as f64 * w00
                        + ((c10 >> sh) & 255) as f64 * w10
                        + ((c01 >> sh) & 255) as f64 * w01
                        + ((c11 >> sh) & 255) as f64 * w11
                        + 0.5) as u32
                };
                out.push((ch(24) << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0));
            }
        }
        Img { w: tw, h: th, px: out }
    }
}

pub fn encode_png(w: usize, h: usize, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w as u32, h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Best);
        let mut wr = enc.write_header().unwrap();
        wr.write_image_data(rgba).unwrap();
    }
    out
}

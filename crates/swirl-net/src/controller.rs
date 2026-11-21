pub struct Obb {
    pub c: [f32; 3],
    pub r: [f32; 9],
    pub half: [f32; 3],
}

pub struct Controller {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub yaw: f32,
    pub grounded: bool,
    pub walk_speed: f32,
    pub jump_power: f32,
    pub state: u8,
}

pub struct Input {
    pub forward: f32,
    pub right: f32,
    pub jump: bool,
}

const GRAVITY: f32 = 196.2;
const RADIUS: f32 = 0.9;
const SPHERES: [f32; 3] = [-2.1, 0.0, 1.2];

impl Controller {
    pub fn new(pos: [f32; 3]) -> Controller {
        Controller { pos, vel: [0.0; 3], yaw: 0.0, grounded: false, walk_speed: 16.0, jump_power: 50.0, state: 3 }
    }

    pub fn teleport(&mut self, pos: [f32; 3]) {
        self.pos = pos;
        self.vel = [0.0; 3];
    }

    fn resolve(&mut self, boxes: &[Obb]) {
        for _ in 0..3 {
            for &dy in SPHERES.iter() {
                let c = [self.pos[0], self.pos[1] + dy, self.pos[2]];
                for b in boxes {
                    let d = [c[0] - b.c[0], c[1] - b.c[1], c[2] - b.c[2]];
                    let m = (b.half[0] + b.half[1] + b.half[2]) + RADIUS + 1.0;
                    if d[0].abs() > m || d[1].abs() > m || d[2].abs() > m {
                        continue;
                    }
                    let l = [
                        b.r[0] * d[0] + b.r[3] * d[1] + b.r[6] * d[2],
                        b.r[1] * d[0] + b.r[4] * d[1] + b.r[7] * d[2],
                        b.r[2] * d[0] + b.r[5] * d[1] + b.r[8] * d[2],
                    ];
                    let cl = [l[0].clamp(-b.half[0], b.half[0]), l[1].clamp(-b.half[1], b.half[1]), l[2].clamp(-b.half[2], b.half[2])];
                    let diff = [l[0] - cl[0], l[1] - cl[1], l[2] - cl[2]];
                    let dist = (diff[0] * diff[0] + diff[1] * diff[1] + diff[2] * diff[2]).sqrt();
                    let (nl, push) = if dist > 1e-6 {
                        if dist >= RADIUS {
                            continue;
                        }
                        ([diff[0] / dist, diff[1] / dist, diff[2] / dist], RADIUS - dist)
                    } else {
                        let mut best = 0;
                        let mut bd = f32::INFINITY;
                        for k in 0..3 {
                            let e = b.half[k] - l[k].abs();
                            if e < bd {
                                bd = e;
                                best = k;
                            }
                        }
                        let mut n = [0.0; 3];
                        n[best] = if l[best] >= 0.0 { 1.0 } else { -1.0 };
                        (n, bd + RADIUS)
                    };
                    let n = [
                        b.r[0] * nl[0] + b.r[1] * nl[1] + b.r[2] * nl[2],
                        b.r[3] * nl[0] + b.r[4] * nl[1] + b.r[5] * nl[2],
                        b.r[6] * nl[0] + b.r[7] * nl[1] + b.r[8] * nl[2],
                    ];
                    self.pos[0] += n[0] * push;
                    self.pos[1] += n[1] * push;
                    self.pos[2] += n[2] * push;
                    let vn = self.vel[0] * n[0] + self.vel[1] * n[1] + self.vel[2] * n[2];
                    if vn < 0.0 {
                        self.vel[0] -= vn * n[0];
                        self.vel[1] -= vn * n[1];
                        self.vel[2] -= vn * n[2];
                    }
                    if n[1] > 0.6 {
                        self.grounded = true;
                    }
                }
            }
        }
    }

    pub fn step(&mut self, dt: f32, input: &Input, cam_yaw: f32, boxes: &[Obb]) {
        let steps = ((dt / (1.0 / 120.0)).ceil() as i32).clamp(1, 12);
        let h = dt / steps as f32;
        let fwd = [-cam_yaw.sin(), -cam_yaw.cos()];
        let right = [-fwd[1], fwd[0]];
        for _ in 0..steps {
            let mut mx = fwd[0] * input.forward + right[0] * input.right;
            let mut mz = fwd[1] * input.forward + right[1] * input.right;
            let l = (mx * mx + mz * mz).sqrt();
            if l > 1.0 {
                mx /= l;
                mz /= l;
            }
            self.vel[0] = mx * self.walk_speed;
            self.vel[2] = mz * self.walk_speed;
            if l > 0.05 {
                self.yaw = (-mx).atan2(-mz);
            }
            let was = self.grounded;
            if input.jump && was {
                self.vel[1] = self.jump_power;
            }
            self.vel[1] -= GRAVITY * h;
            self.grounded = false;
            self.pos[0] += self.vel[0] * h;
            self.pos[1] += self.vel[1] * h;
            self.pos[2] += self.vel[2] * h;
            self.resolve(boxes);
        }
        self.state = if !self.grounded { if self.vel[1] > 0.0 { 2 } else { 3 } } else if input.forward != 0.0 || input.right != 0.0 { 1 } else { 0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor() -> Obb {
        Obb { c: [0.0, -0.6, 0.0], r: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0], half: [256.0, 0.6, 256.0] }
    }

    #[test]
    fn lands_and_walks() {
        let mut c = Controller::new([0.0, 10.0, 0.0]);
        let boxes = vec![floor()];
        for _ in 0..120 {
            c.step(1.0 / 60.0, &Input { forward: 0.0, right: 0.0, jump: false }, 0.0, &boxes);
        }
        assert!(c.grounded);
        assert!((c.pos[1] - 3.0).abs() < 0.2, "y {}", c.pos[1]);
        for _ in 0..60 {
            c.step(1.0 / 60.0, &Input { forward: 1.0, right: 0.0, jump: false }, 0.0, &boxes);
        }
        assert!(c.pos[2] < -10.0, "z {}", c.pos[2]);
        c.step(1.0 / 60.0, &Input { forward: 0.0, right: 0.0, jump: true }, 0.0, &boxes);
        assert!(c.vel[1] > 40.0 || !c.grounded);
    }
}

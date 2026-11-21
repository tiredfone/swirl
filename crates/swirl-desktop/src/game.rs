use std::collections::HashSet;
use swirl_net::controller::{Controller, Input};
use swirl_net::mat;
use swirl_net::world::{Part, World};

pub struct Frame {
    pub vp: mat::M4,
    pub parts: Vec<Part>,
}

pub struct Game {
    pub world: World,
    ctl: Option<Controller>,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    target: [f32; 3],
    fitted: bool,
    follow: bool,
    send_acc: f32,
    pub keys: HashSet<String>,
    pub out: Vec<String>,
}

impl Game {
    pub fn new() -> Game {
        Game { world: World::new(), ctl: None, yaw: 2.4, pitch: 0.5, dist: 60.0, target: [0.0, 5.0, 0.0], fitted: false, follow: false, send_acc: 0.0, keys: HashSet::new(), out: Vec::new() }
    }

    pub fn apply(&mut self, text: &str) {
        if let Some(kind) = self.world.apply(text) {
            if kind == "init" {
                self.fitted = false;
            }
        }
    }

    pub fn step(&mut self, dt: f32, aspect: f32) -> Option<Frame> {
        if !self.world.ready {
            return None;
        }
        if self.world.own.is_some() {
            if let Some(cf) = self.world.teleport.take() {
                let (ws, jp) = self.world.humanoid_of(self.world.own.as_ref().unwrap().model);
                let mut c = Controller::new([cf[0], cf[1], cf[2]]);
                c.walk_speed = ws;
                c.jump_power = jp;
                self.ctl = Some(c);
                if !self.follow {
                    self.follow = true;
                    self.dist = 14.0;
                    self.fitted = true;
                    self.pitch = 0.35;
                }
            }
            let boxes = self.world.colliders();
            let k = |s: &str| self.keys.contains(s);
            let input = Input {
                forward: (k("w") || k("arrowup")) as i32 as f32 - (k("s") || k("arrowdown")) as i32 as f32,
                right: (k("d") || k("arrowright")) as i32 as f32 - (k("a") || k("arrowleft")) as i32 as f32,
                jump: k(" "),
            };
            let yaw = self.yaw;
            if let Some(c) = self.ctl.as_mut() {
                c.step(dt, &input, yaw, &boxes);
                if c.pos[1] < -400.0 {
                    c.teleport([0.0, 10.0, 0.0]);
                }
            }
            if let Some(c) = self.ctl.as_ref() {
                let cf = mat::cf_yaw(c.pos, c.yaw);
                let (pos, vel, state) = (c.pos, c.vel, c.state);
                self.world.move_own(cf);
                self.target = [pos[0], pos[1] + 1.5, pos[2]];
                self.send_acc += dt;
                if self.send_acc >= 1.0 / 30.0 {
                    self.send_acc = 0.0;
                    let r = |v: f32| (v * 10000.0).round() / 10000.0;
                    self.out.push(format!(
                        "{{\"t\":\"char\",\"s\":{},\"c\":[{}],\"v\":[{},{},{}],\"st\":{}}}",
                        self.world.tp_seq,
                        cf.iter().map(|x| r(*x).to_string()).collect::<Vec<_>>().join(","),
                        (vel[0] * 100.0).round() / 100.0,
                        (vel[1] * 100.0).round() / 100.0,
                        (vel[2] * 100.0).round() / 100.0,
                        state
                    ));
                }
            }
        }
        if !self.fitted {
            let (c, r) = self.world.bounds();
            self.target = c;
            self.dist = r * 1.6;
            self.fitted = true;
        }
        let eye = [
            self.target[0] + self.dist * self.pitch.cos() * self.yaw.sin(),
            self.target[1] + self.dist * self.pitch.sin(),
            self.target[2] + self.dist * self.pitch.cos() * self.yaw.cos(),
        ];
        let proj = mat::perspective(1.0, aspect, 0.5, 20000.0);
        let view = mat::look_at(eye, self.target, [0.0, 1.0, 0.0]);
        let vp = mat::mul(&proj, &view);
        let parts = self.world.parts();
        let mut solid: Vec<Part> = parts.iter().filter(|p| p.alpha >= 0.99).cloned().collect();
        let mut glass: Vec<Part> = parts.iter().filter(|p| p.alpha < 0.99 && p.alpha > 0.01).cloned().collect();
        glass.sort_by(|a, b| {
            let da = (a.cf[0] - eye[0]).powi(2) + (a.cf[1] - eye[1]).powi(2) + (a.cf[2] - eye[2]).powi(2);
            let db = (b.cf[0] - eye[0]).powi(2) + (b.cf[1] - eye[1]).powi(2) + (b.cf[2] - eye[2]).powi(2);
            db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
        });
        solid.extend(glass);
        Some(Frame { vp, parts: solid })
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

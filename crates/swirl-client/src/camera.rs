use swirl_core::instance::{DataModel, InstId, Val};
use swirl_physics::cf;
use swirl_physics::Physics;

const PITCH: f64 = 80.0 * std::f64::consts::PI / 180.0;

pub struct Camera {
    pub yaw: f64,
    pub pitch: f64,
    pub zoom: f64,
    pub target_zoom: f64,
    pub first_person: bool,
    pub shift_lock: bool,
    pub locked: bool,
    pub lock_blend: f64,
    pub key_yaw: f64,
    pub cf: cf::CF,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera { yaw: 0.0, pitch: -0.35, zoom: 14.0, target_zoom: 14.0, first_person: false, shift_lock: false, locked: false, lock_blend: 0.0, key_yaw: 0.0, cf: [0.0, 20.0, 20.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] }
    }
}

fn num(d: &DataModel, id: InstId, k: &str) -> f64 {
    match d.get_prop(id, k) {
        Some(Val::Num(n)) => *n,
        _ => 0.0,
    }
}

fn transparent_collidable(d: &DataModel, p: InstId) -> bool {
    num(d, p, "Transparency") < 0.95 && matches!(d.get_prop(p, "CanCollide"), Some(Val::Bool(true)))
}

impl Camera {
    pub fn zoom_limits(&self, d: &DataModel) -> [f64; 2] {
        let (mut lo, mut hi) = (0.5, 400.0);
        if let Some(pl) = d.local_player.filter(|p| d.get(*p).map(|i| !i.destroyed).unwrap_or(false)) {
            let a = num(d, pl, "CameraMinZoomDistance");
            let b = num(d, pl, "CameraMaxZoomDistance");
            if a > 0.0 {
                lo = a;
            }
            if b > 0.0 {
                hi = lo.max(b);
            }
            if num(d, pl, "CameraMode") as i64 == 1 {
                hi = lo.min(0.5);
                lo = hi;
            }
        }
        [lo, hi]
    }

    pub fn step_zoom(&mut self, d: &DataModel, dir: i32) {
        let mut z = self.target_zoom;
        let [lo, hi] = self.zoom_limits(d);
        if dir > 0 {
            z = if z < 1.0 { 0.5 } else if z / 1.25 < 1.0 { 0.5 } else { z / 1.25 };
        } else {
            z = if z < 1.0 { 1.25 } else { z * 1.25 };
        }
        z = lo.max(hi.min(z));
        let from = self.target_zoom.max(1.0);
        let to = z.max(1.0);
        self.pitch = (-PITCH).max(PITCH.min(self.pitch - 0.15 * (to / from).ln()));
        self.target_zoom = z;
        self.zoom = z;
    }

    pub fn subject_pos(&self, d: &DataModel, own_model: Option<InstId>) -> Option<[f64; 3]> {
        let ws = d.services.get("Workspace").copied();
        let cam = ws.and_then(|w| match d.get_prop(w, "CurrentCamera") {
            Some(Val::Ref(Some(c))) => Some(*c),
            _ => None,
        });
        let mut part: Option<InstId> = None;
        let mut root: Option<InstId> = None;
        if let Some(Val::Ref(Some(subj))) = cam.and_then(|c| d.get_prop(c, "CameraSubject")) {
            let subj = *subj;
            if d.get(subj).map(|i| !i.destroyed).unwrap_or(false) {
                let class = d.get(subj).map(|i| i.class.clone()).unwrap_or_default();
                if class == "Humanoid" {
                    if let Some(par) = d.get(subj).and_then(|i| i.parent) {
                        root = d.find_child(par, "HumanoidRootPart", false).or_else(|| d.find_child(par, "Torso", false));
                        part = root.or_else(|| d.find_child(par, "Head", false));
                    }
                } else if d.is_a(subj, "BasePart") {
                    part = Some(subj);
                } else if class == "Model" {
                    part = match d.get_prop(subj, "PrimaryPart") {
                        Some(Val::Ref(Some(p))) => Some(*p),
                        _ => d.get(subj).and_then(|i| i.children.iter().copied().find(|c| d.is_a(*c, "BasePart"))),
                    };
                }
            }
        }
        if part.is_none() {
            if let Some(m) = own_model.filter(|m| d.get(*m).map(|i| !i.destroyed).unwrap_or(false)) {
                root = d.find_child(m, "HumanoidRootPart", false).or_else(|| d.find_child(m, "Torso", false));
                part = root.or_else(|| d.find_child(m, "Head", false));
            }
        }
        let p = part?;
        let c = match d.get_prop(p, "CFrame") {
            Some(Val::Arr(a)) if a.len() >= 3 => [a[0], a[1], a[2]],
            _ => return None,
        };
        Some([c[0], c[1] + if root.is_some() { 1.5 } else { 0.0 }, c[2]])
    }

    pub fn forward_flat(&self) -> [f64; 3] {
        [-self.yaw.sin(), 0.0, -self.yaw.cos()]
    }

    pub fn right_flat(&self) -> [f64; 3] {
        [self.yaw.cos(), 0.0, -self.yaw.sin()]
    }

    pub fn update(&mut self, d: &mut DataModel, ph: &Physics, own_model: Option<InstId>, dt: f64, pointer_locked: bool) -> cf::CF {
        let ws = d.services.get("Workspace").copied();
        let cam = match ws.and_then(|w| match d.get_prop(w, "CurrentCamera") {
            Some(Val::Ref(Some(c))) => Some(*c),
            _ => None,
        }) {
            Some(c) => c,
            None => return self.cf,
        };
        let ctype = num(d, cam, "CameraType") as i64;
        self.yaw += self.key_yaw * dt * 2.2;
        let zl = self.zoom_limits(d);
        self.target_zoom = zl[0].max(zl[1].min(self.target_zoom));
        self.zoom = self.target_zoom;
        let fp = self.zoom < 1.0;
        self.first_person = fp;
        if ctype == 6 || ctype == 0 {
            if let Some(Val::Arr(a)) = d.get_prop(cam, "CFrame") {
                if let Some(c) = cf::arr(a) {
                    self.cf = c;
                }
            }
        } else {
            let mut focus = self.subject_pos(d, own_model);
            let lock_on = focus.is_some() && self.shift_lock && !fp && pointer_locked;
            self.lock_blend += ((if lock_on { 1.0 } else { 0.0 }) - self.lock_blend) * (dt * 8.0).min(1.0);
            if !lock_on && self.lock_blend < 0.01 {
                self.lock_blend = 0.0;
            }
            let ignore: Vec<InstId> = own_model.into_iter().collect();
            if let Some(f) = focus.as_mut() {
                if self.lock_blend > 0.0 {
                    let rt = self.right_flat();
                    let amt = 1.75 * self.lock_blend;
                    let want = [f[0] + rt[0] * amt, f[1], f[2] + rt[2] * amt];
                    let hit = ph.raycast(d, *f, [want[0] - f[0], 0.0, want[2] - f[2]], &ignore, Some(&|p| transparent_collidable(d, p)));
                    let k = if hit.part.is_some() { (((hit.point[0] - f[0]).hypot(hit.point[2] - f[2]) - 0.3).max(0.0) / amt).min(1.0).max(0.0) } else { 1.0 };
                    f[0] += rt[0] * amt * k;
                    f[2] += rt[2] * amt * k;
                }
            }
            if let Some(f) = focus {
                let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
                let back = [self.yaw.sin() * cp, -sp, self.yaw.cos() * cp];
                let mut dist = self.zoom;
                if !fp {
                    let hit = ph.raycast(d, f, [back[0] * dist, back[1] * dist, back[2] * dist], &ignore, Some(&|p| transparent_collidable(d, p)));
                    if hit.part.is_some() {
                        let dd = ((hit.point[0] - f[0]).powi(2) + (hit.point[1] - f[1]).powi(2) + (hit.point[2] - f[2]).powi(2)).sqrt();
                        dist = 0.5f64.max(dd - 0.5);
                    }
                }
                let pos = [f[0] + back[0] * dist, f[1] + back[1] * dist, f[2] + back[2] * dist];
                self.cf = if fp { cf::look_at(f[0], f[1], f[2], f[0] - back[0], f[1] - back[1], f[2] - back[2]) } else { cf::look_at(pos[0], pos[1], pos[2], f[0], f[1], f[2]) };
                d.set_prop_quiet(cam, "CFrame", Val::Arr(self.cf.to_vec()));
                let mut fc = cf::IDENTITY;
                fc[0] = f[0];
                fc[1] = f[1];
                fc[2] = f[2];
                d.set_prop_quiet(cam, "Focus", Val::Arr(fc.to_vec()));
            }
        }
        self.cf
    }
}

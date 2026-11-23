use crate::cf::{self, CF};
use crate::world::{Physics, Special};
use indexmap::{IndexMap, IndexSet};
use rapier3d::control::{CharacterAutostep, CharacterCollision, CharacterLength, KinematicCharacterController};
use rapier3d::parry::query::{DefaultQueryDispatcher, ShapeCastOptions};
use rapier3d::parry::shape::Cuboid;
use rapier3d::prelude::*;
use std::collections::HashSet;
use swirl_core::instance::{DataModel, InstId, Val};

pub const IDLE: u8 = 0;
pub const WALK: u8 = 1;
pub const JUMP: u8 = 2;
pub const FALL: u8 = 3;
pub const SIT: u8 = 4;
pub const CLIMB: u8 = 5;
pub const STAND: u8 = 7;
pub const SWIM: u8 = 8;
const LANDED_DELAY: f64 = 0.5;
const CLIMB_PUSH: f64 = 8.0;
const CLIMB_PUSH_TIME: f64 = 0.3;
const CLIMB_REGRAB: f64 = 0.5;
const STEP: f32 = 1.0 / 60.0;

pub fn root_of(dm: &DataModel, model: InstId) -> Option<InstId> {
    dm.find_child(model, "Torso", false).or_else(|| dm.find_child(model, "HumanoidRootPart", false))
}

pub fn live_character_of(ph: &Physics, dm: &DataModel, part: InstId) -> Option<InstId> {
    let mut cur = dm.get(part).and_then(|i| i.parent);
    while let Some(m) = cur {
        let class = dm.get(m)?.class.clone();
        if class == "Model" || class == "Workspace" {
            if class == "Workspace" {
                return None;
            }
            let h = dm.find_child_of_class(m, "Humanoid");
            if let (Some(h), Some(torso)) = (h, root_of(dm, m)) {
                if dm.find_child(m, "Head", false).is_some() && Physics::get_num(dm, h, "Health", 100.0) > 0.0 && !ph.dead_chars.contains(&m) {
                    if Physics::get_bool(dm, torso, "Anchored", false) {
                        return None;
                    }
                    return Some(m);
                }
            }
            return None;
        }
        cur = dm.get(m).and_then(|i| i.parent);
    }
    None
}

pub fn flat_yaw(c: &CF) -> CF {
    let lv = cf::look_vector(c);
    let l = (lv[0] * lv[0] + lv[2] * lv[2]).sqrt();
    if l < 1e-6 {
        return [c[0], c[1], c[2], 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    }
    cf::look_at(c[0], c[1], c[2], c[0] + lv[0] / l, c[1], c[2] + lv[2] / l)
}

pub struct Rig {
    pub model: InstId,
    pub torso: Option<InstId>,
    pub order: Vec<(InstId, InstId, InstId, bool)>,
    pub parts: IndexSet<InstId>,
}

impl Rig {
    pub fn new(dm: &DataModel, model: InstId) -> Rig {
        let mut r = Rig { model, torso: None, order: vec![], parts: IndexSet::new() };
        r.build(dm);
        r
    }

    pub fn build(&mut self, dm: &DataModel) {
        let m = self.model;
        self.torso = root_of(dm, m);
        let mut edges: IndexMap<InstId, Vec<(InstId, InstId, bool)>> = IndexMap::new();
        let mut all = vec![];
        dm.descendants(m, &mut all);
        for j in all {
            if !dm.is_a(j, "JointInstance") {
                continue;
            }
            let a = match dm.get_prop(j, "Part0") {
                Some(Val::Ref(Some(p))) => *p,
                _ => continue,
            };
            let b = match dm.get_prop(j, "Part1") {
                Some(Val::Ref(Some(p))) => *p,
                _ => continue,
            };
            if a == b || !dm.is_descendant_of(a, m) || !dm.is_descendant_of(b, m) {
                continue;
            }
            edges.entry(a).or_default().push((j, b, true));
            edges.entry(b).or_default().push((j, a, false));
        }
        self.order.clear();
        self.parts.clear();
        let hrp = dm.find_child(m, "HumanoidRootPart", false);
        if let Some(h) = hrp {
            if dm.is_a(h, "BasePart") && edges.contains_key(&h) {
                let torso = dm.find_child(m, "Torso", false);
                if torso.is_none() || edges[&h].iter().any(|(_, o, _)| Some(*o) == torso) {
                    self.torso = Some(h);
                }
            }
        }
        let torso = match self.torso {
            Some(t) => t,
            None => return,
        };
        let mut seen: IndexSet<InstId> = IndexSet::new();
        seen.insert(torso);
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(torso);
        while let Some(p) = queue.pop_front() {
            if let Some(es) = edges.get(&p) {
                for (j, o, fwd) in es {
                    if seen.contains(o) {
                        continue;
                    }
                    seen.insert(*o);
                    self.order.push((p, *j, *o, *fwd));
                    queue.push_back(*o);
                }
            }
        }
        self.parts = seen;
    }

    pub fn pose(&self, ph: &Physics, dm: &DataModel, torso_cf: &CF) -> IndexMap<InstId, CF> {
        let mut out: IndexMap<InstId, CF> = IndexMap::new();
        let torso = match self.torso {
            Some(t) => t,
            None => return out,
        };
        out.insert(torso, *torso_cf);
        for (p, j, o, fwd) in &self.order {
            let base = match out.get(p) {
                Some(b) => *b,
                None => continue,
            };
            let c0 = match dm.get_prop(*j, "C0") {
                Some(Val::Arr(a)) => cf::arr(a).unwrap_or(cf::IDENTITY),
                _ => cf::IDENTITY,
            };
            let c1 = match dm.get_prop(*j, "C1") {
                Some(Val::Arr(a)) => cf::arr(a).unwrap_or(cf::IDENTITY),
                _ => cf::IDENTITY,
            };
            let is_motor = dm.is_a(*j, "Motor") || dm.get(*j).map(|i| i.class == "VelocityMotor").unwrap_or(false);
            let ang_v = if is_motor { Physics::get_num(dm, *j, "CurrentAngle", 0.0) } else { 0.0 };
            let ang = if ang_v != 0.0 { Some(cf::axis_angle(0.0, 0.0, 1.0, ang_v)) } else { None };
            let rot = match ph.anim_cf.get(j) {
                Some(a) => Some(match ang {
                    Some(g) => cf::mul(&g, a),
                    None => *a,
                }),
                None => ang,
            };
            let res = if *fwd {
                let mid = match &rot {
                    Some(r) => cf::mul(&c0, r),
                    None => c0,
                };
                cf::mul(&cf::mul(&base, &mid), &cf::inv(&c1))
            } else {
                let mid = match &rot {
                    Some(r) => cf::mul(&c1, &cf::inv(r)),
                    None => c1,
                };
                cf::mul(&cf::mul(&base, &mid), &cf::inv(&c0))
            };
            out.insert(*o, res);
        }
        out
    }
}

pub struct CharacterProxy {
    pub model: InstId,
    pub body: RigidBodyHandle,
    pub solid: Option<ColliderHandle>,
    pub limbs: IndexMap<InstId, ColliderHandle>,
    pub seated: bool,
    pub sensors: bool,
}

impl CharacterProxy {
    pub fn new(ph: &mut Physics, dm: &DataModel, model: InstId) -> CharacterProxy {
        Self::with_sensors(ph, dm, model, true)
    }

    pub fn with_sensors(ph: &mut Physics, dm: &DataModel, model: InstId, sensors: bool) -> CharacterProxy {
        let t0 = root_of(dm, model).map(|t| Physics::get_cf(dm, t)).unwrap_or(cf::IDENTITY);
        let body = RigidBodyBuilder::kinematic_position_based().position(pose_of(&t0)).build();
        let bh = ph.bodies.insert(body);
        let mut p = CharacterProxy { model, body: bh, solid: None, limbs: IndexMap::new(), seated: false, sensors };
        p.refresh(ph, dm);
        p
    }

    pub fn refresh(&mut self, ph: &mut Physics, dm: &DataModel) {
        for (_, h) in self.limbs.drain(..) {
            ph.special.remove(&h);
            ph.colliders.remove(h, &mut ph.islands, &mut ph.bodies, &mut ph.soft, false);
        }
        if let Some(h) = self.solid.take() {
            ph.special.remove(&h);
            ph.solid_proxies.remove(&h);
            ph.colliders.remove(h, &mut ph.islands, &mut ph.bodies, &mut ph.soft, false);
        }
        let torso = match root_of(dm, self.model) {
            Some(t) => t,
            None => return,
        };
        let solid = ColliderBuilder::cylinder(2.45, 0.9).translation(Vector::new(0.0, -0.5, 0.0)).enabled(!self.seated).active_hooks(ActiveHooks::FILTER_CONTACT_PAIRS).build();
        let h = ph.colliders.insert_with_parent(solid, self.body, &mut ph.bodies);
        ph.special.insert(h, Special { part: Some(torso), no_ray: true, character: Some(self.model), solid_proxy: true, sensor_only: false });
        ph.solid_proxies.insert(h);
        self.solid = Some(h);
        let (bt, br) = {
            let b = ph.bodies.get(self.body).unwrap();
            (b.translation(), *b.rotation())
        };
        let inv = cf::inv(&cf::from_quat(bt.x as f64, bt.y as f64, bt.z as f64, br.x as f64, br.y as f64, br.z as f64, br.w as f64));
        let mut all = vec![];
        dm.descendants(self.model, &mut all);
        for p in all {
            if !dm.is_a(p, "BasePart") {
                continue;
            }
            let s = Physics::get_v3(dm, p, "Size", [1.0; 3]);
            let pad = if self.sensors { 0.12 } else { 0.0 };
            let local = cf::mul(&inv, &Physics::get_cf(dm, p));
            let c = ColliderBuilder::cuboid((s[0] / 2.0 + pad) as f32, (s[1] / 2.0 + pad) as f32, (s[2] / 2.0 + pad) as f32)
                .position(pose_of(&local))
                .sensor(true)
                .active_collision_types(ActiveCollisionTypes::all())
                .active_events(if self.sensors { ActiveEvents::COLLISION_EVENTS } else { ActiveEvents::empty() })
                .build();
            let ch = ph.colliders.insert_with_parent(c, self.body, &mut ph.bodies);
            self.limbs.insert(p, ch);
            ph.special.insert(ch, Special { part: Some(p), no_ray: false, character: Some(self.model), solid_proxy: false, sensor_only: true });
        }
    }

    pub fn set_seated(&mut self, ph: &mut Physics, on: bool) {
        self.seated = on;
        if let Some(c) = self.solid.and_then(|h| ph.colliders.get_mut(h)) {
            c.set_enabled(!on);
        }
    }

    pub fn update(&mut self, ph: &mut Physics, dm: &DataModel, torso_cf: &CF, poses: &IndexMap<InstId, CF>) {
        if let Some(b) = ph.bodies.get_mut(self.body) {
            let p = pose_of(torso_cf);
            b.set_next_kinematic_position(p);
        }
        let inv = cf::inv(torso_cf);
        for (p, h) in &self.limbs {
            let c = poses.get(p).copied().unwrap_or_else(|| Physics::get_cf(dm, *p));
            let local = cf::mul(&inv, &c);
            if let Some(col) = ph.colliders.get_mut(*h) {
                col.set_position_wrt_parent(pose_of(&local));
            }
        }
    }

    pub fn destroy(&mut self, ph: &mut Physics) {
        for (_, h) in self.limbs.drain(..) {
            ph.special.remove(&h);
        }
        if let Some(h) = self.solid.take() {
            ph.special.remove(&h);
            ph.solid_proxies.remove(&h);
        }
        ph.bodies.remove(self.body, &mut ph.islands, &mut ph.colliders, &mut ph.joints_set, &mut ph.multibody, &mut ph.soft, true);
    }
}

pub fn pose_of(c: &CF) -> Pose {
    let q = cf::to_quat(c);
    Pose::from_parts(Vector::new(c[0] as f32, c[1] as f32, c[2] as f32), Rotation::from_xyzw(q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32))
}

pub struct Input {
    pub mv: [f64; 3],
    pub jump: bool,
    pub face: Option<[f64; 3]>,
}

pub struct Step {
    pub cf: CF,
    pub vel: [f64; 3],
    pub state: u8,
}

pub struct Controller {
    pub model: InstId,
    pub hum: InstId,
    off: f64,
    pub vel: [f64; 3],
    pub state: u8,
    grounded: bool,
    yaw_cf: CF,
    pub body: RigidBodyHandle,
    pub collider: ColliderHandle,
    jump_cooldown: f64,
    landed_for: f64,
    hh: f64,
    cy: f64,
    kcc: KinematicCharacterController,
    pub pos: [f64; 3],
    rot: [f32; 4],
    rot_cf: CF,
    pass: HashSet<ColliderHandle>,
    jump_held_for: f64,
    climb_block: f64,
    climb_dir: Option<[f64; 3]>,
    climb_off: Option<([f64; 3], f64)>,
    ledge_top: Option<f64>,
    jumped: bool,
    since_ground: f64,
    air_for: f64,
    slope_deg: f64,
    drop: f64,
    lift_age: f64,
    lift_value: Option<f64>,
}

impl Controller {
    pub fn new(ph: &mut Physics, dm: &DataModel, model: InstId, hum: InstId) -> Controller {
        let torso = root_of(dm, model).unwrap();
        let c = Physics::get_cf(dm, torso);
        let off = if dm.find_child(model, "Torso", false).is_some() { 0.5 } else { -0.1 };
        let pos = [c[0], c[1] - off, c[2]];
        let body = RigidBodyBuilder::kinematic_position_based().translation(Vector::new(pos[0] as f32, pos[1] as f32, pos[2] as f32)).build();
        let bh = ph.bodies.insert(body);
        let col = ColliderBuilder::cuboid(1.0, 2.45, 0.5).active_collision_types(ActiveCollisionTypes::all()).build();
        let ch = ph.colliders.insert_with_parent(col, bh, &mut ph.bodies);
        ph.special.insert(ch, Special { part: Some(torso), no_ray: true, character: Some(model), solid_proxy: false, sensor_only: false });
        let mut kcc = KinematicCharacterController::default();
        kcc.offset = CharacterLength::Absolute(0.04);
        kcc.up = Vector::new(0.0, 1.0, 0.0);
        kcc.max_slope_climb_angle = (60.0f64 * std::f64::consts::PI / 180.0) as f32;
        kcc.min_slope_slide_angle = (75.0f64 * std::f64::consts::PI / 180.0) as f32;
        kcc.autostep = Some(CharacterAutostep { max_height: CharacterLength::Absolute(1.3), min_width: CharacterLength::Absolute(0.2), include_dynamic_bodies: true });
        kcc.snap_to_ground = Some(CharacterLength::Absolute(0.6));
        kcc.slide = true;
        kcc.normal_nudge_factor = 0.01;
        Controller {
            model,
            hum,
            off,
            vel: [0.0; 3],
            state: FALL,
            grounded: false,
            yaw_cf: flat_yaw(&c),
            body: bh,
            collider: ch,
            jump_cooldown: 0.0,
            landed_for: 0.0,
            hh: 2.45,
            cy: 0.0,
            kcc,
            pos,
            rot: [0.0, 0.0, 0.0, 1.0],
            rot_cf: cf::IDENTITY,
            pass: HashSet::new(),
            jump_held_for: 0.0,
            climb_block: 0.0,
            climb_dir: None,
            climb_off: None,
            ledge_top: None,
            jumped: false,
            since_ground: 0.0,
            air_for: 0.0,
            slope_deg: -1.0,
            drop: 0.0,
            lift_age: 0.0,
            lift_value: None,
        }
    }

    pub fn destroy(&mut self, ph: &mut Physics) {
        ph.special.remove(&self.collider);
        ph.bodies.remove(self.body, &mut ph.islands, &mut ph.colliders, &mut ph.joints_set, &mut ph.multibody, &mut ph.soft, true);
    }

    pub fn teleport(&mut self, ph: &mut Physics, dm: &DataModel, c: &CF) {
        self.pos = [c[0], c[1] - self.off, c[2]];
        if let Some(b) = ph.bodies.get_mut(self.body) {
            b.set_translation(Vector::new(self.pos[0] as f32, self.pos[1] as f32, self.pos[2] as f32), true);
        }
        self.yaw_cf = flat_yaw(c);
        self.vel = [0.0; 3];
        if dm.era == 2018 {
            let yaw = cf::rot_only(&self.yaw_cf);
            let q = cf::to_quat(&yaw);
            self.rot = [q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32];
            self.rot_cf = yaw;
            if let Some(b) = ph.bodies.get_mut(self.body) {
                b.set_rotation(Rotation::from_xyzw(self.rot[0], self.rot[1], self.rot[2], self.rot[3]), true);
            }
        }
    }

    fn e18(dm: &DataModel) -> bool {
        dm.era == 2018
    }

    fn foot(&self, dm: &DataModel, dx: f64, dz: f64) -> [f64; 2] {
        if !Self::e18(dm) {
            return [dx, dz];
        }
        let v = cf::vector(&self.rot_cf, dx, 0.0, dz);
        [v[0], v[2]]
    }

    fn body_lift(&mut self, dm: &DataModel, dt: f64) -> f64 {
        self.lift_age += dt;
        if self.lift_age < 0.1 {
            if let Some(v) = self.lift_value {
                return v;
            }
        }
        self.lift_age = 0.0;
        let mut fy = 0.0;
        let mut mass = 0.0;
        let mut forces = false;
        let mut all = vec![];
        dm.descendants(self.model, &mut all);
        for d in all {
            let class = dm.get(d).map(|i| i.class.clone()).unwrap_or_default();
            if class == "BodyForce" {
                let f = Physics::get_v3(dm, d, "Force", [0.0; 3]);
                if dm.get(d).and_then(|i| i.parent).map(|p| dm.is_a(p, "BasePart")).unwrap_or(false) {
                    fy += f[1];
                    forces = true;
                }
            } else if dm.is_a(d, "BasePart") {
                let s = Physics::get_v3(dm, d, "Size", [1.0; 3]);
                mass += s[0] * s[1] * s[2] * 0.7;
            }
        }
        let v = if forces && mass > 0.0 { fy / mass } else { 0.0 };
        self.lift_value = Some(v);
        v
    }

    fn pass_ok(&self, ph: &Physics, dm: &DataModel, h: ColliderHandle) -> bool {
        if h == self.collider || self.pass.contains(&h) {
            return false;
        }
        let own = root_of(dm, self.model).or_else(|| dm.find_child(self.model, "HumanoidRootPart", false));
        let own_group = dm.find_child(self.model, "HumanoidRootPart", false).or_else(|| dm.find_child(self.model, "Torso", false)).map(|r| Physics::get_num(dm, r, "CollisionGroupId", 0.0) as i32).unwrap_or(0);
        let _ = own;
        if ph.terrain_handles.contains(&h) {
            return true;
        }
        let (part, character, sensor_only) = if let Some(p) = ph.by_handle.get(&h) {
            (Some(*p), None, false)
        } else if let Some(sp) = ph.special.get(&h) {
            (sp.part, sp.character, sp.sensor_only)
        } else {
            return true;
        };
        if character == Some(self.model) {
            return false;
        }
        if let Some(p) = part {
            if p == self.model || dm.is_descendant_of(p, self.model) {
                return false;
            }
        }
        if sensor_only {
            return false;
        }
        if let Some(p) = part {
            if !Physics::get_bool(dm, p, "CanCollide", true) {
                return false;
            }
            if !ph.groups_collide(own_group, Physics::get_num(dm, p, "CollisionGroupId", 0.0) as i32) {
                return false;
            }
        }
        true
    }

    fn intersects(&self, ph: &Physics, dm: &DataModel, shape: &Cuboid, pose: Pose, skip_proxies: bool) -> bool {
        let pred = |h: ColliderHandle, _c: &Collider| -> bool {
            if skip_proxies && ph.solid_proxies.contains(&h) {
                return false;
            }
            self.pass_ok(ph, dm, h)
        };
        let filter = QueryFilter { flags: QueryFilterFlags::EXCLUDE_SENSORS, groups: None, exclude_collider: Some(self.collider), exclude_rigid_body: Some(self.body), predicate: Some(&pred) };
        let qp = ph.broad.as_query_pipeline(&DefaultQueryDispatcher, &ph.bodies, &ph.colliders, filter);
        let found = qp.intersect_shape(pose, shape).next().is_some();
        found
    }

    fn probe_inner(&self) -> Cuboid {
        Cuboid::new(Vector::new(0.95, (self.hh - 0.05) as f32, 0.45))
    }

    fn rot_rotation(&self) -> Rotation {
        Rotation::from_xyzw(self.rot[0], self.rot[1], self.rot[2], self.rot[3])
    }

    fn crouch_top(&self, dm: &DataModel) -> Option<f64> {
        if !Self::e18(dm) {
            return None;
        }
        let mut top = f64::NEG_INFINITY;
        for n in ["Head", "Torso", "UpperTorso"] {
            if let Some(p) = dm.find_child(self.model, n, false) {
                let c = Physics::get_cf(dm, p);
                let s = Physics::get_v3(dm, p, "Size", [1.0; 3]);
                top = top.max(c[1] + (c[6].abs() * s[0] + c[7].abs() * s[1] + c[8].abs() * s[2]) / 2.0);
            }
        }
        if top.is_finite() { Some(top - (self.pos[1] - 2.45)) } else { None }
    }

    fn crouch_fit(&mut self, ph: &mut Physics, dm: &DataModel) {
        let top = self.crouch_top(dm);
        let mut want = 2.45;
        if let Some(t) = top {
            if t < 4.5 && self.grounded {
                want = ((t + 0.02) / 2.0).min(2.45).max(1.05);
            }
        }
        if (want - self.hh).abs() < 0.02 {
            return;
        }
        if want > self.hh {
            let shape = Cuboid::new(Vector::new(0.95, (want - 0.05) as f32, 0.45));
            let pose = Pose::from_parts(Vector::new(self.pos[0] as f32, (self.pos[1] + want - 2.45) as f32, self.pos[2] as f32), self.rot_rotation());
            if self.intersects(ph, dm, &shape, pose, false) {
                return;
            }
        }
        self.hh = want;
        self.cy = want - 2.45;
        if let Some(c) = ph.colliders.get_mut(self.collider) {
            c.set_shape(SharedShape::cuboid(1.0, want as f32, 0.5));
            c.set_position_wrt_parent(Pose::from_parts(Vector::new(0.0, self.cy as f32, 0.0), Rotation::IDENTITY));
        }
    }

    fn turn_box(&mut self, ph: &mut Physics, dm: &DataModel) {
        if !Self::e18(dm) {
            return;
        }
        let yaw = cf::rot_only(&self.yaw_cf);
        let q = cf::to_quat(&yaw);
        let rot = [q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32];
        let d = (rot[0] * self.rot[0] + rot[1] * self.rot[1] + rot[2] * self.rot[2] + rot[3] * self.rot[3]).abs();
        if d > 0.99999 {
            return;
        }
        let shape = self.probe_inner();
        let pose = Pose::from_parts(Vector::new(self.pos[0] as f32, (self.pos[1] + self.cy + 0.1) as f32, self.pos[2] as f32), Rotation::from_xyzw(rot[0], rot[1], rot[2], rot[3]));
        if self.intersects(ph, dm, &shape, pose, true) {
            return;
        }
        self.rot = rot;
        self.rot_cf = yaw;
        if let Some(b) = ph.bodies.get_mut(self.body) {
            b.set_next_kinematic_rotation(Rotation::from_xyzw(rot[0], rot[1], rot[2], rot[3]).normalize());
        }
    }

    fn climb_check(&mut self, ph: &Physics, dm: &DataModel, dir: [f64; 3]) -> bool {
        if (dir[0] * dir[0] + dir[2] * dir[2]).sqrt() < 0.1 {
            return false;
        }
        let origin = [self.pos[0], self.pos[1] - 1.5, self.pos[2]];
        for dy in [-0.8, 0.0, 1.5, 3.0] {
            let o = [origin[0], origin[1] + dy, origin[2]];
            let f = |q: InstId| live_character_of(ph, dm, q).is_none();
            let r = ph.raycast(dm, o, [dir[0] * 1.6, 0.0, dir[2] * 1.6], &[self.model], Some(&f));
            if let Some(p) = r.part {
                if dm.get(p).map(|i| i.class == "TrussPart").unwrap_or(false) {
                    self.ledge_top = None;
                    return true;
                }
            }
        }
        self.ledge_ahead(ph, dm, dir)
    }

    fn ledge_ahead(&mut self, ph: &Physics, dm: &DataModel, dir: [f64; 3]) -> bool {
        if self.hh < 2.2 {
            return false;
        }
        let reach = 1.7;
        let foot = self.pos[1] - 2.5;
        let mut prev = f64::INFINITY;
        let min_rise = if self.state == CLIMB && self.ledge_top.is_some() { 0.3 } else { 2.6 };
        let mut last_hit = f64::NEG_INFINITY;
        let mut ds: Vec<f64> = vec![];
        for k in 0..=22 {
            let y = foot + k as f64 * 0.25;
            let f = |q: InstId| dm.get(q).map(|i| i.class != "Terrain").unwrap_or(false) && Physics::get_bool(dm, q, "CanCollide", true) && live_character_of(ph, dm, q).is_none();
            let r = ph.raycast(dm, [self.pos[0], y, self.pos[2]], [dir[0] * reach, 0.0, dir[2] * reach], &[self.model], Some(&f));
            let facing = r.part.is_some() && r.normal[1].abs() < 0.35 && r.normal[0] * dir[0] + r.normal[2] * dir[2] < -0.5;
            ds.push(if facing { ((r.point[0] - self.pos[0]).powi(2) + (r.point[2] - self.pos[2]).powi(2)).sqrt() } else { f64::INFINITY });
        }
        let (mut seen, mut gap, mut rungs) = (false, 0, false);
        for d in &ds {
            if *d < f64::INFINITY {
                if seen && gap >= 3 {
                    rungs = true;
                }
                seen = true;
                gap = 0;
            } else if seen {
                gap += 1;
            }
        }
        for k in 0..=22 {
            let y = foot + k as f64 * 0.25;
            let d = ds[k];
            if d < f64::INFINITY {
                last_hit = y;
            }
            if k as f64 * 0.25 >= min_rise && prev <= reach && d > prev + 0.8 {
                self.ledge_top = Some(last_hit + 0.25);
                return true;
            }
            prev = d;
        }
        if rungs {
            self.ledge_top = None;
            return true;
        }
        false
    }

    fn unstick(&mut self, ph: &mut Physics, dm: &DataModel) -> bool {
        let inner = self.probe_inner();
        let pose = Pose::from_parts(Vector::new(self.pos[0] as f32, (self.pos[1] + self.cy) as f32, self.pos[2] as f32), self.rot_rotation());
        if !self.intersects(ph, dm, &inner, pose, true) {
            return false;
        }
        let lift = 3.2;
        let outer = Cuboid::new(Vector::new(1.0, self.hh as f32, 0.5));
        let from = Pose::from_parts(Vector::new(self.pos[0] as f32, (self.pos[1] + self.cy + lift) as f32, self.pos[2] as f32), self.rot_rotation());
        let pred = |h: ColliderHandle, _c: &Collider| -> bool { !ph.solid_proxies.contains(&h) && self.pass_ok(ph, dm, h) };
        let filter = QueryFilter { flags: QueryFilterFlags::EXCLUDE_SENSORS, groups: None, exclude_collider: Some(self.collider), exclude_rigid_body: Some(self.body), predicate: Some(&pred) };
        let qp = ph.broad.as_query_pipeline(&DefaultQueryDispatcher, &ph.bodies, &ph.colliders, filter);
        let hit = qp.cast_shape(&from, Vector::new(0.0, -1.0, 0.0), &outer, ShapeCastOptions { max_time_of_impact: lift as f32, target_distance: 0.0, stop_at_penetration: true, compute_impact_geometry_on_penetration: true });
        let toi = match hit {
            Some((_, h)) if h.time_of_impact > 0.0 => h.time_of_impact as f64,
            _ => return false,
        };
        let y = self.pos[1] + self.cy + lift - self.cy - toi + 0.05;
        let y = (from.translation.y as f64) - self.cy - toi + 0.05;
        let _ = y;
        let y = (self.pos[1] + self.cy + lift) - self.cy - toi + 0.05;
        if y <= self.pos[1] {
            return false;
        }
        self.pos[1] = y;
        if let Some(b) = ph.bodies.get_mut(self.body) {
            b.set_translation(Vector::new(self.pos[0] as f32, self.pos[1] as f32, self.pos[2] as f32), true);
        }
        if self.vel[1] < 0.0 {
            self.vel[1] = 0.0;
        }
        true
    }

    fn separate(&mut self, ph: &Physics, dm: &DataModel, dt: f64) -> [f64; 2] {
        let inner = self.probe_inner();
        let pose = Pose::from_parts(Vector::new(self.pos[0] as f32, (self.pos[1] + self.cy) as f32, self.pos[2] as f32), self.rot_rotation());
        let mut skip: HashSet<ColliderHandle> = HashSet::new();
        let mut push = [0.0f64; 2];
        {
            let pred = |h: ColliderHandle, _c: &Collider| -> bool { h != self.collider };
            let filter = QueryFilter { flags: QueryFilterFlags::EXCLUDE_SENSORS, groups: None, exclude_collider: Some(self.collider), exclude_rigid_body: Some(self.body), predicate: Some(&pred) };
            let qp = ph.broad.as_query_pipeline(&DefaultQueryDispatcher, &ph.bodies, &ph.colliders, filter);
            for (h, c) in qp.intersect_shape(pose, &inner) {
                let sp = match ph.special.get(&h) {
                    Some(s) if s.solid_proxy && s.character != Some(self.model) => *s,
                    _ => continue,
                };
                let _ = sp;
                skip.insert(h);
                let t = c.translation();
                let (mut dx, mut dz) = (self.pos[0] - t.x as f64, self.pos[2] - t.z as f64);
                let mut l = (dx * dx + dz * dz).sqrt();
                if l < 1e-3 {
                    dx = 1.0;
                    dz = 0.0;
                    l = 1.0;
                }
                let k = (2.2 - l).max(0.0) / 2.2;
                push[0] += dx / l * k;
                push[1] += dz / l * k;
            }
        }
        let _ = dm;
        self.pass = skip;
        if self.pass.is_empty() {
            return [0.0, 0.0];
        }
        [push[0] * 10.0 * dt, push[1] * 10.0 * dt]
    }

    fn character_mover(&self, dm: &DataModel) -> Option<InstId> {
        for name in ["HumanoidRootPart", "Torso", "UpperTorso"] {
            if let Some(part) = dm.find_child(self.model, name, false) {
                for c in dm.get(part)?.children.clone() {
                    if dm.get(c).map(|i| i.class == "BodyVelocity" && !i.destroyed).unwrap_or(false) {
                        return Some(c);
                    }
                }
            }
        }
        None
    }

    fn feet_on_ground(&self, ph: &Physics, dm: &DataModel) -> bool {
        let p = self.pos;
        let from = p[1] - 2.3;
        for (ox, oz) in [(0.0, 0.0), (0.9, 0.4), (-0.9, 0.4), (0.9, -0.4), (-0.9, -0.4)] {
            let d = self.foot(dm, ox, oz);
            let f = |q: InstId| Physics::get_bool(dm, q, "CanCollide", true);
            let r = ph.raycast(dm, [p[0] + d[0], from, p[2] + d[1]], [0.0, -0.55, 0.0], &[self.model], Some(&f));
            if r.part.is_some() && r.normal[1] > 0.5 {
                return true;
            }
        }
        false
    }

    fn ground_clamp(&mut self, ph: &Physics, dm: &DataModel) -> bool {
        let p = self.pos;
        let from = p[1] - 2.15;
        let mut top = f64::NEG_INFINITY;
        for (ox, oz) in [(0.0, 0.0), (0.95, 0.45), (-0.95, 0.45), (0.95, -0.45), (-0.95, -0.45)] {
            let d = self.foot(dm, ox, oz);
            let f = |q: InstId| Physics::get_bool(dm, q, "CanCollide", true);
            let r = ph.raycast(dm, [p[0] + d[0], from, p[2] + d[1]], [0.0, -0.6, 0.0], &[self.model], Some(&f));
            if r.part.is_none() || r.normal[1] < 0.99 {
                return false;
            }
            top = top.max(r.point[1]);
        }
        let y = top + 2.49;
        if (y - p[1]).abs() > 0.15 {
            return false;
        }
        self.pos[1] = y;
        true
    }

    fn apply_slope(&mut self, dm: &DataModel) {
        let e18 = Self::e18(dm);
        let a = if e18 { Physics::get_num(dm, self.hum, "MaxSlopeAngle", 89.0).max(0.0).min(89.0) } else { 60.0 };
        let deg = if a.is_finite() { a } else if e18 { 89.0 } else { 60.0 };
        if deg == self.slope_deg {
            return;
        }
        self.slope_deg = deg;
        self.kcc.max_slope_climb_angle = (deg * std::f64::consts::PI / 180.0) as f32;
        self.kcc.min_slope_slide_angle = (75.0f64.max(deg + 0.5) * std::f64::consts::PI / 180.0) as f32;
    }

    pub fn update(&mut self, ph: &mut Physics, dm: &mut DataModel, dt: f64, input: &Input) -> Step {
        let hum = self.hum;
        let e18 = Self::e18(dm);
        self.crouch_fit(ph, dm);
        self.apply_slope(dm);
        self.unstick(ph, dm);
        let ws = Physics::get_num(dm, hum, "WalkSpeed", 16.0);
        let mv = input.mv;
        let ml = (mv[0] * mv[0] + mv[2] * mv[2]).sqrt();
        let dir = if ml > 1e-3 { [mv[0] / ml, 0.0, mv[2] / ml] } else { [0.0; 3] };
        let platform = Physics::get_bool(dm, hum, "PlatformStand", false);
        let speed = if platform { 0.0 } else { ws };
        self.vel[0] = dir[0] * speed;
        self.vel[2] = dir[2] * speed;
        let jump_prop = Physics::get_bool(dm, hum, "Jump", false);
        if (input.jump || jump_prop) && !platform {
            self.jump_held_for = if e18 { 0.05 } else { 0.3 };
        } else {
            self.jump_held_for = (self.jump_held_for - dt).max(0.0);
        }
        let want_jump = self.jump_held_for > 0.0 && !platform;
        if jump_prop {
            let _ = dm.set_prop(hum, "Jump", Val::Bool(false));
        }
        self.climb_block = (self.climb_block - dt).max(0.0);
        let jump_power = |dm: &DataModel| -> f64 {
            let j = Physics::get_num(dm, hum, "JumpPower", 50.0);
            if j == 0.0 { 50.0 } else { j }
        };
        if !platform && want_jump && self.state == CLIMB && self.climb_dir.is_some() {
            let cd = self.climb_dir.unwrap();
            self.climb_off = Some(([-cd[0], 0.0, -cd[2]], CLIMB_PUSH_TIME));
            self.climb_block = CLIMB_REGRAB;
            self.vel[1] = jump_power(dm) * 0.8;
            self.grounded = false;
            self.jumped = true;
            self.jump_held_for = 0.0;
            self.ledge_top = None;
            self.state = JUMP;
        }
        if let Some((d, t)) = self.climb_off {
            if t > 0.0 {
                let push = CLIMB_PUSH.max(ws * 0.6);
                self.vel[0] = d[0] * push;
                self.vel[2] = d[2] * push;
                self.climb_off = Some((d, t - dt));
            }
        }
        let can_climb = !platform && self.climb_block <= 0.0;
        let moving = can_climb && self.climb_check(ph, dm, dir);
        let dotc = match self.climb_dir {
            Some(cd) if ml > 1e-3 => dir[0] * cd[0] + dir[2] * cd[2],
            _ => 0.0,
        };
        let holding = !moving && can_climb && !self.grounded && self.state == CLIMB && self.climb_dir.is_some() && (ml <= 1e-3 || dotc <= 0.5) && {
            let cd = self.climb_dir.unwrap();
            self.climb_check(ph, dm, cd)
        };
        let climbing = moving || holding;
        let mut fell = false;
        let swimming = false;
        if holding {
            let cd = self.climb_dir.unwrap();
            self.vel[1] = if dotc < -0.5 { -ws * 0.7 } else { 0.0 };
            self.vel[0] = if ml > 1e-3 { (dir[0] - cd[0] * dotc) * ws * 0.7 } else { 0.0 };
            self.vel[2] = if ml > 1e-3 { (dir[2] - cd[2] * dotc) * ws * 0.7 } else { 0.0 };
            self.state = CLIMB;
        } else if climbing {
            self.vel[1] = ws * 0.7;
            self.state = CLIMB;
            self.climb_dir = Some(dir);
        } else if self.state == CLIMB && ml > 1e-3 && !platform && self.climb_dir.map(|cd| dir[0] * cd[0] + dir[2] * cd[2] > 0.5).unwrap_or(false) {
            let ledge_hop = self.ledge_top.is_some();
            let rise = if ledge_hop { 2.4f64.min((self.ledge_top.unwrap() - (self.pos[1] - 2.5) + 0.2).max(0.3)) } else { 2.4 };
            let up = (2.0 * ph.gravity() * rise).sqrt();
            self.vel[1] = if ledge_hop { up } else { self.vel[1].max(up) };
            self.ledge_top = None;
        } else {
            let lift = self.body_lift(dm, dt);
            self.vel[1] -= (ph.gravity() - lift) * dt;
            fell = true;
            self.jump_cooldown = (self.jump_cooldown - dt).max(0.0);
            let footing = if e18 { (self.grounded && self.feet_on_ground(ph, dm)) || (!self.jumped && self.since_ground < 0.12) } else { self.grounded || (!self.jumped && self.since_ground < 0.12) };
            if footing && want_jump && (e18 || self.landed_for > LANDED_DELAY) && self.jump_cooldown <= 0.0 && self.vel[1] <= 0.0 {
                self.vel[1] = jump_power(dm) - ph.gravity() * dt;
                self.grounded = false;
                self.jumped = true;
                self.landed_for = 0.0;
                self.jump_cooldown = if e18 { 0.08 } else { 0.35 };
                self.jump_held_for = 0.0;
            }
        }
        let bv = if platform { None } else { self.character_mover(dm) };
        if let Some(b) = bv {
            let tv = Physics::get_v3(dm, b, "Velocity", [0.0; 3]);
            let mf = Physics::get_v3(dm, b, "MaxForce", [0.0; 3]);
            for k in 0..3 {
                if !(mf[k] > 0.0) {
                    continue;
                }
                let acc = (mf[k] / 9.8) * dt;
                self.vel[k] += (tv[k] - self.vel[k]).min(acc).max(-acc);
            }
            if self.vel[1] > 0.0 {
                self.grounded = false;
            }
        }
        let settle = self.grounded && self.vel[1] <= 0.0 && !climbing && !swimming;
        let sep = self.separate(ph, dm, dt);
        let wall_hold = moving && self.ledge_top.is_some();
        let gravity = ph.gravity();
        let desired = [
            (if wall_hold { 0.0 } else { self.vel[0] * dt }) + sep[0],
            if settle { 0.0 } else { (self.vel[1] + if fell { 0.5 * gravity * dt } else { 0.0 }) * dt },
            (if wall_hold { 0.0 } else { self.vel[2] * dt }) + sep[1],
        ];
        let (mvd, grounded0) = self.compute_movement(ph, dm, desired);
        let mut grounded = grounded0;
        self.pos = [self.pos[0] + mvd[0], self.pos[1] + mvd[1], self.pos[2] + mvd[2]];
        if settle && self.ground_clamp(ph, dm) {
            grounded = true;
        }
        if let Some(b) = ph.bodies.get_mut(self.body) {
            b.set_next_kinematic_translation(Vector::new(self.pos[0] as f32, self.pos[1] as f32, self.pos[2] as f32));
        }
        if grounded && self.vel[1] <= 0.0 {
            self.vel[1] = 0.0;
        }
        if !grounded && mvd[1] > desired[1] + 1e-4 && self.vel[1] < 0.0 {
            self.vel[1] = self.vel[1].max(mvd[1] / dt);
        }
        if self.vel[1] > 0.0 && mvd[1] < desired[1] - 1e-4 {
            self.vel[1] = 0.0;
        }
        self.grounded = grounded;
        if grounded && self.vel[1] <= 0.0 && (!self.jumped || !e18 || self.feet_on_ground(ph, dm)) {
            self.since_ground = 0.0;
            self.jumped = false;
        } else {
            self.since_ground += dt;
        }
        self.air_for = if grounded && self.vel[1] <= 0.0 { 0.0 } else { self.air_for + dt };
        self.landed_for = if grounded && self.vel[1] <= 0.0 {
            self.landed_for + dt
        } else if self.air_for > 0.15 || self.jumped {
            0.0
        } else {
            self.landed_for
        };
        if swimming {
            self.state = SWIM;
        } else if !climbing {
            if grounded {
                self.state = if ml > 1e-3 && speed > 0.0 { WALK } else { IDLE };
            } else {
                self.state = if self.vel[1] > 0.0 { JUMP } else { FALL };
            }
        }
        if platform {
            self.state = STAND;
        }
        if climbing && self.climb_dir.is_some() && !platform {
            let cd = self.climb_dir.unwrap();
            self.yaw_cf = cf::look_at(0.0, 0.0, 0.0, cd[0], 0.0, cd[2]);
        } else if let Some(f) = input.face.filter(|f| (f[0] * f[0] + f[2] * f[2]).sqrt() > 1e-3 && !platform) {
            self.yaw_cf = cf::look_at(0.0, 0.0, 0.0, f[0], 0.0, f[2]);
        } else if Physics::get_bool(dm, hum, "AutoRotate", true) && ml > 1e-3 && !platform {
            let target = cf::look_at(0.0, 0.0, 0.0, dir[0], 0.0, dir[2]);
            self.yaw_cf = cf::lerp(&cf::rot_only(&self.yaw_cf), &target, (dt * 12.0).min(1.0));
        }
        self.turn_box(ph, dm);
        let drop = 0.0;
        self.drop += (drop - self.drop) * (dt * 15.0).min(1.0);
        if self.drop < 0.005 {
            self.drop = 0.0;
        }
        let c = cf::with_pos(&self.yaw_cf, self.pos[0], self.pos[1] + self.off - self.drop, self.pos[2]);
        Step { cf: c, vel: [mvd[0] / dt, mvd[1] / dt, mvd[2] / dt], state: self.state }
    }

    fn compute_movement(&self, ph: &mut Physics, dm: &DataModel, desired: [f64; 3]) -> ([f64; 3], bool) {
        let pose = match ph.colliders.get(self.collider) {
            Some(c) => *c.position(),
            None => return ([0.0; 3], false),
        };
        let shape = match ph.colliders.get(self.collider) {
            Some(c) => c.shared_shape().clone(),
            None => return ([0.0; 3], false),
        };
        let mut collisions: Vec<CharacterCollision> = vec![];
        let eff = {
            let pred = |h: ColliderHandle, _c: &Collider| -> bool { self.pass_ok(ph, dm, h) };
            let filter = QueryFilter { flags: QueryFilterFlags::EXCLUDE_SENSORS, groups: None, exclude_collider: Some(self.collider), exclude_rigid_body: None, predicate: Some(&pred) };
            let qp = ph.broad.as_query_pipeline(&DefaultQueryDispatcher, &ph.bodies, &ph.colliders, filter);
            self.kcc.move_shape(STEP, &qp, shape.as_ref(), &pose, Vector::new(desired[0] as f32, desired[1] as f32, desired[2] as f32), |c| collisions.push(c))
        };
        if !collisions.is_empty() {
            let mass = ph.colliders.get(self.collider).map(|c| c.mass()).unwrap_or(9.8);
            let pass = self.pass.clone();
            let model = self.model;
            let own = self.collider;
            let _ = (pass, model, own);
            let pred = |_h: ColliderHandle, _c: &Collider| -> bool { true };
            let filter = QueryFilter { flags: QueryFilterFlags::EXCLUDE_SENSORS, groups: None, exclude_collider: Some(self.collider), exclude_rigid_body: None, predicate: Some(&pred) };
            let mut qpm = ph.broad.as_query_pipeline_mut(&DefaultQueryDispatcher, &mut ph.bodies, &mut ph.colliders, filter);
            self.kcc.solve_character_collision_impulses(STEP, &mut qpm, shape.as_ref(), mass, collisions.iter());
        }
        ([eff.translation.x as f64, eff.translation.y as f64, eff.translation.z as f64], eff.grounded)
    }
}

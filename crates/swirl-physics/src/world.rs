use crate::cf::{self, CF};
use crate::groups;
use indexmap::{IndexMap, IndexSet};
use rapier3d::prelude::*;
use std::collections::{HashMap, HashSet};
use swirl_core::instance::{Change, DataModel, InstId, Val};

pub const GRAVITY: f64 = 196.2;
const STEP: f64 = 1.0 / 60.0;

fn f3(v: &[f64]) -> [f64; 3] {
    [v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)]
}

fn iso(c: &CF) -> Pose {
    let q = cf::to_quat(c);
    Pose::from_parts(Vector::new(c[0] as f32, c[1] as f32, c[2] as f32), Rotation::from_xyzw(q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32))
}

fn iso_body(c: &CF) -> Pose {
    iso(c)
}

fn cf_of(t: Vector, r: &Rotation) -> CF {
    cf::from_quat(t.x as f64, t.y as f64, t.z as f64, r.x as f64, r.y as f64, r.z as f64, r.w as f64)
}

fn wedge_hull(hx: f64, hy: f64, hz: f64) -> Vec<Vector> {
    let p = |x: f64, y: f64, z: f64| Vector::new(x as f32, y as f32, z as f32);
    vec![p(-hx, -hy, -hz), p(hx, -hy, -hz), p(-hx, -hy, hz), p(hx, -hy, hz), p(-hx, hy, hz), p(hx, hy, hz)]
}

fn corner_hull(hx: f64, hy: f64, hz: f64) -> Vec<Vector> {
    let p = |x: f64, y: f64, z: f64| Vector::new(x as f32, y as f32, z as f32);
    vec![p(-hx, -hy, -hz), p(hx, -hy, -hz), p(-hx, -hy, hz), p(hx, -hy, hz), p(hx, hy, -hz)]
}

const SURFACE_PROPS: [&str; 6] = ["RightSurface", "TopSurface", "BackSurface", "LeftSurface", "BottomSurface", "FrontSurface"];
const FACE_AXES: [(usize, f64); 6] = [(0, 1.0), (1, 1.0), (2, 1.0), (0, -1.0), (1, -1.0), (2, -1.0)];

fn can_join(a: i32, b: i32) -> bool {
    if a == 9 || b == 9 {
        return false;
    }
    if a == 2 || b == 2 || a == 1 || b == 1 {
        return true;
    }
    if (6..=8).contains(&a) || (6..=8).contains(&b) {
        return true;
    }
    if a == 0 || b == 0 {
        return false;
    }
    let has = |x: i32| a == x || b == x;
    if has(3) && (has(4) || has(5)) {
        return true;
    }
    if has(5) && (has(4) || has(5)) {
        return true;
    }
    false
}

pub(crate) struct PartRec {
    collider: Option<ColliderHandle>,
    rot: Option<CF>,
    asm: Option<usize>,
    local: Option<CF>,
    volume: f64,
    touch: bool,
}

pub(crate) struct Asm {
    pub(crate) parts: IndexSet<InstId>,
    anchored: bool,
    root: InstId,
    body: Option<RigidBodyHandle>,
    sleeping: bool,
}

#[derive(Clone, Copy, Default)]
pub struct Special {
    pub part: Option<InstId>,
    pub no_ray: bool,
    pub character: Option<InstId>,
    pub solid_proxy: bool,
    pub sensor_only: bool,
}

pub struct TouchEvent {
    pub a: InstId,
    pub b: InstId,
    pub started: bool,
}

pub struct RayHit {
    pub part: Option<InstId>,
    pub point: [f64; 3],
    pub normal: [f64; 3],
}

pub struct Physics {
    pub(crate) gravity: f64,
    pub(crate) pipeline: PhysicsPipeline,
    pub(crate) params: IntegrationParameters,
    pub(crate) islands: IslandManager,
    pub(crate) broad: BroadPhaseBvh,
    pub(crate) soft: SoftBodySet,
    pub(crate) narrow: NarrowPhase,
    pub(crate) bodies: RigidBodySet,
    pub(crate) colliders: ColliderSet,
    pub(crate) joints_set: ImpulseJointSet,
    pub(crate) multibody: MultibodyJointSet,
    pub(crate) ccd: CCDSolver,
    pub(crate) parts: IndexMap<InstId, PartRec>,
    pub(crate) by_handle: HashMap<ColliderHandle, InstId>,
    pub(crate) assemblies: IndexMap<usize, Asm>,
    asm_seq: usize,
    pub asm_epoch: u64,
    dirty: IndexSet<InstId>,
    auto_joints: IndexMap<InstId, IndexSet<InstId>>,
    joints: IndexSet<InstId>,
    joint_parts: HashMap<InstId, (Option<InstId>, Option<InstId>)>,
    movers: IndexSet<InstId>,
    touching: HashMap<InstId, IndexSet<InstId>>,
    pending_join: IndexSet<InstId>,
    pub excluded: HashSet<InstId>,
    ws: Option<InstId>,
    loaded: bool,
    acc: f64,
    stir: bool,
    stir_write: bool,
    any_awake: Option<bool>,
    skip: HashSet<(InstId, &'static str)>,
    ph_sent: HashMap<InstId, [f64; 16]>,
    sent_sleep: HashMap<usize, bool>,
    cursor: usize,
    forced: IndexSet<RigidBodyHandle>,
    pub(crate) group_tbl: groups::Table,
    pub dead_chars: HashSet<InstId>,
    pub anim_cf: HashMap<InstId, CF>,
    pub fallen: Vec<Vec<InstId>>,
    pub touches: Vec<TouchEvent>,
    pub special: HashMap<ColliderHandle, Special>,
    pub solid_proxies: HashSet<ColliderHandle>,
    pub terrain_handles: HashSet<ColliderHandle>,
}

impl Default for Physics {
    fn default() -> Self {
        Physics::new()
    }
}

impl Physics {
    pub fn new() -> Physics {
        let mut params = IntegrationParameters::default();
        params.dt = STEP as f32;
        Physics {
            gravity: GRAVITY,
            pipeline: PhysicsPipeline::new(),
            params,
            islands: IslandManager::new(),
            broad: BroadPhaseBvh::new(),
            soft: SoftBodySet::new(),
            narrow: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            joints_set: ImpulseJointSet::new(),
            multibody: MultibodyJointSet::new(),
            ccd: CCDSolver::new(),
            parts: IndexMap::new(),
            by_handle: HashMap::new(),
            assemblies: IndexMap::new(),
            asm_seq: 0,
            asm_epoch: 0,
            dirty: IndexSet::new(),
            auto_joints: IndexMap::new(),
            joints: IndexSet::new(),
            joint_parts: HashMap::new(),
            movers: IndexSet::new(),
            touching: HashMap::new(),
            pending_join: IndexSet::new(),
            excluded: HashSet::new(),
            ws: None,
            loaded: false,
            acc: 0.0,
            stir: false,
            stir_write: false,
            any_awake: None,
            skip: HashSet::new(),
            ph_sent: HashMap::new(),
            sent_sleep: HashMap::new(),
            cursor: 0,
            forced: IndexSet::new(),
            group_tbl: groups::parse(""),
            dead_chars: HashSet::new(),
            anim_cf: HashMap::new(),
            fallen: vec![],
            touches: vec![],
            special: HashMap::new(),
            solid_proxies: HashSet::new(),
            terrain_handles: HashSet::new(),
        }
    }

    pub(crate) fn get_cf(dm: &DataModel, id: InstId) -> CF {
        match dm.get_prop(id, "CFrame") {
            Some(Val::Arr(a)) => cf::arr(a).unwrap_or(cf::IDENTITY),
            _ => cf::IDENTITY,
        }
    }

    pub(crate) fn get_v3(dm: &DataModel, id: InstId, name: &str, def: [f64; 3]) -> [f64; 3] {
        match dm.get_prop(id, name) {
            Some(Val::Arr(a)) if a.len() >= 3 => f3(a),
            _ => def,
        }
    }

    pub(crate) fn get_bool(dm: &DataModel, id: InstId, name: &str, def: bool) -> bool {
        match dm.get_prop(id, name) {
            Some(Val::Bool(b)) => *b,
            _ => def,
        }
    }

    pub(crate) fn get_num(dm: &DataModel, id: InstId, name: &str, def: f64) -> f64 {
        match dm.get_prop(id, name) {
            Some(Val::Num(n)) => *n,
            _ => def,
        }
    }

    fn set_own(&mut self, dm: &mut DataModel, id: InstId, name: &'static str, v: Val) {
        if dm.set_prop(id, name, v).is_ok() {
            self.skip.insert((id, name));
        }
    }

    pub(crate) fn refresh_groups(&mut self, dm: &DataModel) {
        let s = match self.ws.and_then(|w| dm.get_prop(w, "CollisionGroups")) {
            Some(Val::Str(s)) => s.clone(),
            _ => String::new(),
        };
        self.group_tbl = groups::parse(&s);
    }

    pub fn groups_collide(&self, a: i32, b: i32) -> bool {
        groups::collide(&self.group_tbl, a, b)
    }

    pub fn stir(&mut self) {
        self.stir = true;
    }

    pub fn gravity(&self) -> f64 {
        self.gravity
    }

    pub fn set_gravity(&mut self, g: f64) {
        self.gravity = if g.is_finite() { g } else { GRAVITY };
    }

    fn in_world(&self, dm: &DataModel, id: InstId) -> bool {
        match self.ws {
            Some(w) => id == w || dm.is_descendant_of(id, w),
            None => false,
        }
    }

    pub fn attach_workspace(&mut self, dm: &mut DataModel, ws: InstId) {
        self.ws = Some(ws);
        self.refresh_groups(dm);
        let g = Self::get_num(dm, ws, "Gravity", GRAVITY);
        self.set_gravity(g);
        let mut all = vec![];
        dm.descendants(ws, &mut all);
        for d in all {
            self.enter(dm, d);
        }
        self.auto_join_all(dm);
        self.loaded = true;
        self.flush(dm);
        self.cursor = dm.journal.len();
    }

    fn is_excluded(&self, dm: &DataModel, id: InstId) -> bool {
        if self.excluded.is_empty() {
            return false;
        }
        let mut cur = Some(id);
        while let Some(c) = cur {
            if self.excluded.contains(&c) {
                return true;
            }
            cur = dm.get(c).and_then(|i| i.parent);
        }
        false
    }

    fn enter(&mut self, dm: &mut DataModel, id: InstId) {
        let class = match dm.get(id) {
            Some(i) if !i.destroyed => i.class.clone(),
            _ => return,
        };
        if dm.is_a(id, "BasePart") {
            if class == "Terrain" {
                return;
            }
            if !self.parts.contains_key(&id) && !self.is_excluded(dm, id) {
                self.add_part(dm, id);
            }
        } else if dm.is_a(id, "JointInstance") || class == "WeldConstraint" {
            self.enter_joint(dm, id);
        } else if dm.is_a(id, "BodyMover") {
            if let Some(p) = dm.get(id).and_then(|i| i.parent) {
                if dm.is_a(p, "BasePart") {
                    self.movers.insert(id);
                }
            }
        }
    }

    fn leave(&mut self, dm: &DataModel, id: InstId) {
        if self.parts.contains_key(&id) {
            self.remove_part(id);
        }
        if self.joints.contains(&id) {
            self.leave_joint(id);
        }
        self.movers.shift_remove(&id);
        let _ = dm;
    }

    fn enter_joint(&mut self, dm: &mut DataModel, j: InstId) {
        let class = dm.get(j).map(|i| i.class.clone()).unwrap_or_default();
        const RIGID: [&str; 9] = ["WeldConstraint", "Weld", "ManualWeld", "ManualGlue", "Snap", "Glue", "Motor", "Motor6D", "VelocityMotor"];
        if !RIGID.contains(&class.as_str()) {
            return;
        }
        if class == "WeldConstraint" && !Self::get_bool(dm, j, "Enabled", true) {
            return;
        }
        let a = match dm.get_prop(j, "Part0") {
            Some(Val::Ref(Some(p))) => *p,
            _ => return,
        };
        let b = match dm.get_prop(j, "Part1") {
            Some(Val::Ref(Some(p))) => *p,
            _ => return,
        };
        if a == b {
            return;
        }
        if !self.in_world(dm, j) && !self.in_world(dm, a) {
            return;
        }
        self.joints.insert(j);
        self.joint_parts.insert(j, (Some(a), Some(b)));
        self.mark_dirty(a);
        self.mark_dirty(b);
    }

    fn leave_joint(&mut self, j: InstId) {
        if !self.joints.shift_remove(&j) {
            return;
        }
        if let Some((a, b)) = self.joint_parts.remove(&j) {
            if let Some(a) = a {
                self.mark_dirty(a);
            }
            if let Some(b) = b {
                self.mark_dirty(b);
            }
        }
    }

    fn mark_dirty(&mut self, id: InstId) {
        self.dirty.insert(id);
    }

    fn shape_builder(&self, dm: &DataModel, id: InstId) -> (ColliderBuilder, Option<CF>, f64) {
        let s = Self::get_v3(dm, id, "Size", [4.0, 1.0, 2.0]);
        let half = |v: f64| if v.is_finite() { (v / 2.0).max(0.05).min(1e5) } else { 0.05 };
        let (hx, hy, hz) = (half(s[0]), half(s[1]), half(s[2]));
        let class = dm.get(id).map(|i| i.class.clone()).unwrap_or_default();
        let has_shape = dm.prop_def(id, "Shape").is_some();
        let shape = Self::get_num(dm, id, "Shape", 1.0) as i32;
        let mut rot = None;
        let b = if class == "WedgePart" {
            ColliderBuilder::convex_hull(&wedge_hull(hx, hy, hz))
        } else if class == "CornerWedgePart" {
            ColliderBuilder::convex_hull(&corner_hull(hx, hy, hz))
        } else if has_shape && shape == 0 {
            Some(ColliderBuilder::ball(hx.min(hy).min(hz) as f32))
        } else if has_shape && shape == 2 {
            rot = Some(cf::axis_angle(0.0, 0.0, 1.0, std::f64::consts::PI / 2.0));
            Some(ColliderBuilder::cylinder(hx as f32, hy.min(hz) as f32))
        } else {
            None
        };
        let b = b.unwrap_or_else(|| ColliderBuilder::cuboid(hx as f32, hy as f32, hz as f32));
        (b, rot, s[0] * s[1] * s[2])
    }

    fn make_collider(&mut self, dm: &DataModel, id: InstId, body: Option<RigidBodyHandle>, offset: Option<CF>) {
        self.stir = true;
        if let Some(old) = self.parts.get_mut(&id).and_then(|r| r.collider.take()) {
            self.by_handle.remove(&old);
            self.colliders.remove(old, &mut self.islands, &mut self.bodies, &mut self.soft, true);
        }
        let (builder, rot, volume) = self.shape_builder(dm, id);
        let raw = offset.unwrap_or_else(|| Self::get_cf(dm, id));
        let base = if cf::is_finite(&raw) { raw } else { [0.0, -1e4, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] };
        let full = match &rot {
            Some(r) => cf::mul(&base, r),
            None => base,
        };
        let gid = Self::get_num(dm, id, "CollisionGroupId", 0.0) as i32;
        let packed = groups::rapier(&self.group_tbl, gid);
        let touch = self.parts.get(&id).map(|r| r.touch).unwrap_or(false);
        let mut b = builder
            .position(iso(&full))
            .friction(Self::get_num(dm, id, "Friction", 0.3) as f32)
            .restitution((Self::get_num(dm, id, "Elasticity", 0.5) * 0.3) as f32)
            .density(0.7)
            .sensor(!Self::get_bool(dm, id, "CanCollide", true))
            .collision_groups(InteractionGroups::new(Group::from_bits_truncate(packed >> 16), Group::from_bits_truncate(packed & 0xffff), InteractionTestMode::And))
            .active_collision_types(ActiveCollisionTypes::default() | ActiveCollisionTypes::KINEMATIC_FIXED | ActiveCollisionTypes::KINEMATIC_KINEMATIC);
        if touch {
            b = b.active_events(ActiveEvents::COLLISION_EVENTS);
        }
        let col = b.build();
        let h = match body {
            Some(bh) => self.colliders.insert_with_parent(col, bh, &mut self.bodies),
            None => self.colliders.insert(col),
        };
        self.by_handle.insert(h, id);
        if let Some(r) = self.parts.get_mut(&id) {
            r.collider = Some(h);
            r.rot = rot;
            r.volume = volume;
        }
    }

    fn add_part(&mut self, dm: &mut DataModel, id: InstId) {
        self.parts.insert(id, PartRec { collider: None, rot: None, asm: None, local: None, volume: 1.0, touch: false });
        self.make_collider(dm, id, None, None);
        self.mark_dirty(id);
    }

    pub fn touch_interest(&mut self, id: InstId) {
        if let Some(r) = self.parts.get_mut(&id) {
            if r.touch {
                return;
            }
            r.touch = true;
            if let Some(c) = r.collider.and_then(|h| self.colliders.get_mut(h)) {
                c.set_active_events(ActiveEvents::COLLISION_EVENTS);
            }
        }
    }

    pub fn remove_part(&mut self, id: InstId) {
        let rec = match self.parts.shift_remove(&id) {
            Some(r) => r,
            None => return,
        };
        self.stir = true;
        if let Some(h) = rec.collider {
            self.by_handle.remove(&h);
            self.colliders.remove(h, &mut self.islands, &mut self.bodies, &mut self.soft, true);
        }
        self.drop_auto(id);
        if let Some(t) = self.touching.remove(&id) {
            for o in t {
                if let Some(ot) = self.touching.get_mut(&o) {
                    ot.shift_remove(&id);
                }
            }
        }
        if let Some(ai) = rec.asm {
            if let Some(a) = self.assemblies.get_mut(&ai) {
                a.parts.shift_remove(&id);
                let rest: Vec<InstId> = a.parts.iter().copied().collect();
                for p in rest {
                    self.dirty.insert(p);
                }
            }
            self.destroy_assembly(ai);
        }
        self.dirty.shift_remove(&id);
    }

    fn rebuild_part(&mut self, dm: &DataModel, id: InstId) {
        let (body, local) = match self.parts.get(&id).and_then(|r| r.asm).and_then(|a| self.assemblies.get(&a)) {
            Some(a) if a.body.is_some() => (a.body, self.parts.get(&id).and_then(|r| r.local)),
            _ => (None, None),
        };
        if !self.parts.contains_key(&id) {
            return;
        }
        self.make_collider(dm, id, body, local);
    }

    fn add_auto(&mut self, a: InstId, b: InstId) {
        self.auto_joints.entry(a).or_default().insert(b);
        self.auto_joints.entry(b).or_default().insert(a);
        self.mark_dirty(a);
        self.mark_dirty(b);
    }

    fn drop_auto(&mut self, part: InstId) {
        if let Some(s) = self.auto_joints.shift_remove(&part) {
            for o in s {
                if let Some(os) = self.auto_joints.get_mut(&o) {
                    os.shift_remove(&part);
                }
                self.mark_dirty(o);
            }
        }
    }

    pub fn break_joints(&mut self, dm: &mut DataModel, part: InstId) {
        self.drop_auto(part);
        let js: Vec<InstId> = self.joints.iter().copied().collect();
        for j in js {
            if let Some((a, b)) = self.joint_parts.get(&j).copied() {
                if a == Some(part) || b == Some(part) {
                    dm.destroy(j);
                    self.joints.shift_remove(&j);
                    self.joint_parts.remove(&j);
                    if let Some(a) = a {
                        self.mark_dirty(a);
                    }
                    if let Some(b) = b {
                        self.mark_dirty(b);
                    }
                }
            }
        }
        self.mark_dirty(part);
    }

    fn aabb_of(dm: &DataModel, id: InstId, pad: f64) -> ([f64; 3], [f64; 3]) {
        let c = Self::get_cf(dm, id);
        let s = Self::get_v3(dm, id, "Size", [4.0, 1.0, 2.0]);
        let mut lo = [0.0; 3];
        let mut hi = [0.0; 3];
        for k in 0..3 {
            let e = c[3 + k * 3].abs() * s[0] / 2.0 + c[4 + k * 3].abs() * s[1] / 2.0 + c[5 + k * 3].abs() * s[2] / 2.0 + pad;
            lo[k] = c[k] - e;
            hi[k] = c[k] + e;
        }
        (lo, hi)
    }

    fn surface_of(dm: &DataModel, id: InstId, face: usize) -> i32 {
        Self::get_num(dm, id, SURFACE_PROPS[face], 0.0) as i32
    }

    fn overlap2d(ca: &CF, sa: [f64; 3], ax_a: usize, cb: &CF, sb: [f64; 3], ax_b: usize) -> bool {
        let tangents: Vec<[f64; 3]> = (0..3).filter(|k| *k != ax_a).map(|k| [ca[3 + k], ca[6 + k], ca[9 + k]]).collect();
        let proj = |c: &CF, s: [f64; 3], t: [f64; 3]| -> (f64, f64) {
            let mut ext = 0.0;
            for k in 0..3 {
                ext += (c[3 + k] * t[0] + c[6 + k] * t[1] + c[9 + k] * t[2]).abs() * s[k] / 2.0;
            }
            let ctr = c[0] * t[0] + c[1] * t[1] + c[2] * t[2];
            (ctr - ext, ctr + ext)
        };
        for t in tangents {
            let mut sa2 = sa;
            sa2[ax_a] = 0.0;
            let mut sb2 = sb;
            sb2[ax_b] = 0.0;
            let (a0, a1) = proj(ca, sa2, t);
            let (b0, b1) = proj(cb, sb2, t);
            if a1.min(b1) - a0.max(b0) < 0.01 {
                return false;
            }
        }
        true
    }

    fn face_contact(dm: &DataModel, a: InstId, b: InstId, deep: bool) -> Option<(usize, usize)> {
        let ca = Self::get_cf(dm, a);
        let cb = Self::get_cf(dm, b);
        let sa = Self::get_v3(dm, a, "Size", [4.0, 1.0, 2.0]);
        let sb = Self::get_v3(dm, b, "Size", [4.0, 1.0, 2.0]);
        let mut nb = [[0.0; 3]; 6];
        for (fb, n) in nb.iter_mut().enumerate() {
            let (ax, sg) = FACE_AXES[fb];
            *n = [cb[3 + ax] * sg, cb[6 + ax] * sg, cb[9 + ax] * sg];
        }
        for fa in 0..6 {
            let (ax_a, sg_a) = FACE_AXES[fa];
            let na = [ca[3 + ax_a] * sg_a, ca[6 + ax_a] * sg_a, ca[9 + ax_a] * sg_a];
            for fb in 0..6 {
                let (ax_b, sg_b) = FACE_AXES[fb];
                let _ = sg_b;
                let n2 = nb[fb];
                if na[0] * n2[0] + na[1] * n2[1] + na[2] * n2[2] > -0.999 {
                    continue;
                }
                let pa = sa[ax_a] / 2.0;
                let pb = sb[ax_b] / 2.0;
                let ctr_a = [ca[0] + na[0] * pa, ca[1] + na[1] * pa, ca[2] + na[2] * pa];
                let ctr_b = [cb[0] + n2[0] * pb, cb[1] + n2[1] * pb, cb[2] + n2[2] * pb];
                let d = (ctr_b[0] - ctr_a[0]) * na[0] + (ctr_b[1] - ctr_a[1]) * na[1] + (ctr_b[2] - ctr_a[2]) * na[2];
                if d.abs() > 0.08 {
                    if !deep || d > 0.0 || -d > sa[ax_a].min(sb[ax_b]) {
                        continue;
                    }
                    let ha = Self::surface_of(dm, a, fa);
                    let hb = Self::surface_of(dm, b, fb);
                    if !(6..=8).contains(&ha) && !(6..=8).contains(&hb) {
                        continue;
                    }
                }
                if Self::overlap2d(&ca, sa, ax_a, &cb, sb, ax_b) {
                    return Some((fa, fb));
                }
            }
        }
        None
    }

    pub fn contact_joint_class(&self, dm: &DataModel, a: InstId, b: InstId) -> Option<&'static str> {
        let f = Self::face_contact(dm, a, b, true)?;
        let sa = Self::surface_of(dm, a, f.0);
        let sb = Self::surface_of(dm, b, f.1);
        if !can_join(sa, sb) {
            return None;
        }
        if sa == 2 || sb == 2 {
            Some("Weld")
        } else if sa == 1 || sb == 1 {
            Some("Glue")
        } else {
            Some("Snap")
        }
    }

    fn make_joints(&mut self, dm: &DataModel, list: &[InstId]) {
        if list.is_empty() {
            return;
        }
        const CELL: f64 = 8.0;
        let mut grid: IndexMap<String, Vec<(InstId, [f64; 3], [f64; 3])>> = IndexMap::new();
        for p in self.parts.keys() {
            let (lo, hi) = Self::aabb_of(dm, *p, 0.1);
            if hi[0] - lo[0] > 400.0 || hi[1] - lo[1] > 400.0 || hi[2] - lo[2] > 400.0 {
                grid.entry("big".into()).or_default().push((*p, lo, hi));
                continue;
            }
            for x in (lo[0] / CELL).floor() as i64..=(hi[0] / CELL).floor() as i64 {
                for y in (lo[1] / CELL).floor() as i64..=(hi[1] / CELL).floor() as i64 {
                    for z in (lo[2] / CELL).floor() as i64..=(hi[2] / CELL).floor() as i64 {
                        grid.entry(format!("{},{},{}", x, y, z)).or_default().push((*p, lo, hi));
                    }
                }
            }
        }
        for &p in list {
            let (lo, hi) = Self::aabb_of(dm, p, 0.1);
            let mut out: IndexSet<InstId> = IndexSet::new();
            let mut test = |e: &(InstId, [f64; 3], [f64; 3])| {
                if e.0 == p {
                    return;
                }
                if e.1[0] > hi[0] || e.2[0] < lo[0] || e.1[1] > hi[1] || e.2[1] < lo[1] || e.1[2] > hi[2] || e.2[2] < lo[2] {
                    return;
                }
                out.insert(e.0);
            };
            if let Some(b) = grid.get("big") {
                for e in b {
                    test(e);
                }
            }
            for x in (lo[0] / CELL).floor() as i64..=(hi[0] / CELL).floor() as i64 {
                for y in (lo[1] / CELL).floor() as i64..=(hi[1] / CELL).floor() as i64 {
                    for z in (lo[2] / CELL).floor() as i64..=(hi[2] / CELL).floor() as i64 {
                        if let Some(l) = grid.get(&format!("{},{},{}", x, y, z)) {
                            for e in l {
                                test(e);
                            }
                        }
                    }
                }
            }
            for o in out {
                if o == p {
                    continue;
                }
                let f = match Self::face_contact(dm, p, o, true) {
                    Some(f) => f,
                    None => continue,
                };
                let sp = Self::surface_of(dm, p, f.0);
                let so = Self::surface_of(dm, o, f.1);
                if !can_join(sp, so) {
                    continue;
                }
                if (6..=8).contains(&sp) || (6..=8).contains(&so) {
                    continue;
                }
                self.add_auto(p, o);
            }
        }
    }

    fn auto_join_all(&mut self, dm: &DataModel) {
        let loose: Vec<InstId> = self.parts.keys().copied().filter(|p| !Self::get_bool(dm, *p, "Anchored", false)).collect();
        let mut explicit: HashSet<InstId> = HashSet::new();
        for (a, b) in self.joint_parts.values() {
            if let Some(a) = a {
                explicit.insert(*a);
            }
            if let Some(b) = b {
                explicit.insert(*b);
            }
        }
        let list: Vec<InstId> = loose.into_iter().filter(|p| !explicit.contains(p)).collect();
        self.make_joints(dm, &list);
    }

    fn connected(&self, start: InstId) -> IndexSet<InstId> {
        let mut index: HashMap<InstId, Vec<InstId>> = HashMap::new();
        for j in &self.joints {
            if let Some((Some(a), Some(b))) = self.joint_parts.get(j) {
                index.entry(*a).or_default().push(*b);
                index.entry(*b).or_default().push(*a);
            }
        }
        let mut out: IndexSet<InstId> = IndexSet::new();
        out.insert(start);
        let mut stack = vec![start];
        while let Some(p) = stack.pop() {
            let mut adj: Vec<InstId> = self.auto_joints.get(&p).map(|s| s.iter().copied().collect()).unwrap_or_default();
            if let Some(v) = index.get(&p) {
                adj.extend(v.iter().copied());
            }
            for o in adj {
                if !out.contains(&o) && self.parts.contains_key(&o) {
                    out.insert(o);
                    stack.push(o);
                }
            }
        }
        out
    }

    fn destroy_assembly(&mut self, ai: usize) {
        let asm = match self.assemblies.shift_remove(&ai) {
            Some(a) => a,
            None => return,
        };
        self.asm_epoch += 1;
        for p in &asm.parts {
            if let Some(r) = self.parts.get_mut(p) {
                if r.asm == Some(ai) {
                    r.asm = None;
                }
            }
        }
        if let Some(body) = asm.body {
            for p in &asm.parts {
                if let Some(r) = self.parts.get_mut(p) {
                    if let Some(h) = r.collider {
                        if self.colliders.get(h).and_then(|c| c.parent()) == Some(body) {
                            self.by_handle.remove(&h);
                            r.collider = None;
                        }
                    }
                }
            }
            self.bodies.remove(body, &mut self.islands, &mut self.colliders, &mut self.joints_set, &mut self.multibody, &mut self.soft, true);
        }
    }

    fn build_assembly(&mut self, dm: &DataModel, group: &IndexSet<InstId>) {
        let mut old: IndexSet<usize> = IndexSet::new();
        for p in group {
            if let Some(a) = self.parts.get(p).and_then(|r| r.asm) {
                old.insert(a);
            }
        }
        let mut vel: Option<([f32; 3], [f32; 3])> = None;
        for a in old {
            if let Some(bh) = self.assemblies.get(&a).and_then(|x| x.body) {
                if vel.is_none() {
                    if let Some(b) = self.bodies.get(bh) {
                        let l = b.linvel();
                        let w = b.angvel();
                        vel = Some(([l.x, l.y, l.z], [w.x, w.y, w.z]));
                    }
                }
            }
            self.destroy_assembly(a);
        }
        let parts: Vec<InstId> = group.iter().copied().collect();
        let anchored = parts.iter().any(|p| Self::get_bool(dm, *p, "Anchored", false));
        let mut root = parts[0];
        let mut best = -1.0f64;
        for p in &parts {
            let a = Self::get_bool(dm, *p, "Anchored", false);
            if a && !anchored {
                continue;
            }
            let v = self.parts[p].volume + if a { 1e9 } else { 0.0 };
            if v > best {
                best = v;
                root = *p;
            }
        }
        self.asm_seq += 1;
        let ai = self.asm_seq;
        let mut asm = Asm { parts: group.clone(), anchored, root, body: None, sleeping: false };
        self.asm_epoch += 1;
        if anchored {
            let inv0 = if parts.len() > 1 { Some(cf::inv(&Self::get_cf(dm, root))) } else { None };
            self.assemblies.insert(ai, asm);
            for p in &parts {
                let local = inv0.map(|i| cf::mul(&i, &Self::get_cf(dm, *p)));
                let has = {
                    let r = self.parts.get_mut(p).unwrap();
                    r.asm = Some(ai);
                    r.local = local;
                    r.collider.is_some()
                };
                if !has {
                    self.make_collider(dm, *p, None, None);
                }
            }
            return;
        }
        let rcf = Self::get_cf(dm, root);
        let rb = RigidBodyBuilder::dynamic().position(iso_body(&rcf)).can_sleep(true).ccd_enabled(false).build();
        let mut body = rb;
        let (lv, av) = match vel {
            Some(v) => (Vector::new(v.0[0], v.0[1], v.0[2]), Vector::new(v.1[0], v.1[1], v.1[2])),
            None => {
                let v = Self::get_v3(dm, root, "Velocity", [0.0; 3]);
                let w = Self::get_v3(dm, root, "RotVelocity", [0.0; 3]);
                (Vector::new(v[0] as f32, v[1] as f32, v[2] as f32), Vector::new(w[0] as f32, w[1] as f32, w[2] as f32))
            }
        };
        body.set_linvel(lv, true);
        body.set_angvel(av, true);
        let bh = self.bodies.insert(body);
        asm.body = Some(bh);
        self.assemblies.insert(ai, asm);
        let inv = cf::inv(&rcf);
        for p in &parts {
            let local = cf::mul(&inv, &Self::get_cf(dm, *p));
            {
                let r = self.parts.get_mut(p).unwrap();
                r.asm = Some(ai);
                r.local = Some(local);
            }
            self.make_collider(dm, *p, Some(bh), Some(local));
        }
    }

    pub fn flush(&mut self, dm: &DataModel) {
        if !self.pending_join.is_empty() || !self.dirty.is_empty() {
            self.stir = true;
        }
        if !self.pending_join.is_empty() {
            let list: Vec<InstId> = self.pending_join.iter().copied().filter(|p| self.parts.contains_key(p)).collect();
            self.pending_join.clear();
            self.make_joints(dm, &list);
        }
        if self.dirty.is_empty() {
            return;
        }
        let seeds: Vec<InstId> = self.dirty.iter().copied().filter(|p| self.parts.contains_key(p)).collect();
        self.dirty.clear();
        let mut seen: HashSet<InstId> = HashSet::new();
        for s in seeds {
            if seen.contains(&s) {
                continue;
            }
            let group = self.connected(s);
            for p in &group {
                seen.insert(*p);
            }
            self.build_assembly(dm, &group);
        }
    }

    fn moved(&mut self, dm: &mut DataModel, id: InstId) {
        let cfv = Self::get_cf(dm, id);
        let (collider, asm, local, rot) = match self.parts.get(&id) {
            Some(r) => (r.collider, r.asm, r.local, r.rot),
            None => return,
        };
        let collider = match collider {
            Some(c) => c,
            None => return,
        };
        self.stir = true;
        if let Some(ai) = asm {
            if let Some(bh) = self.assemblies.get(&ai).and_then(|a| a.body) {
                if let Some(l) = local {
                    let target = cf::mul(&cfv, &cf::inv(&l));
                    if let Some(b) = self.bodies.get_mut(bh) {
                        b.set_position(iso_body(&target), true);
                    }
                    let others: Vec<InstId> = self.assemblies[&ai].parts.iter().copied().filter(|p| *p != id).collect();
                    for p in others {
                        if let Some(pl) = self.parts.get(&p).and_then(|r| r.local) {
                            self.set_own(dm, p, "CFrame", Val::Arr(cf::mul(&target, &pl).to_vec()));
                        }
                    }
                }
                return;
            }
            let (anch, n) = self.assemblies.get(&ai).map(|a| (a.anchored, a.parts.len())).unwrap_or((false, 0));
            if anch && n > 1 {
                if let Some(l) = local {
                    let root_cf = cf::mul(&cfv, &cf::inv(&l));
                    let others: Vec<InstId> = self.assemblies[&ai].parts.iter().copied().filter(|p| *p != id).collect();
                    for p in others {
                        let (pl, prot, pcol) = match self.parts.get(&p) {
                            Some(r) => match r.local {
                                Some(l) => (l, r.rot, r.collider),
                                None => continue,
                            },
                            None => continue,
                        };
                        let ncf = cf::mul(&root_cf, &pl);
                        self.set_own(dm, p, "CFrame", Val::Arr(ncf.to_vec()));
                        if let Some(c) = pcol.and_then(|h| self.colliders.get_mut(h)) {
                            let f = match &prot {
                                Some(r) => cf::mul(&ncf, r),
                                None => ncf,
                            };
                            c.set_position(iso(&f));
                        }
                    }
                }
            }
        }
        let full = match &rot {
            Some(r) => cf::mul(&cfv, r),
            None => cfv,
        };
        if let Some(c) = self.colliders.get_mut(collider) {
            c.set_position(iso(&full));
        }
        if !Self::get_bool(dm, id, "Anchored", false) {
            self.mark_dirty(id);
        }
    }

    pub fn reset_cursor(&mut self) {
        self.cursor = 0;
    }

    pub fn sync(&mut self, dm: &mut DataModel) {
        let end = dm.journal.len();
        let from = self.cursor.min(end);
        let changes: Vec<Change> = dm.journal[from..end].to_vec();
        self.cursor = end;
        let mut sweep = false;
        let mut drop_idx: Vec<usize> = vec![];
        for (n, ch) in changes.into_iter().enumerate() {
            match ch {
                Change::Prop(id, name) => {
                    if self.skip.contains(&(id, if name == "CFrame" { "CFrame" } else if name == "Velocity" { "Velocity" } else if name == "RotVelocity" { "RotVelocity" } else { "" })) {
                        drop_idx.push(from + n);
                        continue;
                    }
                    self.changed(dm, id, &name);
                }
                Change::Parent { id, new, .. } => {
                    let mut list = vec![id];
                    dm.descendants(id, &mut list);
                    let inside = new.is_some() && self.in_world(dm, id);
                    for x in list {
                        if inside {
                            let had = self.parts.contains_key(&x);
                            self.enter(dm, x);
                            if !had && self.parts.contains_key(&x) && !Self::get_bool(dm, x, "Anchored", false) && self.loaded {
                                self.pending_join.insert(x);
                            }
                        } else {
                            self.leave(dm, x);
                        }
                    }
                    if inside && (dm.is_a(id, "JointInstance") || dm.get(id).map(|i| i.class == "WeldConstraint").unwrap_or(false)) {
                        self.enter(dm, id);
                    }
                }
                Change::Destroy { id, .. } => {
                    self.leave(dm, id);
                    sweep = true;
                }
            }
        }
        if sweep {
            let dead: Vec<InstId> = self.parts.keys().copied().filter(|p| dm.get(*p).map(|i| i.destroyed).unwrap_or(true)).collect();
            for p in dead {
                self.remove_part(p);
            }
            let dj: Vec<InstId> = self.joints.iter().copied().filter(|j| dm.get(*j).map(|i| i.destroyed).unwrap_or(true)).collect();
            for j in dj {
                self.leave_joint(j);
            }
        }
        self.skip.clear();
        if !drop_idx.is_empty() {
            let drop: HashSet<usize> = drop_idx.into_iter().collect();
            let mut i = 0usize;
            dm.journal.retain(|_| {
                let keep = !drop.contains(&i);
                i += 1;
                keep
            });
            self.cursor = dm.journal.len();
        }
    }

    fn changed(&mut self, dm: &mut DataModel, id: InstId, name: &str) {
        if Some(id) == self.ws {
            if name == "Gravity" {
                let g = Self::get_num(dm, id, "Gravity", GRAVITY);
                self.set_gravity(g);
            } else if name == "CollisionGroups" {
                self.refresh_groups(dm);
                let ids: Vec<InstId> = self.parts.keys().copied().collect();
                for p in ids {
                    self.apply_groups(dm, p);
                }
            }
            return;
        }
        if dm.is_a(id, "BasePart") {
            if !self.parts.contains_key(&id) {
                return;
            }
            match name {
                "CFrame" => self.moved(dm, id),
                "Size" | "Shape" | "CanCollide" => self.rebuild_part(dm, id),
                "Anchored" => {
                    if dm.is_a(id, "PartOperation") {
                        self.rebuild_part(dm, id);
                    }
                    self.mark_dirty(id);
                }
                "CollisionGroupId" => self.apply_groups(dm, id),
                "Velocity" | "RotVelocity" => {
                    let bh = self.parts.get(&id).and_then(|r| r.asm).and_then(|a| self.assemblies.get(&a)).filter(|a| !a.anchored).and_then(|a| a.body);
                    if let Some(bh) = bh {
                        let v = Self::get_v3(dm, id, name, [0.0; 3]);
                        if let Some(b) = self.bodies.get_mut(bh) {
                            let vv = Vector::new(v[0] as f32, v[1] as f32, v[2] as f32);
                            if name == "Velocity" {
                                b.set_linvel(vv, true);
                            } else {
                                b.set_angvel(vv, true);
                            }
                        }
                    }
                }
                "Friction" | "Elasticity" => {
                    let f = Self::get_num(dm, id, "Friction", 0.3) as f32;
                    let e = (Self::get_num(dm, id, "Elasticity", 0.5) * 0.3) as f32;
                    if let Some(c) = self.parts.get(&id).and_then(|r| r.collider).and_then(|h| self.colliders.get_mut(h)) {
                        c.set_friction(f);
                        c.set_restitution(e);
                    }
                }
                _ => {}
            }
        } else if dm.is_a(id, "JointInstance") || dm.get(id).map(|i| i.class == "WeldConstraint").unwrap_or(false) {
            if matches!(name, "Part0" | "Part1" | "C0" | "C1" | "Enabled") {
                self.leave_joint(id);
                if self.in_world(dm, id) {
                    self.enter_joint(dm, id);
                }
            }
        }
    }

    fn apply_groups(&mut self, dm: &DataModel, id: InstId) {
        let gid = Self::get_num(dm, id, "CollisionGroupId", 0.0) as i32;
        let packed = groups::rapier(&self.group_tbl, gid);
        if let Some(c) = self.parts.get(&id).and_then(|r| r.collider).and_then(|h| self.colliders.get_mut(h)) {
            c.set_collision_groups(InteractionGroups::new(Group::from_bits_truncate(packed >> 16), Group::from_bits_truncate(packed & 0xffff), InteractionTestMode::And));
        }
    }

    fn apply_movers(&mut self, dm: &DataModel, dt: f64) {
        for bh in self.forced.drain(..) {
            if let Some(b) = self.bodies.get_mut(bh) {
                b.reset_forces(false);
                b.reset_torques(false);
            }
        }
        let movers: Vec<InstId> = self.movers.iter().copied().collect();
        for m in movers {
            let (class, parent) = match dm.get(m) {
                Some(i) if !i.destroyed && i.parent.is_some() => (i.class.clone(), i.parent.unwrap()),
                _ => {
                    self.movers.shift_remove(&m);
                    continue;
                }
            };
            let bh = match self.parts.get(&parent).and_then(|r| r.asm).and_then(|a| self.assemblies.get(&a)).and_then(|a| a.body) {
                Some(b) => b,
                None => continue,
            };
            let gravity = self.gravity;
            let body = match self.bodies.get_mut(bh) {
                Some(b) => b,
                None => continue,
            };
            let mass = body.mass() as f64;
            let v = body.linvel();
            let g = |name: &str| Self::get_v3(dm, m, name, [0.0; 3]);
            match class.as_str() {
                "BodyVelocity" => {
                    let tv = g("Velocity");
                    let mf = g("MaxForce");
                    let lim = |k: usize, d: f64| -> f64 { d.min(mf[k] * dt).max(-mf[k] * dt) };
                    let gr = gravity * dt * mass;
                    body.apply_impulse(Vector::new(lim(0, (tv[0] - v.x as f64) * mass) as f32, lim(1, (tv[1] - v.y as f64) * mass + gr) as f32, lim(2, (tv[2] - v.z as f64) * mass) as f32), true);
                }
                "BodyPosition" => {
                    let tp = g("Position");
                    let mf = g("MaxForce");
                    let p = Self::get_num(dm, m, "P", 10000.0).max(0.0);
                    let d = Self::get_num(dm, m, "D", 1250.0).max(0.0);
                    let pos = body.translation();
                    let err = [tp[0] - pos.x as f64, tp[1] - pos.y as f64, tp[2] - pos.z as f64];
                    let vel = [v.x as f64, v.y as f64, v.z as f64];
                    let k = p / mass.max(1e-6);
                    let c = d / mass.max(1e-6);
                    let lift = if mf[1] > mass * gravity { mass * gravity } else { 0.0 };
                    let mut f = [0.0f64; 3];
                    for i in 0..3 {
                        let want = (vel[i] + dt * k * err[i]) / (1.0 + dt * c + dt * dt * k);
                        let fi = mass * (want - vel[i]) / dt + if i == 1 { lift } else { 0.0 };
                        f[i] = fi.min(mf[i]).max(-mf[i]);
                    }
                    body.add_force(Vector::new(f[0] as f32, f[1] as f32, f[2] as f32), true);
                    self.forced.insert(bh);
                }
                "BodyForce" => {
                    let f = g("Force");
                    body.add_force(Vector::new(f[0] as f32, f[1] as f32, f[2] as f32), true);
                    self.forced.insert(bh);
                }
                "BodyAngularVelocity" => {
                    let tv = g("AngularVelocity");
                    let mt = g("MaxTorque");
                    let w = body.angvel();
                    let inertia = mass * 2.0;
                    let lim = |k: usize, d: f64| -> f64 { (d * inertia * 0.5).min(mt[k] * dt).max(-mt[k] * dt) };
                    body.apply_torque_impulse(Vector::new(lim(0, tv[0] - w.x as f64) as f32, lim(1, tv[1] - w.y as f64) as f32, lim(2, tv[2] - w.z as f64) as f32), true);
                }
                _ => {}
            }
        }
    }

    pub fn step(&mut self, dm: &mut DataModel, dt: f64) {
        self.sync(dm);
        self.flush(dm);
        self.acc = (self.acc + dt).min(STEP * 4.0);
        while self.acc >= STEP {
            self.acc -= STEP;
            let busy = self.stir || self.any_awake != Some(false) || !self.forced.is_empty();
            if !busy {
                continue;
            }
            self.stir = false;
            self.apply_movers(dm, STEP);
            let (tx, rx) = std::sync::mpsc::channel::<CollisionEvent>();
            let (ctx, _crx) = std::sync::mpsc::channel::<ContactForceEvent>();
            let (stx, _srx) = std::sync::mpsc::channel::<SoftBodyTearEvent>();
            let events = ChannelEventCollector::new(tx, ctx, stx);
            let g = Vector::new(0.0, -self.gravity as f32, 0.0);
            let hooks = ProxyHooks { solid: self.solid_proxies.clone() };
            self.pipeline.step(g, &self.params, &mut self.islands, &mut self.broad, &mut self.narrow, &mut self.bodies, &mut self.colliders, &mut self.joints_set, &mut self.multibody, &mut self.soft, &mut self.ccd, &hooks, &events);
            while let Ok(ev) = rx.try_recv() {
                let (h1, h2, started) = match ev {
                    CollisionEvent::Started(a, b, _) => (a, b, true),
                    CollisionEvent::Stopped(a, b, _) => (a, b, false),
                };
                if let (Some(a), Some(b)) = (self.by_handle.get(&h1).copied(), self.by_handle.get(&h2).copied()) {
                    self.touch_pair(a, b, started);
                }
            }
            self.stir_write = true;
        }
        if self.stir_write || self.any_awake != Some(false) {
            self.stir_write = false;
            self.write_back(dm);
        }
        self.sync(dm);
    }

    fn touch_pair(&mut self, a: InstId, b: InstId, started: bool) {
        let mut ta = self.touching.remove(&a).unwrap_or_default();
        let mut tb = self.touching.remove(&b).unwrap_or_default();
        let ok = if started {
            if ta.contains(&b) {
                false
            } else {
                ta.insert(b);
                tb.insert(a);
                true
            }
        } else if !ta.contains(&b) {
            false
        } else {
            ta.shift_remove(&b);
            tb.shift_remove(&a);
            true
        };
        self.touching.insert(a, ta);
        self.touching.insert(b, tb);
        if ok {
            self.touches.push(TouchEvent { a, b, started });
        }
    }

    fn write_back(&mut self, dm: &mut DataModel) {
        let destroy_y = self.ws.map(|w| Self::get_num(dm, w, "FallenPartsDestroyHeight", -500.0)).unwrap_or(-500.0);
        let mut awake = 0;
        let mut fallen: Vec<usize> = vec![];
        let keys: Vec<usize> = self.assemblies.keys().copied().collect();
        for ai in keys {
            let bh = match self.assemblies[&ai].body {
                Some(b) => b,
                None => continue,
            };
            let (sleeping, bcf, lv, av) = {
                let b = match self.bodies.get(bh) {
                    Some(b) => b,
                    None => continue,
                };
                (b.is_sleeping(), cf_of(b.translation(), b.rotation()), b.linvel(), b.angvel())
            };
            let parts: Vec<InstId> = self.assemblies[&ai].parts.iter().copied().collect();
            if sleeping {
                if !self.assemblies[&ai].sleeping {
                    self.assemblies.get_mut(&ai).unwrap().sleeping = true;
                    for p in parts {
                        self.set_own(dm, p, "Velocity", Val::Arr(vec![0.0; 3]));
                        self.set_own(dm, p, "RotVelocity", Val::Arr(vec![0.0; 3]));
                    }
                }
                continue;
            }
            self.assemblies.get_mut(&ai).unwrap().sleeping = false;
            awake += 1;
            let vel = vec![lv.x as f64, lv.y as f64, lv.z as f64];
            let avel = vec![av.x as f64, av.y as f64, av.z as f64];
            if bcf[1] < destroy_y || !cf::is_finite(&bcf) {
                fallen.push(ai);
                continue;
            }
            for p in parts {
                let local = match self.parts.get(&p).and_then(|r| r.local) {
                    Some(l) => l,
                    None => continue,
                };
                self.set_own(dm, p, "CFrame", Val::Arr(cf::mul(&bcf, &local).to_vec()));
                self.set_own(dm, p, "Velocity", Val::Arr(vel.clone()));
                self.set_own(dm, p, "RotVelocity", Val::Arr(avel.clone()));
            }
        }
        self.any_awake = Some(awake > 0);
        for ai in fallen {
            let parts: Vec<InstId> = self.assemblies[&ai].parts.iter().copied().collect();
            self.fallen.push(parts);
        }
    }

    pub fn collect_phys(&mut self, dm: &DataModel) -> Vec<[f64; 16]> {
        let r = |v: f64| (v * 1000.0).round() / 1000.0;
        let mut out = vec![];
        let keys: Vec<usize> = self.assemblies.keys().copied().collect();
        for ai in keys {
            let bh = match self.assemblies[&ai].body {
                Some(b) => b,
                None => continue,
            };
            let sleeping = self.bodies.get(bh).map(|b| b.is_sleeping()).unwrap_or(true);
            if sleeping && self.sent_sleep.get(&ai).copied().unwrap_or(false) {
                continue;
            }
            self.sent_sleep.insert(ai, sleeping);
            let parts: Vec<InstId> = self.assemblies[&ai].parts.iter().copied().collect();
            for p in parts {
                let c = Self::get_cf(dm, p);
                let v = Self::get_v3(dm, p, "Velocity", [0.0; 3]);
                let mut rec = [p as f64, r(c[0]), r(c[1]), r(c[2]), r(c[3]), r(c[4]), r(c[5]), r(c[6]), r(c[7]), r(c[8]), r(c[9]), r(c[10]), r(c[11]), r(v[0]), r(v[1]), r(v[2])];
                let last = self.ph_sent.get(&p);
                if sleeping {
                    rec[13] = 0.0;
                    rec[14] = 0.0;
                    rec[15] = 0.0;
                    if let Some(l) = last {
                        if l.iter().zip(rec.iter()).all(|(a, b)| a == b) {
                            continue;
                        }
                    }
                } else if let Some(l) = last {
                    let same = (1..4).all(|i| (l[i] - rec[i]).abs() <= 0.01) && (4..13).all(|i| (l[i] - rec[i]).abs() <= 0.002) && (13..16).all(|i| (l[i] - rec[i]).abs() <= 0.05);
                    if same {
                        continue;
                    }
                }
                self.ph_sent.insert(p, rec);
                out.push(rec);
            }
        }
        out
    }

    pub fn assembly_list(&self) -> IndexMap<InstId, Vec<i64>> {
        let mut out = IndexMap::new();
        for a in self.assemblies.values() {
            if a.body.is_none() || a.anchored || a.parts.len() < 8 {
                continue;
            }
            let mut ids: Vec<i64> = a.parts.iter().map(|p| *p as i64).collect();
            ids.sort();
            let mut prev = 0;
            let d: Vec<i64> = ids
                .iter()
                .map(|id| {
                    let x = id - prev;
                    prev = *id;
                    x
                })
                .collect();
            out.insert(a.root, d);
        }
        out
    }

    pub fn exclude_model(&mut self, dm: &DataModel, model: InstId) {
        self.excluded.insert(model);
        let mut all = vec![model];
        dm.descendants(model, &mut all);
        for p in all {
            if self.parts.contains_key(&p) {
                self.remove_part(p);
            }
        }
    }

    pub fn include_model(&mut self, model: InstId) {
        self.excluded.remove(&model);
    }

    pub fn write_prop(&mut self, dm: &mut DataModel, id: InstId, name: &'static str, v: Val) {
        self.set_own(dm, id, name, v);
    }

    pub fn remove_char_parts(&mut self, dm: &DataModel, model: InstId) {
        let mut all = vec![];
        dm.descendants(model, &mut all);
        for p in all {
            if self.parts.contains_key(&p) {
                self.remove_part(p);
            }
        }
    }

    pub fn activate_parts(&mut self, dm: &mut DataModel, parts: &[InstId], vel: [f64; 3], jitter: &mut dyn FnMut() -> [f64; 2]) {
        for p in parts {
            self.break_joints(dm, *p);
        }
        for p in parts {
            if dm.get(*p).map(|i| i.destroyed).unwrap_or(true) {
                continue;
            }
            self.enter(dm, *p);
        }
        self.flush(dm);
        for p in parts {
            let bh = self.parts.get(p).and_then(|r| r.asm).and_then(|a| self.assemblies.get(&a)).and_then(|a| a.body);
            if let Some(b) = bh.and_then(|h| self.bodies.get_mut(h)) {
                let j = jitter();
                b.set_linvel(Vector::new((vel[0] + j[0]) as f32, (vel[1].max(0.0) + 4.0) as f32, (vel[2] + j[1]) as f32), true);
            }
        }
    }

    pub fn root_part(&self, id: InstId) -> InstId {
        self.parts.get(&id).and_then(|r| r.asm).and_then(|a| self.assemblies.get(&a)).map(|a| a.root).unwrap_or(id)
    }

    pub fn connected_parts(&self, id: InstId) -> Vec<InstId> {
        match self.parts.get(&id).and_then(|r| r.asm).and_then(|a| self.assemblies.get(&a)) {
            Some(a) => a.parts.iter().copied().collect(),
            None => vec![id],
        }
    }

    pub fn awake_assemblies(&self) -> usize {
        self.assemblies.values().filter(|a| a.body.and_then(|b| self.bodies.get(b)).map(|b| !b.is_sleeping()).unwrap_or(false)).count()
    }

    pub fn raycast(&self, dm: &DataModel, origin: [f64; 3], dir: [f64; 3], ignore: &[InstId], filter: Option<&dyn Fn(InstId) -> bool>) -> RayHit {
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        if len < 1e-9 {
            return RayHit { part: None, point: origin, normal: [0.0, 1.0, 0.0] };
        }
        let max_len = len.min(5000.0);
        let d = [dir[0] / len, dir[1] / len, dir[2] / len];
        let ray = Ray::new(Vector::new(origin[0] as f32, origin[1] as f32, origin[2] as f32), Vector::new(d[0] as f32, d[1] as f32, d[2] as f32));
        let pred = |h: ColliderHandle, _c: &Collider| -> bool {
            let id = match self.by_handle.get(&h) {
                Some(i) => *i,
                None => match self.special.get(&h) {
                    Some(sp) if !sp.no_ray && sp.part.is_some() => sp.part.unwrap(),
                    _ => return false,
                },
            };
            for x in ignore {
                if id == *x || dm.is_descendant_of(id, *x) {
                    return false;
                }
            }
            if let Some(f) = filter {
                if !f(id) {
                    return false;
                }
            }
            true
        };
        let qf = QueryFilter::default().predicate(&pred);
        let qp = self.broad.as_query_pipeline(&rapier3d::parry::query::DefaultQueryDispatcher, &self.bodies, &self.colliders, qf);
        match qp.cast_ray_and_get_normal(&ray, max_len as f32, true) {
            Some((h, hit)) => {
                let t = hit.time_of_impact as f64;
                RayHit { part: self.by_handle.get(&h).copied().or_else(|| self.special.get(&h).and_then(|s| s.part)), point: [origin[0] + d[0] * t, origin[1] + d[1] * t, origin[2] + d[2] * t], normal: [hit.normal.x as f64, hit.normal.y as f64, hit.normal.z as f64] }
            }
            None => RayHit { part: None, point: [origin[0] + d[0] * max_len, origin[1] + d[1] * max_len, origin[2] + d[2] * max_len], normal: [0.0, 1.0, 0.0] },
        }
    }
}

struct ProxyHooks {
    solid: HashSet<ColliderHandle>,
}

impl PhysicsHooks for ProxyHooks {
    fn filter_contact_pair(&self, ctx: &PairFilterContext) -> Option<SolverFlags> {
        let other = if self.solid.contains(&ctx.collider1) { ctx.rigid_body2 } else { ctx.rigid_body1 };
        if let Some(b) = other.and_then(|h| ctx.bodies.get(h)) {
            if b.is_dynamic() && b.mass() > 150.0 {
                return None;
            }
        }
        Some(SolverFlags::COMPUTE_RIGID_IMPULSES)
    }
}

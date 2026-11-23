use crate::cf::{self, CF};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::rc::Rc;
use swirl_core::instance::{DataModel, InstId, Val};
use swirl_core::rbx::{Inst, Value};

const HOLD_WHILE_LOADING: f64 = 3.0;
const ALWAYS_LOOP: [u64; 1] = [180426354];

pub struct Key {
    pub t: f64,
    pub cf: CF,
    pub style: i64,
    pub dir: i64,
}

pub struct Seq {
    pub looped: bool,
    pub length: f64,
    pub frames: Vec<(f64, String)>,
    pub keys: IndexMap<String, Vec<Key>>,
    pub priority: f64,
}

pub fn parse_sequence(ks: &Inst) -> Seq {
    let mut frames: Vec<(f64, String)> = vec![];
    let mut keys: IndexMap<String, Vec<Key>> = IndexMap::new();
    for k in &ks.children {
        if k.cls != "Keyframe" {
            continue;
        }
        let t = k.props.get("Time").and_then(|v| v.as_f64()).unwrap_or(0.0);
        frames.push((t, k.name.clone()));
        walk(k, t, &mut keys);
    }
    frames.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    for list in keys.values_mut() {
        list.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    }
    let looped = matches!(ks.props.get("Loop"), Some(Value::Bool(true)));
    let priority = ks.props.get("Priority").and_then(|v| v.as_f64()).unwrap_or(1000.0);
    Seq { looped, length: frames.last().map(|f| f.0).unwrap_or(0.0), frames, keys, priority }
}

fn walk(p: &Inst, t: f64, keys: &mut IndexMap<String, Vec<Key>>) {
    for c in &p.children {
        if c.cls != "Pose" {
            continue;
        }
        let w = c.props.get("Weight").and_then(|v| v.as_f64());
        if !matches!(w, Some(w) if w <= 0.0) {
            let cfv = match c.props.get("CFrame") {
                Some(Value::CFrame(p, r)) => [p[0], p[1], p[2], r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8]],
                _ => cf::IDENTITY,
            };
            let style = c.props.get("EasingStyle").and_then(|v| v.as_f64()).unwrap_or(0.0) as i64;
            let dir = c.props.get("EasingDirection").and_then(|v| v.as_f64()).unwrap_or(0.0) as i64;
            keys.entry(c.name.clone()).or_default().push(Key { t, cf: cfv, style, dir });
        }
        walk(c, t, keys);
    }
}

pub fn find_sequence(nodes: &[Inst]) -> Option<&Inst> {
    for n in nodes {
        if n.cls == "KeyframeSequence" {
            return Some(n);
        }
        if let Some(s) = find_sequence(&n.children) {
            return Some(s);
        }
    }
    None
}

fn bounce_out(mut x: f64) -> f64 {
    if x < 1.0 / 2.75 {
        7.5625 * x * x
    } else if x < 2.0 / 2.75 {
        x -= 1.5 / 2.75;
        7.5625 * x * x + 0.75
    } else if x < 2.5 / 2.75 {
        x -= 2.25 / 2.75;
        7.5625 * x * x + 0.9375
    } else {
        x -= 2.625 / 2.75;
        7.5625 * x * x + 0.984375
    }
}

fn ease(style: i64, dir: i64, x: f64) -> f64 {
    if style == 1 {
        return 0.0;
    }
    let f = |v: f64| -> f64 {
        match style {
            3 => v * v * v,
            4 => 1.0 - bounce_out(1.0 - v),
            2 => {
                if v <= 0.0 || v >= 1.0 {
                    v
                } else {
                    -(2.0f64).powf(10.0 * (v - 1.0)) * (((v - 1.075) * 2.0 * std::f64::consts::PI) / 0.3).sin()
                }
            }
            _ => v,
        }
    };
    if !matches!(style, 2 | 3 | 4) {
        return x;
    }
    if dir == 0 {
        f(x)
    } else if dir == 1 {
        1.0 - f(1.0 - x)
    } else if x < 0.5 {
        f(x * 2.0) / 2.0
    } else {
        1.0 - f((1.0 - x) * 2.0) / 2.0
    }
}

fn sample_keys(list: &[Key], t: f64) -> CF {
    if t <= list[0].t {
        return list[0].cf;
    }
    let last = &list[list.len() - 1];
    if t >= last.t {
        return last.cf;
    }
    for i in 1..list.len() {
        let b = &list[i];
        if t <= b.t {
            let a = &list[i - 1];
            let span = b.t - a.t;
            if span <= 0.0 {
                return b.cf;
            }
            let k = ease(a.style, a.dir, (t - a.t) / span);
            return if k <= 0.0 { a.cf } else { cf::lerp(&a.cf, &b.cf, k) };
        }
    }
    last.cf
}

type Levels = Vec<(i64, IndexMap<String, (CF, f64)>)>;

fn accumulate(levels: &mut Levels, prio: f64, seq: &Seq, t: f64, w: f64) {
    let p = prio as i64;
    let idx = match levels.iter().position(|(k, _)| *k == p) {
        Some(i) => i,
        None => {
            levels.push((p, IndexMap::new()));
            levels.len() - 1
        }
    };
    for (name, list) in &seq.keys {
        let c = sample_keys(list, t);
        let lv = &mut levels[idx].1;
        match lv.get_mut(name) {
            None => {
                lv.insert(name.clone(), (c, w));
            }
            Some(e) => {
                e.1 += w;
                e.0 = cf::lerp(&e.0, &c, w / e.1);
            }
        }
    }
}

fn rank(p: i64) -> i64 {
    if p >= 1000 {
        -1
    } else {
        p
    }
}

fn resolve(levels: &Levels) -> IndexMap<String, CF> {
    let mut out: IndexMap<String, CF> = IndexMap::new();
    let mut order: Vec<&(i64, IndexMap<String, (CF, f64)>)> = levels.iter().collect();
    order.sort_by_key(|(p, _)| rank(*p));
    for (_, lv) in order {
        for (name, (c, acc)) in lv {
            let k = acc.min(1.0);
            let v = if k >= 1.0 { *c } else { cf::lerp(out.get(name).unwrap_or(&cf::IDENTITY), c, k) };
            out.insert(name.clone(), v);
        }
    }
    out
}

fn num(v: Option<f64>, d: f64) -> f64 {
    match v {
        Some(x) if x.is_finite() => x,
        _ => d,
    }
}

pub struct Track {
    pub key: String,
    pub id: u64,
    pub inst: Option<InstId>,
    pub seq: Option<Rc<Seq>>,
    pub t: f64,
    pub w: f64,
    pub target: f64,
    pub fade: f64,
    pub speed: f64,
    pub weight: f64,
    pub wait: f64,
    pub failed: bool,
    pub prio: Option<f64>,
}

#[derive(Default)]
pub struct HumAnim {
    pub tracks: Vec<Track>,
    pub joints: Vec<InstId>,
}

pub enum SeqState {
    Ready(Rc<Seq>),
    Loading,
    Retry(u32, f64),
    Failed,
}

pub struct Event {
    pub inst: InstId,
    pub name: &'static str,
    pub arg: Option<String>,
}

#[derive(Default)]
pub struct Animator {
    pub by_hum: IndexMap<InstId, HumAnim>,
    pub seqs: HashMap<u64, SeqState>,
    pub wanted: Vec<u64>,
    pub anim: HashMap<InstId, CF>,
    pub now: f64,
}

pub fn anim_asset_id(url: &str) -> Option<u64> {
    let s = url.trim();
    let low = s.to_ascii_lowercase();
    let digits = |t: &str| -> Option<u64> {
        let d: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        if d.is_empty() {
            None
        } else {
            d.parse().ok()
        }
    };
    for pre in ["?id=", "&id="] {
        if let Some(i) = low.find(pre) {
            return digits(&low[i + pre.len()..]);
        }
    }
    if let Some(r) = low.strip_prefix("rbxassetid://") {
        return digits(r);
    }
    if !low.is_empty() && low.bytes().all(|b| b.is_ascii_digit()) {
        return low.parse().ok();
    }
    None
}

impl Animator {
    pub fn provide(&mut self, id: u64, bytes: Option<&[u8]>) {
        let seq = bytes.and_then(|b| swirl_core::rbx::parse(b).ok()).and_then(|t| find_sequence(&t.roots).map(parse_sequence));
        match seq {
            Some(mut s) => {
                if ALWAYS_LOOP.contains(&id) {
                    s.looped = true;
                }
                self.seqs.insert(id, SeqState::Ready(Rc::new(s)));
            }
            None => {
                let n = match self.seqs.get(&id) {
                    Some(SeqState::Retry(n, _)) => n + 1,
                    _ => 1,
                };
                self.seqs.insert(id, if n >= 4 { SeqState::Failed } else { SeqState::Retry(n, self.now + 0.6 * 2f64.powi(n as i32 - 1)) });
            }
        }
    }

    fn want(&mut self, id: u64) {
        if !self.seqs.contains_key(&id) {
            self.seqs.insert(id, SeqState::Loading);
            self.wanted.push(id);
        }
    }

    pub fn op(&mut self, d: &mut DataModel, hum: InstId, key: &str, op: &str, id: Option<u64>, a: Option<f64>, b: Option<f64>, c: Option<f64>, inst: Option<InstId>, start: f64) {
        if d.get(hum).map(|i| i.destroyed).unwrap_or(true) || !matches!(op, "play" | "stop" | "speed" | "weight" | "time") {
            return;
        }
        if !self.by_hum.contains_key(&hum) {
            if op != "play" {
                return;
            }
            self.by_hum.insert(hum, HumAnim::default());
        }
        let pos = self.by_hum[&hum].tracks.iter().position(|x| x.key == key);
        if op == "play" {
            let id = match id {
                Some(i) => i,
                None => return,
            };
            let an = self.by_hum.get_mut(&hum).unwrap();
            let mut track = match pos {
                Some(p) => {
                    let t = an.tracks.remove(p);
                    if t.id == id {
                        Some(t)
                    } else {
                        None
                    }
                }
                None => None,
            };
            if track.is_none() {
                if let Some(i) = inst {
                    if d.get(i).map(|x| !x.destroyed && x.props.contains_key("Priority")).unwrap_or(false) && matches!(d.get_prop(i, "Priority"), Some(Val::Num(p)) if *p == 0.0) {
                        let _ = d.set_prop(i, "Priority", Val::Num(-1.0));
                    }
                }
                track = Some(Track { key: key.to_string(), id, inst, seq: None, t: 0.0, w: 0.0, target: 1.0, fade: 0.1, speed: 1.0, weight: 1.0, wait: 0.0, failed: false, prio: None });
            }
            let mut tr = track.unwrap();
            tr.t = num(Some(start), 0.0).max(0.0);
            tr.target = 1.0;
            tr.fade = num(a, 0.1).max(0.0);
            tr.weight = num(b, 1.0).max(0.0);
            tr.speed = num(c, 1.0);
            if tr.fade == 0.0 {
                tr.w = 1.0;
            }
            if let Some(i) = inst {
                if d.get(i).map(|x| !x.destroyed).unwrap_or(false) {
                    let _ = d.set_prop(i, "IsPlaying", Val::Bool(true));
                }
            }
            self.by_hum.get_mut(&hum).unwrap().tracks.push(tr);
            self.want(id);
            return;
        }
        let an = self.by_hum.get_mut(&hum).unwrap();
        let tr = match pos {
            Some(p) => &mut an.tracks[p],
            None => return,
        };
        match op {
            "stop" => {
                tr.target = 0.0;
                tr.fade = num(a, 0.1).max(0.0);
                if tr.fade == 0.0 {
                    tr.w = 0.0;
                }
            }
            "speed" => tr.speed = num(a, 1.0),
            "time" => tr.t = num(a, 0.0).max(0.0),
            _ => {
                tr.weight = num(a, 1.0).max(0.0);
                tr.fade = num(b, 0.1).max(0.0);
            }
        }
    }

    pub fn playing(&self, hum: InstId) -> Vec<InstId> {
        self.by_hum.get(&hum).map(|an| an.tracks.iter().filter(|t| t.inst.is_some() && t.target > 0.0).filter_map(|t| t.inst).collect()).unwrap_or_default()
    }

    fn priority_of(d: &DataModel, tr: &Track) -> f64 {
        if let Some(i) = tr.inst {
            if let Some(Val::Num(p)) = d.get(i).filter(|x| !x.destroyed).and_then(|_| d.get_prop(i, "Priority")) {
                if *p >= 0.0 {
                    return *p;
                }
            }
        }
        if let Some(p) = tr.prio {
            return p;
        }
        tr.seq.as_ref().map(|s| s.priority).unwrap_or(1000.0)
    }

    pub fn step(&mut self, d: &mut DataModel, dt: f64) -> Vec<Event> {
        self.now += dt;
        let mut events = vec![];
        let retry: Vec<u64> = self.seqs.iter().filter_map(|(k, v)| matches!(v, SeqState::Retry(_, at) if *at <= self.now).then_some(*k)).collect();
        for id in retry {
            if let Some(SeqState::Retry(n, _)) = self.seqs.get(&id) {
                let n = *n;
                self.seqs.insert(id, SeqState::Loading);
                let _ = n;
                self.wanted.push(id);
            }
        }
        let hums: Vec<InstId> = self.by_hum.keys().copied().collect();
        let mut new_anim: HashMap<InstId, CF> = HashMap::new();
        for hum in hums {
            let model = d.get(hum).filter(|i| !i.destroyed).and_then(|i| i.parent);
            let model = match model {
                Some(m) => m,
                None => {
                    self.by_hum.shift_remove(&hum);
                    continue;
                }
            };
            let mut an = self.by_hum.shift_remove(&hum).unwrap();
            for tr in an.tracks.iter_mut() {
                if tr.seq.is_none() {
                    match self.seqs.get(&tr.id) {
                        Some(SeqState::Ready(s)) => {
                            tr.seq = Some(s.clone());
                            if let Some(i) = tr.inst {
                                if d.get(i).map(|x| !x.destroyed).unwrap_or(false) {
                                    let _ = d.set_prop(i, "Length", Val::Num(s.length));
                                    if matches!(d.get_prop(i, "Priority"), Some(Val::Num(p)) if *p < 0.0) {
                                        let _ = d.set_prop(i, "Priority", Val::Num(s.priority));
                                    }
                                }
                            }
                        }
                        Some(SeqState::Failed) => tr.failed = true,
                        _ => {}
                    }
                }
            }
            let mut levels: Levels = vec![];
            let mut pending = false;
            for tr in an.tracks.iter_mut() {
                if tr.seq.is_some() || tr.failed || tr.target == 0.0 {
                    continue;
                }
                tr.wait += dt;
                if tr.wait < HOLD_WHILE_LOADING {
                    pending = true;
                }
            }
            let mut i = 0;
            while i < an.tracks.len() {
                if an.tracks[i].failed {
                    an.tracks.remove(i);
                    continue;
                }
                let prio = Self::priority_of(d, &an.tracks[i]);
                let tr = &mut an.tracks[i];
                if pending && tr.seq.is_some() && tr.target == 0.0 && tr.w > 0.0 {
                    let w = (tr.w * tr.weight).min(1.0);
                    if w > 0.0 {
                        accumulate(&mut levels, prio, tr.seq.as_ref().unwrap(), tr.t, w);
                    }
                    i += 1;
                    continue;
                }
                if tr.seq.is_none() && tr.target > 0.0 {
                    i += 1;
                    continue;
                }
                let rate = if tr.fade > 0.0 { dt / tr.fade } else { f64::INFINITY };
                tr.w = if tr.target > tr.w { tr.target.min(tr.w + rate) } else { tr.target.max(tr.w - rate) };
                if tr.target == 0.0 && tr.w <= 0.0 {
                    let tr = an.tracks.remove(i);
                    if let Some(inst) = tr.inst {
                        if d.get(inst).map(|x| !x.destroyed).unwrap_or(false) {
                            let _ = d.set_prop(inst, "IsPlaying", Val::Bool(false));
                            events.push(Event { inst, name: "Stopped", arg: None });
                        }
                    }
                    continue;
                }
                let seq = match tr.seq.clone() {
                    Some(s) => s,
                    None => {
                        i += 1;
                        continue;
                    }
                };
                let prev = tr.t;
                let mut now = prev + dt * tr.speed;
                let len = seq.length;
                if now < 0.0 {
                    now = if seq.looped && len > 0.0 { ((now % len) + len) % len } else { 0.0 };
                }
                let mut reached: Vec<String> = vec![];
                if tr.target > 0.0 {
                    if len > 0.0 && now >= len {
                        for f in &seq.frames {
                            if f.0 > prev && f.0 <= len {
                                reached.push(f.1.clone());
                            }
                        }
                        if seq.looped {
                            now %= len;
                            for f in &seq.frames {
                                if f.0 <= now {
                                    reached.push(f.1.clone());
                                }
                            }
                        } else {
                            now = len;
                            tr.target = 0.0;
                            tr.fade = 0.0;
                            tr.w = 0.0;
                        }
                    } else {
                        for f in &seq.frames {
                            if f.0 > prev && f.0 <= now {
                                reached.push(f.1.clone());
                            }
                        }
                    }
                }
                tr.t = now;
                if let Some(inst) = tr.inst {
                    for n in reached {
                        if n != "Keyframe" && d.get(inst).map(|x| !x.destroyed).unwrap_or(false) {
                            events.push(Event { inst, name: "KeyframeReached", arg: Some(n) });
                        }
                    }
                }
                let w = (tr.w * tr.weight).min(1.0);
                if w > 0.0 {
                    accumulate(&mut levels, prio, &seq, tr.t, w);
                }
                i += 1;
            }
            let blend = resolve(&levels);
            let mut used: Vec<InstId> = vec![];
            if !blend.is_empty() {
                let mut all = vec![];
                d.descendants(model, &mut all);
                for j in all {
                    let is_motor = d.is_a(j, "Motor") || d.get(j).map(|x| x.class == "VelocityMotor").unwrap_or(false);
                    if !is_motor {
                        continue;
                    }
                    let name = match d.get_prop(j, "Part1") {
                        Some(Val::Ref(Some(p))) => d.name(*p).to_string(),
                        _ => continue,
                    };
                    if let Some(c) = blend.get(&name) {
                        new_anim.insert(j, *c);
                        used.push(j);
                    }
                }
            }
            an.joints = used;
            if !an.tracks.is_empty() {
                self.by_hum.insert(hum, an);
            }
        }
        self.anim = new_anim;
        events
    }

    pub fn snapshot(&self) -> Vec<(InstId, String, u64, f64, f64, f64)> {
        let mut out = vec![];
        for (hum, an) in &self.by_hum {
            for tr in &an.tracks {
                if tr.target > 0.0 {
                    out.push((*hum, tr.key.clone(), tr.id, tr.weight, tr.speed, tr.t));
                }
            }
        }
        out
    }

    pub fn drop_keys(&mut self, prefix: &str) {
        for an in self.by_hum.values_mut() {
            for tr in an.tracks.iter_mut() {
                if tr.key.starts_with(prefix) {
                    tr.target = 0.0;
                }
            }
        }
    }
}

pub fn step_motors(d: &mut DataModel, model: InstId, dt: f64) {
    let torso = match d.find_child(model, "Torso", false) {
        Some(t) => t,
        None => return,
    };
    let kids: Vec<InstId> = d.get(torso).map(|i| i.children.clone()).unwrap_or_default();
    for j in kids {
        if !d.is_a(j, "Motor") {
            continue;
        }
        let get = |d: &DataModel, k: &str| match d.get_prop(j, k) {
            Some(Val::Num(n)) => *n,
            _ => 0.0,
        };
        let cur = get(d, "CurrentAngle");
        let des = get(d, "DesiredAngle");
        let mv = get(d, "MaxVelocity").abs() * 60.0 * dt;
        if mv <= 0.0 {
            continue;
        }
        let diff = des - cur;
        let next = if diff.abs() <= mv { des } else { cur + diff.signum() * mv };
        let _ = d.set_prop(j, "CurrentAngle", Val::Num(next));
    }
}

pub fn set_motor(d: &mut DataModel, model: InstId, name: &str, desired: f64, max_vel: f64) {
    let torso = match d.find_child(model, "Torso", false) {
        Some(t) => t,
        None => return,
    };
    let j = match d.find_child(torso, name, false) {
        Some(j) if d.is_a(j, "Motor") => j,
        _ => return,
    };
    d.set_prop_quiet(j, "DesiredAngle", Val::Num(desired));
    d.set_prop_quiet(j, "MaxVelocity", Val::Num(max_vel));
}

pub fn holds_tool(d: &DataModel, model: InstId) -> bool {
    d.get(model).map(|i| i.children.iter().any(|c| d.get(*c).map(|x| x.class == "Tool").unwrap_or(false))).unwrap_or(false)
}

pub fn apply_limb_angles(d: &mut DataModel, model: InstId, angles: &[f64]) {
    let torso = match d.find_child(model, "Torso", false) {
        Some(t) => t,
        None => return,
    };
    for (i, name) in ["Right Shoulder", "Left Shoulder", "Right Hip", "Left Hip"].iter().enumerate() {
        let a = match angles.get(i) {
            Some(a) if a.is_finite() => *a,
            _ => continue,
        };
        if let Some(j) = d.find_child(torso, name, false) {
            if d.is_a(j, "Motor") {
                d.set_prop_quiet(j, "CurrentAngle", Val::Num((-7.0f64).max(7.0f64.min(a))));
            }
        }
    }
}

pub fn animate_procedural(d: &mut DataModel, model: InstId, state: u8, speed: f64, t: f64, holding: bool) {
    let pi = std::f64::consts::PI;
    let mv = 0.15;
    match state {
        2 | 3 => {
            set_motor(d, model, "Right Shoulder", if holding { pi / 2.0 } else { pi }, 0.5);
            set_motor(d, model, "Left Shoulder", -pi, 0.5);
            set_motor(d, model, "Right Hip", 0.0, 0.5);
            set_motor(d, model, "Left Hip", 0.0, 0.5);
            return;
        }
        8 => {
            let sw = (if speed > 0.5 { 0.9 } else { 0.25 }) * (t * if speed > 0.5 { 7.0 } else { 2.0 }).sin();
            set_motor(d, model, "Right Shoulder", if holding { pi / 2.0 } else { pi - sw }, 0.4);
            set_motor(d, model, "Left Shoulder", -(pi - sw), 0.4);
            set_motor(d, model, "Right Hip", sw * 0.6, 0.4);
            set_motor(d, model, "Left Hip", -sw * 0.6, 0.4);
            return;
        }
        4 => {
            set_motor(d, model, "Right Shoulder", pi / 2.0, 0.15);
            set_motor(d, model, "Left Shoulder", -pi / 2.0, 0.15);
            set_motor(d, model, "Right Hip", pi / 2.0, 0.15);
            set_motor(d, model, "Left Hip", -pi / 2.0, 0.15);
            return;
        }
        5 => {
            let dd = (t * 9.0).sin();
            set_motor(d, model, "Right Shoulder", pi - dd, 0.5);
            set_motor(d, model, "Left Shoulder", -(pi - dd), 0.5);
            set_motor(d, model, "Right Hip", dd, 0.5);
            set_motor(d, model, "Left Hip", dd, 0.5);
            return;
        }
        _ => {}
    }
    let (amp, freq) = if state == 1 && speed > 0.5 { (1.0, 9.0) } else { (0.1, 1.0) };
    let dd = amp * (t * freq).sin();
    set_motor(d, model, "Right Shoulder", if holding { pi / 2.0 } else { dd }, if holding { 0.5 } else { mv });
    set_motor(d, model, "Left Shoulder", dd, mv);
    set_motor(d, model, "Right Hip", -dd, mv);
    set_motor(d, model, "Left Hip", -dd, mv);
}

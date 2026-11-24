use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value as J};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use swirl_server::appearance;
use swirl_server::character;
use swirl_server::config::Config;
use swirl_server::fetch::Site;
use swirl_server::game::UserInfo;
use swirl_server::instance::{self, Cmd, Handle, Out, Spec};
use swirl_server::redeem::Redeemer;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;

const IDLE: Duration = Duration::from_secs(90);
const MAX_MSG: usize = 64 * 1024;
const MAX_RATE: u32 = 120;
const RATE_STRIKES: u32 = 6;
const MAX_SOCKETS_PER_USER: usize = 4;
const MAX_INSTANCES: usize = 60;
const MAX_STARTS_PER_USER: usize = 4;
const START_WINDOW: Duration = Duration::from_secs(300);
const DEFAULT_MAX: i64 = 20;
const JOIN_TIMEOUT: Duration = Duration::from_secs(20);

fn arg(name: &str) -> Option<String> {
    std::env::args().find_map(|a| a.strip_prefix(&format!("--{}=", name)).map(|s| s.to_string()))
}

struct Member {
    id: i64,
    name: String,
    joined: Instant,
    last_active: Instant,
    acts: u32,
    kinds: HashSet<String>,
    cells: HashSet<String>,
    px: Option<(f64, f64)>,
}

struct Place {
    job: String,
    place: u64,
    kind: String,
    private_id: i64,
    max: usize,
    era: i32,
    allow_gear: bool,
    handle: Handle,
    members: HashMap<u32, Member>,
    started: Instant,
    empty_since: Option<Instant>,
    dead: bool,
}

struct Sock {
    user: i64,
    ctl: UnboundedSender<Out>,
}

struct Sup {
    places: HashMap<String, Place>,
    socks: HashMap<u32, Sock>,
    starts: HashMap<i64, Vec<Instant>>,
    job_seq: u64,
    presence_dirty: bool,
}

struct Env {
    assets: String,
    data_dir: String,
    site: Option<Site>,
    redeemer: Option<Redeemer>,
    key: String,
    default_place: Option<u64>,
    default_era: i32,
    presence_path: Option<String>,
    log_file: String,
}

fn log(line: &str) {
    eprintln!("{}", line);
}

fn key_ok(given: &str, key: &str) -> bool {
    if key.is_empty() || given.len() != key.len() {
        return false;
    }
    let mut d = 0u8;
    for (a, b) in given.bytes().zip(key.bytes()) {
        d |= a ^ b;
    }
    d == 0
}

fn clean_hex(v: &J) -> Option<String> {
    let s = v.as_str()?;
    if s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(s.to_ascii_lowercase())
    } else {
        None
    }
}

fn user_from(info: &J) -> UserInfo {
    let dev_avatar = std::env::var("SWIRL_DEV_R15").is_ok();
    let dev_scale: J = if dev_avatar { std::env::var("SWIRL_DEV_SCALE").ok().and_then(|v| serde_json::from_str(&v).ok()).unwrap_or(J::Null) } else { J::Null };
    let info = &if dev_scale.is_object() { let mut i = info.clone(); i["scale"] = dev_scale; i } else { info.clone() };
    let perks = &info["perks"];
    let badge = perks["badge"].as_str().filter(|b| b.starts_with("/asset/?id=") && b[11..].len() <= 12 && b[11..].bytes().all(|c| c.is_ascii_digit()) && b.len() > 11).unwrap_or("").to_string();
    let mut scale = json!({"height": 1.0, "width": 1.0, "depth": 1.0, "head": 1.0});
    if let Some(sc) = info["scale"].as_object() {
        for k in ["height", "width", "depth", "head"] {
            if let Some(v) = sc.get(k).and_then(|v| v.as_f64()) {
                scale[k] = json!(v.clamp(0.5, 1.5));
            }
        }
        if sc.get("depth").is_none() {
            scale["depth"] = scale["width"].clone();
        }
    }
    let strs = |v: &J, n: usize| -> Vec<String> { v.as_array().map(|a| a.iter().map(|g| g.as_str().unwrap_or("").chars().take(24).collect::<String>()).take(n).collect()).unwrap_or_default() };
    UserInfo {
        id: info["userId"].as_i64().unwrap_or(0),
        name: info["username"].as_str().unwrap_or("Player").to_string(),
        membership: info["membership"].as_i64().unwrap_or(0),
        age: info["age"].as_i64().unwrap_or(0),
        dev: info["dev"].as_bool().unwrap_or(false),
        r15: info["avatarType"] == "R15" || dev_avatar,
        scale,
        perks_badge: badge,
        perks_chat: clean_hex(&perks["chatColor"]),
        perks_name: clean_hex(&perks["nameColor"]),
        animations: info["animations"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).filter(|s| !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit())).take(16).collect()).unwrap_or_default(),
        gear_genres: strs(&info["gearGenres"], 16),
        gear_map: info["gearMap"].clone(),
    }
}

async fn place_bytes(env: &Env, place: u64) -> Option<Vec<u8>> {
    if let Ok(b) = std::fs::read(format!("{}/assets/{}", env.assets, place)) {
        return Some(b);
    }
    match &env.site {
        Some(s) => s.get(&format!("/asset/?id={}", place)).await,
        None => None,
    }
}

enum Pick {
    Place(String),
    Busy(String),
    Full,
}

async fn choose(sup: &Rc<RefCell<Sup>>, env: &Rc<Env>, info: &J) -> Pick {
    let place = info["placeId"].as_u64().unwrap_or(0);
    let max = (info["maxPlayers"].as_i64().filter(|m| *m > 0).unwrap_or(DEFAULT_MAX)).clamp(1, 100) as usize;
    let allow_gear = info["allowGear"].as_bool() != Some(false);
    let era = if info["clientVersion"].as_i64() == Some(2018) { 2018 } else { 2012 };
    let uid = info["userId"].as_i64().unwrap_or(0);
    let key = info["serverKey"].as_str().unwrap_or("").to_string();
    let parsed = key.split_once(':').and_then(|(k, v)| if k == "priv" || k == "res" { v.parse::<i64>().ok().map(|n| (if k == "priv" { "private" } else { "reserved" }, n)) } else { None });
    let find = |sup: &Sup, kind: &str, pid: i64| -> Option<String> { sup.places.values().find(|p| !p.dead && p.place == place && p.kind == kind && p.private_id == pid && p.era == era).map(|p| p.job.clone()) };
    if let Some((kind, pid)) = parsed {
        let existing = find(&sup.borrow(), kind, pid);
        let job = match existing {
            Some(j) => j,
            None => match start_instance(sup, env, place, kind, pid, max, allow_gear, era, uid).await {
                Ok(j) => j,
                Err(e) => return Pick::Busy(e),
            },
        };
        let mut s = sup.borrow_mut();
        let p = s.places.get_mut(&job).unwrap();
        p.max = max;
        return if p.members.len() >= p.max { Pick::Full } else { Pick::Place(job) };
    }
    {
        let s = sup.borrow();
        if let Some(j) = key.strip_prefix("job:") {
            if let Some(p) = s.places.get(j) {
                if !p.dead && p.kind == "public" && p.place == place && p.era == era && p.members.len() < p.max && p.handle.failed.lock().unwrap().is_none() {
                    return Pick::Place(p.job.clone());
                }
            }
        }
        let mut open: Vec<&Place> = s.places.values().filter(|p| !p.dead && p.kind == "public" && p.place == place && p.era == era && p.handle.failed.lock().unwrap().is_none() && p.members.len() < p.max).collect();
        open.sort_by(|a, b| b.members.len().cmp(&a.members.len()).then(a.started.cmp(&b.started)));
        if let Some(p) = open.first() {
            return Pick::Place(p.job.clone());
        }
    }
    match start_instance(sup, env, place, "public", 0, max, allow_gear, era, uid).await {
        Ok(j) => Pick::Place(j),
        Err(e) => Pick::Busy(e),
    }
}

#[allow(clippy::too_many_arguments)]
async fn start_instance(sup: &Rc<RefCell<Sup>>, env: &Rc<Env>, place: u64, kind: &str, private_id: i64, max: usize, allow_gear: bool, era: i32, uid: i64) -> Result<String, String> {
    {
        let mut s = sup.borrow_mut();
        let live = s.places.values().filter(|p| !p.dead).count();
        if live >= MAX_INSTANCES {
            return Err("The server is busy right now. Try again in a moment.".into());
        }
        let now = Instant::now();
        let list = s.starts.entry(uid).or_default();
        list.retain(|t| now.duration_since(*t) < START_WINDOW);
        if list.len() >= MAX_STARTS_PER_USER {
            return Err("You have started too many servers recently. Wait a few minutes and try again.".into());
        }
        list.push(now);
    }
    let bytes = place_bytes(env, place).await.ok_or_else(|| "This game could not be started".to_string())?;
    let mut s = sup.borrow_mut();
    s.job_seq += 1;
    let job = format!("{:x}{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0), s.job_seq);
    let spec = Spec { place, job: job.clone(), kind: kind.to_string(), private_id, era, allow_gear, bytes: Arc::new(bytes), assets: env.assets.clone(), data_dir: env.data_dir.clone(), social: env.redeemer.as_ref().map(|r| (r.clone(), r.sibling_path("social"))), site: env.site.clone() };
    log(&format!("[place {} {}] starting (max {}, {} client)", place, if kind == "public" { job.clone() } else { format!("{} {}", kind, private_id) }, max, era));
    let handle = instance::spawn(spec);
    s.places.insert(job.clone(), Place { job: job.clone(), place, kind: kind.to_string(), private_id, max, era, allow_gear, handle, members: HashMap::new(), started: Instant::now(), empty_since: None, dead: false });
    Ok(job)
}

fn status_json(sup: &Sup) -> J {
    let places: Vec<J> = sup
        .places
        .values()
        .filter(|p| !p.dead && p.kind != "test")
        .map(|p| {
            json!({
                "id": p.place, "job": p.job, "kind": p.kind, "privateId": if p.private_id != 0 { json!(p.private_id) } else { J::Null }, "max": p.max,
                "ready": p.handle.ready.load(Ordering::SeqCst),
                "started": p.started.elapsed().as_millis() as u64,
                "players": p.members.len(),
                "users": p.members.values().map(|m| json!({"id": m.id, "name": m.name})).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({"ok": true, "places": places, "studio": []})
}

fn shutdown_place(sup: &mut Sup, job: &str, reason: Option<&str>) -> bool {
    if let Some(p) = sup.places.get_mut(job) {
        if p.dead {
            return false;
        }
        p.dead = true;
        if let Some(r) = reason {
            for cid in p.members.keys() {
                if let Some(s) = sup.socks.get(cid) {
                    let _ = s.ctl.send(Out::Kick(r.to_string()));
                }
            }
        }
        let _ = p.handle.tx.send(Cmd::Stop);
        sup.presence_dirty = true;
        return true;
    }
    false
}

async fn push_presence(sup: &Rc<RefCell<Sup>>, env: &Rc<Env>) {
    let (r, path) = match (&env.redeemer, &env.presence_path) {
        (Some(r), Some(p)) => (r, p.clone()),
        _ => return,
    };
    let users: Vec<J> = {
        let s = sup.borrow();
        let mut seen = HashSet::new();
        let mut out = vec![];
        for p in s.places.values().filter(|p| !p.dead) {
            for m in p.members.values() {
                if seen.insert(m.id) {
                    out.push(json!({"id": m.id, "kind": "play", "place": p.place, "job": p.job, "srv": p.kind, "players": p.members.len(), "idle": m.last_active.elapsed().as_secs(), "session": m.joined.elapsed().as_secs(), "acts": m.acts, "kinds": m.kinds.len(), "cells": m.cells.len()}));
                }
            }
        }
        out
    };
    let _ = r.post(&path, "application/json", &json!({"users": users, "server": "swirl"}).to_string()).await;
}

fn note_activity(m: &mut Member, text: &str) {
    let v: J = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    let t = v["t"].as_str().unwrap_or("");
    let mut active = false;
    if t == "char" {
        if let Some(c) = v["c"].as_array() {
            if c.len() >= 3 {
                if let (Some(x), Some(z)) = (c[0].as_f64(), c[2].as_f64()) {
                    match m.px {
                        None => m.px = Some((x, z)),
                        Some((px, pz)) => {
                            if (x - px).hypot(z - pz) > 1.5 {
                                m.px = Some((x, z));
                                active = true;
                                if m.cells.len() < 64 {
                                    m.cells.insert(format!("{},{}", (x / 8.0).round(), (z / 8.0).round()));
                                }
                            }
                        }
                    }
                }
            }
        }
    } else if t == "mouse" {
        active = v["b"] == "down";
    } else if ["jump", "key", "click", "chat", "act", "drive", "equip", "unequip", "drop", "hop"].contains(&t) {
        active = true;
    }
    if active {
        m.last_active = Instant::now();
        m.acts += 1;
        if m.kinds.len() < 16 {
            m.kinds.insert(t.to_string());
        }
    }
}

async fn read_http(stream: &mut TcpStream) -> Option<(String, String, std::collections::HashMap<String, String>, Vec<u8>)> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let split = loop {
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p;
        }
        if buf.len() > 16384 {
            return None;
        }
        let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut tmp)).await.ok()?.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..split]).to_string();
    let mut lines = head.split("\r\n");
    let first = lines.next()?;
    let mut parts = first.split(' ');
    let method = parts.next()?.to_string();
    let url = parts.next()?.to_string();
    let mut headers = HashMap::new();
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let len: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0).min(262144);
    let mut body = buf[split + 4..].to_vec();
    while body.len() < len {
        let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut tmp)).await.ok()?.ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    Some((method, url, headers, body))
}

async fn respond(stream: &mut TcpStream, code: u16, body: &str) {
    let text = match code {
        200 => "OK",
        403 => "Forbidden",
        _ => "Not Found",
    };
    let msg = format!("HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", code, text, body.len(), body);
    let _ = stream.write_all(msg.as_bytes()).await;
    let _ = stream.shutdown().await;
}

async fn http_request(mut stream: TcpStream, sup: Rc<RefCell<Sup>>, env: Rc<Env>) {
    let (method, url, headers, body) = match read_http(&mut stream).await {
        Some(r) => r,
        None => return,
    };
    let path = url.split('?').next().unwrap_or("").to_string();
    if path == "/status" && method == "GET" {
        let j = status_json(&sup.borrow()).to_string();
        return respond(&mut stream, 200, &j).await;
    }
    let keyed = ["/kick", "/shutdown", "/run", "/restart"].contains(&path.as_str()) && method == "POST";
    if !keyed {
        return respond(&mut stream, 404, "").await;
    }
    if !key_ok(headers.get("x-chocolate-key").map(|s| s.as_str()).unwrap_or(""), &env.key) {
        return respond(&mut stream, 403, "").await;
    }
    let q: J = serde_json::from_slice(&body).unwrap_or(J::Null);
    match path.as_str() {
        "/kick" => {
            let uid = q["userId"].as_i64().unwrap_or(0);
            let reason: String = q["reason"].as_str().unwrap_or("You have been removed from the game").chars().take(300).collect();
            let mut n = 0;
            if uid > 0 {
                for s in sup.borrow().socks.values() {
                    if s.user == uid {
                        let _ = s.ctl.send(Out::Kick(reason.clone()));
                        n += 1;
                    }
                }
            }
            if n > 0 {
                log(&format!("kicked user {} from {} session(s)", uid, n));
            }
            respond(&mut stream, 200, &json!({"ok": true, "kicked": n}).to_string()).await;
        }
        "/shutdown" => {
            let jobs: Vec<String> = {
                let s = sup.borrow();
                s.places
                    .values()
                    .filter(|p| !p.dead)
                    .filter(|p| match (q["job"].as_str(), q["placeId"].as_i64()) {
                        (Some(j), _) => p.job == j,
                        (None, Some(pl)) => p.place as i64 == pl,
                        _ => false,
                    })
                    .map(|p| p.job.clone())
                    .collect()
            };
            let mut n = 0;
            for j in jobs {
                if shutdown_place(&mut sup.borrow_mut(), &j, Some("This server was shut down")) {
                    n += 1;
                }
            }
            respond(&mut stream, 200, &json!({"ok": true, "stopped": n}).to_string()).await;
        }
        "/run" => {
            let source = q["source"].as_str().unwrap_or("").to_string();
            let job = q["job"].as_str().unwrap_or("").to_string();
            if source.trim().is_empty() || source.len() > 200000 {
                return respond(&mut stream, 200, &json!({"ok": false, "error": "No code given"}).to_string()).await;
            }
            let cap = {
                let s = sup.borrow();
                match s.places.get(&job) {
                    Some(p) if !p.dead && p.kind != "test" => {
                        if !p.handle.ready.load(Ordering::SeqCst) {
                            None
                        } else {
                            *p.handle.capture.lock().unwrap() = Some(vec![]);
                            let by: String = q["by"].as_str().unwrap_or("a developer").chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '.' || *c == '-' || *c == '_').take(40).collect();
                            let _ = p.handle.tx.send(Cmd::Run { source: source.clone(), by });
                            Some(p.handle.capture.clone())
                        }
                    }
                    _ => None,
                }
            };
            let cap = match cap {
                Some(c) => c,
                None => return respond(&mut stream, 200, &json!({"ok": false, "error": "That server is not running"}).to_string()).await,
            };
            let wait = q["wait"].as_i64().filter(|w| *w > 0).unwrap_or(1500).clamp(200, 5000) as u64;
            tokio::time::sleep(Duration::from_millis(wait)).await;
            let out = cap.lock().unwrap().take().unwrap_or_default();
            respond(&mut stream, 200, &json!({"ok": true, "output": out}).to_string()).await;
        }
        _ => {
            let n = sup.borrow().places.values().filter(|p| !p.dead).count();
            respond(&mut stream, 200, &json!({"ok": true, "places": n}).to_string()).await;
            restart_self(&sup, &env).await;
        }
    }
}

async fn restart_self(sup: &Rc<RefCell<Sup>>, env: &Rc<Env>) {
    log("restarting");
    let jobs: Vec<String> = sup.borrow().places.keys().cloned().collect();
    for j in jobs {
        shutdown_place(&mut sup.borrow_mut(), &j, Some("The game server is restarting"));
    }
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let exe = std::env::current_exe().ok();
    if let Some(exe) = exe {
        let mut cmd = std::process::Command::new("setsid");
        cmd.arg("sh").arg("-c").arg("sleep 2; exec \"$0\" \"$@\"").arg(exe);
        for a in std::env::args().skip(1) {
            cmd.arg(a);
        }
        if let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(&env.log_file) {
            if let Ok(f2) = f.try_clone() {
                cmd.stdout(f).stderr(f2);
            }
        }
        cmd.stdin(std::process::Stdio::null());
        let _ = cmd.spawn();
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    std::process::exit(0);
}

const MAX_SOCKETS_PER_IP: u32 = 40;
const MAX_PENDING_PER_IP: u32 = 8;
const MAX_CONNECTS_PER_MIN: usize = 90;
const MAX_BAD_TICKETS: u32 = 20;
const MAX_REDEEMS_INFLIGHT: u32 = 24;

struct IpEntry {
    open: u32,
    pending: u32,
    connects: Vec<Instant>,
    bad: u32,
    bad_at: Option<Instant>,
}

thread_local! {
    static IPS: RefCell<HashMap<String, IpEntry>> = RefCell::new(HashMap::new());
    static REDEEMS: std::cell::Cell<u32> = std::cell::Cell::new(0);
}

fn private_ip(s: &str) -> bool {
    let t = s.to_ascii_lowercase();
    t.starts_with("127.") || t.starts_with("10.") || t.starts_with("192.168.") || t == "::1" || t.starts_with("fc") || t.starts_with("fd")
        || (t.starts_with("172.") && t.split('.').nth(1).and_then(|x| x.parse::<u32>().ok()).map(|n| (16..=31).contains(&n)).unwrap_or(false))
}

fn client_ip(peer: &str, xff: &str, real: &str) -> String {
    let peer = peer.trim_start_matches("::ffff:").to_string();
    if private_ip(&peer) {
        let src = if xff.is_empty() { real } else { xff };
        let hops: Vec<String> = src
            .split(',')
            .map(|h| h.trim().trim_start_matches("::ffff:").to_string())
            .filter(|h| h.len() >= 3 && h.len() <= 45 && h.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.'))
            .collect();
        for h in hops.iter().rev() {
            if !private_ip(h) {
                return h.clone();
            }
        }
        if let Some(h) = hops.first() {
            return h.clone();
        }
    }
    if peer.is_empty() { "unknown".into() } else { peer }
}

fn ip_gate(ip: &str) -> bool {
    IPS.with(|m| {
        let mut m = m.borrow_mut();
        let now = Instant::now();
        m.retain(|_, e| {
            e.connects.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
            if e.bad_at.map(|t| now.duration_since(t) > Duration::from_secs(60)).unwrap_or(true) {
                e.bad = 0;
            }
            e.open > 0 || e.pending > 0 || !e.connects.is_empty() || e.bad > 0
        });
        let e = m.entry(ip.to_string()).or_insert(IpEntry { open: 0, pending: 0, connects: vec![], bad: 0, bad_at: None });
        if e.open >= MAX_SOCKETS_PER_IP || e.pending >= MAX_PENDING_PER_IP || e.connects.len() >= MAX_CONNECTS_PER_MIN || e.bad >= MAX_BAD_TICKETS {
            log(&format!("refused connection from {}: open {}, pending {}, recent {}, bad tickets {}", ip, e.open, e.pending, e.connects.len(), e.bad));
            return false;
        }
        e.connects.push(now);
        true
    })
}

struct IpGuard {
    ip: String,
    pending: std::cell::Cell<bool>,
}

impl IpGuard {
    fn new(ip: String) -> IpGuard {
        IPS.with(|m| {
            if let Some(e) = m.borrow_mut().get_mut(&ip) {
                e.open += 1;
                e.pending += 1;
            }
        });
        IpGuard { ip, pending: std::cell::Cell::new(true) }
    }

    fn release(&self) {
        if self.pending.replace(false) {
            IPS.with(|m| {
                if let Some(e) = m.borrow_mut().get_mut(&self.ip) {
                    e.pending = e.pending.saturating_sub(1);
                }
            });
        }
    }

    fn bad(&self) {
        IPS.with(|m| {
            if let Some(e) = m.borrow_mut().get_mut(&self.ip) {
                e.bad += 1;
                e.bad_at = Some(Instant::now());
            }
        });
    }
}

impl Drop for IpGuard {
    fn drop(&mut self) {
        self.release();
        IPS.with(|m| {
            if let Some(e) = m.borrow_mut().get_mut(&self.ip) {
                e.open = e.open.saturating_sub(1);
            }
        });
    }
}

struct RedeemSlot;

impl RedeemSlot {
    fn take() -> Option<RedeemSlot> {
        REDEEMS.with(|c| {
            if c.get() >= MAX_REDEEMS_INFLIGHT {
                None
            } else {
                c.set(c.get() + 1);
                Some(RedeemSlot)
            }
        })
    }
}

impl Drop for RedeemSlot {
    fn drop(&mut self) {
        REDEEMS.with(|c| c.set(c.get().saturating_sub(1)));
    }
}

async fn ws_connection(stream: TcpStream, cid: u32, sup: Rc<RefCell<Sup>>, env: Rc<Env>) {
    let uri = Rc::new(RefCell::new(String::new()));
    let u2 = uri.clone();
    let peer = stream.peer_addr().map(|a| a.ip().to_string()).unwrap_or_default();
    let ip = Rc::new(RefCell::new(String::new()));
    let ip2 = ip.clone();
    let ws = match tokio_tungstenite::accept_hdr_async(stream, move |req: &tokio_tungstenite::tungstenite::handshake::server::Request, resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
        *u2.borrow_mut() = req.uri().to_string();
        let h = |n: &str| req.headers().get(n).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
        let cip = client_ip(&peer, &h("x-forwarded-for"), &h("x-real-ip"));
        if !ip_gate(&cip) {
            let mut r = tokio_tungstenite::tungstenite::handshake::server::ErrorResponse::new(None);
            *r.status_mut() = tokio_tungstenite::tungstenite::http::StatusCode::TOO_MANY_REQUESTS;
            return Err(r);
        }
        *ip2.borrow_mut() = cip;
        Ok(resp)
    })
    .await
    {
        Ok(w) => w,
        Err(_) => return,
    };
    let guard = IpGuard::new(ip.borrow().clone());
    let join_deadline = Instant::now() + JOIN_TIMEOUT;
    let (mut tx, mut rx) = ws.split();
    let ticket = uri.borrow().split("ticket=").nth(1).map(|s| s.split('&').next().unwrap_or("").to_string()).unwrap_or_default();
    let kick = |reason: String| serde_json::json!({"t": "kick", "m": reason}).to_string();
    let info: J = match &env.redeemer {
        Some(r) => {
            let valid = ticket.len() == 64 && ticket.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'));
            if !valid {
                guard.bad();
                let _ = tx.close().await;
                return;
            }
            let slot = RedeemSlot::take();
            if slot.is_none() {
                let _ = tx.send(Message::Text(kick("The server is busy. Try again in a moment.".into()))).await;
                let _ = tx.close().await;
                return;
            }
            let res = match tokio::time::timeout(join_deadline.saturating_duration_since(Instant::now()), r.redeem(&ticket)).await {
                Ok(v) => v,
                Err(_) => {
                    drop(slot);
                    let _ = tx.send(Message::Text(kick("Joining the game took too long. Reload the page to try again.".into()))).await;
                    let _ = tx.close().await;
                    return;
                }
            };
            drop(slot);
            guard.release();
            match res {
            Some(j) => j,
            None => {
                guard.bad();
                let _ = tx.send(Message::Text(kick("Your join ticket is invalid or has expired. Reload the page to try again.".into()))).await;
                let _ = tx.close().await;
                return;
            }
            }
        }
        None => json!({"ok": true, "mode": "play", "placeId": env.default_place.unwrap_or(0), "userId": 1000 + cid as i64, "username": format!("Player{}", cid), "clientVersion": env.default_era, "maxPlayers": 20, "allowGear": true}),
    };
    if info["mode"].as_str().map(|m| m != "play").unwrap_or(false) {
        let _ = tx.send(Message::Text(kick("This server only hosts game sessions.".into()))).await;
        let _ = tx.close().await;
        return;
    }
    let uid = info["userId"].as_i64().unwrap_or(0);
    {
        let n = sup.borrow().socks.values().filter(|s| s.user == uid).count();
        if n >= MAX_SOCKETS_PER_USER {
            let _ = tx.send(Message::Text(kick("You have too many game connections open.".into()))).await;
            let _ = tx.close().await;
            return;
        }
    }
    let (otx, mut orx) = unbounded_channel::<Out>();
    for s in sup.borrow().socks.values() {
        if s.user == uid {
            let _ = s.ctl.send(Out::Kick("You joined from another session".into()));
        }
    }
    sup.borrow_mut().socks.insert(cid, Sock { user: uid, ctl: otx.clone() });
    let job = match choose(&sup, &env, &info).await {
        Pick::Place(j) => j,
        Pick::Busy(m) => {
            sup.borrow_mut().socks.remove(&cid);
            let _ = tx.send(Message::Text(kick(m))).await;
            let _ = tx.close().await;
            return;
        }
        Pick::Full => {
            sup.borrow_mut().socks.remove(&cid);
            let _ = tx.send(Message::Text(kick("This server is full. Try again in a moment.".into()))).await;
            let _ = tx.close().await;
            return;
        }
    };
    let user = user_from(&info);
    let allow_gear = sup.borrow().places.get(&job).map(|p| p.allow_gear).unwrap_or(false);
    let look_fut = async {
        match &env.site {
        Some(st) => {
            let mut l = appearance::fetch(st, &env.assets, user.id).await;
            if allow_gear {
                let mut ids = appearance::gear_ids(st, user.id).await;
                let genres = &user.gear_genres;
                if !genres.is_empty() {
                    ids.retain(|id| user.gear_map[id.to_string()].as_array().map(|g| g.iter().any(|x| x.as_str().map(|s| genres.iter().any(|a| a == s)).unwrap_or(false))).unwrap_or(false));
                }
                l.gear = appearance::load_gear(st, &env.assets, &ids).await;
            }
            l
        }
        None => appearance::Look::default(),
        }
    };
    let look = match tokio::time::timeout(join_deadline.saturating_duration_since(Instant::now()), look_fut).await {
        Ok(l) => l,
        Err(_) => {
            sup.borrow_mut().socks.remove(&cid);
            let _ = tx.send(Message::Text(kick("Joining the game took too long. Reload the page to try again.".into()))).await;
            let _ = tx.close().await;
            return;
        }
    };

    let cmd_tx = {
        let mut s = sup.borrow_mut();
        let p = match s.places.get_mut(&job) {
            Some(p) if !p.dead => p,
            _ => {
                s.socks.remove(&cid);
                drop(s);
                let _ = tx.send(Message::Text(kick("The game server stopped.".into()))).await;
                let _ = tx.close().await;
                return;
            }
        };
        let now = Instant::now();
        p.empty_since = None;
        p.members.insert(cid, Member { id: user.id, name: user.name.clone(), joined: now, last_active: now, acts: 0, kinds: HashSet::new(), cells: HashSet::new(), px: None });
        let t = p.handle.tx.clone();
        let _ = t.send(Cmd::Join { cid, user: user.clone(), look, out: otx.clone() });
        guard.release();
        s.presence_dirty = true;
        t
    };
    log(&format!("[place {} {}] {} ({}) connected", info["placeId"], job, user.name, user.id));
    let mut window = Instant::now();
    let mut count = 0u32;
    let mut over = false;
    let mut strikes = 0u32;
    let mut last_seen = Instant::now();
    let mut ping = tokio::time::interval(Duration::from_secs(20));
    let _ = JOIN_TIMEOUT;
    loop {
        tokio::select! {
            m = rx.next() => {
                last_seen = Instant::now();
                match m {
                    Some(Ok(Message::Text(t))) => {
                        if t.len() > MAX_MSG {
                            let _ = tx.send(Message::Text(kick("You were disconnected for sending too much data.".into()))).await;
                            break;
                        }
                        if window.elapsed() > Duration::from_secs(1) {
                            window = Instant::now();
                            count = 0;
                            strikes = if over { strikes + 1 } else { 0 };
                            over = false;
                            if strikes >= RATE_STRIKES {
                                let _ = tx.send(Message::Text(kick("You were disconnected for sending too many messages.".into()))).await;
                                break;
                            }
                        }
                        count += 1;
                        if count > MAX_RATE {
                            over = true;
                            continue;
                        }
                        let v: Option<J> = serde_json::from_str(&t).ok();
                        if let Some(v) = &v {
                            if v["t"] == "ping" {
                                let _ = tx.send(Message::Text(json!({"t": "pong", "x": v["x"]}).to_string())).await;
                                continue;
                            }
                        }
                        if let Some(p) = sup.borrow_mut().places.get_mut(&job) {
                            if let Some(m) = p.members.get_mut(&cid) {
                                note_activity(m, &t);
                            }
                        }
                        let _ = cmd_tx.send(Cmd::Msg { cid, text: t });
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
            out = orx.recv() => {
                match out {
                    Some(Out::Text(s)) => { if tx.send(Message::Text(s)).await.is_err() { break; } }
                    Some(Out::Kick(r)) => {
                        let _ = tx.send(Message::Text(kick(r))).await;
                        let _ = tx.close().await;
                        break;
                    }
                    None => break,
                }
            }
            _ = ping.tick() => {
                if last_seen.elapsed() > Duration::from_secs(60) {
                    break;
                }
                let _ = tx.send(Message::Ping(vec![])).await;
            }
        }
    }
    let _ = cmd_tx.send(Cmd::Leave { cid });
    let mut s = sup.borrow_mut();
    s.socks.remove(&cid);
    if let Some(p) = s.places.get_mut(&job) {
        p.members.remove(&cid);
        if p.members.is_empty() {
            p.empty_since = Some(Instant::now());
        }
    }
    s.presence_dirty = true;
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let port: u16 = arg("port").and_then(|s| s.parse().ok()).or_else(|| Config::load(&arg("config").unwrap_or_else(swirl_server::config::default_config_path)).get("port").and_then(|p| p.parse().ok())).expect("--port=N required");
    if port == 4201 || port == 4202 {
        eprintln!("refusing to use a live Chocolate port");
        std::process::exit(2);
    }
    let cfg = Config::load(&arg("config").unwrap_or_else(swirl_server::config::default_config_path));
    character::set_content_dir(&arg("data").unwrap_or_else(swirl_server::config::default_data_dir));
    let assets = arg("assets").or_else(|| cfg.get("assets_dir")).unwrap_or("/var/www/assets".into());
    let data_dir = arg("store").or_else(|| cfg.get("data_dir")).unwrap_or_else(swirl_server::config::default_store_dir);
    let no_redeem = std::env::args().any(|a| a == "--no-redeem");
    let redeem_url = arg("redeem-url").or_else(|| cfg.get("redeem_url"));
    let redeem_key = match arg("redeem-key-file") {
        Some(f) => Some(std::fs::read_to_string(&f).expect("redeem key file").trim().to_string()),
        None => cfg.get("redeem_key"),
    };
    let redeemer = match (no_redeem, redeem_url, redeem_key.clone()) {
        (false, Some(u), Some(k)) if !k.is_empty() => Some(Redeemer::parse(&u, k).expect("redeem url must be http on loopback")),
        _ => None,
    };
    if redeemer.is_none() {
        eprintln!("tickets are not checked: no redeem key configured or --no-redeem given");
    }
    let presence_path = redeemer.as_ref().map(|r| cfg.get("presence_path").unwrap_or_else(|| r.sibling_path("presence")));
    let site: Option<Site> = arg("site").or_else(|| cfg.get("site_url")).and_then(|u| Site::parse(&u));
    let env = Rc::new(Env {
        assets,
        data_dir,
        site,
        redeemer,
        key: redeem_key.unwrap_or_default(),
        default_place: arg("place").and_then(|s| s.parse().ok()),
        default_era: arg("era").and_then(|s| s.parse().ok()).unwrap_or(2012),
        presence_path,
        log_file: cfg.get("log_file").unwrap_or_else(|| "/tmp/swirl-server.log".into()),
    });
    let sup = Rc::new(RefCell::new(Sup { places: HashMap::new(), socks: HashMap::new(), starts: HashMap::new(), job_seq: 0, presence_dirty: false }));
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            let listener = TcpListener::bind(("127.0.0.1", port)).await.expect("bind");
            eprintln!("swirl-server listening on 127.0.0.1:{}", port);
            {
                let sup = sup.clone();
                let env = env.clone();
                tokio::task::spawn_local(async move {
                    push_presence(&sup, &env).await;
                    let mut iv = tokio::time::interval(Duration::from_secs(1));
                    let mut since = 0u32;
                    loop {
                        iv.tick().await;
                        since += 1;
                        let now = Instant::now();
                        let (dirty, stale): (bool, Vec<String>) = {
                            let mut s = sup.borrow_mut();
                            let d = std::mem::take(&mut s.presence_dirty);
                            let stale = s.places.values().filter(|p| !p.dead && p.members.is_empty() && (p.empty_since.map(|t| now.duration_since(t) > IDLE).unwrap_or(false) || p.handle.failed.lock().unwrap().is_some())).map(|p| p.job.clone()).collect();
                            (d, stale)
                        };
                        for j in stale {
                            log(&format!("[place {}] idle, stopping", j));
                            shutdown_place(&mut sup.borrow_mut(), &j, None);
                        }
                        sup.borrow_mut().places.retain(|_, p| !p.dead || p.empty_since.map(|t| now.duration_since(t) < Duration::from_secs(10)).unwrap_or(false) || !p.members.is_empty());
                        if dirty || since >= 60 {
                            since = 0;
                            push_presence(&sup, &env).await;
                        }
                    }
                });
            }
            let mut cid = 0u32;
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(x) => x,
                    Err(_) => continue,
                };
                cid += 1;
                let mut head = [0u8; 2048];
                let n = match tokio::time::timeout(Duration::from_secs(5), stream.peek(&mut head)).await {
                    Ok(Ok(n)) => n,
                    _ => continue,
                };
                let text = String::from_utf8_lossy(&head[..n]).to_ascii_lowercase();
                let sup2 = sup.clone();
                let env2 = env.clone();
                if text.contains("upgrade: websocket") {
                    tokio::task::spawn_local(ws_connection(stream, cid, sup2, env2));
                } else {
                    tokio::task::spawn_local(http_request(stream, sup2, env2));
                }
            }
        })
        .await;
}

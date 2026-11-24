use crate::appearance::Look;
use crate::game::{Game, UserInfo};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

pub enum Out {
    Text(String),
    Kick(String),
}

pub const MAX_BUFFERED: usize = 48 * 1024 * 1024;

#[derive(Clone)]
pub struct OutTx {
    tx: UnboundedSender<Out>,
    pub buffered: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
}

impl OutTx {
    pub fn new(tx: UnboundedSender<Out>) -> OutTx {
        OutTx { tx, buffered: Arc::new(AtomicUsize::new(0)), dropped: Arc::new(AtomicBool::new(false)) }
    }

    pub fn send(&self, out: Out) -> bool {
        match out {
            Out::Text(s) => {
                if self.dropped.load(Ordering::Relaxed) {
                    return false;
                }
                if self.buffered.load(Ordering::Relaxed) > MAX_BUFFERED {
                    self.dropped.store(true, Ordering::Relaxed);
                    return self.tx.send(Out::Kick("You were disconnected because your connection could not keep up with the game.".into())).is_ok();
                }
                self.buffered.fetch_add(s.len(), Ordering::Relaxed);
                self.tx.send(Out::Text(s)).is_ok()
            }
            k => self.tx.send(k).is_ok(),
        }
    }
}

pub enum Cmd {
    Join { cid: u32, user: UserInfo, look: Look, out: OutTx },
    Msg { cid: u32, text: String },
    Leave { cid: u32 },
    Run { source: String, by: String },
    Dbg(serde_json::Value),
    Stop,
}

#[derive(Clone)]
pub struct Spec {
    pub place: u64,
    pub job: String,
    pub kind: String,
    pub private_id: i64,
    pub era: i32,
    pub allow_gear: bool,
    pub bytes: Arc<Vec<u8>>,
    pub assets: String,
    pub data_dir: String,
    pub social: Option<(crate::redeem::Redeemer, String)>,
    pub site: Option<crate::fetch::Site>,
    pub test: Option<serde_json::Value>,
}

pub struct Handle {
    pub tx: UnboundedSender<Cmd>,
    pub ready: Arc<AtomicBool>,
    pub failed: Arc<Mutex<Option<String>>>,
    pub capture: Arc<Mutex<Option<Vec<String>>>>,
    pub frames: Arc<AtomicUsize>,
    pub debug: Arc<Mutex<Vec<serde_json::Value>>>,
}

pub fn spawn(spec: Spec) -> Handle {
    let (tx, mut rx) = unbounded_channel::<Cmd>();
    let ready = Arc::new(AtomicBool::new(false));
    let failed = Arc::new(Mutex::new(None));
    let capture: Arc<Mutex<Option<Vec<String>>>> = Arc::new(Mutex::new(None));
    let frames = Arc::new(AtomicUsize::new(0));
    let debug: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(vec![]));
    let dbg2 = debug.clone();
    let (r2, f2, c2, fr2) = (ready.clone(), failed.clone(), capture.clone(), frames.clone());
    std::thread::Builder::new()
        .name(format!("place-{}", spec.place))
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let tag = format!("[place {} {}]", spec.place, if spec.kind == "public" { spec.job.clone() } else { format!("{} {}", spec.kind, spec.private_id) });
                let mut game = match Game::load_opts(spec.place, &spec.bytes, spec.era, &spec.assets, &spec.data_dir, spec.test.clone()) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("{} failed to start: {}", tag, e);
                        *f2.lock().unwrap() = Some(e);
                        return;
                    }
                };
                {
                    let libs = preload_libraries(&spec.assets, spec.site.as_ref()).await;
                    game.api.borrow_mut().lib_src = libs;
                }
                game.allow_gear = spec.allow_gear;
                game.job = spec.job.clone();
                r2.store(true, Ordering::SeqCst);
                let (stx, mut srx) = unbounded_channel::<(u32, crate::api::SocialKind, Option<serde_json::Value>)>();
                let social = spec.social.clone();
                let (utx, mut urx) = unbounded_channel::<(u64, Option<Vec<u8>>)>();
                let (ltx, mut lrx) = unbounded_channel::<(u32, Result<String, String>)>();
                let assets_dir = spec.assets.clone();
                let (atx, mut arx) = unbounded_channel::<(u32, Option<Vec<u8>>)>();
                let site = spec.site.clone();
                let mut clients: HashMap<u32, (InstIdAlias, OutTx)> = HashMap::new();
                let mut iv = tokio::time::interval(std::time::Duration::from_micros(33333));
                iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    tokio::select! {
                        cmd = rx.recv() => {
                            match cmd {
                                None | Some(Cmd::Stop) => break,
                                Some(Cmd::Join { cid, user, look, out }) => {
                                    let name = user.name.clone();
                                    let (init, player) = game.join_user(&user, look);
                                    let _ = out.send(Out::Text(init.to_string()));
                                    let _ = out.send(Out::Text(serde_json::json!({"t": "sys", "m": format!("{} has joined the game.", name)}).to_string()));
                                    clients.insert(cid, (player, out));
                                }
                                Some(Cmd::Msg { cid, text }) => {
                                    if let Some((p, _)) = clients.get(&cid) {
                                        game.message(*p, &text);
                                    }
                                }
                                Some(Cmd::Leave { cid }) => {
                                    if let Some((p, _)) = clients.remove(&cid) {
                                        game.leave(p);
                                    }
                                }
                                Some(Cmd::Dbg(m)) => {
                                    game.debug_message(&m);
                                }
                                Some(Cmd::Run { source, by }) => {
                                    game.run_code(&source, &by);
                                }
                            }
                        }
                        res = arx.recv() => {
                            if let Some((token, bytes)) = res {
                                game.asset_done(token, bytes);
                            }
                        }
                        res = urx.recv() => {
                            if let Some((id, bytes)) = res {
                                game.union_done(id, bytes);
                            }
                        }
                        res = lrx.recv() => {
                            if let Some((id, src)) = res {
                                game.host.provide_linked(id, src);
                            }
                        }
                        res = srx.recv() => {
                            if let Some((token, kind, j)) = res {
                                game.social_done(token, kind, j);
                            }
                        }
                        _ = iv.tick() => {
                            for (token, id) in game.asset_requests() {
                                let (atx, site) = (atx.clone(), site.clone());
                                tokio::task::spawn_local(async move {
                                    let bytes = match site {
                                        Some(s) => s.get(&format!("/asset/?id={}", id)).await,
                                        None => None,
                                    };
                                    let _ = atx.send((token, bytes));
                                });
                            }
                            for id in game.union_requests() {
                                let (utx, site, dir) = (utx.clone(), site.clone(), assets_dir.clone());
                                tokio::task::spawn_local(async move {
                                    let bytes = match std::fs::read(format!("{}/assets/{}", dir, id)) {
                                        Ok(b) => Some(b),
                                        Err(_) => match site {
                                            Some(s) => s.get(&format!("/asset/?id={}", id)).await,
                                            None => None,
                                        },
                                    };
                                    let _ = utx.send((id, bytes));
                                });
                            }
                            for (id, url) in game.host.take_linked() {
                                let (ltx, site, dir) = (ltx.clone(), site.clone(), assets_dir.clone());
                                tokio::task::spawn_local(async move {
                                    let r = fetch_linked(&dir, site.as_ref(), &url).await;
                                    let _ = ltx.send((id, r));
                                });
                            }
                            for req in game.social_requests() {
                                match &social {
                                    Some((r, path)) => {
                                        let path = if matches!(req.kind, crate::api::SocialKind::Report) { r.sibling_path("report") } else { path.clone() };
                                        let (r, stx) = (r.clone(), stx.clone());
                                        tokio::task::spawn_local(async move {
                                            let out = r.post(&path, "application/json", &req.body.to_string()).await.and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim()).ok()).filter(|j| j["ok"].as_bool() == Some(true));
                                            let _ = stx.send((req.token, req.kind, out));
                                        });
                                    }
                                    None => {
                                        let _ = stx.send((req.token, req.kind, None));
                                    }
                                }
                            }
                            fr2.fetch_add(1, Ordering::Relaxed);
                            let logs = game.tick(1.0 / 30.0);
                            let paused = game.take_debug();
                            if !paused.is_empty() {
                                dbg2.lock().unwrap().extend(paused);
                            }
                            for l in logs {
                                let line = format!("[{}] {}", l.kind, l.msg);
                                eprintln!("{} {}", tag, line);
                                if matches!(l.kind.as_str(), "error" | "warn" | "print") {
                                    if let Some(c) = c2.lock().unwrap().as_mut() {
                                        if c.len() < 200 {
                                            c.push(line.chars().take(2000).collect());
                                        }
                                    }
                                }
                            }
                            let remote = std::mem::take(&mut game.remote_out);
                            for (target, m) in remote {
                                let text = m.to_string();
                                for (pl, tx) in clients.values() {
                                    if target.is_empty() || target.contains(pl) {
                                        let _ = tx.send(Out::Text(text.clone()));
                                    }
                                }
                            }
                            let players: Vec<InstIdAlias> = clients.values().map(|(p, _)| *p).collect();
                            for (p, m) in game.flush_all(&players) {
                                for (pl, tx) in clients.values() {
                                    if *pl == p {
                                        let _ = tx.send(Out::Text(m.clone()));
                                    }
                                }
                            }
                            let queued = std::mem::take(&mut game.api.borrow_mut().kicks);
                            game.kicks.extend(queued);
                            for (target, reason) in std::mem::take(&mut game.kicks) {
                                for (pl, tx) in clients.values() {
                                    if *pl == target {
                                        let _ = tx.send(Out::Kick(reason.clone()));
                                    }
                                }
                            }
                        }
                    }
                }
                game.stop();
            });
        })
        .expect("spawn instance thread");
    Handle { tx, ready, failed, capture, frames, debug }
}

type InstIdAlias = u32;

#[cfg(test)]
mod out_tests {
    use super::*;

    #[test]
    fn drops_past_the_buffer_limit() {
        let (tx, mut rx) = unbounded_channel::<Out>();
        let o = OutTx::new(tx);
        let chunk = "x".repeat(1024 * 1024);
        let mut sent = 0;
        for _ in 0..60 {
            o.send(Out::Text(chunk.clone()));
            sent += 1;
        }
        let mut texts = 0;
        let mut kicks = 0;
        while let Ok(m) = rx.try_recv() {
            match m {
                Out::Text(_) => texts += 1,
                Out::Kick(_) => kicks += 1,
            }
        }
        assert_eq!(sent, 60);
        assert_eq!(texts, 49);
        assert_eq!(kicks, 1);
    }
}

pub const LIBRARIES: &[(&str, u64)] = &[("RbxGui", 45284430), ("RbxGear", 45374389), ("RbxUtility", 60595411), ("RbxStamper", 73157242)];

pub fn linked_from_disk(dir: &str, url: &str) -> Option<Result<String, String>> {
    let id = match crate::api::anim_asset_id(url) {
        Some(i) => i,
        None => return Some(Err("bad asset id".into())),
    };
    let b = std::fs::read(format!("{}/assets/{}", dir, id)).ok()?;
    if b.len() > 32 * 1024 * 1024 {
        return Some(Err("asset too large".into()));
    }
    Some(Ok(String::from_utf8_lossy(&b).to_string()))
}

async fn fetch_linked(dir: &str, site: Option<&crate::fetch::Site>, url: &str) -> Result<String, String> {
    if let Some(r) = linked_from_disk(dir, url) {
        return r;
    }
    let id = crate::api::anim_asset_id(url).ok_or_else(|| "bad asset id".to_string())?;
    match site {
        Some(s) => match s.get(&format!("/asset/?id={}", id)).await {
            Some(b) => Ok(String::from_utf8_lossy(&b).to_string()),
            None => Err("HTTP error".into()),
        },
        None => Err("endpoint not configured".into()),
    }
}

pub fn clean_library(mut s: String) -> Option<String> {
    if let Some(r) = s.strip_prefix('\u{feff}') {
        s = r.to_string();
    }
    if s.starts_with('%') {
        s = match s.find('\n') {
            Some(i) => s[i + 1..].to_string(),
            None => String::new(),
        };
    }
    if s.starts_with('<') {
        return None;
    }
    Some(s)
}

pub fn libraries_disk(dir: &str) -> HashMap<u64, String> {
    let mut out = HashMap::new();
    for (_, id) in LIBRARIES {
        if let Some(s) = std::fs::read_to_string(format!("{}/assets/{}.lua", dir, id)).ok().and_then(clean_library) {
            out.insert(*id, s);
        }
    }
    out
}

async fn preload_libraries(dir: &str, site: Option<&crate::fetch::Site>) -> HashMap<u64, String> {
    let mut out = libraries_disk(dir);
    for (_, id) in LIBRARIES {
        if out.contains_key(id) {
            continue;
        }
        if let Some(site) = site {
            if let Some(b) = site.get(&format!("/asset/?id={}", id)).await {
                if let Some(s) = clean_library(String::from_utf8_lossy(&b).to_string()) {
                    out.insert(*id, s);
                }
            }
        }
    }
    out
}

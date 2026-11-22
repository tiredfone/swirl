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

pub enum Cmd {
    Join { cid: u32, user: UserInfo, look: Look, out: UnboundedSender<Out> },
    Msg { cid: u32, text: String },
    Leave { cid: u32 },
    Run { source: String, by: String },
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
}

pub struct Handle {
    pub tx: UnboundedSender<Cmd>,
    pub ready: Arc<AtomicBool>,
    pub failed: Arc<Mutex<Option<String>>>,
    pub capture: Arc<Mutex<Option<Vec<String>>>>,
    pub frames: Arc<AtomicUsize>,
}

pub fn spawn(spec: Spec) -> Handle {
    let (tx, mut rx) = unbounded_channel::<Cmd>();
    let ready = Arc::new(AtomicBool::new(false));
    let failed = Arc::new(Mutex::new(None));
    let capture: Arc<Mutex<Option<Vec<String>>>> = Arc::new(Mutex::new(None));
    let frames = Arc::new(AtomicUsize::new(0));
    let (r2, f2, c2, fr2) = (ready.clone(), failed.clone(), capture.clone(), frames.clone());
    std::thread::Builder::new()
        .name(format!("place-{}", spec.place))
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let tag = format!("[place {} {}]", spec.place, if spec.kind == "public" { spec.job.clone() } else { format!("{} {}", spec.kind, spec.private_id) });
                let mut game = match Game::load_with(spec.place, &spec.bytes, spec.era, &spec.assets, &spec.data_dir) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("{} failed to start: {}", tag, e);
                        *f2.lock().unwrap() = Some(e);
                        return;
                    }
                };
                game.allow_gear = spec.allow_gear;
                game.job = spec.job.clone();
                r2.store(true, Ordering::SeqCst);
                let (stx, mut srx) = unbounded_channel::<(u32, crate::api::SocialKind, Option<serde_json::Value>)>();
                let social = spec.social.clone();
                let (atx, mut arx) = unbounded_channel::<(u32, Option<Vec<u8>>)>();
                let site = spec.site.clone();
                let mut clients: HashMap<u32, (InstIdAlias, UnboundedSender<Out>)> = HashMap::new();
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
    Handle { tx, ready, failed, capture, frames }
}

type InstIdAlias = u32;

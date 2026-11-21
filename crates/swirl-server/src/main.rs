use futures_util::{SinkExt, StreamExt};
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use swirl_server::appearance;
use swirl_server::character;
use swirl_server::fetch::Site;
use swirl_server::game::Game;
use swirl_server::redeem::Redeemer;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

fn arg(name: &str) -> Option<String> {
    std::env::args().find_map(|a| a.strip_prefix(&format!("--{}=", name)).map(|s| s.to_string()))
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let place: u64 = arg("place").and_then(|s| s.parse().ok()).expect("--place=ID required");
    let port: u16 = arg("port").and_then(|s| s.parse().ok()).expect("--port=N required");
    if port == 4201 || port == 4202 {
        eprintln!("refusing to use a live Chocolate port");
        std::process::exit(2);
    }
    let era: i32 = arg("era").and_then(|s| s.parse().ok()).unwrap_or(2012);
    character::set_content_dir(&arg("chocolate").unwrap_or("/var/www/swirl/vendor/chocolate".into()));
    let assets = arg("assets").unwrap_or("/var/www/assets".into());
    let redeemer: Option<Rc<Redeemer>> = match (arg("redeem-url"), arg("redeem-key-file")) {
        (Some(u), Some(f)) => {
            let key = std::fs::read_to_string(&f).expect("redeem key file").trim().to_string();
            Some(Rc::new(Redeemer::parse(&u, key).expect("redeem url must be http on loopback")))
        }
        _ => None,
    };
    let site: Option<Site> = arg("site").and_then(|u| Site::parse(&u));
    let bytes = std::fs::read(format!("{}/assets/{}", assets, place)).expect("place file");
    let game = Rc::new(RefCell::new(Game::load(place, &bytes, era).unwrap_or_else(|e| panic!("{}", e))));
    let clients: Rc<RefCell<HashMap<u32, (u32, UnboundedSender<String>)>>> = Rc::new(RefCell::new(HashMap::new()));
    let local = tokio::task::LocalSet::new();
    local.run_until(async move {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    eprintln!("swirl-server listening on 127.0.0.1:{}", port);
    {
        let game = game.clone();
        let clients = clients.clone();
        tokio::task::spawn_local(async move {
            let mut iv = tokio::time::interval(std::time::Duration::from_millis(33));
            loop {
                iv.tick().await;
                let logs = game.borrow_mut().tick(0.033);
                for l in logs {
                    eprintln!("[{}] {}", l.kind, l.msg);
                }
                let remote = std::mem::take(&mut game.borrow_mut().remote_out);
                for (target, m) in remote {
                    let text = m.to_string();
                    for (pl, tx) in clients.borrow().values() {
                        if target.is_empty() || target.contains(pl) {
                            let _ = tx.send(text.clone());
                        }
                    }
                }
                let players: Vec<u32> = clients.borrow().values().map(|(p, _)| *p).collect();
                let outs = game.borrow_mut().flush_all(&players);
                for (p, m) in outs {
                    for (pl, tx) in clients.borrow().values() {
                        if *pl == p {
                            let _ = tx.send(m.clone());
                        }
                    }
                }
            }
        });
    }
    let mut cid = 0u32;
    loop {
        let (stream, _) = listener.accept().await.unwrap();
        cid += 1;
        let game = game.clone();
        let n = cid;
        let clients = clients.clone();
        let redeemer = redeemer.clone();
        let site = site.clone();
        let assets = assets.clone();
        tokio::task::spawn_local(async move {
            let uri = Rc::new(RefCell::new(String::new()));
            let u2 = uri.clone();
            let ws = match tokio_tungstenite::accept_hdr_async(stream, move |req: &tokio_tungstenite::tungstenite::handshake::server::Request, resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
                *u2.borrow_mut() = req.uri().to_string();
                Ok(resp)
            })
            .await
            {
                Ok(w) => w,
                Err(_) => return,
            };
            let ticket = uri.borrow().split("ticket=").nth(1).map(|s| s.split('&').next().unwrap_or("").to_string()).unwrap_or_default();
            let mut user_name = format!("Player{}", n);
            let mut user_id: Option<i64> = None;
            if let Some(r) = &redeemer {
                match r.redeem(&ticket).await {
                    Some(j) if j["mode"] == "play" && j["placeId"].as_u64() == Some(place) => {
                        if let Some(nm) = j["username"].as_str() {
                            user_name = nm.to_string();
                        }
                        user_id = j["userId"].as_i64();
                    }
                    _ => {
                        let (mut tx, _rx) = ws.split();
                        let _ = tx.send(Message::Text(serde_json::json!({"t": "kick", "m": "Your join ticket is invalid or has expired. Reload the page to try again."}).to_string())).await;
                        let _ = tx.close().await;
                        return;
                    }
                }
            }
            let (mut tx, mut rx) = ws.split();
            let look = match &site {
                Some(st) => appearance::fetch(st, &assets, user_id.unwrap_or(n as i64)).await,
                None => appearance::Look::default(),
            };
            let (init, player) = game.borrow_mut().join(&user_name, user_id.or(site.as_ref().map(|_| n as i64)), look);
            let (otx, mut orx) = unbounded_channel::<String>();
            clients.borrow_mut().insert(n, (player, otx));
            if tx.send(Message::Text(init.to_string())).await.is_err() {
                return;
            }
            let _ = tx.send(Message::Text(serde_json::json!({"t": "sys", "m": format!("{} has joined the game.", user_name)}).to_string())).await;
            loop {
                tokio::select! {
                    m = rx.next() => {
                        match m {
                            Some(Ok(Message::Text(t))) => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
                                    if v["t"] != "ping" {
                                        game.borrow_mut().message(player, &t);
                                    }
                                    if v["t"] == "ping" {
                                        let _ = tx.send(Message::Text(serde_json::json!({"t": "pong", "x": v["x"]}).to_string())).await;
                                    }
                                }
                            }
                            Some(Ok(_)) => {}
                            _ => break,
                        }
                    }
                    out = orx.recv() => {
                        match out {
                            Some(s) => { if tx.send(Message::Text(s)).await.is_err() { break; } }
                            None => break,
                        }
                    }
                }
            }
            clients.borrow_mut().remove(&n);
            game.borrow_mut().leave(player);
        });
    }
    }).await;
}
use futures_util::{SinkExt, StreamExt};
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use swirl_server::game::Game;
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
    let assets = arg("assets").unwrap_or("/var/www/assets".into());
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
        tokio::task::spawn_local(async move {
            let ws = match tokio_tungstenite::accept_async(stream).await {
                Ok(w) => w,
                Err(_) => return,
            };
            let (mut tx, mut rx) = ws.split();
            let (init, player) = game.borrow_mut().join(&format!("Player{}", n));
            let (otx, mut orx) = unbounded_channel::<String>();
            clients.borrow_mut().insert(n, (player, otx));
            if tx.send(Message::Text(init.to_string())).await.is_err() {
                return;
            }
            let _ = tx.send(Message::Text(serde_json::json!({"t": "sys", "m": format!("Player{} has joined the game.", n)}).to_string())).await;
            loop {
                tokio::select! {
                    m = rx.next() => {
                        match m {
                            Some(Ok(Message::Text(t))) => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
                                    if v["t"] == "char" {
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
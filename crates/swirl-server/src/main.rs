use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use swirl_server::game::Game;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

fn arg(name: &str) -> Option<String> {
    std::env::args().find_map(|a| a.strip_prefix(&format!("--{}=", name)).map(|s| s.to_string()))
}

#[tokio::main]
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
    let game = Arc::new(Mutex::new(Game::load(place, &bytes, era).unwrap_or_else(|e| panic!("{}", e))));
    let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    eprintln!("swirl-server listening on 127.0.0.1:{}", port);
    let mut cid = 0u32;
    loop {
        let (stream, _) = listener.accept().await.unwrap();
        cid += 1;
        let game = game.clone();
        let n = cid;
        tokio::spawn(async move {
            let ws = match tokio_tungstenite::accept_async(stream).await {
                Ok(w) => w,
                Err(_) => return,
            };
            let (mut tx, mut rx) = ws.split();
            let (init, player) = {
                let mut g = game.lock().unwrap();
                g.join(&format!("Player{}", n))
            };
            if tx.send(Message::Text(init.to_string())).await.is_err() {
                return;
            }
            let _ = tx.send(Message::Text(serde_json::json!({"t": "sys", "m": format!("Player{} has joined the game.", n)}).to_string())).await;
            while let Some(Ok(m)) = rx.next().await {
                if let Message::Text(t) = m {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
                        if v["t"] == "ping" {
                            let _ = tx.send(Message::Text(serde_json::json!({"t": "pong", "x": v["x"]}).to_string())).await;
                        }
                    }
                }
            }
            game.lock().unwrap().leave(player);
        });
    }
}

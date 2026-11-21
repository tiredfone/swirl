use serde_json::Value;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::Message;

pub enum Msg {
    Text(String),
    Closed(String),
}

pub struct Config {
    pub ticket: String,
    pub ws: String,
    pub era: i64,
    pub place: i64,
    pub user: String,
}

pub fn redeem(base: &str, token: &str) -> Result<Config, String> {
    let url = format!("{}/api/v1/client/launch/redeem", base.trim_end_matches('/'));
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(10)).timeout_read(Duration::from_secs(20)).user_agent("SolarPlayer/1.0").build();
    let r = match agent.post(&url).send_form(&[("token", token)]) {
        Ok(r) => r,
        Err(ureq::Error::Status(_, resp)) => {
            let v: Value = resp.into_string().ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
            return Err(v.get("error").and_then(|e| e.as_str()).unwrap_or("The launch was refused").to_string());
        }
        Err(e) => return Err(format!("Cannot reach Solar ({})", e.kind_name())),
    };
    let v: Value = serde_json::from_str(&r.into_string().map_err(|e| e.to_string())?).map_err(|_| "Unexpected answer from Solar".to_string())?;
    let ticket = v.get("ticket").and_then(|x| x.as_str()).ok_or("The launch has no ticket")?.to_string();
    let ws = v.get("ws").and_then(|x| x.as_str()).unwrap_or("/chocolate/ws").to_string();
    Ok(Config {
        ticket,
        ws: ws_url(base, &ws),
        era: v.get("era").and_then(|x| x.as_i64()).unwrap_or(2012),
        place: v.get("placeId").and_then(|x| x.as_i64()).unwrap_or(0),
        user: v.pointer("/user/name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
    })
}

trait KindName {
    fn kind_name(&self) -> String;
}

impl KindName for ureq::Error {
    fn kind_name(&self) -> String {
        match self {
            ureq::Error::Transport(t) => format!("{:?}", t.kind()),
            _ => "error".into(),
        }
    }
}

pub fn ws_url(base: &str, ws: &str) -> String {
    if ws.starts_with("ws://") || ws.starts_with("wss://") {
        return ws.to_string();
    }
    let b = base.trim_end_matches('/');
    let b = if let Some(r) = b.strip_prefix("https://") {
        format!("wss://{}", r)
    } else if let Some(r) = b.strip_prefix("http://") {
        format!("ws://{}", r)
    } else {
        b.to_string()
    };
    format!("{}{}{}", b, if ws.starts_with('/') { "" } else { "/" }, ws)
}

pub fn with_ticket(url: &str, ticket: &str) -> String {
    let enc: String = ticket.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{:02X}", b) }).collect();
    format!("{}{}ticket={}", url, if url.contains('?') { "&" } else { "?" }, enc)
}

pub fn connect(url: String) -> (Receiver<Msg>, Sender<String>) {
    let (tx, rx) = channel::<Msg>();
    let (otx, orx) = channel::<String>();
    std::thread::spawn(move || {
        let (mut sock, _) = match tungstenite::connect(url) {
            Ok(x) => x,
            Err(e) => {
                let _ = tx.send(Msg::Closed(format!("Cannot join the game server ({})", e)));
                return;
            }
        };
        match sock.get_mut() {
            MaybeTlsStream::Plain(s) => {
                let _ = s.set_read_timeout(Some(Duration::from_millis(15)));
            }
            MaybeTlsStream::Rustls(s) => {
                let _ = s.get_ref().set_read_timeout(Some(Duration::from_millis(15)));
            }
            _ => {}
        }
        let mut last_ping = Instant::now();
        loop {
            while let Ok(m) = orx.try_recv() {
                if sock.send(Message::Text(m)).is_err() {
                    let _ = tx.send(Msg::Closed("Connection lost".into()));
                    return;
                }
            }
            if last_ping.elapsed() > Duration::from_secs(5) {
                last_ping = Instant::now();
                let _ = sock.send(Message::Text("{\"t\":\"ping\",\"x\":1}".into()));
            }
            match sock.read() {
                Ok(Message::Text(t)) => {
                    if tx.send(Msg::Text(t)).is_err() {
                        return;
                    }
                }
                Ok(Message::Close(_)) => {
                    let _ = tx.send(Msg::Closed("The server closed the connection".into()));
                    return;
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(e) => {
                    let _ = tx.send(Msg::Closed(format!("Connection lost ({})", e)));
                    return;
                }
            }
        }
    });
    (rx, otx)
}

use serde_json::Value as J;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Clone)]
pub struct Redeemer {
    pub host: String,
    pub port: u16,
    pub path: String,
    pub key: String,
}

impl Redeemer {
    pub fn parse(url: &str, key: String) -> Option<Redeemer> {
        let rest = url.strip_prefix("http://")?;
        let (hostport, path) = match rest.find('/') {
            Some(i) => (&rest[..i], rest[i..].to_string()),
            None => (rest, "/".to_string()),
        };
        let (host, port) = match hostport.split_once(':') {
            Some((h, p)) => (h.to_string(), p.parse().ok()?),
            None => (hostport.to_string(), 80),
        };
        if host != "127.0.0.1" && host != "localhost" {
            return None;
        }
        Some(Redeemer { host, port, path, key })
    }

    pub async fn redeem(&self, ticket: &str) -> Option<J> {
        if ticket.len() != 64 || !ticket.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let text = self.post(&self.path, "application/x-www-form-urlencoded", &format!("ticket={}", ticket)).await?;
        let j: J = serde_json::from_str(text.trim()).ok()?;
        if j["ok"].as_bool() == Some(true) {
            Some(j)
        } else {
            None
        }
    }

    pub fn sibling_path(&self, name: &str) -> String {
        match self.path.rfind('/') {
            Some(i) => format!("{}/{}", &self.path[..i], name),
            None => format!("/{}", name),
        }
    }

    pub async fn post(&self, path: &str, content_type: &str, body: &str) -> Option<String> {
        let req = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: Swirl/1.0\r\nContent-Type: {}\r\nX-Chocolate-Key: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            path,
            self.host,
            content_type,
            self.key,
            body.len(),
            body
        );
        let mut s = tokio::time::timeout(std::time::Duration::from_secs(5), TcpStream::connect((self.host.as_str(), self.port))).await.ok()?.ok()?;
        s.write_all(req.as_bytes()).await.ok()?;
        let mut buf = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(8), s.read_to_end(&mut buf)).await.ok()?.ok()?;
        let text = String::from_utf8_lossy(&buf).to_string();
        let (head, body) = text.split_once("\r\n\r\n")?;
        if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
            return None;
        }
        Some(if head.to_ascii_lowercase().contains("transfer-encoding: chunked") { dechunk(body) } else { body.to_string() })
    }
}

fn dechunk(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while let Some((len, tail)) = rest.split_once("\r\n") {
        let n = usize::from_str_radix(len.trim(), 16).unwrap_or(0);
        if n == 0 || tail.len() < n {
            break;
        }
        out.push_str(&tail[..n]);
        rest = tail[n..].trim_start_matches("\r\n");
    }
    out
}

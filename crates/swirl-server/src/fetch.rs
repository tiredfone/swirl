use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Clone)]
pub struct Site {
    pub host: String,
    pub port: u16,
}

impl Site {
    pub fn parse(url: &str) -> Option<Site> {
        let rest = url.strip_prefix("http://")?.trim_end_matches('/');
        let (host, port) = match rest.split_once(':') {
            Some((h, p)) => (h.to_string(), p.parse().ok()?),
            None => (rest.to_string(), 80),
        };
        if host != "127.0.0.1" && host != "localhost" {
            return None;
        }
        Some(Site { host, port })
    }

    pub async fn get(&self, path: &str) -> Option<Vec<u8>> {
        let req = format!("GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: Swirl/1.0\r\nConnection: close\r\n\r\n", path, self.host);
        let mut s = tokio::time::timeout(std::time::Duration::from_secs(5), TcpStream::connect((self.host.as_str(), self.port))).await.ok()?.ok()?;
        s.write_all(req.as_bytes()).await.ok()?;
        let mut buf = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(20), s.read_to_end(&mut buf)).await.ok()?.ok()?;
        let split = buf.windows(4).position(|w| w == b"\r\n\r\n")?;
        let head = String::from_utf8_lossy(&buf[..split]).to_string();
        if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
            return None;
        }
        let body = &buf[split + 4..];
        if head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
            Some(dechunk(body))
        } else {
            Some(body.to_vec())
        }
    }
}

fn dechunk(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let end = match body[i..].windows(2).position(|w| w == b"\r\n") {
            Some(e) => i + e,
            None => break,
        };
        let n = usize::from_str_radix(String::from_utf8_lossy(&body[i..end]).trim(), 16).unwrap_or(0);
        if n == 0 || end + 2 + n > body.len() {
            break;
        }
        out.extend_from_slice(&body[end + 2..end + 2 + n]);
        i = end + 2 + n + 2;
    }
    out
}

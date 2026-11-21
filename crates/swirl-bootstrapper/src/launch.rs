use crate::era::Era;

#[derive(Clone, Debug, Default)]
pub struct Launch {
    pub place: String,
    pub token: String,
    pub channel: String,
    pub server: Option<String>,
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn parse(era: Era, url: &str) -> Result<Launch, String> {
    let prefix = format!("{}:", era.scheme());
    let rest = url.strip_prefix(&prefix).ok_or("This link is not for this version of Solar")?;
    let query = rest.split_once('?').map(|x| x.1).unwrap_or("");
    let query = query.split('#').next().unwrap_or("");
    let mut l = Launch { channel: "live".into(), ..Default::default() };
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = decode(v);
        match k {
            "place" => l.place = v,
            "token" => l.token = v,
            "channel" => l.channel = v,
            "server" if !v.is_empty() => l.server = Some(v),
            _ => {}
        }
    }
    if l.place.is_empty() || l.place.len() > 12 || !l.place.bytes().all(|c| c.is_ascii_digit()) {
        return Err("The launch link has no valid place".into());
    }
    if l.token.len() != 64 || !l.token.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("The launch link has no valid token".into());
    }
    if l.channel.is_empty() || l.channel.len() > 32 || !l.channel.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_') {
        return Err("The launch link has an invalid channel".into());
    }
    if let Some(s) = &l.server {
        if s.len() > 100 || !s.bytes().all(|c| c.is_ascii_alphanumeric() || b":_.-".contains(&c)) {
            return Err("The launch link has an invalid server".into());
        }
    }
    Ok(l)
}

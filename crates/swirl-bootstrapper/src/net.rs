use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent(concat!("SolarLauncher/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn describe(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, _) => format!("The server answered {}", code),
        ureq::Error::Transport(t) => format!("Cannot reach the server ({})", t.kind()),
    }
}

pub fn fetch_json(url: &str) -> Result<Value, String> {
    let r = agent().get(url).set("Accept", "application/json").call().map_err(describe)?;
    let text = r.into_string().map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|_| "The server sent an unexpected answer".to_string())
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

pub fn download(url: &str, dest: &Path, sha: &str, size: u64, progress: &mut dyn FnMut(u64, u64), cancel: &dyn Fn() -> bool) -> Result<(), String> {
    if let Some(d) = dest.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let r = agent().get(url).call().map_err(describe)?;
    let total = r.header("Content-Length").and_then(|v| v.parse::<u64>().ok()).unwrap_or(size);
    let mut reader = r.into_reader();
    let part = dest.with_extension("part");
    let mut f = fs::File::create(&part).map_err(|e| format!("Cannot write {}: {}", part.display(), e))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 65536];
    let mut got = 0u64;
    loop {
        if cancel() {
            let _ = fs::remove_file(&part);
            return Err("Cancelled".into());
        }
        let n = reader.read(&mut buf).map_err(|e| format!("The download was interrupted ({})", e))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        got += n as u64;
        progress(got, total);
    }
    drop(f);
    if size > 0 && got != size {
        let _ = fs::remove_file(&part);
        return Err("The download is incomplete".into());
    }
    if hex(&h.finalize()) != sha.to_lowercase() {
        let _ = fs::remove_file(&part);
        return Err("The download is damaged (checksum mismatch)".into());
    }
    fs::rename(&part, dest).map_err(|e| e.to_string())
}

use crate::era::Era;
use std::path::PathBuf;

pub struct Paths {
    pub root: PathBuf,
}

pub fn platform() -> &'static str {
    if cfg!(windows) {
        "windows-x64"
    } else {
        "linux-x64"
    }
}

fn data_dir() -> PathBuf {
    if cfg!(windows) {
        if let Some(p) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(p).join("Solar");
        }
        return std::env::temp_dir().join("Solar");
    }
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"),
    };
    base.join("solar")
}

impl Paths {
    pub fn new(era: Era) -> Paths {
        Paths { root: data_dir().join(era.year().to_string()) }
    }

    pub fn versions(&self) -> PathBuf {
        self.root.join(if cfg!(windows) { "Versions" } else { "versions" })
    }

    pub fn downloads(&self) -> PathBuf {
        self.root.join("Downloads")
    }

    pub fn state_file(&self) -> PathBuf {
        self.root.join("current.json")
    }

    pub fn log_file(&self) -> PathBuf {
        self.root.join("launcher.log")
    }

    pub fn client_dir(&self, version: &str) -> PathBuf {
        self.versions().join(version)
    }

    pub fn launcher_dir(&self, version: &str) -> PathBuf {
        self.versions().join(format!("launcher-{}", version))
    }
}

pub fn version_key(v: &str) -> Option<(u32, u32)> {
    let mut it = v.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b))
}

pub fn own_version() -> String {
    let raw = option_env!("SOLAR_BOOTSTRAPPER_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));
    let mut it = raw.split('.');
    format!("{}.{}", it.next().unwrap_or("1"), it.next().unwrap_or("0"))
}

pub fn base_url() -> String {
    let b = std::env::var("SOLAR_BASE_URL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| option_env!("SOLAR_BASE_URL").unwrap_or("https://imfone.dev").to_string());
    b.trim_end_matches('/').to_string()
}

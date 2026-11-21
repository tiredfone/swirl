use crate::era::{Era, Phase};
use crate::install;
use crate::launch::{self, Launch};
use crate::net;
use crate::paths::{self, Paths};
use crate::register;
use crate::state;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Shared {
    pub phase: Phase,
    pub percent: Option<u32>,
    pub error: Option<String>,
    pub cancel: bool,
    pub retry: bool,
    pub finished: bool,
    pub installed_only: bool,
}

impl Shared {
    pub fn new() -> Shared {
        Shared { phase: Phase::Connecting, percent: None, error: None, cancel: false, retry: false, finished: false, installed_only: false }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

pub type Sh = Arc<Mutex<Shared>>;

pub struct Ctx {
    pub era: Era,
    pub paths: Paths,
    pub launch: Option<Launch>,
    pub raw_args: Vec<String>,
    pub channel: String,
    pub headless: bool,
}

pub fn log(ctx: &Ctx, msg: &str) {
    if ctx.headless {
        eprintln!("[launcher] {}", msg);
    }
    let _ = std::fs::create_dir_all(&ctx.paths.root);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(ctx.paths.log_file()) {
        use std::io::Write;
        let _ = writeln!(f, "{}", msg);
    }
}

fn set_phase(sh: &Sh, ctx: &Ctx, phase: Phase, percent: Option<u32>) {
    {
        let mut s = sh.lock().unwrap();
        s.phase = phase;
        s.percent = percent;
    }
    log(ctx, &format!("phase {:?} {:?}", phase, percent));
}

fn cancelled(sh: &Sh) -> bool {
    sh.lock().unwrap().cancel
}

struct Release {
    version: String,
    url: String,
    sha: String,
    size: u64,
}

fn release(v: &Value) -> Option<Release> {
    let o = v.as_object()?;
    Some(Release {
        version: o.get("version")?.as_str()?.to_string(),
        url: o.get("url")?.as_str()?.to_string(),
        sha: o.get("sha256")?.as_str()?.to_string(),
        size: o.get("size").and_then(|x| x.as_u64()).unwrap_or(0),
    })
}

fn newer(a: &str, b: &str) -> bool {
    match (paths::version_key(a), paths::version_key(b)) {
        (Some(x), Some(y)) => x > y,
        _ => false,
    }
}

fn fetch_with_progress(sh: &Sh, ctx: &Ctx, r: &Release, dest: &Path) -> Result<(), String> {
    set_phase(sh, ctx, Phase::Downloading, Some(0));
    let mut last = 101u32;
    net::download(
        &r.url,
        dest,
        &r.sha,
        r.size,
        &mut |got, total| {
            let p = if total > 0 { ((got * 100) / total).min(100) as u32 } else { 0 };
            if p != last {
                last = p;
                let mut s = sh.lock().unwrap();
                s.percent = Some(p);
            }
        },
        &|| cancelled(sh),
    )
}

pub enum Outcome {
    Launched,
    Installed,
    Replaced,
}

pub fn run_once(sh: &Sh, ctx: &Ctx) -> Result<Outcome, String> {
    let era = ctx.era;
    let paths = &ctx.paths;
    set_phase(sh, ctx, Phase::Connecting, None);
    let url = format!("{}/api/v1/client/desktop?era={}&platform={}&channel={}", paths::base_url(), era.year(), paths::platform(), ctx.channel);
    let mut st = state::load(paths);
    let info = match net::fetch_json(&url) {
        Ok(v) => {
            if v.get("success").and_then(|x| x.as_bool()) == Some(false) {
                return Err(v.get("error").and_then(|x| x.as_str()).unwrap_or("The update server refused the request").to_string());
            }
            Some(v)
        }
        Err(e) => {
            log(ctx, &format!("update check failed: {}", e));
            let have = st.client.as_ref().map(|c| paths.client_dir(c).join(era.client_name()).exists()).unwrap_or(false);
            if have && ctx.launch.is_some() {
                None
            } else {
                return Err(e);
            }
        }
    };
    if cancelled(sh) {
        return Err("Cancelled".into());
    }
    set_phase(sh, ctx, Phase::Checking, None);
    let me = install::current_exe()?;
    if let Some(info) = &info {
        let own = paths::own_version();
        if std::env::var_os("SOLAR_UPDATED").is_none() {
            if let Some(b) = info.get("bootstrapper").and_then(release) {
                if newer(&b.version, &own) {
                    log(ctx, &format!("launcher update {} -> {}", own, b.version));
                    let dl = paths.downloads().join(format!("launcher-{}{}", b.version, std::env::consts::EXE_SUFFIX));
                    fetch_with_progress(sh, ctx, &b, &dl)?;
                    set_phase(sh, ctx, Phase::Installing, None);
                    let target = install::install_launcher_file(era, paths, &b.version, &dl)?;
                    st = install::set_current(paths, None, Some(&target))?;
                    register::register(era, &target)?;
                    install::cleanup(paths, &st);
                    log(ctx, &format!("restarting as {}", target.display()));
                    Command::new(&target).args(&ctx.raw_args).env("SOLAR_UPDATED", "1").spawn().map_err(|e| format!("Cannot start the new launcher: {}", e))?;
                    return Ok(Outcome::Replaced);
                }
            }
        }
        match info.get("client").and_then(release) {
            Some(c) => {
                let installed = st.client.as_deref() == Some(c.version.as_str()) && paths.client_dir(&c.version).join(era.client_name()).exists();
                if !installed {
                    log(ctx, &format!("client {} -> {}", st.client.clone().unwrap_or_default(), c.version));
                    let dl = paths.downloads().join(format!("client-{}.zip", c.version));
                    fetch_with_progress(sh, ctx, &c, &dl)?;
                    set_phase(sh, ctx, Phase::Installing, None);
                    let dir = install::install_client(era, paths, &c.version, &dl)?;
                    let copy = install::clone_self(era, &dir)?;
                    st = install::set_current(paths, Some(&c.version), Some(&copy))?;
                    set_phase(sh, ctx, Phase::Configuring, None);
                    register::register(era, &copy)?;
                    install::cleanup(paths, &st);
                }
            }
            None => {
                if st.client.is_none() {
                    return Err("No client is available for this channel yet".into());
                }
            }
        }
    }
    set_phase(sh, ctx, Phase::Configuring, None);
    if st.launcher.as_deref().map(Path::new) == Some(me.as_path()) {
        register::register(era, &me)?;
    }
    let cv = st.client.clone().ok_or("Solar is not installed")?;
    let client = paths.client_dir(&cv).join(era.client_name());
    if !client.exists() {
        return Err("The installed client is missing, try again to reinstall".into());
    }
    match &ctx.launch {
        None => {
            let mut s = sh.lock().unwrap();
            s.installed_only = true;
            s.phase = Phase::Done;
            drop(s);
            log(ctx, &format!("installed client {}", cv));
            Ok(Outcome::Installed)
        }
        Some(l) => {
            set_phase(sh, ctx, Phase::Starting, None);
            start_client(sh, ctx, &client, l)?;
            Ok(Outcome::Launched)
        }
    }
}

fn start_client(sh: &Sh, ctx: &Ctx, client: &PathBuf, l: &Launch) -> Result<(), String> {
    let mut cmd = Command::new(client);
    cmd.arg("--place").arg(&l.place).arg("--token").arg(&l.token).arg("--channel").arg(&l.channel);
    if let Some(s) = &l.server {
        cmd.arg("--server").arg(s);
    }
    cmd.arg("--base-url").arg(paths::base_url());
    if let Some(d) = client.parent() {
        cmd.current_dir(d);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::null()).stdin(Stdio::null());
    let mut child = cmd.spawn().map_err(|e| format!("Cannot start the player: {}", e))?;
    let out = child.stdout.take().ok_or("Cannot read the player")?;
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let started = Instant::now();
    let mut last_error = String::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                if line.trim() == "SOLAR_READY" {
                    log(ctx, "client window is shown");
                    return Ok(());
                }
                if let Some(m) = line.strip_prefix("SOLAR_ERROR ") {
                    last_error = m.trim().to_string();
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
                return Err(if last_error.is_empty() { format!("The player closed before it started (code {})", code) } else { last_error });
            }
            Err(_) => {}
        }
        if cancelled(sh) {
            let _ = child.kill();
            return Err("Cancelled".into());
        }
        if started.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            return Err("The player took too long to start".into());
        }
    }
}

pub fn parse_launch(era: Era, args: &[String]) -> Result<Option<Launch>, String> {
    for a in args {
        if a.starts_with("solar-") {
            return launch::parse(era, a).map(Some);
        }
    }
    Ok(None)
}

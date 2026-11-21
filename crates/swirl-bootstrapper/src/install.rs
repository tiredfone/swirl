use crate::era::Era;
use crate::paths::{version_key, Paths};
use crate::state::{self, State};
use std::fs;
use std::path::{Path, PathBuf};

pub fn current_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

#[cfg(unix)]
pub fn make_executable(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
pub fn make_executable(_p: &Path) {}

pub fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let f = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|_| "The client package is not valid".to_string())?;
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| e.to_string())?;
        let rel = e.enclosed_name().ok_or("The client package has an unsafe path")?;
        let out = dest.join(rel);
        if e.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(p) = out.parent() {
            fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut w = fs::File::create(&out).map_err(|e| format!("Cannot write {}: {}", out.display(), e))?;
        std::io::copy(&mut e, &mut w).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn install_client(era: Era, paths: &Paths, version: &str, zip_path: &Path) -> Result<PathBuf, String> {
    let final_dir = paths.client_dir(version);
    let tmp = paths.versions().join(format!("{}.tmp", version));
    let _ = fs::remove_dir_all(&tmp);
    extract_zip(zip_path, &tmp)?;
    let client = tmp.join(era.client_name());
    if !client.exists() {
        let _ = fs::remove_dir_all(&tmp);
        return Err("The client package has no player".into());
    }
    make_executable(&client);
    let _ = fs::remove_dir_all(&final_dir);
    fs::rename(&tmp, &final_dir).map_err(|e| format!("Cannot install into {}: {}", final_dir.display(), e))?;
    Ok(final_dir)
}

pub fn clone_self(era: Era, dir: &Path) -> Result<PathBuf, String> {
    let me = current_exe()?;
    let target = dir.join(era.launcher_name());
    if me != target {
        let tmp = dir.join(format!("{}.new", era.launcher_name()));
        fs::copy(&me, &tmp).map_err(|e| format!("Cannot copy the launcher: {}", e))?;
        make_executable(&tmp);
        let _ = fs::remove_file(&target);
        fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
    }
    Ok(target)
}

pub fn install_launcher_file(era: Era, paths: &Paths, version: &str, downloaded: &Path) -> Result<PathBuf, String> {
    let dir = paths.launcher_dir(version);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(era.launcher_name());
    let _ = fs::remove_file(&target);
    fs::copy(downloaded, &target).map_err(|e| e.to_string())?;
    make_executable(&target);
    Ok(target)
}

pub fn cleanup(paths: &Paths, st: &State) {
    let entries = match fs::read_dir(paths.versions()) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut clients: Vec<((u32, u32), PathBuf)> = Vec::new();
    let mut others: Vec<PathBuf> = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(k) = version_key(&name) {
            clients.push((k, e.path()));
        } else {
            others.push(e.path());
        }
    }
    clients.sort();
    let keep_n = 2;
    let drop = clients.len().saturating_sub(keep_n);
    let launcher_dir = st.launcher.as_ref().and_then(|l| Path::new(l).parent().map(|p| p.to_path_buf()));
    for (_, p) in clients.into_iter().take(drop) {
        if Some(&p) != launcher_dir.as_ref() {
            let _ = fs::remove_dir_all(p);
        }
    }
    for p in others {
        if Some(&p) != launcher_dir.as_ref() {
            let _ = fs::remove_dir_all(p);
        }
    }
    if let Ok(d) = fs::read_dir(paths.downloads()) {
        for e in d.flatten() {
            let _ = fs::remove_file(e.path());
        }
    }
}

pub fn set_current(paths: &Paths, client: Option<&str>, launcher: Option<&Path>) -> Result<State, String> {
    let mut st = state::load(paths);
    if let Some(c) = client {
        st.client = Some(c.to_string());
    }
    if let Some(l) = launcher {
        st.launcher = Some(l.display().to_string());
    }
    state::save(paths, &st)?;
    Ok(st)
}

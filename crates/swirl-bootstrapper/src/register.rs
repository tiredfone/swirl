use crate::era::Era;
use std::path::Path;
use std::process::{Command, Stdio};

fn skip() -> bool {
    std::env::var_os("SOLAR_NO_REGISTER").is_some()
}

#[cfg(windows)]
fn reg(args: &[&str]) {
    let mut c = Command::new("reg");
    c.args(args).stdout(Stdio::null()).stderr(Stdio::null());
    use std::os::windows::process::CommandExt;
    c.creation_flags(0x08000000);
    let _ = c.status();
}

#[cfg(windows)]
pub fn register(era: Era, launcher: &Path) -> Result<(), String> {
    if skip() {
        return Ok(());
    }
    let key = format!("HKCU\\Software\\Classes\\{}", era.scheme());
    let cmd = format!("\"{}\" \"%1\"", launcher.display());
    reg(&["add", &key, "/ve", "/d", &format!("URL:{} Protocol", era.display()), "/f"]);
    reg(&["add", &key, "/v", "URL Protocol", "/d", "", "/f"]);
    reg(&["add", &format!("{}\\DefaultIcon", key), "/ve", "/d", &format!("\"{}\",0", launcher.display()), "/f"]);
    reg(&["add", &format!("{}\\shell\\open\\command", key), "/ve", "/d", &cmd, "/f"]);
    let menu = std::env::var("APPDATA").map(|a| std::path::PathBuf::from(a).join("Microsoft\\Windows\\Start Menu\\Programs")).ok();
    if let Some(dir) = menu {
        let lnk = dir.join(format!("{}.lnk", era.display()));
        let script = format!(
            "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.WorkingDirectory='{}';$s.Save()",
            lnk.display(),
            launcher.display(),
            launcher.parent().map(|p| p.display().to_string()).unwrap_or_default()
        );
        use std::os::windows::process::CommandExt;
        let _ = Command::new("powershell").args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script]).creation_flags(0x08000000).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn register(era: Era, launcher: &Path) -> Result<(), String> {
    let apps = match std::env::var_os("XDG_DATA_HOME") {
        Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
        _ => std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"),
    }
    .join("applications");
    std::fs::create_dir_all(&apps).map_err(|e| e.to_string())?;
    let file = format!("{}.desktop", era.scheme());
    let text = format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment=Play Solar games\nExec=\"{}\" %u\nTerminal=false\nCategories=Game;\nMimeType=x-scheme-handler/{};\nNoDisplay=false\n",
        era.display(),
        launcher.display(),
        era.scheme()
    );
    let path = apps.join(&file);
    let same = std::fs::read_to_string(&path).map(|t| t == text).unwrap_or(false);
    if same {
        return Ok(());
    }
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    if skip() {
        return Ok(());
    }
    let _ = Command::new("update-desktop-database").arg(&apps).stdout(Stdio::null()).stderr(Stdio::null()).status();
    let _ = Command::new("xdg-mime").args(["default", &file, &format!("x-scheme-handler/{}", era.scheme())]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    Ok(())
}

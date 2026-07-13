//! Installed app listing + launch/quit.
//!
//! Launch paths are allowlisted under known Applications directories (Swift parity).

use std::path::{Path, PathBuf};

use tracing::{info, warn};
use webdock_protocol::AppInfo;

use crate::traits::*;

pub struct NativeApps;

impl AppCatalog for NativeApps {
    fn list_apps(&self) -> Result<Vec<AppInfo>, PlatformError> {
        #[cfg(target_os = "macos")]
        {
            return list_macos_apps();
        }
        #[cfg(target_os = "windows")]
        {
            return list_windows_apps();
        }
        #[cfg(target_os = "linux")]
        {
            return list_linux_apps();
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Ok(vec![])
        }
    }

    fn launch(&self, path: &str, new_instance: bool) -> Result<(), PlatformError> {
        if !is_allowed_launch_path(path) {
            warn!(path, "launch rejected — path not allowlisted");
            return Err(PlatformError::Other(
                "launch path not allowed (must be under Applications / known app dirs)".into(),
            ));
        }
        // Canonical path after allowlist (no shell, no injection).
        let path = canonicalize_launch_path(path).unwrap_or_else(|| path.to_string());
        info!(%path, new_instance, "launch app");
        #[cfg(target_os = "macos")]
        {
            let mut cmd = std::process::Command::new("open");
            if new_instance {
                cmd.arg("-n");
            }
            // Pass as single argv — never through a shell.
            cmd.arg(&path)
                .status()
                .map_err(|e| PlatformError::Other(e.to_string()))?;
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        {
            // Use ShellExecute-style open without cmd metacharacters.
            std::process::Command::new(&path)
                .spawn()
                .map_err(|e| PlatformError::Other(e.to_string()))?;
            return Ok(());
        }
        #[cfg(target_os = "linux")]
        {
            if path.ends_with(".desktop") {
                let stem = Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                if !stem.is_empty() && !stem.contains('/') {
                    let _ = std::process::Command::new("gtk-launch").arg(stem).status();
                    return Ok(());
                }
            }
            std::process::Command::new(&path)
                .spawn()
                .map_err(|e| PlatformError::Other(e.to_string()))?;
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = (path, new_instance);
            Err(PlatformError::Other("unsupported OS".into()))
        }
    }

    fn quit_pid(&self, pid: i32) -> Result<(), PlatformError> {
        if pid <= 0 {
            return Err(PlatformError::Other("invalid pid".into()));
        }
        #[cfg(unix)]
        {
            let status = std::process::Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status()
                .map_err(|e| PlatformError::Other(e.to_string()))?;
            if status.success() {
                Ok(())
            } else {
                Err(PlatformError::Other(format!("kill {pid} failed")))
            }
        }
        #[cfg(windows)]
        {
            std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T"])
                .status()
                .map_err(|e| PlatformError::Other(e.to_string()))?;
            Ok(())
        }
    }
}

/// Only allow launching bundles/binaries under known application directories.
pub fn is_allowed_launch_path(path: &str) -> bool {
    if path.is_empty() || path.contains('\0') {
        return false;
    }
    // Reject shell metacharacters / injection vectors early.
    if path
        .chars()
        .any(|c| matches!(c, '|' | '&' | ';' | '`' | '$' | '\n' | '\r'))
    {
        return false;
    }
    let Ok(canon) = PathBuf::from(path).canonicalize() else {
        // Fall back to standardized absolute check without requiring the path exist yet.
        return is_under_search_dirs(Path::new(path));
    };
    if canon
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return false;
    }
    is_under_search_dirs(&canon)
}

fn canonicalize_launch_path(path: &str) -> Option<String> {
    PathBuf::from(path)
        .canonicalize()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

fn is_under_search_dirs(path: &Path) -> bool {
    let s = path.to_string_lossy();
    #[cfg(target_os = "macos")]
    {
        if !s.ends_with(".app") && !path.extension().map(|e| e == "app").unwrap_or(false) {
            // Also accept path that is Foo.app/...
            if !s.contains(".app/") && !s.ends_with(".app") {
                return false;
            }
        }
        for dir in macos_search_dirs() {
            if path_under(path, Path::new(&dir)) {
                return true;
            }
        }
        return false;
    }
    #[cfg(target_os = "windows")]
    {
        for dir in windows_search_dirs() {
            if path_under(path, Path::new(&dir)) {
                return true;
            }
        }
        let lower = s.to_ascii_lowercase();
        return (lower.ends_with(".exe") || lower.ends_with(".lnk"))
            && (lower.contains("\\program files")
                || lower.contains("\\program files (x86)")
                || lower.contains("\\programs\\"));
    }
    #[cfg(target_os = "linux")]
    {
        if s.ends_with(".desktop") {
            return s.starts_with("/usr/share/applications")
                || s.starts_with("/usr/local/share/applications")
                || s.contains("/.local/share/applications/");
        }
        return s.starts_with("/usr/bin/")
            || s.starts_with("/usr/local/bin/")
            || s.starts_with("/opt/")
            || s.starts_with("/snap/bin/");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn path_under(path: &Path, dir: &Path) -> bool {
    let Ok(p) = path
        .canonicalize()
        .or_else(|_| Ok::<_, std::io::Error>(path.to_path_buf()))
    else {
        return false;
    };
    let Ok(d) = dir
        .canonicalize()
        .or_else(|_| Ok::<_, std::io::Error>(dir.to_path_buf()))
    else {
        return false;
    };
    p.starts_with(&d)
}

#[cfg(target_os = "macos")]
fn macos_search_dirs() -> Vec<String> {
    let mut v = vec![
        "/Applications".into(),
        "/Applications/Utilities".into(),
        "/System/Applications".into(),
        "/System/Applications/Utilities".into(),
    ];
    if let Ok(home) = std::env::var("HOME") {
        v.push(format!("{home}/Applications"));
    }
    v
}

#[cfg(target_os = "windows")]
fn windows_search_dirs() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(pf) = std::env::var("ProgramFiles") {
        v.push(pf);
    }
    if let Ok(pf) = std::env::var("ProgramFiles(x86)") {
        v.push(pf);
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        v.push(format!("{local}\\Programs"));
    }
    v
}

#[cfg(target_os = "macos")]
fn list_macos_apps() -> Result<Vec<AppInfo>, PlatformError> {
    let mut apps = Vec::new();
    let dirs = [
        "/Applications",
        "/System/Applications",
        &format!("{}/Applications", std::env::var("HOME").unwrap_or_default()),
    ];
    for dir in dirs {
        let path = Path::new(dir);
        if !path.is_dir() {
            continue;
        }
        let rd = match std::fs::read_dir(path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("app") {
                continue;
            }
            let name = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("App")
                .to_string();
            let path = p.to_string_lossy().into_owned();
            let icon = super::icons::data_url_for_key(&path);
            apps.push(AppInfo {
                name,
                path: path.clone(),
                icon,
                icon_key: Some(path),
            });
        }
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.path == b.path);
    Ok(apps)
}

#[cfg(target_os = "windows")]
fn list_windows_apps() -> Result<Vec<AppInfo>, PlatformError> {
    // Start Menu shortcuts — shallow scan.
    let mut apps = Vec::new();
    let mut roots = Vec::new();
    if let Ok(p) = std::env::var("ProgramData") {
        roots.push(Path::new(&p).join("Microsoft/Windows/Start Menu/Programs"));
    }
    if let Ok(p) = std::env::var("APPDATA") {
        roots.push(Path::new(&p).join("Microsoft/Windows/Start Menu/Programs"));
    }
    for root in roots {
        collect_lnk(&root, &mut apps, 0);
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(apps)
}

#[cfg(target_os = "windows")]
fn collect_lnk(dir: &Path, out: &mut Vec<AppInfo>, depth: u32) {
    if depth > 3 || !dir.is_dir() {
        return;
    }
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_lnk(&p, out, depth + 1);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("lnk"))
            .unwrap_or(false)
        {
            let name = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("App")
                .to_string();
            out.push(AppInfo {
                name,
                path: p.to_string_lossy().into_owned(),
                icon: None,
                icon_key: Some(p.to_string_lossy().into_owned()),
            });
        }
    }
}

#[cfg(target_os = "linux")]
fn list_linux_apps() -> Result<Vec<AppInfo>, PlatformError> {
    let mut apps = Vec::new();
    let dirs = [
        "/usr/share/applications".to_string(),
        "/usr/local/share/applications".to_string(),
        format!(
            "{}/.local/share/applications",
            std::env::var("HOME").unwrap_or_default()
        ),
    ];
    for dir in dirs {
        let path = Path::new(&dir);
        if !path.is_dir() {
            continue;
        }
        let rd = match std::fs::read_dir(path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let name = parse_desktop_name(&p).unwrap_or_else(|| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("App")
                    .to_string()
            });
            apps.push(AppInfo {
                name,
                path: p.to_string_lossy().into_owned(),
                icon: None,
                icon_key: Some(p.to_string_lossy().into_owned()),
            });
        }
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(apps)
}

#[cfg(target_os = "linux")]
fn parse_desktop_name(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("Name=") {
            return Some(v.trim().to_string());
        }
    }
    None
}

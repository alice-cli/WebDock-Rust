//! GitHub Releases auto-update (check → download → install → relaunch).
//!
//! No external update server / Cloudflare — uses public GitHub Releases only.
//! Pattern inspired by Tauri updater / aliceRust, implemented for the native
//! WebRust tray app (tao/wry), not the Tauri plugin.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use tracing::info;

/// Override via `WEBRUST_UPDATE_REPO=owner/name` if needed.
const DEFAULT_REPO: &str = "alice-cli/WebDock-Rust";
const USER_AGENT: &str = concat!("WebRust/", env!("CARGO_PKG_VERSION"));
/// Refuse absurd downloads (DoS / wrong asset).
const MAX_DOWNLOAD_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallKind {
    /// macOS Developer ID installer → /Applications
    MacPkg,
    /// macOS zip of WebRust.app (in-place ditto when writable)
    MacAppZip,
    /// Windows Inno Setup
    WinSetupExe,
    /// Windows portable zip
    WinZip,
    /// Linux tarball next to current install
    LinuxTarGz,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    /// Platform-matching asset (installer / zip / tarball).
    pub download_url: Option<String>,
    /// GitHub release HTML page.
    pub html_url: Option<String>,
    pub asset_name: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub install_kind: InstallKind,
}

impl Default for InstallKind {
    fn default() -> Self {
        InstallKind::Unknown
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    /// idle | checking | downloading | installing | done | error
    pub phase: String,
    /// 0–100 when known
    pub percent: u8,
    pub message: String,
    pub downloaded: u64,
    pub total: u64,
}

impl UpdateProgress {
    pub fn idle() -> Self {
        Self {
            phase: "idle".into(),
            percent: 0,
            message: String::new(),
            downloaded: 0,
            total: 0,
        }
    }
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: Option<String>,
    body: Option<String>,
    assets: Vec<GhAsset>,
    draft: Option<bool>,
    prerelease: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

fn repo() -> String {
    std::env::var("WEBRUST_UPDATE_REPO").unwrap_or_else(|_| DEFAULT_REPO.into())
}

/// Compare dotted semver-ish strings (0.1.0 vs 0.1.2). Returns true if `remote` > `local`.
pub fn is_newer(remote: &str, local: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.trim()
            .trim_start_matches('v')
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.parse().ok())
            .collect()
    };
    let a = parse(remote);
    let b = parse(local);
    let n = a.len().max(b.len());
    for i in 0..n {
        let av = a.get(i).copied().unwrap_or(0);
        let bv = b.get(i).copied().unwrap_or(0);
        if av != bv {
            return av > bv;
        }
    }
    false
}

fn classify_asset(name: &str) -> (i32, InstallKind) {
    let n = name.to_ascii_lowercase();
    #[cfg(target_os = "windows")]
    {
        if n.ends_with(".exe") && (n.contains("setup") || n.contains("install")) {
            return (100, InstallKind::WinSetupExe);
        }
        if n.ends_with(".msi") {
            return (90, InstallKind::WinSetupExe);
        }
        if n.contains("windows") && n.ends_with(".zip") {
            return (80, InstallKind::WinZip);
        }
        if n.ends_with(".exe") {
            return (70, InstallKind::WinSetupExe);
        }
        return (0, InstallKind::Unknown);
    }
    #[cfg(target_os = "macos")]
    {
        if n.contains("macos") || n.contains("darwin") || n.contains("apple") {
            // Prefer .pkg for /Applications install (in-app update path).
            if n.ends_with(".pkg") {
                return (100, InstallKind::MacPkg);
            }
            if n.ends_with(".zip") {
                return (80, InstallKind::MacAppZip);
            }
            if n.ends_with(".dmg") {
                return (60, InstallKind::Unknown);
            }
            return (40, InstallKind::Unknown);
        }
        return (0, InstallKind::Unknown);
    }
    #[cfg(target_os = "linux")]
    {
        if n.contains("linux") {
            if n.ends_with(".tar.gz") || n.ends_with(".tgz") {
                return (100, InstallKind::LinuxTarGz);
            }
            if n.ends_with(".appimage") {
                return (90, InstallKind::Unknown);
            }
            if n.ends_with(".deb") {
                return (80, InstallKind::Unknown);
            }
            return (50, InstallKind::Unknown);
        }
        return (0, InstallKind::Unknown);
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = n;
        (0, InstallKind::Unknown)
    }
}

/// Query GitHub Releases for the latest non-draft version.
pub fn check_for_update() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo());

    let body = ureq::get(&url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("GitHub API: {e}"))?
        .into_string()
        .map_err(|e| format!("read body: {e}"))?;

    let rel: GhRelease =
        serde_json::from_str(&body).map_err(|e| format!("parse release JSON: {e}"))?;

    if rel.draft.unwrap_or(false) {
        return Err("latest release is a draft".into());
    }

    let latest = rel.tag_name.trim().trim_start_matches('v').to_string();
    let update_available = is_newer(&latest, &current) && !rel.prerelease.unwrap_or(false);

    let mut best: Option<(i32, InstallKind, &GhAsset)> = None;
    for a in &rel.assets {
        let (score, kind) = classify_asset(&a.name);
        if score <= 0 {
            continue;
        }
        match best {
            None => best = Some((score, kind, a)),
            Some((s, _, _)) if score > s => best = Some((score, kind, a)),
            _ => {}
        }
    }

    let (install_kind, download_url, asset_name) = match best {
        Some((_, kind, a)) => (
            kind,
            Some(a.browser_download_url.clone()),
            Some(a.name.clone()),
        ),
        None => (InstallKind::Unknown, None, None),
    };

    Ok(UpdateInfo {
        current_version: current,
        latest_version: latest,
        update_available,
        download_url,
        html_url: rel.html_url,
        asset_name,
        notes: rel.body.map(|b| {
            if b.len() > 800 {
                format!("{}…", &b[..800])
            } else {
                b
            }
        }),
        install_kind,
    })
}

/// Download the update asset to a temp file. `on_progress(downloaded, total)`.
pub fn download_update(
    info: &UpdateInfo,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<PathBuf, String> {
    let url = info
        .download_url
        .as_deref()
        .ok_or_else(|| "no download URL for this platform".to_string())?;
    if !url.starts_with("https://") {
        return Err("refusing non-HTTPS download URL".into());
    }
    let name = info
        .asset_name
        .clone()
        .unwrap_or_else(|| "webrust-update.bin".into());
    // Basic path traversal guard
    let safe_name = Path::new(&name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("webrust-update.bin");

    let resp = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("download: {e}"))?;

    let total = resp
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    if total > MAX_DOWNLOAD_BYTES {
        return Err(format!(
            "download too large ({total} bytes > {MAX_DOWNLOAD_BYTES})"
        ));
    }

    let dir = std::env::temp_dir().join(format!(
        "webrust-update-{}-{}",
        info.latest_version,
        std::process::id()
    ));
    fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {e}"))?;
    let path = dir.join(safe_name);

    let mut reader = resp.into_reader();
    let mut file = File::create(&path).map_err(|e| format!("create file: {e}"))?;
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded: u64 = 0;
    on_progress(0, total);
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("read download: {e}"))?;
        if n == 0 {
            break;
        }
        downloaded = downloaded.saturating_add(n as u64);
        if downloaded > MAX_DOWNLOAD_BYTES {
            let _ = fs::remove_file(&path);
            return Err("download exceeded size limit".into());
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("write download: {e}"))?;
        on_progress(downloaded, total);
    }
    file.flush().map_err(|e| e.to_string())?;
    info!(?path, downloaded, total, "update downloaded");
    Ok(path)
}

/// Result of kicking off an install.
#[derive(Debug)]
pub struct InstallOutcome {
    /// Human message for the UI.
    pub message: String,
    /// Caller should exit the process so files can be replaced.
    pub should_exit: bool,
    /// Optional delayed relaunch (Windows: after Setup; mac pkg: open app after delay).
    pub relaunch_hint: Option<String>,
}

/// Install a downloaded package. May spawn an external installer.
pub fn install_update(path: &Path, info: &UpdateInfo) -> Result<InstallOutcome, String> {
    if !path.is_file() {
        return Err(format!("missing package: {}", path.display()));
    }
    match info.install_kind {
        #[cfg(target_os = "macos")]
        InstallKind::MacPkg => install_mac_pkg(path),
        #[cfg(target_os = "macos")]
        InstallKind::MacAppZip => install_mac_app_zip(path),
        #[cfg(target_os = "windows")]
        InstallKind::WinSetupExe => install_win_setup(path),
        #[cfg(target_os = "windows")]
        InstallKind::WinZip => install_win_zip(path),
        #[cfg(target_os = "linux")]
        InstallKind::LinuxTarGz => install_linux_tar(path),
        _ => Err(format!(
            "unsupported install kind {:?} for {}",
            info.install_kind,
            path.display()
        )),
    }
}

/// Full pipeline: check (if needed) → download → install. Progress via callback.
pub fn apply_update(
    info: &UpdateInfo,
    mut on_progress: impl FnMut(UpdateProgress),
) -> Result<InstallOutcome, String> {
    if !info.update_available {
        return Err("no update available".into());
    }
    if info.download_url.is_none() {
        return Err("no platform asset on latest release".into());
    }

    on_progress(UpdateProgress {
        phase: "downloading".into(),
        percent: 0,
        message: format!(
            "Downloading {}…",
            info.asset_name.as_deref().unwrap_or("update")
        ),
        downloaded: 0,
        total: 0,
    });

    let path = download_update(info, |done, total| {
        let pct = if total > 0 {
            ((done as f64 / total as f64) * 100.0).min(99.0) as u8
        } else {
            0
        };
        on_progress(UpdateProgress {
            phase: "downloading".into(),
            percent: pct,
            message: if total > 0 {
                format!("Downloading… {pct}%")
            } else {
                format!("Downloading… {} KB", done / 1024)
            },
            downloaded: done,
            total,
        });
    })?;

    on_progress(UpdateProgress {
        phase: "installing".into(),
        percent: 100,
        message: "Installing…".into(),
        downloaded: 0,
        total: 0,
    });

    let outcome = install_update(&path, info)?;
    on_progress(UpdateProgress {
        phase: "done".into(),
        percent: 100,
        message: outcome.message.clone(),
        downloaded: 0,
        total: 0,
    });
    Ok(outcome)
}

// ── macOS ──────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn install_mac_pkg(path: &Path) -> Result<InstallOutcome, String> {
    // Open Installer.app UI (handles admin auth + /Applications). Then quit so
    // the package can replace the running binary.
    Command::new("open")
        .arg(path)
        .spawn()
        .map_err(|e| format!("open pkg: {e}"))?;
    // Best-effort relaunch after user finishes installer (~30s delay).
    let app = app_bundle_path().unwrap_or_else(|| PathBuf::from("/Applications/WebRust.app"));
    let _ = Command::new("sh")
        .args([
            "-c",
            &format!("sleep 25; open \"{}\" 2>/dev/null || true", app.display()),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    Ok(InstallOutcome {
        message: "Installer opened — complete the wizard, then WebRust will relaunch.".into(),
        should_exit: true,
        relaunch_hint: Some(app.display().to_string()),
    })
}

#[cfg(target_os = "macos")]
fn install_mac_app_zip(path: &Path) -> Result<InstallOutcome, String> {
    let dest = app_bundle_path()
        .filter(|p| p.ends_with("WebRust.app"))
        .or_else(|| {
            let p = PathBuf::from("/Applications/WebRust.app");
            if p.exists() {
                Some(p)
            } else {
                None
            }
        })
        .ok_or_else(|| "cannot locate WebRust.app to replace".to_string())?;

    // Need write access to parent.
    let parent = dest.parent().ok_or("no parent dir")?;
    let probe = parent.join(".webrust-write-test");
    if File::create(&probe).is_err() {
        // Fall back to opening zip for manual install.
        let _ = Command::new("open").arg(path).spawn();
        return Ok(InstallOutcome {
            message: format!(
                "No write access to {}. Opened zip for manual install.",
                parent.display()
            ),
            should_exit: false,
            relaunch_hint: None,
        });
    }
    let _ = fs::remove_file(&probe);

    let extract = path
        .parent()
        .unwrap_or(Path::new("/tmp"))
        .join("webrust-extract");
    let _ = fs::remove_dir_all(&extract);
    fs::create_dir_all(&extract).map_err(|e| e.to_string())?;
    let status = Command::new("ditto")
        .args(["-x", "-k"])
        .arg(path)
        .arg(&extract)
        .status()
        .map_err(|e| format!("ditto extract: {e}"))?;
    if !status.success() {
        return Err("failed to extract app zip".into());
    }
    // Find WebRust.app under extract
    let src = find_app_bundle(&extract).ok_or("WebRust.app not found in zip")?;
    // Replace destination
    let _ = Command::new("ditto")
        .arg(&src)
        .arg(&dest)
        .status()
        .map_err(|e| format!("ditto install: {e}"))?;
    let _ = Command::new("open").arg(&dest).spawn();
    Ok(InstallOutcome {
        message: format!("Updated {}", dest.display()),
        should_exit: true,
        relaunch_hint: Some(dest.display().to_string()),
    })
}

#[cfg(target_os = "macos")]
fn app_bundle_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    // …/WebRust.app/Contents/MacOS/WebRust
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    if app.extension().and_then(|e| e.to_str()) == Some("app") {
        return Some(app.to_path_buf());
    }
    None
}

#[cfg(target_os = "macos")]
fn find_app_bundle(root: &Path) -> Option<PathBuf> {
    if root.join("WebRust.app").is_dir() {
        return Some(root.join("WebRust.app"));
    }
    let rd = fs::read_dir(root).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) == Some("app") {
            return Some(p);
        }
        if p.is_dir() {
            if let Some(f) = find_app_bundle(&p) {
                return Some(f);
            }
        }
    }
    None
}

// ── Windows ────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn install_win_setup(path: &Path) -> Result<InstallOutcome, String> {
    // Inno Setup silent install; CloseApplications=yes in iss.
    let status = Command::new(path)
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
        ])
        .spawn()
        .map_err(|e| format!("spawn setup: {e}"))?;
    let _ = status; // detached — installer continues after we exit
                    // Relaunch after a short delay so Setup can finish.
    let relaunch = std::env::current_exe()
        .ok()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "WebRust.exe".into());
    let _ = Command::new("cmd")
        .args([
            "/C",
            &format!("ping -n 15 127.0.0.1 >nul & start \"\" \"{relaunch}\""),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    Ok(InstallOutcome {
        message: "Installer started — WebRust will restart after setup.".into(),
        should_exit: true,
        relaunch_hint: Some(relaunch),
    })
}

#[cfg(target_os = "windows")]
fn install_win_zip(path: &Path) -> Result<InstallOutcome, String> {
    // Prefer opening explorer for portable zip — silent unzip over Program Files needs admin.
    let _ = Command::new("explorer").arg(path).spawn();
    Ok(InstallOutcome {
        message: "Opened portable zip — extract over your install folder.".into(),
        should_exit: false,
        relaunch_hint: None,
    })
}

// ── Linux ──────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn install_linux_tar(path: &Path) -> Result<InstallOutcome, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .ok_or("cannot resolve install directory")?
        .to_path_buf();
    // Extract into a sibling temp, then move WebRust binary + webui
    let extract = dir.join(".webrust-update-extract");
    let _ = fs::remove_dir_all(&extract);
    fs::create_dir_all(&extract).map_err(|e| e.to_string())?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(path)
        .arg("-C")
        .arg(&extract)
        .status()
        .map_err(|e| format!("tar: {e}"))?;
    if !status.success() {
        return Err("tar extract failed".into());
    }
    // tarball layout: webrust/WebRust + webrust/webui
    let src_bin = find_named_file(&extract, "WebRust")
        .ok_or_else(|| "WebRust binary not found in tarball".to_string())?;
    let dest_bin = dir.join("WebRust");
    // Replace via rename dance
    let bak = dir.join("WebRust.bak");
    let _ = fs::remove_file(&bak);
    if dest_bin.exists() {
        fs::rename(&dest_bin, &bak).map_err(|e| format!("backup: {e}"))?;
    }
    fs::copy(&src_bin, &dest_bin).map_err(|e| format!("copy binary: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dest_bin, fs::Permissions::from_mode(0o755));
    }
    if let Some(webui) = find_named_dir(&extract, "webui") {
        let dest_webui = dir.join("webui");
        let _ = fs::remove_dir_all(&dest_webui);
        copy_dir_all(&webui, &dest_webui).map_err(|e| format!("copy webui: {e}"))?;
    }
    let _ = fs::remove_dir_all(&extract);
    let _ = Command::new(&dest_bin).spawn();
    Ok(InstallOutcome {
        message: format!("Updated {}", dest_bin.display()),
        should_exit: true,
        relaunch_hint: Some(dest_bin.display().to_string()),
    })
}

#[cfg(target_os = "linux")]
fn find_named_file(root: &Path, name: &str) -> Option<PathBuf> {
    if root.join(name).is_file() {
        return Some(root.join(name));
    }
    let rd = fs::read_dir(root).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        if p.is_file() && p.file_name().and_then(|s| s.to_str()) == Some(name) {
            return Some(p);
        }
        if p.is_dir() {
            if let Some(f) = find_named_file(&p, name) {
                return Some(f);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn find_named_dir(root: &Path, name: &str) -> Option<PathBuf> {
    if root.join(name).is_dir() {
        return Some(root.join(name));
    }
    let rd = fs::read_dir(root).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().and_then(|s| s.to_str()) == Some(name) {
                return Some(p);
            }
            if let Some(f) = find_named_dir(&p, name) {
                return Some(f);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_compare() {
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("1.0.0", "0.9.9"));
    }

    #[test]
    fn classify_macos_prefers_pkg() {
        #[cfg(target_os = "macos")]
        {
            let (s_pkg, k) = classify_asset("WebRust-macOS-0.1.9.pkg");
            let (s_zip, _) = classify_asset("WebRust-macOS-0.1.9.zip");
            assert!(s_pkg > s_zip);
            assert_eq!(k, InstallKind::MacPkg);
        }
    }
}

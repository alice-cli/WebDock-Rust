//! GitHub Releases auto-update check (inspired by Tauri updater / aliceRust pattern).
//!
//! Polls `https://api.github.com/repos/{owner}/{repo}/releases/latest`,
//! compares semver to `CARGO_PKG_VERSION`, and returns download URLs for the
//! current platform asset when available.

use serde::Deserialize;

/// Override via `WEBRUST_UPDATE_REPO=owner/name` if needed.
const DEFAULT_REPO: &str = "alice-cli/WebDock-Rust";
const USER_AGENT: &str = concat!("WebRust/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone, serde::Serialize)]
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

fn prefer_asset(name: &str) -> i32 {
    let n = name.to_ascii_lowercase();
    #[cfg(target_os = "windows")]
    {
        if n.ends_with(".exe") && (n.contains("setup") || n.contains("install")) {
            return 100;
        }
        if n.ends_with(".msi") {
            return 90;
        }
        if n.contains("windows") && n.ends_with(".zip") {
            return 80;
        }
        if n.ends_with(".exe") {
            return 70;
        }
        return 0;
    }
    #[cfg(target_os = "macos")]
    {
        if n.contains("macos") || n.contains("darwin") || n.contains("apple") {
            if n.ends_with(".zip") {
                return 100;
            }
            if n.ends_with(".dmg") {
                return 90;
            }
            return 50;
        }
        return 0;
    }
    #[cfg(target_os = "linux")]
    {
        if n.contains("linux") {
            if n.ends_with(".tar.gz") || n.ends_with(".tgz") {
                return 100;
            }
            if n.ends_with(".appimage") {
                return 90;
            }
            if n.ends_with(".deb") {
                return 80;
            }
            return 50;
        }
        return 0;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = n;
        0
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

    let mut best: Option<(i32, &GhAsset)> = None;
    for a in &rel.assets {
        let score = prefer_asset(&a.name);
        if score <= 0 {
            continue;
        }
        match best {
            None => best = Some((score, a)),
            Some((s, _)) if score > s => best = Some((score, a)),
            _ => {}
        }
    }

    Ok(UpdateInfo {
        current_version: current,
        latest_version: latest,
        update_available,
        download_url: best.map(|(_, a)| a.browser_download_url.clone()),
        html_url: rel.html_url,
        asset_name: best.map(|(_, a)| a.name.clone()),
        notes: rel.body.map(|b| {
            if b.len() > 800 {
                format!("{}…", &b[..800])
            } else {
                b
            }
        }),
    })
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
}
